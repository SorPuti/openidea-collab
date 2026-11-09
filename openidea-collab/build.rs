fn main() {
    // Set cfg(collab_dev) when the installer builds via `openidea-plugin link`
    // so the panel can show a DEV badge and skip marketplace checks.
    if std::env::var("COLLAB_DEV").is_ok() {
        println!("cargo:rustc-cfg=collab_dev");
    }
    println!("cargo:rerun-if-env-changed=COLLAB_DEV");
}
