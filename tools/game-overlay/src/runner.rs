//! Per-game launch boundary, after MAKO and its managed HUD adapter.
//! Preparation failures preserve the original game invocation and environment.
use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
};
#[path = "../shortcut.rs"]
mod shortcut_key;
const LAYER: &str = "VK_LAYER_PELIGAMES_overlay";
fn layer_manifest(library: &Path) -> String {
    // pressure-vessel parses implementation_version as a JSON string. The
    // host Vulkan loader also accepts a number, which hides this error in
    // host-only probes. library_arch was introduced in manifest 1.2.1.
    serde_json::json!({"file_format_version":"1.2.1","layer":{
        "name":LAYER,"type":"GLOBAL","library_path":library,"api_version":"1.0.0",
        "implementation_version":"1","description":"PeliGames interactive overlay",
        "library_arch":"64",
        "functions":{"vkNegotiateLoaderLayerInterfaceVersion":"peliNegotiateLoaderLayerInterfaceVersion"}}}).to_string()
}
fn checked_elf(path: &Path) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
        return Err("Biblioteca sem propriedade/permissões seguras.".into());
    }
    let mut header = [0; 20];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|e| e.to_string())?;
    if header[..7] != [0x7f, b'E', b'L', b'F', 2, 1, 1] || header[18..20] != [62, 0] {
        return Err("O overlay interno exige Linux ELF x86_64.".into());
    }
    Ok(())
}
fn prepend(path: &Path, previous: Option<OsString>) -> Result<OsString, String> {
    let mut paths = vec![path.to_owned()];
    if let Some(previous) = previous {
        for p in env::split_paths(&previous) {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
    }
    env::join_paths(paths).map_err(|e| e.to_string())
}
fn order(existing: &str, mako_enabled: bool) -> Result<String, String> {
    let mut names: Vec<&str> = existing
        .split(':')
        .filter(|n| !n.is_empty() && *n != LAYER)
        .collect();
    if names
        .iter()
        .any(|n| !n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    {
        return Err("Lista Vulkan inválida.".into());
    }
    if mako_enabled || names.contains(&"VK_LAYER_MAKO_render") {
        // MAKO owns presentation. Place the renderer below the managed post-process chain;
        // never enable competing-FG or remove its private implicit boundary.
        names.retain(|n| *n != "VK_LAYER_MAKO_render");
        names.insert(0, "VK_LAYER_MAKO_render");
        let after = names
            .iter()
            .take_while(|n| {
                matches!(
                    **n,
                    "VK_LAYER_MAKO_render"
                        | "VK_LAYER_MANGOHUD_overlay_x86_64"
                        | "VK_LAYER_MANGOHUD_overlay_x86"
                        | "VK_LAYER_VKBASALT_post_processing"
                )
            })
            .count();
        names.insert(after, LAYER);
    } else {
        names.insert(0, LAYER);
    }
    Ok(names.join(":"))
}
fn prepare() -> Result<Vec<(&'static str, OsString)>, String> {
    let shortcut = shortcut()?;
    let binary = env::current_exe().map_err(|e| e.to_string())?;
    let directory = binary.parent().ok_or("Diretório do overlay ausente")?;
    let library = directory.join("libpeligames_game_overlay.so");
    checked_elf(&library)?;
    let audit = directory.join("libpeligames_overlay_audit.so");
    checked_elf(&audit)?;
    let manifests = directory.join("vulkan");
    match fs::create_dir(&manifests) {
        Ok(()) => fs::set_permissions(&manifests, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let meta = fs::symlink_metadata(&manifests).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err("Diretório de manifests do overlay inseguro.".into());
    }
    let manifest_path = manifests.join("VkLayer_PELIGAMES_overlay.json");
    let expected = layer_manifest(&library);
    let intact = fs::symlink_metadata(&manifest_path).is_ok_and(|m| {
        m.is_file() && m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0
    }) && fs::read_to_string(&manifest_path).is_ok_and(|text| text == expected);
    if !intact {
        let temporary = manifests.join(format!(".{}.json", std::process::id()));
        let result = (|| {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .and_then(|mut f| f.write_all(expected.as_bytes()))
                .map_err(|e| e.to_string())?;
            fs::rename(&temporary, &manifest_path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
    }
    let mut layer_paths = env::var_os("VK_LAYER_PATH");
    if let Some(private) = env::var_os("VK_IMPLICIT_LAYER_PATH") {
        // Explicitly requesting MAKO must still find its private manifest.
        for p in env::split_paths(&private) {
            layer_paths = Some(prepend(&p, layer_paths)?);
        }
    }
    let paths = prepend(&manifests, layer_paths)?;
    let layers = order(
        &env::var("VK_INSTANCE_LAYERS").unwrap_or_default(),
        env::var("ENABLE_MAKO").ok().as_deref() == Some("1"),
    )?;
    let preload = append_dso(&library, env::var_os("LD_PRELOAD"))?;
    let audit_path = append_dso(&audit, env::var_os("LD_AUDIT"))?;
    Ok(vec![
        ("VK_LAYER_PATH", paths),
        ("VK_INSTANCE_LAYERS", layers.into()),
        ("PELI_OVERLAY_SHORTCUT", shortcut.into()),
        ("LD_PRELOAD", preload),
        ("LD_AUDIT", audit_path),
    ])
}
fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return Err("Uso: peligames-overlay-run -- COMANDO [ARGUMENTOS].".into());
    }
    let program = args.next().ok_or("Informe o comando real do jogo.")?;
    let mut command = Command::new(program);
    command.args(args);
    match prepare() {
        Ok(environment) => {
            command.envs(environment);
        }
        Err(error) => {
            // No partial LD_*/Vulkan activation: an unavailable overlay must
            // not stop the user's game or replace MAKO's launch boundary.
            eprintln!("PeliGames: overlay não carregado: {error}");
        }
    }
    Err(command.exec().to_string())
}

fn shortcut() -> Result<String, String> {
    let path = PathBuf::from(
        env::var_os("MAKO_CONFIG").ok_or("Use MAKO_CONFIG com um perfil do PeliGames.")?,
    );
    let root = path
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "profiles"))
        .and_then(Path::parent)
        .filter(|p| p.file_name().is_some_and(|n| n == "FrameGeneration"))
        .and_then(Path::parent)
        .ok_or("Perfil fora de FrameGeneration/profiles")?;
    let profile = path.with_extension("json");
    let meta = fs::symlink_metadata(&profile).map_err(|e| e.to_string())?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o022 != 0
        || meta.len() > 65536
    {
        return Err("Perfil do overlay sem propriedade/permissões seguras.".into());
    }
    let selected: serde_json::Value =
        serde_json::from_slice(&fs::read(profile).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if selected["backend"] != "mako"
        || selected["enabled"] != true
        || selected["overlay_enabled"] != true
        || selected["flatpak"] == true
    {
        return Err("Overlay não habilitado neste jogo nativo.".into());
    }
    let preferences = root.join("frame-overlay.json");
    let meta = match fs::symlink_metadata(&preferences) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok("F8".into()),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o022 != 0
        || meta.len() > 65536
    {
        return Err("Preferências de atalho inválidas.".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(preferences).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let key = value
        .get("shortcut")
        .and_then(|v| v.as_str())
        .unwrap_or("F8");
    shortcut_key::parse(key)?;
    Ok(key.into())
}
fn append_dso(path: &Path, existing: Option<OsString>) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStrExt;
    if path
        .as_os_str()
        .as_bytes()
        .iter()
        .any(|b| b.is_ascii_whitespace() || *b == b':')
    {
        return Err("O diretório do overlay não pode conter espaços ou dois-pontos.".into());
    }
    let mut value = path.as_os_str().to_owned();
    if let Some(old) = existing {
        if !old.is_empty() {
            value.push(":");
            value.push(old);
        }
    }
    Ok(value)
}
fn main() {
    if let Err(e) = run() {
        eprintln!("PeliGames overlay: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_matches_steam_runtime_string_field_contract() {
        // Mirrors pressure-vessel's required field types, rather than the
        // permissive host loader: a numeric implementation_version must fail.
        #[derive(serde::Deserialize)]
        struct Manifest {
            file_format_version: String,
            layer: RequiredLayerFields,
        }
        #[derive(serde::Deserialize)]
        struct RequiredLayerFields {
            name: String,
            #[serde(rename = "type")]
            kind: String,
            library_path: PathBuf,
            api_version: String,
            implementation_version: String,
            description: String,
            library_arch: String,
        }
        let library = Path::new("/tmp/peligames/libpeligames_game_overlay.so");
        let json = layer_manifest(library);
        let manifest: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest.file_format_version, "1.2.1");
        assert_eq!(manifest.layer.name, LAYER);
        assert_eq!(manifest.layer.kind, "GLOBAL");
        assert_eq!(manifest.layer.library_path, library);
        assert!(!manifest.layer.api_version.is_empty());
        assert_eq!(manifest.layer.implementation_version, "1");
        assert!(!manifest.layer.description.is_empty());
        assert_eq!(manifest.layer.library_arch, "64");
        let mut invalid: serde_json::Value = serde_json::from_str(&json).unwrap();
        invalid["layer"]["implementation_version"] = serde_json::json!(1);
        assert!(serde_json::from_value::<Manifest>(invalid).is_err());
    }
    #[test]
    fn order_works_after_vendor_unsets_enable_mako() {
        let original = "VK_LAYER_MAKO_render:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_MANGOHUD_overlay_x86:VK_LAYER_VALVE_steam_overlay_64";
        assert_eq!(order(original, false).unwrap(), "VK_LAYER_MAKO_render:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_MANGOHUD_overlay_x86:VK_LAYER_PELIGAMES_overlay:VK_LAYER_VALVE_steam_overlay_64");
        assert_eq!(
            order("VK_LAYER_VALVE_steam_overlay_64", true).unwrap(),
            "VK_LAYER_MAKO_render:VK_LAYER_PELIGAMES_overlay:VK_LAYER_VALVE_steam_overlay_64"
        );
        assert_eq!(
            order(&order(original, false).unwrap(), false).unwrap(),
            order(original, false).unwrap()
        );
        assert!(order("invalid-layer", true).is_err());
    }
    #[test]
    fn rejects_unrepresentable_loader_paths() {
        assert!(append_dso(Path::new("/tmp/a b.so"), None).is_err());
        assert!(append_dso(Path::new("/tmp/a:b.so"), None).is_err());
    }
    #[test]
    fn existing_preload_is_preserved() {
        assert_eq!(
            append_dso(Path::new("/tmp/peli.so"), Some("old.so".into())).unwrap(),
            "/tmp/peli.so:old.so"
        );
    }
}
