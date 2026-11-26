use crate::{engine::Engine, library::Library};
use rtx_player::playback::Player;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Options {
    pub software_decode: bool,
    pub adaptive: bool,
    pub scale: f64,
    pub audio_device: String,
    pub subtitles: bool,
    pub theme: String,
    pub seek_seconds: u32,
    pub adapter: Option<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            software_decode: false,
            adaptive: false,
            scale: 2.,
            audio_device: "auto".into(),
            subtitles: true,
            theme: "dark".into(),
            seek_seconds: 5,
            adapter: None,
        }
    }
}
impl Options {
    pub fn recover_audio(&mut self, devices: &[Device]) -> bool {
        if self.audio_device != "auto"
            && !devices.is_empty()
            && !devices.iter().any(|d| d.name == self.audio_device)
        {
            self.audio_device = "auto".into();
            true
        } else {
            false
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if ![1.5, 2., 3., 4.].contains(&self.scale)
            || ![5, 10, 30].contains(&self.seek_seconds)
            || !["dark", "light", "system"].contains(&self.theme.as_str())
            || self.audio_device.is_empty()
            || self.audio_device.len() > 512
            || self
                .adapter
                .as_ref()
                .is_some_and(|a| a.is_empty() || a.len() > 200 || a.contains('\0'))
        {
            return Err("One or more preferences are outside the supported range.".into());
        }
        Ok(())
    }
    pub fn apply(&self, p: &mut Player, safe: bool, interpolating: bool) -> Result<(), String> {
        self.validate()?;
        let hwdec = if safe || self.software_decode {
            "no"
        } else if interpolating {
            "auto-copy"
        } else {
            "auto-safe"
        };
        p.command(&["set", "hwdec", hwdec])?;
        p.command(&["set", "audio-device", &self.audio_device])?;
        p.command(&[
            "set",
            "sub-visibility",
            if self.subtitles { "yes" } else { "no" },
        ])
    }
}
impl Library {
    pub fn options(&self) -> Result<Options, String> {
        let encoded: String = self.with(|db| {
            db.query_row("SELECT value FROM app_options WHERE id=1", [], |r| r.get(0))
        })?;
        let prefs: Options = serde_json::from_str(&encoded)
            .map_err(|_| "Saved preferences could not be read. Reset the playback settings.")?;
        prefs.validate()?;
        Ok(prefs)
    }
    pub fn save_options(&self, prefs: &Options) -> Result<(), String> {
        prefs.validate()?;
        let encoded = serde_json::to_string(prefs).map_err(|e| e.to_string())?;
        self.with(|db| {
            db.execute("UPDATE app_options SET value=?1 WHERE id=1", [encoded])
                .map(|_| ())
        })
    }
}

#[derive(Clone, Serialize)]
pub struct Device {
    pub name: String,
    pub description: String,
}
pub fn audio_devices(player: &Player) -> Vec<Device> {
    let count = player
        .property(c"audio-device-list/count")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
        .min(128);
    let mut devices = Vec::new();
    for i in 0..count {
        let property = |name: &str| {
            std::ffi::CString::new(format!("audio-device-list/{i}/{name}"))
                .ok()
                .and_then(|key| player.property(&key))
        };
        if let Some(name) = property("name") {
            let description = property("description").unwrap_or_else(|| name.clone());
            devices.push(Device { name, description });
        }
    }
    devices
}

#[tauri::command]
pub async fn set_player_options(engine: State<'_, Engine>, options: Options) -> Result<(), String> {
    let response = engine.options(options)?;
    tauri::async_runtime::spawn_blocking(move || {
        response
            .recv_timeout(std::time::Duration::from_secs(8))
            .map_err(|_| "Playback worker did not acknowledge settings".to_owned())?
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_and_roundtrips_preferences_from_old_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let db = Library::open(&dir.path().join("state.db")).unwrap();
        let mut options = db.options().unwrap();
        assert_eq!(options.seek_seconds, 5);
        options.theme = "light".into();
        options.software_decode = true;
        db.save_options(&options).unwrap();
        assert!(db.options().unwrap().software_decode);
        options.scale = 99.;
        assert!(db.save_options(&options).is_err());
    }
}
