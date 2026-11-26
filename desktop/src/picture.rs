use rtx_player::playback::Player;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Picture {
    brightness: i16,
    contrast: i16,
    saturation: i16,
    gamma: i16,
    sharpness: u8,
    deband: bool,
    deinterlace: String,
    aspect: String,
    crop: String,
}
impl Default for Picture {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 0,
            saturation: 0,
            gamma: 0,
            sharpness: 0,
            deband: false,
            deinterlace: "no".into(),
            aspect: "source".into(),
            crop: "source".into(),
        }
    }
}
impl Picture {
    pub fn apply(&self, p: &mut Player) -> Result<(), String> {
        let ratios = ["source", "16:9", "4:3", "1:1", "2.39:1"];
        if [self.brightness, self.contrast, self.saturation, self.gamma]
            .iter()
            .any(|n| !(-100..=100).contains(n))
            || self.sharpness > 100
            || !["no", "auto", "yes"].contains(&self.deinterlace.as_str())
            || !ratios.contains(&self.aspect.as_str())
            || !ratios.contains(&self.crop.as_str())
        {
            return Err("Picture adjustment outside the supported range.".into());
        }
        for (name, value) in [
            ("brightness", self.brightness),
            ("contrast", self.contrast),
            ("saturation", self.saturation),
            ("gamma", self.gamma),
        ] {
            p.command(&["set", name, &value.to_string()])?;
        }
        let ratio = |r: &str| match r {
            "source" => "-1",
            "16:9" => "16:9",
            "4:3" => "4:3",
            "1:1" => "1:1",
            _ => "2.39",
        };
        p.command(&[
            "set",
            "sharpen",
            &(f64::from(self.sharpness) / 100.).to_string(),
        ])?;
        p.command(&["set", "deband", if self.deband { "yes" } else { "no" }])?;
        p.command(&["set", "deinterlace", &self.deinterlace])?;
        p.command(&["set", "video-aspect-override", ratio(&self.aspect)])?;
        let crop = match self.crop.as_str() {
            "source" => "",
            "2.39:1" => "239:100",
            other => other,
        };
        p.command(&["set", "video-crop", crop])
    }
}
