//! Check the optional runtime against the installed files from the pinned downloads.
use sha2::{Digest, Sha256};
use std::io::{BufReader, Read};
use std::path::Path;

pub fn verify_rife(directory: &Path) -> Result<(), String> {
    let root = directory.join("providers/rife");
    if !root.is_dir() {
        return Err("Install the optional RIFE provider to enable interpolation.".into());
    }
    let script = std::fs::read(directory.join("filters/rife.vpy")).ok();
    if script.as_deref() != Some(include_bytes!("../assets/filters/rife.vpy")) {
        return Err("The RIFE script is missing or damaged. Restore the application files.".into());
    }
    for line in include_str!("../build/rife-installed.sha256").lines() {
        let (expected, name) = line.split_once("  ").expect("pinned digest and path");
        if !matches_digest(&root.join(name), expected) {
            return Err(format!(
                "RIFE file {name} is missing or damaged. Reinstall the optional provider."
            ));
        }
    }
    Ok(())
}

fn matches_digest(path: &Path, expected: &str) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(meta) = file.metadata() else {
        return false;
    };
    if !meta.is_file() || meta.len() > 128 * 1024 * 1024 {
        return false;
    }
    let mut reader = BufReader::new(file).take(meta.len() + 1);
    let mut hash = Sha256::new();
    std::io::copy(&mut reader, &mut hash).is_ok_and(|n| n == meta.len())
        && format!("{:x}", hash.finalize()) == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_same_length_corruption() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("weights.bin");
        let expected = format!("{:x}", Sha256::digest(b"model"));
        std::fs::write(&file, b"model").unwrap();
        assert!(matches_digest(&file, &expected));
        std::fs::write(&file, b"Model").unwrap();
        assert!(!matches_digest(&file, &expected));
    }
}
