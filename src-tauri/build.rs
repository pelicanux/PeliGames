fn main() {
    println!("cargo:rerun-if-env-changed=PELIGAMES_DISCORD_RELAY");
    tauri_build::build()
}
