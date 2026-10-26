//! Preserve requested explicit layers and add MangoHUD inside MAKO's layer boundary.
//! The vendor launcher owns isolation; our final exec stage orders the optional
//! host HUD explicitly, without restoring system-wide implicit discovery.
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) const ENABLE_ENV: &str = "PELIGAMES_MAKO_MANGOHUD";
const LAYERS: [(&str, &str, &str); 2] = [
    (
        "MangoHud.x86_64.json",
        "VK_LAYER_MANGOHUD_overlay_x86_64",
        "64",
    ),
    ("MangoHud.x86.json", "VK_LAYER_MANGOHUD_overlay_x86", "32"),
];

pub(super) fn directory(config: &Path) -> PathBuf {
    config
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("layers/mangohud")
}

fn host_directories() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for base in [dirs::config_dir(), dirs::data_local_dir()]
        .into_iter()
        .flatten()
    {
        paths.push(base.join("vulkan/implicit_layer.d"));
    }
    if let Some(bases) = std::env::var_os("XDG_DATA_DIRS") {
        paths.extend(
            std::env::split_paths(&bases)
                .filter(|p| p.is_absolute())
                .take(32)
                .map(|p| p.join("vulkan/implicit_layer.d")),
        );
    }
    paths.extend(
        [
            "/etc/vulkan/implicit_layer.d",
            "/usr/local/share/vulkan/implicit_layer.d",
            "/usr/share/vulkan/implicit_layer.d",
        ]
        .map(PathBuf::from),
    );
    paths
}

fn manifest(source: &Path, name: &str, arch: &str) -> Option<Value> {
    let mut bytes = Vec::new();
    fs::File::open(source)
        .ok()?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 128 * 1024 {
        return None;
    }
    let mut value: Value = serde_json::from_slice(&bytes).ok()?;
    let layer = value.get_mut("layer")?.as_object_mut()?;
    if layer.get("name")?.as_str()? != name
        || layer.get("type")?.as_str()? != "GLOBAL"
        || layer.get("enable_environment")? != &json!({"MANGOHUD":"1"})
        || layer.get("disable_environment")? != &json!({"DISABLE_MANGOHUD":"1"})
        || layer
            .get("library_arch")
            .is_some_and(|v| v.as_str() != Some(arch))
    {
        return None;
    }
    let library = layer.get("library_path")?.as_str()?;
    if library.is_empty() || library.chars().any(char::is_control) {
        return None;
    }
    // Relative paths with directories are relative to the original manifest,
    // not to our managed copy. Bare library names retain loader resolution.
    if library.contains('/') {
        let path = source.parent()?.join(library).canonicalize().ok()?;
        let mut elf = [0; 5];
        fs::File::open(&path).ok()?.read_exact(&mut elf).ok()?;
        if &elf[..4] != b"\x7fELF" || elf[4] != if arch == "64" { 2 } else { 1 } {
            return None;
        }
        layer.insert("library_path".into(), json!(path.to_str()?));
    }
    layer.insert("library_arch".into(), json!(arch));
    Some(value)
}

fn stage(sources: &[PathBuf], destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let metadata = fs::symlink_metadata(destination).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Diretório de integração do MangoHUD inválido.".into());
    }
    let mut found = false;
    for (file, name, arch) in LAYERS {
        let path = destination.join(file);
        if let Some(value) = sources
            .iter()
            .find_map(|dir| manifest(&dir.join(file), name, arch))
        {
            let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
            if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
                super::write(&path, &bytes, 0o600)?;
            }
            found = true;
        } else {
            match fs::remove_file(&path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    if !found {
        return Err("A camada Vulkan do MangoHUD não foi encontrada. Instale ou repare o MangoHUD em Configurações → Monitoramento.".into());
    }
    Ok(())
}

pub(super) fn prepare(config: &Path) -> Result<(), String> {
    stage(&host_directories(), &directory(config))
}

/// AFMF is x86_64-only. Reuse the same validated host manifests as MAKO,
/// while keeping its own discovery directory and launch boundary.
pub(super) fn prepare_afmf(destination: &Path) -> Result<(), String> {
    stage_afmf(&host_directories(), destination)
}
fn stage_afmf(sources: &[PathBuf], destination: &Path) -> Result<(), String> {
    stage(sources, destination)?;
    if !destination.join(LAYERS[0].0).is_file() {
        return Err("A camada Vulkan de 64 bits do MangoHUD não foi encontrada. Instale ou repare o MangoHUD em Configurações → Monitoramento.".into());
    }
    Ok(())
}

pub(super) fn launch(runtime: &Path, config: &Path, overlay: Option<&Path>) -> String {
    let hud = directory(config);
    // Run after mako-launch: it deliberately replaces implicit search paths and
    // may prepend Steam's overlay. Do not edit the checksummed vendor payload.
    let legacy_overlay_guard = overlay.map(|runner| format!(
        "if [ \"${{PELI_OVERLAY_GENERAL:-0}}\" = 1 ] && [ \"$1\" = {} ] && [ \"$2\" = -- ]; then shift 2; fi\n",
        super::quote(&runner.to_string_lossy())
    )).unwrap_or_default();
    let adapter = format!(
        r#"if [ "${{PELIGAMES_MAKO_MANGOHUD:-0}}" = 1 ]; then
    unset PELIGAMES_MAKO_MANGOHUD
    if [ "${{ENABLE_VKBASALT:-0}}" = 1 ]; then
        echo 'PeliGames: MangoHUD e os shaders vkBasalt do MAKO não podem ser usados juntos.' >&2
        exit 1
    fi
    pg_hud_layers=
    if [ -r {hud64} ]; then pg_hud_layers=VK_LAYER_MANGOHUD_overlay_x86_64; fi
    if [ -r {hud32} ]; then pg_hud_layers="${{pg_hud_layers:+$pg_hud_layers:}}VK_LAYER_MANGOHUD_overlay_x86"; fi
    if [ -z "$pg_hud_layers" ]; then
        echo 'PeliGames: camada Vulkan do MangoHUD indisponível. Salve novamente o monitoramento do jogo.' >&2
        exit 1
    fi
    pg_layers="VK_LAYER_MAKO_render:$pg_hud_layers"
    pg_remaining=${{VK_INSTANCE_LAYERS:-}}
    while [ -n "$pg_remaining" ]; do
        pg_layer=${{pg_remaining%%:*}}
        case "$pg_remaining" in *:*) pg_remaining=${{pg_remaining#*:}} ;; *) pg_remaining= ;; esac
        case "$pg_layer" in
            VK_LAYER_MAKO_render|VK_LAYER_MANGOHUD_overlay_x86_64|VK_LAYER_MANGOHUD_overlay_x86|'') ;;
            VK_LAYER_VKBASALT_post_processing)
                echo 'PeliGames: MangoHUD e os shaders vkBasalt do MAKO não podem ser usados juntos.' >&2
                exit 1 ;;
            *) pg_layers="$pg_layers:$pg_layer" ;;
        esac
    done
    export VK_LAYER_PATH={mako_dir}:{hud_dir}${{VK_LAYER_PATH:+:$VK_LAYER_PATH}}
    export VK_INSTANCE_LAYERS="$pg_layers" MANGOHUD=1
    export NODEVICE_SELECT=1 DISABLE_LAYER_MESA_ANTI_LAG=1
    unset ENABLE_MAKO DISABLE_MANGOHUD pg_hud_layers pg_layers pg_remaining pg_layer
else
    unset PELIGAMES_MAKO_MANGOHUD
fi
# VK_LAYER_PATH overrides VK_ADD_LAYER_PATH. Preserve explicitly requested
# NR Control/int4/ReShade layers after the vendor creates its private path.
if [ -n "${{VK_LAYER_PATH:-}}" ] && [ -n "${{VK_ADD_LAYER_PATH:-}}" ]; then
    export VK_LAYER_PATH="$VK_ADD_LAYER_PATH:$VK_LAYER_PATH"
fi
{legacy_overlay_guard}exec "$@"
"#,
        hud64 = super::quote(&hud.join(LAYERS[0].0).to_string_lossy()),
        hud32 = super::quote(&hud.join(LAYERS[1].0).to_string_lossy()),
        mako_dir = super::quote(
            &runtime
                .join("share/mako-render/vulkan/implicit_layer.d")
                .to_string_lossy()
        ),
        hud_dir = super::quote(&hud.to_string_lossy()),
    );
    format!(
        "exec {} /bin/sh -c {} peligames-mako {}\"$@\"\n",
        super::quote(&runtime.join("bin/mako-launch").to_string_lossy()),
        super::quote(&adapter),
        overlay
            .map(|runner| format!("{} -- ", super::quote(&runner.to_string_lossy())))
            .unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use crate::core::test_support::RetryOutput;
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("peligames-hud-chain-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn install(&self, source: &str, index: usize) -> PathBuf {
            let dir = self.0.join(source);
            fs::create_dir_all(&dir).unwrap();
            let (file, name, arch) = LAYERS[index];
            let library = dir.join("libMangoHud.so");
            fs::write(
                &library,
                if arch == "64" {
                    b"\x7fELF\x02"
                } else {
                    b"\x7fELF\x01"
                },
            )
            .unwrap();
            fs::write(
                dir.join(file),
                serde_json::to_vec(&json!({
                    "file_format_version":"1.0.0", "layer": {
                        "name":name, "type":"GLOBAL", "library_path":"./libMangoHud.so",
                        "enable_environment":{"MANGOHUD":"1"},
                        "disable_environment":{"DISABLE_MANGOHUD":"1"}
                    }
                }))
                .unwrap(),
            )
            .unwrap();
            dir
        }
        fn run(&self, hud: bool, layers: &str, vkbasalt: bool) -> std::process::Output {
            self.run_with_overlay(hud, layers, vkbasalt, false)
        }
        fn run_with_overlay(
            &self,
            hud: bool,
            layers: &str,
            vkbasalt: bool,
            overlay: bool,
        ) -> std::process::Output {
            let runtime = self.0.join("runtime ' space");
            fs::create_dir_all(runtime.join("bin")).unwrap();
            let launcher = runtime.join("bin/mako-launch");
            fs::write(&launcher, b"#!/bin/sh\nexport VK_IMPLICIT_LAYER_PATH=/private-mako\nexport VK_LAYER_PATH=/steam-overlays\nexport VK_INSTANCE_LAYERS=\"VK_LAYER_MAKO_render:VK_LAYER_VALVE_steam_overlay_64${VK_INSTANCE_LAYERS:+:$VK_INSTANCE_LAYERS}\"\nunset VK_ADD_IMPLICIT_LAYER_PATH\nexec \"$@\"\n").unwrap();
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
            let config = self.0.join("profiles/quoted ' profile.toml");
            let dir = directory(&config);
            fs::create_dir_all(&dir).unwrap();
            for (file, _, _) in LAYERS {
                fs::write(dir.join(file), b"{}").unwrap();
            }
            let runner = self.0.join("overlay ' runner");
            fs::write(&runner, b"#!/bin/sh\n[ \"$1\" = -- ] || exit 7\nshift\n[ \"$VK_IMPLICIT_LAYER_PATH\" = /private-mako ] || exit 8\nexport PELIGAMES_TEST_OVERLAY=1\nexec \"$@\"\n").unwrap();
            fs::set_permissions(&runner, fs::Permissions::from_mode(0o700)).unwrap();
            let script = self.0.join("launch.sh");
            fs::write(
                &script,
                format!(
                    "#!/bin/sh\n{}",
                    launch(&runtime, &config, overlay.then_some(runner.as_path()))
                ),
            )
            .unwrap();
            std::process::Command::new("/bin/sh").arg(script)
                .args(["/usr/bin/python3", "-c", "import os,sys,json; print(json.dumps({'args':sys.argv[1:],'env':{k:os.environ.get(k) for k in ['VK_INSTANCE_LAYERS','VK_IMPLICIT_LAYER_PATH','VK_ADD_IMPLICIT_LAYER_PATH','VK_LAYER_PATH','MANGOHUD','MANGOHUD_CONFIGFILE','PELIGAMES_MAKO_MANGOHUD','PELIGAMES_TEST_OVERLAY']}}))", "arg with spaces", "$(not-code)"])
                .env(ENABLE_ENV, if hud { "1" } else { "0" })
                .env("ENABLE_VKBASALT", if vkbasalt { "1" } else { "0" })
                .env("VK_INSTANCE_LAYERS", layers).env("VK_ADD_IMPLICIT_LAYER_PATH", "/unrelated")
                .env("VK_ADD_LAYER_PATH", "/game/dlssnr/control:/game/dlssnr/int4")
                .env("MANGOHUD_CONFIGFILE", "/own profile/game.conf").output_retrying()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn general_overlay_skips_the_legacy_runner_without_losing_the_command() {
        let fixture = Fixture::new();
        let runtime = fixture.0.join("runtime");
        fs::create_dir_all(runtime.join("bin")).unwrap();
        let launcher = runtime.join("bin/mako-launch");
        fs::write(&launcher, "#!/bin/sh\nexec \"$@\"\n").unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
        let script = fixture.0.join("launch.sh");
        fs::write(&script, launch(&runtime, &fixture.0.join("profiles/game.toml"), Some(&fixture.0.join("legacy-runner-must-not-run")))).unwrap();
        let result = std::process::Command::new("/bin/sh")
            .arg(script).args(["/bin/echo", "argument with spaces"])
            .env("PELI_OVERLAY_GENERAL", "1")
            .env(ENABLE_ENV, "0")
            .output_retrying();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(result.stdout, b"argument with spaces\n");
    }
    #[test]
    fn overlay_runner_is_optional_and_runs_after_the_private_hud_adapter() {
        for overlay in [false, true] {
            let fixture = Fixture::new();
            let output = fixture.run_with_overlay(true, "", false, overlay);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                value["env"]["PELIGAMES_TEST_OVERLAY"],
                if overlay { json!("1") } else { Value::Null }
            );
            assert_eq!(value["env"]["MANGOHUD"], "1");
            assert_eq!(value["env"]["VK_IMPLICIT_LAYER_PATH"], "/private-mako");
            let paths = value["env"]["VK_LAYER_PATH"].as_str().unwrap();
        assert!(paths.starts_with("/game/dlssnr/control:/game/dlssnr/int4:"));
        assert!(paths.contains("/steam-overlays"));
        assert_eq!(value["args"], json!(["arg with spaces", "$(not-code)"]));
        }
    }
    #[test]
    fn afmf_requires_host_64_bit_hud_and_drops_a_stale_copy() {
        let f = Fixture::new();
        let host64 = f.install("host64", 0);
        let host32 = f.install("host32", 1);
        let destination = f.0.join("afmf-hud");
        stage_afmf(&[host64.clone(), host32.clone()], &destination).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(destination.join(LAYERS[0].0)).unwrap()).unwrap();
        assert_eq!(value["layer"]["library_path"], host64.join("libMangoHud.so").to_string_lossy().as_ref());
        assert!(stage_afmf(&[host32], &destination).unwrap_err().contains("64 bits"));
        assert!(!destination.join(LAYERS[0].0).exists());
    }

    #[test]
    fn stages_only_validated_architectures_and_resolves_relative_libraries() {
        let f = Fixture::new();
        let a = f.install("host64", 0);
        let b = f.install("host32", 1);
        fs::write(a.join("Unrelated.json"), b"{}").unwrap();
        let dest = f.0.join("managed");
        stage(&[a.clone(), b.clone()], &dest).unwrap();
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 2);
        for (index, host) in [a, b].iter().enumerate() {
            let value: Value =
                serde_json::from_slice(&fs::read(dest.join(LAYERS[index].0)).unwrap()).unwrap();
            assert_eq!(value["layer"]["library_arch"], LAYERS[index].2);
            assert_eq!(
                value["layer"]["library_path"],
                host.join("libMangoHud.so").to_string_lossy().as_ref()
            );
        }
    }

    #[test]
    fn rejects_wrong_identity_gate_and_library_abi_and_removes_stale_copies() {
        let f = Fixture::new();
        let source = f.install("host", 0);
        let path = source.join(LAYERS[0].0);
        let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let dest = f.0.join("managed");
        for field in [
            "name",
            "type",
            "enable_environment",
            "disable_environment",
            "library_arch",
        ] {
            stage(std::slice::from_ref(&source), &dest).unwrap();
            let mut bad = original.clone();
            bad["layer"][field] = json!("wrong");
            fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(stage(std::slice::from_ref(&source), &dest).is_err());
            assert!(!dest.join(LAYERS[0].0).exists());
            fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        }
        fs::write(source.join("libMangoHud.so"), b"\x7fELF\x01").unwrap();
        assert!(stage(&[source], &dest).is_err());
    }

    #[test]
    fn does_not_follow_managed_directory_symlink() {
        let f = Fixture::new();
        let source = f.install("host", 0);
        let dest = f.0.join("managed-link");
        std::os::unix::fs::symlink(&source, &dest).unwrap();
        assert!(stage(std::slice::from_ref(&source), &dest).is_err());
        assert!(source.join(LAYERS[0].0).is_file());
    }

    #[test]
    fn native_chain_orders_mako_hud_then_steam_without_opening_implicit_discovery() {
        let f = Fixture::new();
        let output = f.run(true, "VK_LAYER_MANGOHUD_overlay_x86_64:USER_LAYER", false);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["env"]["VK_INSTANCE_LAYERS"], "VK_LAYER_MAKO_render:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_MANGOHUD_overlay_x86:VK_LAYER_VALVE_steam_overlay_64:USER_LAYER");
        assert_eq!(value["env"]["VK_IMPLICIT_LAYER_PATH"], "/private-mako");
        assert_eq!(value["env"]["VK_ADD_IMPLICIT_LAYER_PATH"], Value::Null);
        assert_eq!(value["env"]["PELIGAMES_MAKO_MANGOHUD"], Value::Null);
        assert_eq!(
            value["env"]["MANGOHUD_CONFIGFILE"],
            "/own profile/game.conf"
        );
        assert_eq!(value["args"], json!(["arg with spaces", "$(not-code)"]));
    }

    #[test]
    fn disabled_hud_preserves_vendor_chain_and_shader_mode() {
        let f = Fixture::new();
        let output = f.run(false, "", true);
        assert!(output.status.success());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            value["env"]["VK_INSTANCE_LAYERS"],
            "VK_LAYER_MAKO_render:VK_LAYER_VALVE_steam_overlay_64"
        );
        assert_eq!(value["env"]["VK_LAYER_PATH"], "/game/dlssnr/control:/game/dlssnr/int4:/steam-overlays");
    }

    #[test]
    fn enabled_hud_rejects_incompatible_shader_chain() {
        let f = Fixture::new();
        for (layers, flag) in [("", true), ("VK_LAYER_VKBASALT_post_processing", false)] {
            let output = f.run(true, layers, flag);
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("vkBasalt"));
            assert!(output.stdout.is_empty());
        }
    }
}
