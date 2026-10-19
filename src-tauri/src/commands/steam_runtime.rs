//! A stable Steam launch command; mutable settings live in a private JSON file.
//! Neither editing a profile nor starting its helper modifies Steam's VDF files.
use super::external_library::ExternalGame;
use crate::core::advanced_settings::AdvancedSettings;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

#[path = "../../launch-runtime/src/profile.rs"]
pub(super) mod contract;
use contract as profile;
#[path = "../../launch-runtime/src/profile_io.rs"]
pub(super) mod profile_io;
use contract::{Profile, MARKER, MAX_BYTES, VERSION};

const DIRECTORY: &str = "peligames-launcher";
const KIND: &str = "peligames-steam-profile";
pub(super) const HELPER: &[u8] = include_bytes!(env!("PELIGAMES_LAUNCH_HELPER_EMBEDDED"));
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Serialize, Deserialize)]
struct Source {
    kind: String,
    name: String,
    flatpak: bool,
    advanced: AdvancedSettings,
    mod_options: Option<String>,
}

fn steam_config(flatpak: bool) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let mut found = Vec::new();
    for root in super::steam_installation::roots(&home, flatpak) {
        if !root.join("config/config.vdf").is_file() {
            continue;
        }
        let path = fs::canonicalize(root.join("config")).map_err(|error| error.to_string())?;
        if !found.contains(&path) {
            found.push(path);
        }
    }
    if found.len() != 1 {
        return Err("Não foi possível identificar uma única instalação da Steam para o auxiliar. Abra a Steam desejada e tente novamente.".into());
    }
    Ok(found.remove(0))
}

fn directory(flatpak: bool) -> Result<PathBuf, String> {
    Ok(steam_config(flatpak)?.join(DIRECTORY))
}

pub(super) fn private_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::DirBuilder::new().mode(0o700).create(path) {
                Ok(()) => {},
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return private_directory(path),
                Err(error) => return Err(error.to_string()),
            }
        }
        Err(error) => return Err(error.to_string()),
        Ok(meta)
            if !meta.is_dir()
                || meta.file_type().is_symlink()
                || meta.uid() != unsafe { libc::geteuid() }
                || meta.mode() & 0o022 != 0 =>
        {
            return Err("Diretório do auxiliar Steam inválido ou sem acesso exclusivo.".into());
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.uid() != unsafe { libc::geteuid() }
        {
            return Err("Destino do auxiliar Steam inválido.".into());
        }
    }
    let temporary = path.with_file_name(format!(".peligames-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        fs::File::open(path.parent().ok_or("Destino sem diretório.")?)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn read_at(path: &Path, app: u32) -> Result<Option<Profile>, String> {
    let value = profile_io::legacy(path, app)?;
    Ok(value.map(|mut profile| {
        // Legacy GUI callers keep their AppID; the file remains universal on disk.
        profile.version = VERSION;
        profile.app_id = app;
        profile.id = None;
        profile
    }))
}

fn source(profile: &Profile) -> Result<Source, String> {
    let source: Source =
        serde_json::from_value(profile.source.clone()).map_err(|error| error.to_string())?;
    if source.kind != KIND {
        return Err("Origem do perfil Steam inválida.".into());
    }
    Ok(source)
}

fn build(app: u32, source: &Source) -> Result<Profile, String> {
    let mut advanced = source.advanced.clone();
    advanced.launch_options = super::mod_launch_options::merge(
        advanced.launch_options.as_deref(),
        None,
        source.mod_options.as_deref(),
    )?;
    advanced.validate()?;
    let mut command = crate::core::advanced_settings::configured_launch_command(
        Path::new(MARKER),
        Some(&advanced),
    )?;
    crate::core::mangohud_config::apply_game_logs(&mut command, &source.name)?;
    let mut custom = Command::new("true");
    if let Some(line) = &advanced.launch_options {
        super::nexus_launch_options::apply_custom(&mut custom, line)?;
    }
    let environment = command
        .get_envs()
        .chain(custom.get_envs())
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|value| value.to_string_lossy().into_owned()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut profile = Profile {
        version: VERSION,
        app_id: app,
        id: None, installation: None,
        command: std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|word| word.to_string_lossy().into_owned())
            .collect(),
        environment,
        extra_args: custom
            .get_args()
            .map(|word| word.to_string_lossy().into_owned())
            .collect(),
        source: serde_json::to_value(source).map_err(|error| error.to_string())?,
    };
    crate::core::release_features::restrict(&mut profile.command, &mut profile.environment, crate::core::release_features::enabled());
    profile.validate(app)?;
    Ok(profile)
}

pub(super) fn build_for(name: &str, advanced: &AdvancedSettings, mods: Option<String>) -> Result<Profile, String> {
    build(1, &Source { kind: KIND.into(), name: name.into(), flatpak: false, advanced: advanced.clone(), mod_options: mods })
}

fn publish_at(directory: &Path, profile: &Profile) -> Result<(), String> {
    if let Some(installation) = &profile.installation {
        let saved = super::launch_profiles::save(Path::new(installation), profile.clone())?;
        private_directory(directory)?;
        private_directory(&directory.join("profiles"))?;
        let reference = profile_io::Reference { version: 1, app_id: profile.app_id,
            pelirun_reference: saved.id, directory: PathBuf::from(saved.profiles) };
        return atomic_write(&directory.join("profiles").join(format!("{}.json", profile.app_id)),
            &serde_json::to_vec_pretty(&reference).map_err(|e| e.to_string())?, 0o600);
    }
    let bytes = serde_json::to_vec_pretty(profile).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Perfil do auxiliar Steam muito grande.".into());
    }
    private_directory(directory)?;
    let profiles = directory.join("profiles");
    private_directory(&profiles)?;
    atomic_write(
        &profiles.join(format!("{}.json", profile.app_id)),
        &bytes,
        0o600,
    )
}

pub(super) fn prefix(game: &ExternalGame) -> Result<String, String> {
    let app = contract::app_id(game.app_id.as_deref().ok_or("Steam AppID ausente.")?)?;
    let binary = directory(game.flatpak)?.join("pelirun");
    let binary = binary.to_str().ok_or("O caminho da Steam não usa UTF-8.")?;
    Ok(format!(
        "'{}' --app {app} -- ",
        binary.replace('\'', "'\"'\"'")
    ))
}

/// Read the previous mod selection even when it is explicitly None (removed).
/// Legacy launch-option records are imported only once, at profile creation.
pub(super) fn mod_options(game: &ExternalGame) -> Result<Option<String>, String> {
    let app = contract::app_id(game.app_id.as_deref().ok_or("Steam AppID ausente.")?)?;
    if let Some(profile) = read_at(
        &directory(game.flatpak)?
            .join("profiles")
            .join(format!("{app}.json")),
        app,
    )? {
        return Ok(source(&profile)?.mod_options);
    }
    super::steam_launch_options::runtime_mod_options(&app.to_string())
}

pub(super) fn has_profile(game: &ExternalGame) -> Result<bool, String> {
    let app = contract::app_id(game.app_id.as_deref().ok_or("Steam AppID ausente.")?)?;
    Ok(read_at(
        &directory(game.flatpak)?
            .join("profiles")
            .join(format!("{app}.json")),
        app,
    )?
    .is_some())
}

pub(super) fn prepare(
    game: &ExternalGame,
    advanced: &AdvancedSettings,
    mods: Option<String>,
) -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|_| "Perfil Steam ocupado.")?;
    if HELPER.len() < 64 {
        return Err("O auxiliar Steam não está disponível neste pacote.".into());
    }
    let app = contract::app_id(game.app_id.as_deref().ok_or("Steam AppID ausente.")?)?;
    let source = Source {
        kind: KIND.into(),
        name: game.name.clone(),
        flatpak: game.flatpak,
        advanced: advanced.clone(),
        mod_options: mods,
    };
    let mut profile = build(app, &source)?;
    if !game.flatpak { profile.installation = Some(super::launch_profiles::installation(Path::new(&game.path))?.to_string_lossy().into_owned()); }
    let directory = directory(game.flatpak)?;
    private_directory(&directory)?;
    let profile_directory = directory.join("profiles");
    private_directory(&profile_directory)?;
    let profile_path = profile_directory.join(format!("{app}.json"));
    let previous = read_at(&profile_path, app)?;
    let binary = directory.join("pelirun");
    // Existing processes keep their inode; a replacement never truncates a
    // running helper. No per-profile executable is generated.
    let metadata = fs::symlink_metadata(&binary);
    let current = matches!(&metadata, Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() && meta.uid() == unsafe { libc::geteuid() } && meta.permissions().mode() & 0o111 != 0 && meta.permissions().mode() & 0o022 == 0)
        && fs::read(&binary).is_ok_and(|bytes| bytes == HELPER);
    if !current {
        atomic_write(&binary, HELPER, 0o700)?;
    }
    if let Err(error) = publish_at(&directory, &profile) {
        let rollback = match previous {
            Some(previous) => publish_at(&directory, &previous),
            None => match fs::remove_file(&profile_path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            },
        };
        rollback.map_err(|rollback| {
            format!("{error}; não foi possível restaurar o perfil: {rollback}")
        })?;
        return Err(error);
    }
    Ok(())
}

/// Only profiles owned by this application participate in reset and refresh.
/// The binary stays in place: a missing profile means normal Steam execution.
pub(super) fn profile_directories() -> Result<Vec<PathBuf>, String> {
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let mut paths = Vec::new();
    for flatpak in [false, true] {
        for root in super::steam_installation::roots(&home, flatpak) {
            let Ok(config) = fs::canonicalize(root.join("config")) else {
                continue;
            };
            let directory = config.join(DIRECTORY);
            if !directory.exists() {
                continue;
            }
            private_directory(&directory)?;
            let profiles = directory.join("profiles");
            if !paths.contains(&profiles) {
                paths.push(profiles);
            }
        }
    }
    Ok(paths)
}

/// Refresh the shared helper before opening the public launcher. Old profile
/// sources remain intact so development builds can restore them on save.
pub(crate) fn enforce_release_policy() -> Result<(), String> {
    if crate::core::release_features::enabled() { return Ok(()); }
    let _lock = LOCK.lock().map_err(|_| "Perfil Steam ocupado.")?;
    for profiles in profile_directories()? {
        let directory = profiles.parent().ok_or("Diretório do perfil inválido")?;
        atomic_write(&directory.join("pelirun"), HELPER, 0o700)?;
        if !profiles.is_dir() { continue; }
        for entry in fs::read_dir(&profiles).map_err(|e|e.to_string())?.flatten() {
            let path=entry.path();
            let Some(app)=path.file_stem().and_then(|s|s.to_str()).and_then(|s|contract::app_id(s).ok()) else {continue;};
            if path.extension().is_none_or(|s|s!="json"){continue;}
            if let Some(mut profile)=read_at(&path,app)? {
                crate::core::release_features::restrict(&mut profile.command,&mut profile.environment,false);
                profile.validate(app)?;
                publish_at(directory,&profile)?;
            }
        }
    }
    Ok(())
}

pub(super) fn refresh_monitoring() -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|_| "Perfil Steam ocupado.")?;
    for profiles in profile_directories()? {
        if !profiles.exists() {
            continue;
        }
        private_directory(&profiles)?;
        for entry in fs::read_dir(&profiles).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(app) = path
                .file_stem()
                .and_then(|value| value.to_str())
                .and_then(|value| contract::app_id(value).ok())
            else {
                continue;
            };
            let Some(previous) = read_at(&path, app)? else {
                continue;
            };
            let mut next = build(app, &source(&previous)?)?;
            next.installation = previous.installation;
            publish_at(profiles.parent().ok_or("Perfil sem diretório.")?, &next)?;
        }
    }
    Ok(())
}

/// Legacy mod writers must not append a second copy outside the fixed helper.
pub(super) fn manages_config(config: &Path, app: &str) -> bool {
    let Ok(app) = contract::app_id(app) else {
        return false;
    };
    let Some(root) = config
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return false;
    };
    let path = root
        .join("config")
        .join(DIRECTORY)
        .join("profiles")
        .join(format!("{app}.json"));
    read_at(&path, app)
        .ok()
        .flatten()
        .and_then(|profile| source(&profile).ok())
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_update_is_atomic_private_and_keeps_steam_line_stable() {
        let root = std::env::temp_dir().join(format!("peligames-runtime-{}", uuid::Uuid::new_v4()));
        let original = Profile {
            version: VERSION,
            app_id: 42,
            id: None, installation: None,
            command: vec![MARKER.into()],
            environment: BTreeMap::new(),
            extra_args: vec!["-before".into()],
            source: serde_json::Value::Null,
        };
        publish_at(&root, &original).unwrap();
        let path = root.join("profiles/42.json");
        let old = fs::File::open(&path).unwrap();
        let mut next = original.clone();
        next.extra_args = vec!["-after".into()];
        publish_at(&root, &next).unwrap();
        assert_eq!(read_at(&path, 42).unwrap().unwrap().extra_args, ["-after"]);
        assert_eq!(
            serde_json::from_reader::<_, Profile>(old)
                .unwrap()
                .extra_args,
            ["-before"]
        );
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn profile_writer_rejects_links_and_writes_only_its_destination() {
        let root =
            std::env::temp_dir().join(format!("peligames-runtime-links-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("profiles")).unwrap();
        let outside = root.join("unrelated");
        fs::write(&outside, b"keep").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("profiles/42.json")).unwrap();
        assert!(atomic_write(&root.join("profiles/42.json"), b"replacement", 0o600).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}
