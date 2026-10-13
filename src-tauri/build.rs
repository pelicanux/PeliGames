fn main() {
    println!("cargo:rerun-if-env-changed=PELIGAMES_NEXUS_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=PELIGAMES_DISCORD_RELAY");
    tauri_build::build()
}
