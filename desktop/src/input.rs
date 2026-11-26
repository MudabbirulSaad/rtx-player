use rtx_player::playback::Player;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Serialize)]
pub struct ProcessingInput {
    pub height: Option<u32>,
    pub automatic: bool,
    pub fps24: bool,
}

impl ProcessingInput {
    pub fn dimensions(self, width: u32, height: u32) -> Result<(u32, u32), String> {
        if width == 0 || height == 0 {
            return Err("Source dimensions unavailable.".into());
        }
        if self
            .height
            .is_some_and(|v| ![480, 720, 1080, 1440, 2160].contains(&v))
            || (self.automatic && self.height.is_some())
        {
            return Err("Choose a supported processing resolution.".into());
        }
        let target = if self.automatic {
            height.min(720)
        } else {
            self.height.unwrap_or(height)
        };
        if height == target {
            return Ok((width, height));
        }
        let scaled = (f64::from(width) * f64::from(target) / f64::from(height) / 2.).round() * 2.;
        if !(2.0..=8192.0).contains(&scaled) {
            return Err(
                "Processing this aspect ratio would exceed the supported dimensions.".into(),
            );
        }
        Ok((scaled as u32, target))
    }

    fn filter(self, p: &Player) -> Result<Option<String>, String> {
        if self == Self::default() {
            return Ok(None);
        }
        let [Some(width), Some(height)] = [c"video-params/w", c"video-params/h"]
            .map(|k| p.property(k).and_then(|s| s.parse().ok()))
        else {
            return Err("Open a video before choosing its processing resolution.".into());
        };
        let (w, h) = self.dimensions(width, height)?;
        let mut stages = Vec::new();
        if self.height.is_some() || self.automatic {
            stages.push(format!("scale={w}:{h}"));
        }
        if self.fps24 {
            stages.push("fps=24".into());
        }
        let graph = stages.join(",");
        Ok(Some(format!("@input:lavfi=graph=%{}%{graph}", graph.len())))
    }

    pub fn change(&mut self, player: &mut Player, next: Self) -> Result<(), String> {
        if *self == next {
            return Ok(());
        }
        let filter = next.filter(player)?;
        let previous = *self;
        if previous != Self::default() {
            player.command(&["vf", "remove", "@input"])?;
            *self = Self::default();
        }
        if let Some(filter) = filter
            && let Err(error) = player.command(&["vf", "pre", &filter])
        {
            if let Ok(Some(old)) = previous.filter(player)
                && player.command(&["vf", "pre", &old]).is_ok()
            {
                *self = previous;
            }
            return Err(error);
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_native_dimensions_and_bounds_work_before_allocation() {
        let mut input = ProcessingInput::default();
        assert_eq!(input.dimensions(3840, 2160).unwrap(), (3840, 2160));
        input.automatic = true;
        assert_eq!(input.dimensions(3840, 2160).unwrap(), (1280, 720));
        assert_eq!(input.dimensions(640, 360).unwrap(), (640, 360));
        input.automatic = false;
        input.height = Some(480);
        assert_eq!(input.dimensions(1920, 1080).unwrap(), (854, 480));
        assert!(input.dimensions(8192, 2).is_err());
        input.height = Some(711);
        assert!(input.dimensions(1920, 1080).is_err());
    }
}
