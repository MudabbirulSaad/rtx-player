use rtx_player::playback::Player;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
pub struct Effects {
    pub super_resolution: bool,
    pub hdr: bool,
}

impl Effects {
    pub fn any(self) -> bool {
        self.super_resolution || self.hdr
    }
}

#[derive(Default)]
pub struct VideoProcessor {
    pub requested: Effects,
    pub accepted: Effects,
}

impl VideoProcessor {
    pub fn set(&mut self, p: &mut Player, settings: Effects, surface: usize) -> Result<(), String> {
        if settings.hdr {
            if !rtx_player::display::for_window(surface).is_ok_and(|s| s.hdr_active) {
                return Err("RTX HDR requires an active HDR display.".into());
            }
            if !matches!(
                p.property(c"video-params/gamma").as_deref(),
                Some("bt.1886" | "srgb" | "gamma2.2" | "gamma2.4")
            ) {
                return Err("RTX HDR conversion requires an SDR source.".into());
            }
        }
        self.clear(p)?;
        if !settings.any() {
            return Ok(());
        }
        if settings.super_resolution {
            p.command(&["change-list", "glsl-shaders", "clr", ""])?;
        }
        let scale = if settings.super_resolution { "2" } else { "1" };
        let mode = if settings.super_resolution {
            "nvidia"
        } else {
            "standard"
        };
        let hdr = if settings.hdr { "yes" } else { "no" };
        let filter = format!(
            "@rtx:d3d11vpp=mode=none:scale={scale}:scaling-mode={mode}:nvidia-true-hdr={hdr}"
        );
        p.command(&["vf", "add", &filter])?;
        self.requested = settings;
        Ok(())
    }

    pub fn clear(&mut self, p: &mut Player) -> Result<(), String> {
        if self.requested.any() {
            p.command(&["vf", "remove", "@rtx"])?;
        }
        *self = Self::default();
        Ok(())
    }
}
