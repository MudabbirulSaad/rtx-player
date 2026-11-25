use rtx_player::playback::Player;
use std::path::{Path, PathBuf};

pub struct Rife {
    directory: PathBuf,
    pub factor: u8,
}

impl Rife {
    pub fn new(directory: &Path) -> Self {
        Self {
            directory: directory.to_owned(),
            factor: 1,
        }
    }

    pub fn available(&self) -> bool {
        [
            "filters/rife.vpy",
            "providers/rife/librife.dll",
            "providers/rife/python/Lib/site-packages/vapoursynth/vsscript.dll",
            "providers/rife/models/rife-v4.25-lite_ensembleFalse/flownet.param",
            "providers/rife/models/rife-v4.25-lite_ensembleFalse/flownet.bin",
        ]
        .iter()
        .all(|name| self.directory.join(name).is_file())
    }

    pub fn set(&mut self, player: &mut Player, factor: u8) -> Result<(), String> {
        if !(1..=6).contains(&factor) {
            return Err("Choose a multiplier from 1 to 6.".into());
        }
        if factor > 1 {
            if !self.available() {
                return Err("The optional RIFE provider is missing.".into());
            }
            if !matches!(
                player.property(c"video-params/gamma").as_deref(),
                Some("bt.1886" | "srgb" | "gamma2.2" | "gamma2.4")
            ) {
                return Err(
                    "RIFE currently requires SDR video with a known transfer function.".into(),
                );
            }
        }
        if self.factor > 1 {
            player.command(&["vf", "remove", "@rife"])?;
            self.factor = 1;
        }
        if factor == 1 {
            return player.command(&["set", "hwdec", "auto-safe"]);
        }
        player.command(&["set", "hwdec", "auto-copy"])?;
        let script = self.directory.join("filters/rife.vpy");
        let script = script.to_str().ok_or("Unsupported provider path")?;
        let filter = format!(
            "@rife:vapoursynth=file=%{}%{}:buffered-frames=4:concurrent-frames=2:user-data={factor}",
            script.len(),
            script
        );
        if let Err(error) = player.command(&["vf", "add", &filter]) {
            player.command(&["set", "hwdec", "auto-safe"])?;
            return Err(error);
        }
        self.factor = factor;
        Ok(())
    }
}
