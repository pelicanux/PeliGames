use crate::{dll_valid, validate_runtime_at, versions, Settings};
use crate::{profile_key, write};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}
fn toml(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub fn config(s: &Settings) -> String {
    let mut out=format!("version = 2\n[global]\ndll = {}\nallow_fp16 = {}\n[[profile]]\nname = \"PeliGames\"\nmultiplier = {}\nflow_scale = {:.2}\nperformance_mode = {}\n",toml(&s.dll),s.allow_fp16,s.multiplier,s.flow_scale,s.performance);
    out.push_str(&format!("frame_generation_provisioned = true\nframe_generation_enabled = {}\nadaptive = {}\ntarget_fps = {}\nadaptive_max_multiplier = {}\nbase_fps_cap = {}\nframe_generation_refresh_threshold = {}\nultra_performance = {}\nscaling_enabled = {}\nscaling_method = {}\nscaling_factor = {:.2}\nscaling_sharpness = {:.2}\n",s.live_enabled,s.adaptive,s.target_fps,s.multiplier,s.base_fps_cap,s.refresh_threshold,s.ultra_performance,s.scaling_enabled,toml(&s.scaling_method),s.scaling_factor,s.scaling_sharpness));
    if !s.gpu.is_empty() {
        out.push_str(&format!("gpu = {}\n", toml(&s.gpu)));
    }
    out
}

pub fn same_launch_profile(previous: &Settings, next: &Settings) -> bool {
    previous.backend == next.backend
        && previous.runtime_version == next.runtime_version
        && previous.dll == next.dll
        && previous.enabled == next.enabled
        && previous.flatpak == next.flatpak
        && previous.gpu == next.gpu
        && previous.allow_fp16 == next.allow_fp16
        && previous.ultra_performance == next.ultra_performance
        && previous.scaling_enabled == next.scaling_enabled
        && previous.scaling_method == next.scaling_method
        && previous.scaling_factor == next.scaling_factor
        && previous.scaling_sharpness == next.scaling_sharpness
}

/// Edits only reloadable keys, retaining pause state and startup configuration.
pub fn live_config(text: &str, settings: &Settings) -> Result<String, String> {
    let values = [
        ("multiplier", settings.multiplier.to_string()),
        ("adaptive_max_multiplier", settings.multiplier.to_string()),
        ("target_fps", settings.target_fps.to_string()),
        ("flow_scale", format!("{:.2}", settings.flow_scale)),
        ("performance_mode", settings.performance.to_string()),
        ("adaptive", settings.adaptive.to_string()),
    ];
    let mut counts = [0; 6];
    let mut profile = false;
    let mut output = String::new();
    for line in text.lines() {
        if line.trim().starts_with('[') {
            profile = line.trim() == "[[profile]]";
        }
        let key = line.split_once('=').map(|(key, _)| key.trim());
        if let Some((index, (key, value))) = values
            .iter()
            .enumerate()
            .find(|(_, (name, _))| profile && Some(*name) == key)
        {
            counts[index] += 1;
            output.push_str(&format!("{key} = {value}\n"));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if counts != [1; 6] {
        return Err("O perfil MAKO não tem um formato de recarga reconhecido. As configurações foram preservadas.".into());
    }
    for (key, value) in [
        ("base_fps_cap", settings.base_fps_cap),
        (
            "frame_generation_refresh_threshold",
            settings.refresh_threshold,
        ),
    ] {
        let mut edited = String::new();
        let mut in_profile = false;
        let mut found = false;
        for line in output.lines() {
            if line.trim().starts_with('[') {
                if in_profile && !found {
                    edited.push_str(&format!("{key} = {value}\n"));
                    found = true;
                }
                in_profile = line.trim() == "[[profile]]";
            }
            if in_profile
                && line
                    .split_once('=')
                    .is_some_and(|(name, _)| name.trim() == key)
            {
                if found {
                    return Err("Chave MAKO duplicada; perfil preservado.".into());
                }
                edited.push_str(&format!("{key} = {value}\n"));
                found = true;
            } else {
                edited.push_str(line);
                edited.push('\n');
            }
        }
        if !found && in_profile {
            edited.push_str(&format!("{key} = {value}\n"));
        }
        output = edited;
    }
    Ok(output)
}

pub fn update_live(
    storage: &Path,
    active_profile: String,
    settings: Settings,
) -> Result<(), String> {
    let _guard = peligames_mod_config::DirectoryLock::acquire(storage)?;
    crate::settings::check_settings(&settings)?;
    let script = Path::new(&active_profile);
    let profiles = storage.join("profiles");
    if script.parent() != Some(profiles.as_path())
        || script.extension().and_then(|v| v.to_str()) != Some("sh")
        || !script
            .file_stem()
            .and_then(|v| v.to_str())
            .is_some_and(peligames_mod_config::names::valid)
    {
        return Err("Selecione um perfil de execução MAKO do PeliGames.".into());
    }
    let path = script.with_extension("json");
    for path in [&path, &script.with_extension("toml")] {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o022 != 0
            || metadata.len() > 65536
        {
            return Err("Perfil MAKO sem propriedade ou permissões válidas.".into());
        }
    }
    let mut previous: Settings =
        serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if !previous.enabled || previous.backend != "mako" {
        return Err("O perfil precisa ter MAKO habilitado antes de iniciar o jogo.".into());
    }
    // Ignore all non-live fields supplied by the UI. They cannot change the
    // DLL, runner, startup wrapper, overlay opt-in, or another launcher's state.
    let switch_changed = previous.live_enabled != settings.live_enabled;
    previous.live_enabled = settings.live_enabled;
    previous.base_fps_cap = settings.base_fps_cap;
    previous.refresh_threshold = settings.refresh_threshold;
    previous.multiplier = settings.multiplier;
    previous.adaptive = settings.adaptive;
    previous.target_fps = settings.target_fps;
    previous.flow_scale = settings.flow_scale;
    previous.performance = settings.performance;
    let toml = script.with_extension("toml");
    let old = fs::read_to_string(&toml).map_err(|e| e.to_string())?;
    let mut updated = live_config(&old, &previous)?;
    if switch_changed {
        updated = updated
            .lines()
            .map(|line| {
                if line
                    .split_once('=')
                    .is_some_and(|(key, _)| key.trim() == "frame_generation_enabled")
                {
                    format!("frame_generation_enabled = {}", previous.live_enabled)
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
    }
    let paused = updated.lines().any(|line| {
        line.split_once('=').is_some_and(|(key, value)| {
            key.trim() == "frame_generation_enabled" && value.trim() == "false"
        })
    });
    write(
        &path,
        &serde_json::to_vec(&previous).map_err(|e| e.to_string())?,
        0o600,
    )?;
    write(&toml, updated.as_bytes(), 0o600)?;
    write(&script.with_extension("live.json"),&serde_json::to_vec(&serde_json::json!({
            "multiplier":previous.multiplier,"adaptive":previous.adaptive,"target_fps":previous.target_fps,
            "flow_scale":previous.flow_scale,"performance":previous.performance,"paused":paused,
            "base_fps_cap":previous.base_fps_cap,"refresh_threshold":previous.refresh_threshold,
            "scaling_enabled":previous.scaling_enabled,"scaling_method":previous.scaling_method,"scaling_sharpness":previous.scaling_sharpness,"ultra_performance":previous.ultra_performance
        })).map_err(|e|e.to_string())?,0o600)
}

pub fn selection_file(profiles: &Path, key: &str, label: &str) -> Result<PathBuf, String> {
    peligames_addon_kit::files::safe_directory(profiles, true)?;
    peligames_addon_kit::files::safe_directory(&profiles.join("selections"), true)?;
    let base = peligames_mod_config::names::allocate(
        &profiles.join("selections"),
        label,
        &profile_key(key),
    )?;
    let path = base.with_extension("json");
    let old = profiles.join(profile_key(key) + ".selection.json");
    if !path.exists() && old.is_file() {
        fs::rename(old, &path).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

pub fn wrapper_script(
    settings: &Settings,
    config: &Path,
    runtime: &Path,
    launcher: &Path,
) -> String {
    if settings.flatpak {
        return crate::flatpak::wrapper(config);
    }
    format!("#!/bin/sh\n# PeliGames per-game MAKO profile; no global activation.\nexport MAKO_CONFIG={} MAKO_PROFILE=PeliGames MAKO_LAUNCH_CONFIG={}\nexport DISABLE_LSFG=1 DISABLE_LSFGVK=1 DISABLE_MAKO=0 ENABLE_MAKO=1 MAKO_ALLOW_COMPETING_LAYERS=0\nunset MAKO_ENV\n{}",quote(&config.to_string_lossy()),quote(&launcher.to_string_lossy()),crate::mangohud::launch(runtime,config,None))
}
pub fn checked_profile(storage: &Path, script: &Path) -> Result<PathBuf, String> {
    let parent = storage.join("profiles");
    if script.parent() != Some(parent.as_path())
        || script.extension().is_none_or(|ext| ext != "sh")
        || !script
            .file_stem()
            .and_then(|v| v.to_str())
            .is_some_and(peligames_mod_config::names::valid)
    {
        return Err("Choose a managed MAKO profile".into());
    }
    peligames_addon_kit::files::check_directory(&parent)?;
    Ok(script.with_extension("json"))
}
pub fn read(storage: &Path, script: &Path) -> Result<Settings, String> {
    let path = checked_profile(storage, script)?;
    serde_json::from_slice(&read_profile_bytes(&path)?).map_err(|e| e.to_string())
}
pub fn prepare(
    storage: &Path,
    key: &str,
    label: &str,
    directory: &Path,
    mut settings: Settings,
    previous: Option<&Path>,
) -> Result<PathBuf, String> {
    peligames_addon_kit::files::safe_directory(storage, true)?;
    let _guard = peligames_mod_config::DirectoryLock::acquire(storage)?;
    crate::settings::check_settings(&settings)?;
    if settings.runtime_version.is_empty() {
        settings.runtime_version = versions::current(storage, "mako")?;
    }
    if !versions::compatible("mako", &settings.runtime_version) {
        return Err("Unsupported MAKO runtime version".into());
    }
    let runtime = versions::location(storage, "mako", &settings.runtime_version)?;
    if settings.enabled {
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("Choose an existing executable directory".into());
        }
        crate::support::check_local_fg(directory)?;
        if !settings.flatpak {
            validate_runtime_at(&runtime, "mako")?;
        }
        let dll = Path::new(&settings.dll);
        if !dll.is_absolute()
            || !dll_valid(dll)
            || !dll
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Lossless.dll"))
        {
            return Err("Choose the Lossless.dll from your Lossless Scaling installation".into());
        }
        settings.dll = dll
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into();
        if settings.flatpak {
            let status =
                crate::flatpak::inspect(storage, key, &settings.dll, &settings.runtime_version)?;
            if !status.ready {
                return Err(status.message);
            }
        }
    }
    let reused = previous
        .filter(|path| read(storage, path).is_ok_and(|old| same_launch_profile(&old, &settings)));
    let identity = profile_key(&format!(
        "{key}:{}",
        serde_json::to_string(&settings).map_err(|e| e.to_string())?
    ));
    let profiles = storage.join("profiles");
    peligames_addon_kit::files::safe_directory(&profiles, true)?;
    let base = match reused {
        Some(path) => path.with_extension(""),
        None => peligames_mod_config::names::allocate(&profiles, label, &identity)?,
    };
    let script = base.with_extension("sh");
    let config_path = base.with_extension("toml");
    let launcher = base.with_extension("launch.conf");
    if !launcher.exists() {
        write(&launcher, b"version=1\ndisable_steam_overlay=0\n", 0o600)?;
    }
    write(&config_path, config(&settings).as_bytes(), 0o600)?;
    write(
        &script,
        wrapper_script(&settings, &config_path, &runtime, &launcher).as_bytes(),
        0o700,
    )?;
    let serialized = serde_json::to_vec(&settings).map_err(|e| e.to_string())?;
    write(&base.with_extension("json"), &serialized, 0o600)?;
    write(&selection_file(&profiles, key, label)?, &serialized, 0o600)?;
    Ok(script)
}

/// Reading a selection must not allocate files or rename a legacy profile.
pub fn selected(storage: &Path, key: &str) -> Result<Option<Settings>, String> {
    let profiles = storage.join("profiles");
    let identity = profile_key(key);
    peligames_addon_kit::files::check_directory(&profiles)?;
    peligames_addon_kit::files::check_directory(&profiles.join("selections"))?;
    let legacy = profiles.join(format!("{identity}.selection.json"));
    let path = peligames_mod_config::names::find(&profiles.join("selections"), &identity)
        .map(|base| base.with_extension("json"))
        .filter(|p| p.is_file())
        .or_else(|| legacy.is_file().then_some(legacy));
    path.map(|path| {
        let bytes = read_profile_bytes(&path)?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    })
    .transpose()
}
fn read_profile_bytes(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 65536 {
        return Err("Invalid MAKO selection".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("MAKO selection is too large".into());
    }
    Ok(bytes)
}
