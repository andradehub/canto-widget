using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Media.Imaging;
using Windows.Media.Control;

namespace CantoWidget;

public record MusicState(string Title, string Artist, bool Playing, bool Available, double Progress, string Source);

public sealed class MediaService
{
    private GlobalSystemMediaTransportControlsSessionManager? manager;
    private GlobalSystemMediaTransportControlsSession? session;
    private string coverKey = "";
    public MusicState State { get; private set; } = new("Nada tocando agora", "Abra seu player de música", false, false, 0, "");
    public BitmapImage? Cover { get; private set; }
    public async Task RefreshAsync()
    {
        try
        {
            manager ??= await GlobalSystemMediaTransportControlsSessionManager.RequestAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5));
            var sessions = manager.GetSessions();
            session = sessions.FirstOrDefault(s => s.GetPlaybackInfo()?.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing)
                ?? manager.GetCurrentSession() ?? sessions.FirstOrDefault();
            if (session == null) { State = new("Nada tocando agora", "Abra seu player de música", false, false, 0, ""); Cover = null; coverKey = ""; return; }
            var props = await session.TryGetMediaPropertiesAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(4));
            var info = session.GetPlaybackInfo();
            var time = session.GetTimelineProperties();
            double duration = (time.EndTime - time.StartTime).TotalSeconds;
            double progress = duration > 0 ? Math.Clamp((time.Position - time.StartTime).TotalSeconds / duration, 0, 1) : 0;
            State = new(string.IsNullOrWhiteSpace(props.Title) ? "Mídia sem título" : props.Title,
                string.IsNullOrWhiteSpace(props.Artist) ? "Em reprodução no Windows" : props.Artist,
                info?.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing, true, progress, session.SourceAppUserModelId);
            var key = State.Source + "|" + props.Title + "|" + props.AlbumTitle + "|" + props.Artist;
            if (key != coverKey)
            {
                Cover = null;
                if (props.Thumbnail != null)
                {
                    using var stream = await props.Thumbnail.OpenReadAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(3));
                    using var input = stream.AsStreamForRead();
                    var bitmap = new BitmapImage(); bitmap.BeginInit(); bitmap.CacheOption = BitmapCacheOption.OnLoad;
                    bitmap.DecodePixelWidth = 128; bitmap.StreamSource = input; bitmap.EndInit(); bitmap.Freeze(); Cover = bitmap;
                }
                coverKey = key;
            }
        }
        catch (Exception ex) { AppLog.Write("Mídia: " + ex.Message); State = new("Player indisponível", "Aguardando mídia do Windows", false, false, 0, ""); Cover = null; }
    }
    public async Task<bool> CommandAsync(string command)
    {
        if (session == null) return false;
        try
        {
            return command switch
            {
                "previous" => await session.TrySkipPreviousAsync(),
                "next" => await session.TrySkipNextAsync(),
                _ => await session.TryTogglePlayPauseAsync()
            };
        }
        catch (Exception ex) { AppLog.Write("Controle: " + ex.Message); return false; }
    }
}
