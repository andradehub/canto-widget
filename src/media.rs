use serde::Serialize;
use std::sync::Arc;
use windows::{
    Media::Control::*,
    Storage::Streams::*,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Music {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub available: bool,
    pub progress: f64,
    #[serde(skip)]
    pub cover: Option<Arc<Vec<u8>>>,
}
pub struct Media {
    manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
    session: Option<GlobalSystemMediaTransportControlsSession>,
    key: String,
    cover: Option<Arc<Vec<u8>>>,
    initialized: bool,
}
impl Media {
    pub fn new() -> Self {
        let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED).is_ok() };
        Self {
            manager: None,
            session: None,
            key: String::new(),
            cover: None,
            initialized,
        }
    }
    fn poll(&mut self) -> windows::core::Result<Music> {
        if self.manager.is_none() {
            self.manager =
                Some(GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?);
        }
        let manager = self.manager.as_ref().unwrap();
        let sessions = manager.GetSessions()?;
        let mut selected = None;
        for i in 0..sessions.Size()? {
            let s = sessions.GetAt(i)?;
            if s.GetPlaybackInfo()?.PlaybackStatus()?
                == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
            {
                selected = Some(s);
                break;
            }
        }
        self.session = selected
            .or_else(|| manager.GetCurrentSession().ok())
            .or_else(|| sessions.GetAt(0).ok());
        let Some(session) = self.session.as_ref() else {
            self.cover = None;
            self.key.clear();
            return Ok(Music {
                title: "Nada tocando agora".into(),
                artist: "Abra seu player de música".into(),
                ..Default::default()
            });
        };
        let props = session.TryGetMediaPropertiesAsync()?.join()?;
        let title = props.Title()?.to_string();
        let artist = props.Artist()?.to_string();
        let time = session.GetTimelineProperties()?;
        let duration = time.EndTime()?.Duration - time.StartTime()?.Duration;
        let progress = if duration > 0 {
            ((time.Position()?.Duration - time.StartTime()?.Duration) as f64 / duration as f64)
                .clamp(0., 1.)
        } else {
            0.
        };
        let key = format!("{}|{}|{}", session.SourceAppUserModelId()?, title, artist);
        if key != self.key {
            self.cover = None;
            if let Ok(reference) = props.Thumbnail() {
                if let Ok(stream) = reference.OpenReadAsync().and_then(|op| op.join()) {
                    if let Ok(size) = stream.Size() {
                        if size > 0 && size <= 4_000_000 {
                            if let Ok(reader) = DataReader::CreateDataReader(&stream) {
                                if reader
                                    .LoadAsync(size as u32)
                                    .and_then(|op| op.join())
                                    .is_ok()
                                {
                                    let mut bytes = vec![0; size as usize];
                                    if reader.ReadBytes(&mut bytes).is_ok() {
                                        self.cover = Some(Arc::new(bytes));
                                    }
                                }
                                let _ = reader.DetachStream();
                            }
                        }
                    }
                }
            }
            self.key = key;
        }
        Ok(Music {
            title: if title.is_empty() {
                "Mídia sem título".into()
            } else {
                title
            },
            artist: if artist.is_empty() {
                "Em reprodução no Windows".into()
            } else {
                artist
            },
            playing: session.GetPlaybackInfo()?.PlaybackStatus()?
                == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing,
            available: true,
            progress,
            cover: self.cover.clone(),
        })
    }
    pub fn refresh(&mut self) -> Music {
        self.poll().unwrap_or_else(|_| Music {
            title: "Player indisponível".into(),
            artist: "Aguardando mídia do Windows".into(),
            ..Default::default()
        })
    }
    pub fn command(&self, action: u32) {
        if let Some(s) = &self.session {
            let op = match action {
                1 => s.TrySkipPreviousAsync(),
                3 => s.TrySkipNextAsync(),
                _ => s.TryTogglePlayPauseAsync(),
            };
            if let Ok(op) = op {
                let _ = op.join();
            }
        }
    }
}
impl Drop for Media {
    fn drop(&mut self) {
        self.session.take();
        self.manager.take();
        if self.initialized {
            unsafe {
                RoUninitialize();
            }
        }
    }
}
