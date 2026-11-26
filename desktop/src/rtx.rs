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

pub struct VideoProcessor {
    pub picture: crate::picture::Picture,
    pub software_decode: bool,
    pub input: crate::input::ProcessingInput,
    pub requested: Effects,
    pub accepted: Effects,
    pub scale: f64,
}

impl Default for VideoProcessor {
    fn default() -> Self {
        Self {
            picture: Default::default(),
            software_decode: false,
            input: Default::default(),
            requested: Effects::default(),
            accepted: Effects::default(),
            scale: 2.0,
        }
    }
}

impl VideoProcessor {
    pub fn decoder(
        &self,
        p: &mut Player,
        factor: f64,
        input: crate::input::ProcessingInput,
    ) -> Result<(), String> {
        let mode = if self.software_decode {
            "no"
        } else if factor > 1. || input != Default::default() {
            "auto-copy"
        } else {
            "auto-safe"
        };
        if p.property(c"hwdec").as_deref() == Some(mode) {
            return Ok(());
        }
        p.command(&["set", "hwdec", mode])
    }
    pub fn set_scale(&mut self, p: &mut Player, scale: f64, surface: usize) -> Result<(), String> {
        if ![1.5, 2.0, 3.0, 4.0].contains(&scale) {
            return Err("Choose a supported spatial scale.".into());
        }
        let previous = self.scale;
        self.scale = scale;
        if self.requested.super_resolution
            && let Err(error) = self.set(p, self.requested, surface)
        {
            self.scale = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn set(&mut self, p: &mut Player, settings: Effects, surface: usize) -> Result<(), String> {
        if settings.super_resolution {
            let dimensions = [c"video-params/w", c"video-params/h"]
                .map(|key| p.property(key).and_then(|v| v.parse::<u32>().ok()));
            let [Some(w), Some(h)] = dimensions else {
                return Err("Source dimensions unavailable.".into());
            };
            let (w, h) = self.input.dimensions(w, h)?;
            let pixels = f64::from(w) * f64::from(h) * self.scale * self.scale;
            if f64::from(w.max(h)) * self.scale > 8192.0 || pixels > 7680.0 * 4320.0 {
                return Err("Choose a smaller scale for this source resolution.".into());
            }
        }
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
        let scale = if settings.super_resolution {
            self.scale
        } else {
            1.0
        };
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
        self.requested = Effects::default();
        self.accepted = Effects::default();
        Ok(())
    }
}
