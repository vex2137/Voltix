use std::process::Command;

fn main() {
    // Android 15/16 can use 16 KiB pages. Apply only to this final cdylib,
    // leaving dependency fingerprints and Linux release linking unchanged.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384");
    }
    println!("cargo:rerun-if-env-changed=VOLTIX_SOURCE_COMMIT");
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(output) = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
    {
        if output.status.success() {
            println!(
                "cargo:rerun-if-changed=.git/{}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }
    }
    let commit = std::env::var("VOLTIX_SOURCE_COMMIT")
        .ok()
        .or_else(|| {
            let output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .filter(|value| value.len() == 40 && value.bytes().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown-source".into());
    println!("cargo:rustc-env=VOLTIX_SOURCE_COMMIT={commit}");
    slint_build::compile_with_config(
        "ui/app-window.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("Failed to compile Slint UI");
}
