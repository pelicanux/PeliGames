fn main() {
    println!("cargo:rerun-if-changed=../release-features.json");
    println!("cargo:rerun-if-env-changed=PELIGAMES_NEXUS_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=PELIGAMES_DISCORD_RELAY");
    launch_helper();
    game_overlay();
    tauri_build::build()
}

/// Separate targets avoid Cargo's outer lock. No C/C++ or compositor service
/// is embedded in the game. Both Rust libraries use the packaged glibc baseline.
fn game_overlay() {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process::Command,
    };
    fn watch(path: &Path) {
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            for entry in fs::read_dir(path).expect("overlay sources").flatten() {
                if entry.file_name() != "target" {
                    watch(&entry.path());
                }
            }
        }
    }
    let sources = PathBuf::from("../tools/game-overlay");
    watch(&sources);
    println!("cargo:rerun-if-env-changed=PELIGAMES_OVERLAY_BUNDLE");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let embedded = output.join("game-overlay");
    fs::create_dir_all(&embedded).expect("overlay output");
    let names = [
        "peligames-overlay-run",
        "libpeligames_game_overlay.so",
        "libpeligames_overlay_audit.so",
    ];
    let features: serde_json::Value = serde_json::from_str(&fs::read_to_string("../release-features.json").expect("release features")).expect("valid release features");
    if features["experimental_frame_generation"] != true
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
    {
        for name in names {
            fs::write(embedded.join(name), b"").expect("overlay placeholder");
        }
        return;
    }
    let bundle = env::var_os("PELIGAMES_OVERLAY_BUNDLE").map(PathBuf::from);
    for (index, name) in names.iter().enumerate() {
        let source = if let Some(bundle) = &bundle {
            let file = bundle.join(name);
            println!("cargo:rerun-if-changed={}", file.display());
            file
        } else {
            let audit = index == 2;
            let target = output.join(if audit {
                "overlay-audit-target"
            } else {
                "overlay-render-target"
            });
            // The first command builds the library and runner together.
            if index != 1 {
                let mut command = Command::new(env::var_os("CARGO").expect("CARGO"));
                command
                    .args(["build", "--manifest-path"])
                    .arg(sources.join(if audit {
                        "audit/Cargo.toml"
                    } else {
                        "Cargo.toml"
                    }))
                    .args([
                        "--release",
                        "--locked",
                        "--offline",
                        "--target",
                        "x86_64-unknown-linux-gnu",
                        "--target-dir",
                    ])
                    .arg(&target)
                    .args(["-j", "2"]);
                if !audit {
                    command.args(["--lib", "--bin", "peligames-overlay-run"]);
                }
                for key in [
                    "RUSTFLAGS",
                    "CARGO_ENCODED_RUSTFLAGS",
                    "CARGO_MAKEFLAGS",
                    "MAKEFLAGS",
                    "MFLAGS",
                ] {
                    command.env_remove(key);
                }
                assert!(
                    command.status().expect("build game overlay").success(),
                    "Falha ao compilar a camada interna do PeliGames."
                );
            }
            target.join("x86_64-unknown-linux-gnu/release").join(name)
        };
        let bytes = fs::read(source).expect("read game overlay payload");
        assert!(
            bytes.len() >= 64 && &bytes[..7] == b"\x7fELF\x02\x01\x01" && bytes[18..20] == [62, 0],
            "Payload do overlay não é ELF Linux x86_64."
        );
        fs::write(embedded.join(name), bytes).expect("embed game overlay");
    }
}

/// Steam can use an older libc than the desktop. Embed a small, static Rust
/// executable rather than launching the Tauri/AppImage binary inside its runtime.
fn launch_helper() {
    use std::{env, fs, path::PathBuf, process::Command};
    for file in ["Cargo.toml", "Cargo.lock", "src/main.rs", "src/profile.rs"] {
        println!("cargo:rerun-if-changed=launch-runtime/{file}");
    }
    println!("cargo:rerun-if-changed=launch-runtime/src/release_features.rs");
    println!("cargo:rerun-if-env-changed=PELIGAMES_LAUNCH_HELPER");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let embedded = output.join("pelirun");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        fs::write(&embedded, b"").expect("helper placeholder");
    } else {
        let architecture = env::var("CARGO_CFG_TARGET_ARCH").expect("target architecture");
        let target = match architecture.as_str() {
            "x86_64" => "x86_64-unknown-linux-musl",
            "aarch64" => "aarch64-unknown-linux-musl",
            _ => panic!("Arquitetura não suportada pelo auxiliar Steam: {architecture}"),
        };
        let binary = if let Some(path) = env::var_os("PELIGAMES_LAUNCH_HELPER") {
            let path = PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
            path
        } else {
            let target_dir = output.join("launch-runtime-target");
            let status = Command::new(env::var_os("CARGO").expect("CARGO"))
                .args([
                    "build",
                    "--manifest-path",
                    "launch-runtime/Cargo.toml",
                    "--release",
                    "--locked",
                    "--offline",
                    "--target",
                    target,
                    "--target-dir",
                ])
                .arg(&target_dir)
                .args(["-j", "1"])
                // Do not inherit application-specific linker flags or share
                // Cargo's outer target directory/jobserver lock.
                .env_remove("RUSTFLAGS")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env_remove("CARGO_MAKEFLAGS")
                .env_remove("MAKEFLAGS")
                .env_remove("MFLAGS")
                .status()
                .expect("iniciar compilação do auxiliar Steam");
            assert!(status.success(), "Não foi possível compilar o auxiliar Steam. Instale antes o alvo Rust com: rustup target add {target}");
            target_dir.join(target).join("release/pelirun")
        };
        let bytes = fs::read(binary).expect("ler auxiliar Steam");
        assert!(
            static_elf(&bytes, &architecture),
            "O auxiliar Steam precisa ser um ELF estático da arquitetura do aplicativo."
        );
        fs::write(&embedded, bytes).expect("incorporar auxiliar Steam");
    }
    println!(
        "cargo:rustc-env=PELIGAMES_LAUNCH_HELPER_EMBEDDED={}",
        embedded.display()
    );
}

fn static_elf(bytes: &[u8], architecture: &str) -> bool {
    if bytes.len() < 64 || &bytes[..6] != b"\x7fELF\x02\x01" {
        return false;
    }
    let u16_at = |i| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let machine = match architecture {
        "x86_64" => 62,
        "aarch64" => 183,
        _ => return false,
    };
    if u16_at(18) != machine {
        return false;
    }
    let offset = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let stride = u16_at(54) as u64;
    let count = u16_at(56) as u64;
    if stride < 56 || count == 0 {
        return false;
    }
    (0..count).all(|i| {
        let Some(at) = i
            .checked_mul(stride)
            .and_then(|i| offset.checked_add(i))
            .and_then(|i| usize::try_from(i).ok())
        else {
            return false;
        };
        let Some(header) = bytes.get(at..).and_then(|bytes| bytes.get(..56)) else {
            return false;
        };
        // PT_INTERP would require the host/Steam runtime's dynamic loader.
        u32::from_le_bytes(header[..4].try_into().unwrap()) != 3
    })
}
