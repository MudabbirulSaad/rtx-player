use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
fn main() {
    let mut refs = vec!["HEAD".to_owned()];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        refs.push(branch)
    }
    for reference in refs {
        if let Some(path) = git(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            &reference,
        ]) {
            println!("cargo:rerun-if-changed={path}")
        }
    }
    println!(
        "cargo:rustc-env=RTX_BUILD={}",
        git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "source-archive".into())
    );
    tauri_build::build()
}
