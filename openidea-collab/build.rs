fn main() {
    println!("cargo:rustc-check-cfg=cfg(collab_dev)");
    if std::env::var("COLLAB_DEV").is_ok() {
        println!("cargo:rustc-cfg=collab_dev");
    }
    println!("cargo:rerun-if-env-changed=COLLAB_DEV");

    // Embed build timestamp so the panel can show it
    let ts = std::process::Command::new("date")
        .args(["+%Y-%m-%d %H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_TIME={ts}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/page.html");
}
