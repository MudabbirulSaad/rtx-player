use crate::{
    adaptive::Adaptive,
    engine::Control,
    input::ProcessingInput,
    rife::Rife,
    rtx::{Effects, VideoProcessor},
};
use rtx_player::playback::Player;
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Preset {
    Original,
    Auto,
    Performance,
    Balanced,
    Quality,
}

struct Pipeline {
    input: ProcessingInput,
    rate: f64,
    effects: Effects,
    anime: bool,
}
impl Pipeline {
    fn apply(
        &self,
        player: &mut Player,
        shader: &Path,
        rife: &mut Rife,
        rtx: &mut VideoProcessor,
        surface: usize,
    ) -> Result<(), String> {
        for command in [
            Control::Rtx(Effects::default()),
            Control::Input(self.input),
            Control::Interpolate(self.rate),
            Control::Upscale(self.anime),
            Control::Rtx(self.effects),
        ] {
            command.apply(player, shader, rife, rtx, surface)?;
        }
        Ok(())
    }
}
impl Preset {
    pub fn apply(
        self,
        p: &mut Player,
        shader: &Path,
        rife: &mut Rife,
        rtx: &mut VideoProcessor,
        surface: usize,
        adaptive: &mut Adaptive,
    ) -> Result<(), String> {
        Control::Compare(false).apply(p, shader, rife, rtx, surface)?;
        let before = Pipeline {
            input: rtx.input,
            rate: rife.factor,
            effects: rtx.requested,
            anime: p.property(c"glsl-shaders").is_some_and(|s| !s.is_empty()),
        };
        let fps = p
            .property(c"container-fps")
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite() && *n > 0.);
        let supported = rife.available()
            && matches!(
                p.property(c"video-params/gamma").as_deref(),
                Some("bt.1886" | "srgb" | "gamma2.2" | "gamma2.4")
            );
        if matches!(self, Self::Balanced | Self::Quality) && !supported {
            return Err(
                "This preset requires an SDR source and the RIFE playback provider.".into(),
            );
        }
        let rate = match self {
            Self::Auto if supported => fps.map_or(1., |n| (60. / n).clamp(1., 2.5)),
            Self::Balanced => 2.,
            Self::Quality => fps.map_or(2.5, |n| (60. / n).clamp(1., 6.)),
            _ => 1.,
        };
        let next = Pipeline {
            input: Default::default(),
            rate,
            effects: Default::default(),
            anime: matches!(self, Self::Performance | Self::Quality),
        };
        if let Err(error) = next.apply(p, shader, rife, rtx, surface) {
            before
                .apply(p, shader, rife, rtx, surface)
                .map_err(|restore| {
                    format!("{error} Previous settings could not be restored: {restore}")
                })?;
            return Err(error);
        }
        adaptive.enabled = matches!(self, Self::Auto | Self::Balanced);
        adaptive.reset(rate);
        Ok(())
    }
}
