use rtx_player::playback::Player;
use std::path::{Path, PathBuf};

pub struct Rife {
    directory: PathBuf,
    pub problem: Option<String>,
    pub factor: f64,
}

impl Rife {
    pub fn new(directory: &Path) -> Self {
        Self {
            directory: directory.to_owned(),
            problem: rtx_player::provider::verify_rife(directory).err(),
            factor: 1.0,
        }
    }

    pub fn available(&self) -> bool {
        self.problem.is_none()
    }

    pub fn set(&mut self, player: &mut Player, factor: f64) -> Result<(), String> {
        if ![1.0, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0].contains(&factor) {
            return Err("Choose a supported RIFE multiplier.".into());
        }
        if factor > 1.0 {
            if let Some(reason) = &self.problem {
                return Err(reason.clone());
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
        if self.factor > 1.0 {
            player.command(&["vf", "remove", "@rife"])?;
            self.factor = 1.0;
        }
        if factor == 1.0 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_or_damaged_provider_does_not_enable_interpolation() {
        let dir = tempfile::tempdir().unwrap();
        for path in [
            "filters/rife.vpy",
            "providers/rife/librife.dll",
            "providers/rife/python/Lib/site-packages/vapoursynth/vsscript.dll",
            "providers/rife/models/rife-v4.25-lite_ensembleFalse/flownet.param",
            "providers/rife/models/rife-v4.25-lite_ensembleFalse/flownet.bin",
        ] {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"damaged").unwrap();
        }
        assert!(!Rife::new(dir.path()).available());
    }
}
