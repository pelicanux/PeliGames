use std::process::Command;
use std::path::Path;
use std::fs;
use std::env;

/// Route names of the backend's install.sh. Newer backends renamed the ReShade
/// routes (reshade → reshade-feeder, vulkan → reshade-feeder-vulkan,
/// dx9 → reshade-feeder-dx9) and refuse the old names; older ones only know
/// the old names. The launcher keeps its own names and passes the one the
/// installed backend understands.
const RENAMED_ROUTES: [(&str, &str); 3] = [
    ("reshade", "reshade-feeder"),
    ("vulkan", "reshade-feeder-vulkan"),
    ("dx9", "reshade-feeder-dx9"),
];
fn backend_route(script: &str, route: &str) -> String {
    let renamed = script.contains("reshade-feeder)");
    for (old, new) in RENAMED_ROUTES {
        if route == old || route == new {
            return if renamed { new } else { old }.to_string();
        }
    }
    route.to_string()
}

/// Optional installation parts: int4 mixed, RenoDX, and LumeniteFX (ReShade only).
/// `None` leaves the
/// backend's default (off when not run in a terminal).
#[derive(Clone, Copy, Default)]
pub struct InstallExtras {
    pub int4: Option<bool>,
    pub renodx: Option<bool>,
    pub model: Option<NeuralModel>,
    pub lumenite: Option<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NeuralModel { Auto, Small, Experimental, Original }

/// Capability detection keeps RDNA3 and older packages on their original CLI.
pub fn unified_networks(script: &str) -> bool {
    script.contains("--model)") && script.contains("CC_SILVER_AARDWOLD_INT4_MIXED")
}
pub fn experimental_network(script: &str) -> bool { script.contains("CC_SMALL_FP8_EXPERIMENTAL") }
pub fn nr_plugin(script: &str) -> bool { script.contains("plugins/dlssnr-amd.asi") }
pub fn selectable_models(script: &str) -> bool {
    unified_networks(script) || (script.contains("--new-model)") && script.contains("--old-model)"))
}
pub fn supports_lumenite(script: &str) -> bool {
    script.contains("--lumenite)") && script.contains("--no-lumenite)")
}

fn model_arguments(script: &str, extras: InstallExtras, has_small: bool, has_original: bool) -> Result<Vec<&'static str>, String> {
    if matches!(extras.model, Some(NeuralModel::Small | NeuralModel::Experimental)) && extras.int4 == Some(true) {
        return Err("int4 misto só funciona com o modelo original (310.8.0).".into());
    }
    let selectable = selectable_models(script);
    if extras.model == Some(NeuralModel::Experimental) && !experimental_network(script) {
        return Err("A rede experimental exige o backend Mochizuki 0.0.8 ou mais recente.".into());
    }
    if matches!(extras.model, Some(NeuralModel::Small | NeuralModel::Experimental)) && (!selectable || !has_small) {
        return Err("Modelo novo indisponível. Atualize o backend e extraia a DLL 310.9.16 no assistente.".into());
    }
    if (extras.model == Some(NeuralModel::Original) || extras.int4 == Some(true)) && !has_original {
        return Err("Modelo original indisponível. Extraia novamente uma DLL compatível no assistente.".into());
    }
    if !selectable { return Ok(vec![]); }
    let old = extras.model == Some(NeuralModel::Original) || extras.int4 == Some(true) || !has_small;
    if unified_networks(script) {
        return Ok(vec!["--model", if extras.int4 == Some(true) {
            "CC_SILVER_AARDWOLD_INT4_MIXED"
        } else if old { "CC_SILVER_AARDWOLD" } else if extras.model == Some(NeuralModel::Experimental) { "CC_SMALL_FP8_EXPERIMENTAL" } else { "CC_SMALL_FP8" }]);
    }
    Ok(vec![if old { "--old-model" } else { "--new-model" }])
}

/// The install.sh arguments for `extras`, or why this backend cannot do them.
/// Older backends know RenoDX only as the route optiscaler-renodx.
fn extra_arguments(script: &str, route: &str, extras: InstallExtras) -> Result<(String, Vec<&'static str>), String> {
    let mut route = route.to_string();
    let mut arguments = Vec::new();
    if let Some(enabled) = extras.lumenite {
        let reshade = RENAMED_ROUTES.iter().any(|(old, new)| route == *old || route == *new);
        if enabled && (!supports_lumenite(script) || !reshade) {
            return Err("LumeniteFX exige um backend compatível e uma rota ReShade, Vulkan ou DX9.".into());
        }
        if supports_lumenite(script) && reshade {
            arguments.push(if enabled { "--lumenite" } else { "--no-lumenite" });
        }
    }
    if let Some(int4) = extras.int4.filter(|_| !unified_networks(script)) {
        if script.contains("--int4)") {
            arguments.push(if int4 { "--int4" } else { "--no-int4" });
        } else if int4 {
            return Err("O backend baixado não tem int4 misto. Atualize o backend e tente de novo.".into());
        }
    }
    if let Some(renodx) = extras.renodx {
        if script.contains("--renodx)") {
            arguments.push(if renodx { "--renodx" } else { "--no-renodx" });
        } else if renodx && route == "optiscaler" && script.contains("optiscaler-renodx") {
            route = "optiscaler-renodx".into();
        } else if renodx {
            return Err("O backend baixado não tem o estilo RenoDX nesta rota. Atualize o backend e tente de novo.".into());
        }
    }
    Ok((route, arguments))
}

/// Executes a standalone installation by copying bundled resources to a temp directory.
pub fn execute_standalone_install(
    resource_path: &Path,
    game_dir: &str,
    route: &str,
    dll_path: Option<&String>,
    bin_path: Option<&String>,
    shortcut_key: &str,
    extras: InstallExtras,
) -> Result<String, String> {
    // 1. Create a unique temporary directory
    let temp_dir = env::temp_dir().join(format!("dlssnr-installer-{}", uuid::Uuid::new_v4()));
    if let Err(e) = fs::create_dir_all(&temp_dir) {
        return Err(format!("Failed to create temporary directory: {}", e));
    }

    // 2. Copy the bundled resources recursively to the temp directory
    let mut cp_cmd = Command::new("cp");
    cp_cmd.arg("-r");
    // Ensure we copy the contents, not the folder itself if we want it to be the base
    cp_cmd.arg(format!("{}/.", resource_path.display()));
    cp_cmd.arg(&temp_dir);
    
    if let Ok(output) = cp_cmd.output() {
        if !output.status.success() {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(format!("Failed to extract bundled resources: {}", String::from_utf8_lossy(&output.stderr)));
        }
    } else {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err("Failed to execute cp command for extraction".to_string());
    }

    // Stage both model identities; keep the same pair for every game architecture.
    if let Some(bin) = bin_path.filter(|_| route != "remove") {
        if let Err(error) = super::neural_models::stage(Path::new(bin), &temp_dir.join("dlssnr-amd")) {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(error);
        }
    }

    // 4. Patch extract_model.sh to print python errors if it fails
    let extract_script = temp_dir.join("model-tools/extract_model.sh");
    if extract_script.exists() {
        if let Ok(content) = fs::read_to_string(&extract_script) {
            let patched = content.replace(
                "echo \"Cannot read nvngx_dlssnr weights",
                "cat \"$work/inspect.err\" >&2\n    echo \"Cannot read nvngx_dlssnr weights"
            );
            let _ = fs::write(&extract_script, patched);
        }
    }

    // 5. Execute `install.sh` from the temporary directory
    let mut cmd = Command::new("bash");
    
    // Fix for AppImage: Strip environment variables that break PyInstaller binaries like dlssnr-amd
    if let Ok(orig_ld) = std::env::var("ORIG_LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", orig_ld);
    } else {
        cmd.env_remove("LD_LIBRARY_PATH");
    }
    cmd.env_remove("APPDIR");
    cmd.env_remove("APPIMAGE");
    cmd.env_remove("PYTHONHOME");
    cmd.env_remove("PYTHONPATH");
    
    let script = fs::read_to_string(temp_dir.join("install.sh")).unwrap_or_default();
    let (route, extra) = match extra_arguments(&script, &backend_route(&script, route), extras) {
        Ok(value) => value,
        Err(error) => { let _ = fs::remove_dir_all(&temp_dir); return Err(error); }
    };
    cmd.current_dir(&temp_dir);
    cmd.arg("install.sh");
    cmd.arg(game_dir);
    cmd.arg(&route);
    cmd.args(extra);
    if route != "remove" {
        let has_small = temp_dir.join("dlssnr-amd/dlssnr-small.bin").is_file() || dll_path.is_some();
        match model_arguments(&script, extras, has_small, temp_dir.join("dlssnr-amd/dlssnr.bin").is_file() || dll_path.is_some()) {
            Ok(arguments) => { cmd.args(arguments); }
            Err(error) => { let _ = fs::remove_dir_all(&temp_dir); return Err(error); }
        }
    }
    
    if let Some(dll) = dll_path {
        cmd.arg("--dll");
        cmd.arg(dll);
    }

    let output = cmd.output().map_err(|e| format!("Failed to execute bash process: {}", e));

    // 5. Handle the output and copy back the .bin if generated
    let result = match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = format!("{}\n{}", stdout, stderr);
            
            let combined_lower = combined.to_lowercase();
            
            // Check for known backend warnings that should be treated as fatal errors
            if combined_lower.contains("will not load") || 
               combined_lower.contains("incompatible") {
                Err(format!("Aviso do Backend (Instalação Incompatível):\nO mod detectou um problema de compatibilidade (provavelmente você selecionou 64 bits para um jogo 32 bits, ou a rota selecionada não é suportada por este jogo).\n\nLog completo:\n{}", combined))
            } else if out.status.success() {
                // Persist both validated models before deleting the temporary package.
                if dll_path.is_some() {
                    if let Err(error) = super::neural_models::cache(&temp_dir.join("dlssnr-amd"), &resource_path.join("dlssnr-amd")) {
                        let _ = fs::remove_dir_all(&temp_dir);
                        return Err(error);
                    }
                }
                Ok(format!("Success:\n{}", combined))
            } else {
                Err(format!("Script Error (code: {:?}):\n{}", out.status.code(), combined))
            }
        }
        Err(e) => Err(e),
    };

    // 6. Cleanup the temporary directory
    let _ = fs::remove_dir_all(&temp_dir);

    if result.is_ok() {
        if route != "remove" { let _ = inject_shortcut_key(game_dir, shortcut_key, &route); }
    }

    result
}

pub fn inject_shortcut_key_pub(game_dir: &str, shortcut_key: &str, route: &str) -> Result<(), String> {
    inject_shortcut_key(game_dir, shortcut_key, route)
}

fn inject_shortcut_key(game_dir: &str, shortcut_key: &str, route: &str) -> Result<(), String> {
    let (key_hex, key_dec) = match shortcut_key {
        "End" => ("0x23", "35"),
        "Home" => ("0x24", "36"),
        "Page Up" => ("0x21", "33"),
        "Page Down" => ("0x22", "34"),
        "Insert" => ("0x2D", "45"),
        "Delete" => ("0x2E", "46"),
        "F1" => ("0x70", "112"),
        "F2" => ("0x71", "113"),
        "F11" => ("0x7A", "122"),
        "F12" => ("0x7B", "123"),
        _ => ("0x2D", "45"), // Default to Insert
    };

    let ini_files: &[&str] = match route {
        "optiscaler" | "optiscaler-renodx" => &["nvngx.ini", "optiscaler.ini"],
        "reshade" | "vulkan" | "dx9" | "reshade-feeder" | "reshade-feeder-vulkan" | "reshade-feeder-dx9" => &["reshade.ini"],
        _ => return Err("Rota inválida para o atalho do menu.".into()),
    };
    let base_path = Path::new(game_dir);

    // Let's also check in common subdirectories where the exe might be (like /Binaries/Win64)
    // To keep it simple, we use a basic recursive search (up to 3 levels) to find these ini files
    for entry in walkdir::WalkDir::new(base_path).max_depth(4).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let file_name_lower = file_name.to_lowercase();
                if ini_files.contains(&file_name_lower.as_str()) {
                    if let Ok(content) = fs::read_to_string(path) {
                        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                        let mut changed = false;

                        if file_name_lower == "reshade.ini" {
                            let mut has_input = false;
                            let mut has_keyoverlay = false;
                            for line in &lines {
                                if line.starts_with("[INPUT]") { has_input = true; }
                                if line.starts_with("KeyOverlay=") { has_keyoverlay = true; }
                            }
                            
                            if !has_keyoverlay {
                                if has_input {
                                    if let Some(idx) = lines.iter().position(|l| l.starts_with("[INPUT]")) {
                                        lines.insert(idx + 1, format!("KeyOverlay={},0,0,0", key_dec));
                                        changed = true;
                                    }
                                } else {
                                    lines.push(String::from("[INPUT]"));
                                    lines.push(format!("KeyOverlay={},0,0,0", key_dec));
                                    changed = true;
                                }
                            } else {
                                for line in &mut lines {
                                    if line.starts_with("KeyOverlay=") {
                                        *line = format!("KeyOverlay={},0,0,0", key_dec);
                                        changed = true;
                                        break;
                                    }
                                }
                            }
                        } else {
                            // optiscaler.ini or nvngx.ini
                            for line in &mut lines {
                                if line.starts_with("MenuKey=") {
                                    *line = format!("MenuKey={}", key_hex);
                                    changed = true;
                                } else if line.starts_with("KeyMenu=") {
                                    *line = format!("KeyMenu={}", key_hex);
                                    changed = true;
                                } else if line.starts_with("ShortcutKey=") {
                                    *line = format!("ShortcutKey={}", key_hex);
                                    changed = true;
                                }
                            }
                        }

                        if changed {
                            fs::write(path, lines.join("\n")).map_err(|error| format!("Não foi possível atualizar {}: {error}", path.display()))?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{backend_route, extra_arguments, InstallExtras};
    const NEW: &str = "case \"$route\" in\n    reshade-feeder) kind=reshade;;\n    reshade-feeder-vulkan) kind=vulkan;;\n";
    const OLD: &str = "case \"$route\" in\n    reshade) kind=reshade;;\n    vulkan) kind=vulkan;;\n";
    #[test]
    fn routes_follow_the_installed_backend() {
        assert_eq!(backend_route(NEW, "reshade"), "reshade-feeder");
        assert_eq!(backend_route(NEW, "vulkan"), "reshade-feeder-vulkan");
        assert_eq!(backend_route(NEW, "dx9"), "reshade-feeder-dx9");
        assert_eq!(backend_route(OLD, "reshade-feeder"), "reshade");
        assert_eq!(backend_route(OLD, "reshade-feeder-dx9"), "dx9");
        for route in ["optiscaler", "remove"] {
            assert_eq!(backend_route(NEW, route), route);
            assert_eq!(backend_route(OLD, route), route);
        }
    }
    #[test]
    fn int4_and_renodx_follow_what_the_backend_knows() {
        let current = "--int4) int4=1;;\n--no-int4) int4=0;;\n--renodx) renodx=1;;\n--no-renodx) renodx=0;;\n";
        let both = InstallExtras { int4: Some(true), renodx: Some(true), ..Default::default() };
        assert_eq!(extra_arguments(current, "reshade-feeder", both).unwrap(), ("reshade-feeder".into(), vec!["--int4", "--renodx"]));
        let off = InstallExtras { int4: Some(false), renodx: Some(false), ..Default::default() };
        assert_eq!(extra_arguments(current, "optiscaler", off).unwrap().1, vec!["--no-int4", "--no-renodx"]);
        assert!(extra_arguments(current, "remove", InstallExtras::default()).unwrap().1.is_empty());
        // Older backend: RenoDX only as the optiscaler-renodx route, no int4 flag.
        let older = "optiscaler|optiscaler-renodx) kind=optiscaler;;\n";
        let renodx = InstallExtras { int4: None, renodx: Some(true), ..Default::default() };
        assert_eq!(extra_arguments(older, "optiscaler", renodx).unwrap(), ("optiscaler-renodx".into(), vec![]));
        assert!(extra_arguments(older, "reshade", renodx).is_err());
        assert!(extra_arguments(older, "optiscaler", InstallExtras { int4: Some(true), renodx: None, ..Default::default() }).is_err());
        assert!(extra_arguments(older, "optiscaler", off).unwrap().1.is_empty(), "desligado num backend antigo não muda nada");
    }
    #[test]
    fn lumenite_requires_capability_and_a_reshade_route() {
        use super::{supports_lumenite, RENAMED_ROUTES};
        let script = "--lumenite) lumenite=1;; --no-lumenite) lumenite=0;;";
        assert!(supports_lumenite(script));
        assert!(!supports_lumenite("--lumenite)"));
        for (old, new) in RENAMED_ROUTES {
            for route in [old, new] {
                for enabled in [false, true] {
                    let extras = InstallExtras { lumenite: Some(enabled), ..Default::default() };
                    assert_eq!(extra_arguments(script, route, extras).unwrap().1, [if enabled { "--lumenite" } else { "--no-lumenite" }]);
                }
            }
        }
        let on = InstallExtras { lumenite: Some(true), ..Default::default() };
        for route in ["optiscaler", "optiscaler-renodx", "remove", "invalid"] {
            assert!(extra_arguments(script, route, on).is_err());
        }
        assert!(extra_arguments("old backend", "reshade", on).is_err());
        assert!(extra_arguments("old backend", "reshade", InstallExtras { lumenite: Some(false), ..Default::default() }).unwrap().1.is_empty());
    }
    #[test]
    fn neural_model_selection_respects_cache_and_int4() {
        use super::{model_arguments, NeuralModel};
        let script = "--new-model) newmodel=1;;\n--old-model) newmodel=0;;";
        let select = |model, int4, small, original| model_arguments(script, InstallExtras {model:Some(model),int4:Some(int4),..Default::default()}, small, original);
        assert_eq!(select(NeuralModel::Auto, false, true, true).unwrap(), vec!["--new-model"]);
        assert_eq!(select(NeuralModel::Auto, false, false, true).unwrap(), vec!["--old-model"]);
        assert_eq!(select(NeuralModel::Auto, true, true, true).unwrap(), vec!["--old-model"]);
        assert_eq!(select(NeuralModel::Original, false, true, true).unwrap(), vec!["--old-model"]);
        assert!(select(NeuralModel::Small, true, true, true).is_err());
        assert!(select(NeuralModel::Small, false, false, true).is_err());
        assert!(select(NeuralModel::Original, false, true, false).is_err());
        assert!(model_arguments("old backend", InstallExtras {model:Some(NeuralModel::Small),..Default::default()}, true, true).is_err());
        assert!(model_arguments("old backend", InstallExtras::default(), false, true).unwrap().is_empty());
    }

    #[test]
    fn unified_networks_use_one_explicit_model_without_overwriting_int4() {
        use super::{model_arguments, unified_networks, NeuralModel};
        let script = "--model) model=$2;; CC_SILVER_AARDWOLD_INT4_MIXED --int4) model=CC_SILVER_AARDWOLD_INT4_MIXED;; --no-int4) ignored;; --new-model) model=CC_SMALL_FP8;; --old-model) model=CC_SILVER_AARDWOLD;; --renodx) renodx=1;;";
        assert!(unified_networks(script));
        for (model, int4, small, expected) in [
            (NeuralModel::Auto, false, true, "CC_SMALL_FP8"),
            (NeuralModel::Small, false, true, "CC_SMALL_FP8"),
            (NeuralModel::Original, false, true, "CC_SILVER_AARDWOLD"),
            (NeuralModel::Original, true, true, "CC_SILVER_AARDWOLD_INT4_MIXED"),
            (NeuralModel::Auto, true, true, "CC_SILVER_AARDWOLD_INT4_MIXED"),
            (NeuralModel::Auto, false, false, "CC_SILVER_AARDWOLD"),
        ] {
            let extras = InstallExtras { model: Some(model), int4: Some(int4), renodx: Some(true), ..Default::default() };
            assert_eq!(model_arguments(script, extras, small, true).unwrap(), ["--model", expected]);
            let (_, flags) = extra_arguments(script, "optiscaler", extras).unwrap();
            assert_eq!(flags, ["--renodx"], "int4/model flags must have one owner");
        }
        assert!(model_arguments(script, InstallExtras {model:Some(NeuralModel::Small), ..Default::default()}, false, true).is_err());
    }

}

#[cfg(test)]
mod v008_tests {
    use super::*;
    const SCRIPT: &str="--model) CC_SMALL_FP8_EXPERIMENTAL CC_SILVER_AARDWOLD_INT4_MIXED plugins/dlssnr-amd.asi";
    #[test] fn experimental_model_requires_its_backend_weights_and_cannot_be_int4() {
        let extras=InstallExtras {model:Some(NeuralModel::Experimental),..Default::default()};
        assert_eq!(model_arguments(SCRIPT,extras,true,true).unwrap(),vec!["--model","CC_SMALL_FP8_EXPERIMENTAL"]);
        assert!(model_arguments(SCRIPT,extras,false,true).is_err());
        assert!(model_arguments("--model) CC_SILVER_AARDWOLD_INT4_MIXED",extras,true,true).is_err());
        assert!(model_arguments(SCRIPT,InstallExtras{int4:Some(true),..extras},true,true).is_err());
        assert!(nr_plugin(SCRIPT)); assert!(!nr_plugin("0.0.7 --model)"));
    }
}

#[cfg(test)]
mod menu_shortcut_tests {
    use super::*;
    #[test]
    fn menu_keys_only_update_the_selected_component() {
        let root = std::env::temp_dir().join(format!("peligames-menu-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let opti = root.join("OptiScaler.ini");
        let nvngx = root.join("nvngx.ini");
        let reshade = root.join("ReShade.ini");
        let original_opti = "[Menu]\nMenuKey=0x2D\n";
        let original_reshade = "[INPUT]\nKeyOverlay=36,0,0,0\n";
        fs::write(&opti, original_opti).unwrap();
        fs::write(&nvngx, original_opti).unwrap();
        fs::write(&reshade, original_reshade).unwrap();
        let dir = root.to_str().unwrap();
        inject_shortcut_key(dir, "F1", "optiscaler").unwrap();
        assert!(fs::read_to_string(&opti).unwrap().contains("MenuKey=0x70"));
        assert_eq!(fs::read_to_string(&reshade).unwrap(), original_reshade);
        let saved_opti = fs::read(&opti).unwrap();
        let saved_nvngx = fs::read(&nvngx).unwrap();
        for route in ["reshade", "vulkan", "dx9", "reshade-feeder", "reshade-feeder-vulkan", "reshade-feeder-dx9"] {
            fs::write(&reshade, original_reshade).unwrap();
            inject_shortcut_key(dir, "F12", route).unwrap();
            assert!(fs::read_to_string(&reshade).unwrap().contains("KeyOverlay=123,0,0,0"));
            assert_eq!(fs::read(&opti).unwrap(), saved_opti);
            assert_eq!(fs::read(&nvngx).unwrap(), saved_nvngx);
        }
        let before = fs::read(&reshade).unwrap();
        assert!(inject_shortcut_key(dir, "F2", "unknown").is_err());
        assert_eq!(fs::read(&reshade).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }
}
