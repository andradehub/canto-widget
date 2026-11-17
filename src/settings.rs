use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(rename = "AutoSync", alias = "auto_sync")]
    pub auto_sync: bool,
    pub floating: bool,
    pub show_player: bool,
    pub auto_hide_player: bool,
    pub background: Option<u32>,
    pub icon_color: u32,
    pub progress_color: u32,
    #[serde(rename = "Left", alias = "left")]
    pub left: Option<i32>,
    #[serde(rename = "Top", alias = "top")]
    pub top: Option<i32>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_sync: true,
            floating: false,
            show_player: true,
            auto_hide_player: false,
            background: None,
            icon_color: 0x85e4cf,
            progress_color: 0x538e91,
            left: None,
            top: None,
        }
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CantoWidget")
}
impl Settings {
    pub fn load() -> Self {
        std::fs::read(data_dir().join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(dir.join("settings.json"), bytes);
        }
    }
    pub fn player_visible(&self, available: bool) -> bool {
        self.show_player && (!self.auto_hide_player || available)
    }
    pub fn width(&self, available: bool) -> i32 {
        if self.player_visible(available) {
            420
        } else {
            100
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battery_only_shrinks_widget() {
        let s = Settings {
            show_player: false,
            ..Settings::default()
        };
        assert_eq!(s.width(true), 100);
        assert!(!s.player_visible(true));
    }
    #[test]
    fn auto_hide_restores_on_session() {
        let s = Settings {
            auto_hide_player: true,
            ..Settings::default()
        };
        assert_eq!(s.width(false), 100);
        assert_eq!(s.width(true), 420);
    }
    #[test]
    fn preferences_round_trip_and_migrate() {
        let s: Settings =
            serde_json::from_str(r#"{"Left":1476,"Top":10,"AutoSync":false}"#).unwrap();
        assert!(!s.auto_sync);
        assert_eq!(s.left, Some(1476));
        assert!(s.show_player);
        let t: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s.icon_color, t.icon_color);
    }
}
