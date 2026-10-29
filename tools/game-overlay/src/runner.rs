//! General per-game overlay launch boundary, optionally alongside MAKO.
//! Preparation failures preserve the original game invocation and environment.
#[path = "../../storage_layout.rs"]
mod storage_layout;
use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
};
#[path = "../shortcut.rs"]
mod shortcut_key;
mod preload;
mod present_routing;
use present_routing::Generator;
use preload::prepend as append_dso;
const LAYER: &str = "VK_LAYER_PELIGAMES_overlay";
const COUNTER_LAYER: &str = "VK_LAYER_PELIGAMES_present_counter";
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
fn counter_manifest(library:&Path)->String {
    let mut manifest:serde_json::Value=serde_json::from_str(&layer_manifest(library)).expect("static layer manifest");
    manifest["layer"]["name"]=COUNTER_LAYER.into();
    manifest["layer"]["description"]="PeliGames passive frame generation counter".into();
    manifest["layer"]["functions"]["vkNegotiateLoaderLayerInterfaceVersion"]="peliCounterNegotiateLoaderLayerInterfaceVersion".into();
    manifest.to_string()
}
fn counter_order(existing: String, generator: Option<Generator>) -> String {
    let mut layers: Vec<_> = existing.split(':').filter(|s| !s.is_empty() && *s != COUNTER_LAYER).collect();
    match generator {
        Some(Generator::Afmf) if layers.contains(&"VK_LAYER_AFMF") => layers.push(COUNTER_LAYER),
        Some(Generator::Mako) if layers.contains(&"VK_LAYER_MAKO_render") => {
            // Only this passive observer goes above MAKO. The renderer/input
            // boundary and all vendor/Steam/MangoHUD ordering stay unchanged.
            let index = layers.iter().position(|name| *name == "VK_LAYER_MAKO_render").unwrap();
            layers.insert(index, COUNTER_LAYER);
        }
        _ => {}
    }
    layers.join(":")
}
// An explicit override must include the game's additive NR Control/int4/ReShade paths.
fn explicit_paths(mut current: Option<OsString>, additional: Option<OsString>) -> Result<Option<OsString>, String> {
    if let Some(additional) = additional {
        let paths: Vec<_> = env::split_paths(&additional).filter(|path| !path.as_os_str().is_empty()).collect();
        for path in paths.into_iter().rev() { current = Some(prepend(&path, current)?); }
    }
    Ok(current)
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
        .filter(|n| !n.is_empty() && *n != LAYER && *n != COUNTER_LAYER)
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
    let mut layer_paths = explicit_paths(env::var_os("VK_LAYER_PATH"), env::var_os("VK_ADD_LAYER_PATH"))?;
    if let Some(private) = env::var_os("VK_IMPLICIT_LAYER_PATH") {
        // Explicitly requesting MAKO must still find its private manifest.
        for p in env::split_paths(&private) {
            layer_paths = Some(prepend(&p, layer_paths)?);
        }
    }
    let paths = prepend(&manifests, layer_paths)?;
    let afmf=env::var("AFMF_ENABLE").ok().as_deref()==Some("1");
    let ordered = order(&env::var("VK_INSTANCE_LAYERS").unwrap_or_default(),
        env::var("ENABLE_MAKO").ok().as_deref() == Some("1"))?;
    let generator = present_routing::detect(afmf, &ordered);
    if generator.is_some() {
        let path=manifests.join("VkLayer_PELIGAMES_present_counter.json");
        let content=counter_manifest(&library);
        let intact=fs::symlink_metadata(&path).is_ok_and(|m|m.is_file()&&m.uid()==unsafe{libc::geteuid()}&&m.mode()&0o077==0)
            && fs::read_to_string(&path).ok().as_deref()==Some(content.as_str());
        if !intact {
            let temporary=manifests.join(format!(".counter-{}.json",std::process::id()));
            let result=(||{fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)?.write_all(content.as_bytes())?;fs::rename(&temporary,&path)})();
            if let Err(error)=result {let _=fs::remove_file(&temporary);return Err(error.to_string());}
        }
    }
    let layers = counter_order(ordered, generator);
    let preload = preload::input(
        &library,
        env::var_os("LD_PRELOAD"),
        env::var_os("PROTON_ENABLE_WAYLAND").as_deref(),
        env::var_os("PROTON_USE_WAYLAND").as_deref(),
    )?;
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
    let rest: Vec<OsString> = args.collect();
    let words: Vec<OsString> = std::iter::once(program.clone()).chain(rest.iter().cloned()).collect();
    let mut command = Command::new(program);
    command.args(rest);
    if env::var("PELI_OVERLAY_DISABLED").ok().as_deref()==Some("1"){return Err(command.exec().to_string());}
    let log = config_root().and_then(|root| session_log(&root, &words));
    match prepare() {
        Ok(environment) => {
            command.envs(environment);
            note(log.as_deref(), "Runner: camada Vulkan e entrada preparadas.");
        }
        Err(error) => {
            // No partial LD_*/Vulkan activation: an unavailable overlay must
            // not stop the user's game or replace MAKO's launch boundary.
            eprintln!("PeliGames: overlay não carregado: {error}");
            note(log.as_deref(), &format!("Runner: overlay não carregado: {error}"));
        }
    }
    if let Some(log) = &log { command.env("PELI_OVERLAY_LOG", log); }
    Err(command.exec().to_string())
}

/// The PeliGames folder this launch belongs to (general overlay or MAKO profile).
fn config_root() -> Option<PathBuf> {
    let root = if env::var("PELI_OVERLAY_GENERAL").ok().as_deref() == Some("1") {
        PathBuf::from(env::var_os("PELI_OVERLAY_CONFIG_ROOT")?)
    } else {
        storage_layout::from_profile(&PathBuf::from(env::var_os("MAKO_CONFIG")?))?
    };
    root.is_absolute().then_some(root)
}
const SESSIONS_KEPT: usize = 30;
/// One diagnosis file per launch, read by the app to explain why the overlay
/// did (not) appear. The command words let it find the game's folder.
/// Any failure only disables the diagnosis, never the game.
fn session_log(root: &Path, words: &[OsString]) -> Option<PathBuf> {
    let directory = storage_layout::join(&root, "overlay-logs");
    match fs::DirBuilder::new().recursive(true).mode(0o700).create(&directory) {
        Ok(()) => fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).ok()?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return None,
    }
    let meta = fs::symlink_metadata(&directory).ok()?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return None;
    }
    let mut old: Vec<PathBuf> = fs::read_dir(&directory).ok()?.flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "log")).collect();
    old.sort();
    for path in old.iter().take(old.len().saturating_sub(SESSIONS_KEPT - 1)) {
        let _ = fs::remove_file(path);
    }
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let path = directory.join(format!("{seconds:012}-{}.log", std::process::id()));
    let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
        .custom_flags(libc::O_NOFOLLOW).open(&path).ok()?;
    let text: String = words.iter().map(|w| w.to_string_lossy().replace(['\n', '\t'], " "))
        .map(|w| format!("cmd\t{w}\n")).collect();
    file.write_all(text.as_bytes()).ok()?;
    Some(path)
}
fn note(log: Option<&Path>, message: &str) {
    let Some(log) = log else { return };
    if let Ok(mut file) = fs::OpenOptions::new().append(true).custom_flags(libc::O_NOFOLLOW).open(log) {
        let _ = file.write_all(format!("runner\t{}\n", message.replace(['\n', '\t'], " ")).as_bytes());
    }
}

fn shortcut() -> Result<String, String> {
    let root = if env::var("PELI_OVERLAY_GENERAL").ok().as_deref()==Some("1") {
        PathBuf::from(env::var_os("PELI_OVERLAY_CONFIG_ROOT").ok_or("Pasta do PeliGames ausente")?)
    } else {
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
        root.to_owned()
    };
    if !root.is_absolute(){return Err("Pasta do overlay inválida".into());}
    let root=if root.file_name().is_some_and(|n|n=="components"){root.parent().ok_or("Pasta do overlay inválida")?.to_owned()}else{root};
    let preferences = storage_layout::join(&root, "frame-overlay.json");
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
    fn afmf_hud_stays_between_generation_and_passive_counter() {
        let original = "VK_LAYER_NR:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64";
        let layers = counter_order(order(original, false).unwrap(), Some(Generator::Afmf));
        assert_eq!(layers, format!("{LAYER}:VK_LAYER_NR:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64:{COUNTER_LAYER}"));
        assert_eq!(counter_order(layers.clone(), Some(Generator::Afmf)), layers);
    }

    #[test]
    fn passive_counter_is_after_afmf_and_absent_without_a_generator() {
        let layers=counter_order(order("VK_LAYER_NR:VK_LAYER_AFMF",false).unwrap(),Some(Generator::Afmf));
        assert_eq!(layers,format!("{LAYER}:VK_LAYER_NR:VK_LAYER_AFMF:{COUNTER_LAYER}"));
        assert_eq!(counter_order(layers.clone(),Some(Generator::Afmf)),layers);
        assert!(!counter_order(order("VK_LAYER_NR",false).unwrap(),None).contains(COUNTER_LAYER));
        let manifest:serde_json::Value=serde_json::from_str(&counter_manifest(Path::new("/fixture/liboverlay.so"))).unwrap();
        assert_eq!(manifest["layer"]["functions"]["vkNegotiateLoaderLayerInterfaceVersion"],"peliCounterNegotiateLoaderLayerInterfaceVersion");
    }
    #[test]
    fn mako_source_counter_keeps_the_existing_renderer_and_vendor_order() {
        for original in [
            "VK_LAYER_MAKO_render:VK_LAYER_VALVE_steam_overlay_64",
            "VK_LAYER_MAKO_render:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_MANGOHUD_overlay_x86:VK_LAYER_VALVE_steam_overlay_64",
            "VK_LAYER_MAKO_render:VK_LAYER_VKBASALT_post_processing:VK_LAYER_VALVE_steam_overlay_64",
        ] {
            let previous = order(original, false).unwrap();
            let generator = present_routing::detect(false, &previous);
            let layers = counter_order(previous.clone(), generator);
            assert_eq!(layers, format!("{COUNTER_LAYER}:{previous}"));
            assert_eq!(counter_order(order(&layers, false).unwrap(), generator), layers,
                "nested runners must not move the renderer above MangoHUD/shaders");
        }
    }

    #[test]
    fn additive_mod_layers_remain_discoverable_under_an_explicit_override() {
        let paths = explicit_paths(Some("/steam:/mako".into()), Some("/game/control:/game/int4".into())).unwrap().unwrap();
        assert_eq!(paths, "/game/control:/game/int4:/steam:/mako");
        assert_eq!(explicit_paths(Some(paths.clone()), Some("/game/control:/game/int4".into())).unwrap(), Some(paths));
        assert_eq!(explicit_paths(None, None).unwrap(), None);
    }
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
    fn each_launch_gets_a_private_diagnosis_and_old_ones_are_pruned() {
        let root = env::temp_dir().join(format!("peli-overlay-session-{}", std::process::id()));
        let logs = storage_layout::join(&root, "overlay-logs");
        fs::create_dir_all(&logs).unwrap();
        fs::set_permissions(&logs, fs::Permissions::from_mode(0o700)).unwrap();
        for n in 0..SESSIONS_KEPT + 5 { fs::write(logs.join(format!("{n:012}-1.log")), "").unwrap(); }
        let path = session_log(&root, &["/games/Hades/Hades.exe".into(), "two\nlines".into()]).unwrap();
        note(Some(&path), "Runner: camada Vulkan e entrada preparadas.");
        assert_eq!(fs::read_to_string(&path).unwrap(),
            "cmd\t/games/Hades/Hades.exe\ncmd\ttwo lines\nrunner\tRunner: camada Vulkan e entrada preparadas.\n");
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(fs::read_dir(&logs).unwrap().count(), SESSIONS_KEPT);
        assert!(!logs.join(format!("{:012}-1.log", 0)).exists());
        fs::remove_dir_all(root).unwrap();
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
    #[test]
    fn nested_runners_replace_their_own_payload_and_preserve_other_overlays() {
        for name in ["libpeligames_game_overlay.so", "libpeligames_overlay_audit.so"] {
            let current = format!("/general/{name}");
            let old = format!("/mako/{name}:steam.so /general/{name}:mangohud.so");
            assert_eq!(
                append_dso(Path::new(&current), Some(old.into())).unwrap(),
                OsString::from(format!("{current}:steam.so:mangohud.so"))
            );
        }
    }
}
