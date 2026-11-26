use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// VSPipe owns decoding and inference. Its bounded stdout pipe feeds FFmpeg.
pub(super) struct Frames {
    child: Child,
    errors: std::fs::File,
}

impl Frames {
    pub fn start(app: &Path, source: &Path, fps: u32) -> Result<Self, String> {
        let python = app.join("providers/rife/python");
        let vs = python.join("Lib/site-packages/vapoursynth");
        let mut paths = vec![vs.clone(), python.clone()];
        if let Some(system) = std::env::var_os("SystemRoot") {
            paths.push(Path::new(&system).join("System32"));
        }
        let path: OsString = std::env::join_paths(paths).map_err(|e| e.to_string())?;
        let errors = tempfile::tempfile().map_err(|e| e.to_string())?;
        let child = Command::new(vs.join("vspipe.exe"))
            .current_dir(app)
            .env("PATH", path)
            .env("PYTHONHOME", python)
            .env("RTX_EXPORT_SOURCE", source)
            .env("RTX_EXPORT_FPS", fps.to_string())
            .args(["--container", "y4m", "--requests", "4"])
            .arg(app.join("filters/rife-export.vpy"))
            .arg("-")
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(errors.try_clone().map_err(|e| e.to_string())?)
            .spawn()
            .map_err(|e| format!("Cannot start the RIFE frame server: {e}"))?;
        Ok(Self { child, errors })
    }

    pub fn output(&mut self) -> Stdio {
        self.child
            .stdout
            .take()
            .expect("frame pipe is taken once")
            .into()
    }

    pub fn finish(&mut self, encode: Result<(), String>) -> Result<(), String> {
        if encode.is_err() {
            // FFmpeg may stop before reading its input, including during indexing.
            let _ = self.child.kill();
        }
        let status = self.child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            let length = self.errors.metadata().map_err(|e| e.to_string())?.len();
            self.errors
                .seek(SeekFrom::Start(length.saturating_sub(2048)))
                .map_err(|e| e.to_string())?;
            let mut tail = String::new();
            let _ = self.errors.read_to_string(&mut tail);
            if let Some(reason) = tail.lines().find(|line| line.contains("Error:")) {
                return Err(format!("RIFE export failed. {reason}"));
            }
            encode?;
            return Err(format!("The RIFE frame server exited with {status}."));
        }
        encode
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires installed RIFE, the export reader and a 24 fps local test video"]
    fn reports_an_interrupted_running_frame_server() {
        let runtime = std::path::PathBuf::from(std::env::var_os("RTX_TEST_RUNTIME").unwrap());
        let media = std::path::PathBuf::from(std::env::var_os("RTX_TEST_MEDIA").unwrap());
        let app = runtime.parent().unwrap();
        crate::provider::verify_rife_export(app).unwrap();
        let mut frames = Frames::start(app, &media, 60).unwrap();
        let mut signature = [0; 9];
        frames
            .child
            .stdout
            .as_mut()
            .unwrap()
            .read_exact(&mut signature)
            .unwrap();
        assert_eq!(&signature, b"YUV4MPEG2");
        frames.child.kill().unwrap();
        assert!(frames.finish(Ok(())).is_err());
        assert!(frames.child.try_wait().unwrap().is_some());
    }
}
