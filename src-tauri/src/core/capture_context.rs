//! Resolve capture settings from the authenticated game's process, never the last UI tab.
use super::{CaptureOptions, CaptureStatus};
use std::{collections::HashMap, fs, os::unix::fs::MetadataExt, path::PathBuf, sync::Mutex};

pub struct Bindings {
    root: PathBuf,
    processes: PathBuf,
    keys: Mutex<HashMap<(u32, u64), String>>,
}
impl Bindings {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            processes: "/proc".into(),
            keys: Default::default(),
        }
    }
    pub fn options(&self, peer: u32) -> Result<CaptureOptions, String> {
        let started = super::capture_lifetime::identity(&self.processes, peer)
            .ok_or("O processo do jogo não está mais disponível.")?;
        let cached = self
            .keys
            .lock()
            .map_err(|e| e.to_string())?
            .get(&(peer, started))
            .cloned();
        if let Some(key) = cached {
            if let Some(options) = super::read_settings(&self.root, &key)? {
                if self.matches(peer, &options)
                    && super::capture_lifetime::identity(&self.processes, peer) == Some(started)
                {
                    return Ok(options);
                }
            }
        }
        let folder = self.root.join("CaptureSettings");
        let mut selected = None;
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
            Err(error) => return Err(error.to_string()),
        };
        for (index, entry) in entries.enumerate() {
            if index >= 4096 {
                return Err(
                    "Muitos perfis de captura. Abra Capturas deste jogo no launcher.".into(),
                );
            }
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_file()
                || meta.uid() != unsafe { libc::geteuid() }
                || meta.mode() & 0o077 != 0
                || meta.len() > 128 * 1024
            {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let Ok(options) = serde_json::from_slice::<CaptureOptions>(&bytes) else {
                continue;
            };
            if super::settings_path(&self.root, &options.game_key) != path
                || !self.matches(peer, &options)
            {
                continue;
            }
            super::validate(&options)?;
            if options.game_name.is_empty() {
                return Err(missing());
            }
            if selected.is_some() {
                return Err("Mais de uma instalação corresponde a este processo. Confira os perfis de captura deste jogo.".into());
            }
            selected = Some(options);
        }
        if super::capture_lifetime::identity(&self.processes, peer) != Some(started) {
            return Err("O jogo foi encerrado durante a identificação da captura.".into());
        }
        let selected = selected.ok_or_else(missing)?;
        let mut keys = self.keys.lock().map_err(|e| e.to_string())?;
        if keys.len() >= 32 {
            keys.clear();
        }
        keys.insert((peer, started), selected.game_key.clone());
        Ok(selected)
    }
    pub fn reply(
        &self,
        manager: &super::CaptureManager,
        command: &str,
        peer: u32,
        notify: super::Notify,
        selection_changed: impl FnOnce(&CaptureOptions),
        encoder_probe: impl FnOnce() -> Result<super::selection::EncoderSupport, String>,
    ) -> serde_json::Value {
        let resolved = self.options(peer);
        let mut options = resolved.as_ref().ok().cloned();
        let mut audio = None;
        let mut encoder = None;
        let result = resolved.and_then(|mut selected| {
            ensure_owner(&manager.status(), &selected.game_key)?;
            if let Some(json) = command.strip_prefix("configure:") {
                if manager.status().phase != "idle" {
                    return Err("Encerre a captura para mudar áudio ou formato.".into());
                }
                let selection: super::selection::Selection =
                    serde_json::from_str(json).map_err(|_| "Ajustes de captura inválidos.")?;
                selection.validate()?;
                selected.selection = selection;
                super::write_settings(&self.root, &selected)?;
                selection_changed(&selected);
                options = Some(selected);
                return Ok(manager.status());
            }
            match command {
                "status" => Ok(manager.status()),
                "sources" => {
                    audio = Some(super::audio_sources()?);
                    encoder = Some(encoder_probe()?);
                    Ok(manager.status())
                }
                "toggle-recording" | "toggle-replay" | "save-replay" => {
                    let action =
                        super::shortcut_action(command, &manager.status(), &selected.game_key)?;
                    if matches!(action, "record" | "replay") {
                        manager.start_from_overlay(
                            selected,
                            action == "replay",
                            notify.clone(),
                            peer,
                        )
                    } else {
                        manager.action_for_game(action, notify.clone(), &selected.game_key)
                    }
                }
                "record" | "replay" => {
                    manager.start_from_overlay(selected, command == "replay", notify.clone(), peer)
                }
                action => manager.action_for_game(action, notify.clone(), &selected.game_key),
            }
        });
        let status = status_for(
            manager.status(),
            options.as_ref().map(|v| v.game_key.as_str()),
        );
        serde_json::json!({"status":status,"prepared":options.as_ref().map(|v|&v.game_name),"selection":options.map(|v|v.selection),"audio":audio,"encoder":encoder,"error":result.err()})
    }
    fn matches(&self, peer: u32, options: &CaptureOptions) -> bool {
        // Same Steam AppID in native and Flatpak installations is not enough
        // to select the other installation's profile.
        if options.game_key.starts_with("Steam:native:")
            || options.game_key.starts_with("Steam:flatpak:")
        {
            let env =
                fs::read(self.processes.join(peer.to_string()).join("environ")).unwrap_or_default();
            let flatpak = env
                .split(|v| *v == 0)
                .any(|v| v == b"FLATPAK_ID=com.valvesoftware.Steam");
            if flatpak != options.game_key.starts_with("Steam:flatpak:") {
                return false;
            }
        }
        super::capture_lifetime::matches(
            &self.processes,
            peer,
            &options.game_directory,
            &options.game_executable,
            &options.steam_app_id,
        )
    }
}
fn missing() -> String {
    "Abra a aba Capturas deste jogo no launcher para configurar a gravação.".into()
}

pub fn ensure_owner(status: &CaptureStatus, game: &str) -> Result<(), String> {
    if status.phase != "idle"
        && !status.phase.is_empty()
        && status.game_key.as_deref() != Some(game)
    {
        return Err("A captura ativa pertence a outro jogo. Encerre-a pelo launcher.".into());
    }
    Ok(())
}
pub fn status_for(status: CaptureStatus, game: Option<&str>) -> CaptureStatus {
    if game.is_some() && status.game_key.as_deref() == game {
        return status;
    }
    if !status.phase.is_empty() && status.phase != "idle" {
        CaptureStatus {
            phase: "other-game".into(),
            error: Some("Há uma captura ativa em outro jogo. Gerencie-a no launcher.".into()),
            ..Default::default()
        }
    } else {
        CaptureStatus {
            phase: "idle".into(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        root: PathBuf,
        bindings: Bindings,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("capture-context-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join("proc")).unwrap();
            let mut bindings = Bindings::new(root.clone());
            bindings.processes = root.join("proc");
            Self { root, bindings }
        }
        fn save(&self, id: &str, name: &str) -> CaptureOptions {
            let options:CaptureOptions=serde_json::from_value(serde_json::json!({"game_key":format!("Steam:native:{id}"),"game_name":name,"steam_app_id":id,"game_directory":format!("/games/{name}"),"method":"portal","monitor":"","replay_seconds":60})).unwrap();
            super::super::write_settings(&self.root, &options).unwrap();
            options
        }
        fn peer(&self, pid: u32, id: &str, start: u64) {
            let p = self.bindings.processes.join(pid.to_string());
            fs::create_dir_all(&p).unwrap();
            let mut fields = vec!["0".to_owned(); 20];
            fields[0] = "S".into();
            fields[19] = start.to_string();
            fs::write(
                p.join("stat"),
                format!("{pid} (render process) {}", fields.join(" ")),
            )
            .unwrap();
            fs::write(p.join("environ"), format!("SteamAppId={id}\0")).unwrap();
            fs::write(p.join("cmdline"), b"Z:\\games\\Example.exe\0").unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn tomb_raider_never_receives_the_diablo_profile_prepared_last() {
        let f = Fixture::new();
        let tomb = f.save("203160", "Tomb Raider");
        f.save("2344520", "Diablo IV");
        f.peer(501, "203160", 10);
        f.peer(502, "2344520", 11);
        let options = f.bindings.options(501).unwrap();
        assert_eq!(options.game_key, tomb.game_key);
        assert_eq!(options.game_name, "Tomb Raider");
        assert_eq!(f.bindings.options(502).unwrap().game_name, "Diablo IV");
        assert_eq!(f.bindings.options(501).unwrap().game_name, "Tomb Raider");
    }
    #[test]
    fn missing_current_game_never_falls_back_to_another_saved_profile() {
        let f = Fixture::new();
        f.save("2344520", "Diablo IV");
        f.peer(501, "203160", 10);
        assert!(f.bindings.options(501).unwrap_err().contains("deste jogo"));
        f.save("203160", "Tomb Raider");
        assert_eq!(f.bindings.options(501).unwrap().game_name, "Tomb Raider");
    }
    #[test]
    fn cached_binding_reads_updated_settings_and_pid_reuse_resolves_again() {
        let f = Fixture::new();
        let mut tomb = f.save("203160", "Tomb Raider");
        f.save("2344520", "Diablo IV");
        f.peer(501, "203160", 10);
        assert_eq!(f.bindings.options(501).unwrap().replay_seconds, 60);
        tomb.replay_seconds = 180;
        super::super::write_settings(&f.root, &tomb).unwrap();
        assert_eq!(f.bindings.options(501).unwrap().replay_seconds, 180);
        f.peer(501, "2344520", 20);
        assert_eq!(f.bindings.options(501).unwrap().game_name, "Diablo IV");
    }
    #[test]
    fn native_and_flatpak_profiles_with_the_same_app_id_do_not_mix() {
        let f = Fixture::new();
        let native = f.save("203160", "Tomb Raider");
        let mut flatpak = native.clone();
        flatpak.game_key = "Steam:flatpak:203160".into();
        flatpak.game_name = "Tomb Raider Flatpak".into();
        super::super::write_settings(&f.root, &flatpak).unwrap();
        f.peer(501, "203160", 10);
        f.peer(502, "203160", 11);
        fs::write(
            f.bindings.processes.join("502/environ"),
            b"SteamAppId=203160\0FLATPAK_ID=com.valvesoftware.Steam\0",
        )
        .unwrap();
        assert_eq!(f.bindings.options(501).unwrap().game_key, native.game_key);
        assert_eq!(f.bindings.options(502).unwrap().game_key, flatpak.game_key);
    }
    #[test]
    fn bridge_reply_uses_the_requesting_game_for_title_configuration_and_save() {
        let f = Fixture::new();
        let tomb = f.save("203160", "Tomb Raider");
        let diablo = f.save("2344520", "Diablo IV");
        f.peer(501, "203160", 10);
        let manager = super::super::CaptureManager::default();
        let reply = f.bindings.reply(
            &manager,
            "status",
            501,
            std::sync::Arc::new(|_| {}),
            |_| panic!("status must not write"),
            || panic!("status must not probe codecs"),
        );
        assert_eq!(reply["prepared"], "Tomb Raider");
        assert!(reply["error"].is_null());
        let mut selection = tomb.selection.clone();
        selection.fps = 30;
        let command = format!("configure:{}", serde_json::to_string(&selection).unwrap());
        let reply = f.bindings.reply(
            &manager,
            &command,
            501,
            std::sync::Arc::new(|_| {}),
            |saved| assert_eq!(saved.game_key, tomb.game_key),
            || panic!("configure must not probe codecs"),
        );
        assert!(reply["error"].is_null());
        assert_eq!(reply["selection"]["fps"], 30);
        assert_eq!(
            super::super::read_settings(&f.root, &tomb.game_key)
                .unwrap()
                .unwrap()
                .selection
                .fps,
            30
        );
        assert_eq!(
            super::super::read_settings(&f.root, &diablo.game_key)
                .unwrap()
                .unwrap()
                .selection
                .fps,
            diablo.selection.fps
        );
    }
    #[test]
    fn bridge_rejects_foreign_controls_and_keeps_the_requesting_games_title() {
        let f = Fixture::new();
        f.save("203160", "Tomb Raider");
        let diablo = f.save("2344520", "Diablo IV");
        f.peer(501, "203160", 10);
        let manager = super::super::CaptureManager::default();
        manager.inner.lock().unwrap().status = CaptureStatus {
            phase: "active".into(),
            game_key: Some(diablo.game_key),
            game_name: Some(diablo.game_name),
            recording: true,
            replay: true,
            ..Default::default()
        };
        for command in [
            "status",
            "record",
            "toggle-replay",
            "save-replay",
            "stop",
            "configure:{}",
        ] {
            let reply = f.bindings.reply(
                &manager,
                command,
                501,
                std::sync::Arc::new(|_| panic!("must not change the other capture")),
                |_| panic!("must not change settings"),
                || panic!("must not probe codecs"),
            );
            assert_eq!(reply["prepared"], "Tomb Raider");
            assert_eq!(reply["status"]["phase"], "other-game");
            assert!(reply["status"]["game_name"].is_null());
            assert!(reply["error"].as_str().unwrap().contains("outro jogo"));
            assert!(manager.status().recording);
        }
    }
    #[test]
    fn foreign_active_capture_is_unavailable_and_does_not_expose_its_title_or_clip() {
        let foreign = CaptureStatus {
            phase: "active".into(),
            game_key: Some("diablo".into()),
            game_name: Some("Diablo IV".into()),
            recording: true,
            replay: true,
            last_file: Some("/diablo/replay.mkv".into()),
            ..Default::default()
        };
        assert!(ensure_owner(&foreign, "tomb").is_err());
        assert!(ensure_owner(&foreign, "diablo").is_ok());
        let projected = status_for(foreign, Some("tomb"));
        assert_eq!(projected.phase, "other-game");
        assert!(projected.game_name.is_none());
        assert!(projected.last_file.is_none());
        assert!(!projected.recording);
        assert!(!projected.replay);
        let previous = CaptureStatus {
            phase: "idle".into(),
            game_key: Some("diablo".into()),
            game_name: Some("Diablo IV".into()),
            last_file: Some("/diablo/replay.mkv".into()),
            ..Default::default()
        };
        let projected = status_for(previous, Some("tomb"));
        assert_eq!(projected.phase, "idle");
        assert!(projected.game_name.is_none());
        assert!(projected.last_file.is_none());
    }
}
