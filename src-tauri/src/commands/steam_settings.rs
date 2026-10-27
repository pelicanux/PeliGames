//! A one-time fixed Steam command reads mutable profiles. Only initial linkage
//! and changes to Steam-selected Proton need confirmation or a closed client.
use super::*;
use std::fs;

#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    before: Option<SavedExternalSettings>,
    after: SavedExternalSettings,
}
fn file() -> Result<PathBuf, String> {
    Ok(crate::core::paths::app_root()?.join("steam-pending-settings.json"))
}
fn read() -> Result<BTreeMap<String, Pending>, String> {
    match fs::read(file()?) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.to_string()),
    }
}
fn write(records: &BTreeMap<String, Pending>) -> Result<(), String> {
    let path = file()?;
    let temporary = path.with_extension("json.part");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(records).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn effective(value: Option<&SavedExternalSettings>) -> serde_json::Value {
    let mut advanced = value.map(|v| v.advanced.clone()).unwrap_or_default();
    advanced.schema_version = 0;
    advanced.options.retain(|_, enabled| *enabled);
    advanced.wrappers.retain(|w| w.enabled);
    if !advanced
        .options
        .get("dll_overrides")
        .copied()
        .unwrap_or(false)
    {
        advanced.dll_overrides.clear();
    }
    if !advanced.gamescope.enabled() {
        advanced.gamescope = Default::default();
    }
    if advanced
        .launch_options
        .as_deref()
        .is_some_and(|s| s.trim().is_empty())
    {
        advanced.launch_options = None;
    }
    // A named alias (GE Latest, for example) and its canonical directory are
    // the same runner. Saving a validated path must not create a false change.
    let proton = value.and_then(|v| v.proton.as_ref()).map(|path| {
        fs::canonicalize(path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.clone())
    });
    serde_json::json!({"advanced": advanced, "proton": proton})
}
pub(super) fn pending_ids() -> Result<Vec<String>, String> {
    Ok(read()?.into_keys().collect())
}
pub(super) fn pending(id: &str) -> Result<bool, String> {
    Ok(read()?.contains_key(id))
}
pub(super) fn stage(
    game: &ExternalGame,
    previous: Option<SavedExternalSettings>,
    next: SavedExternalSettings,
) -> Result<(), String> {
    let mods = super::super::steam_runtime::mod_options(game)?;
    stage_with_mods(game, previous, next, mods)
}
pub(super) fn stage_with_mods(
    game: &ExternalGame,
    previous: Option<SavedExternalSettings>,
    next: SavedExternalSettings,
    mods: Option<String>,
) -> Result<(), String> {
    let mut records = read()?;
    let old = records.clone();
    if linked(game)? {
        // Mutable arguments, environment and wrappers are already applied by
        // the helper. Only a different Steam-selected Proton remains pending.
        update_linked(
            &mut records,
            &game.external_id,
            &next,
            selected_proton(game),
        );
    } else {
        update(
            &mut records,
            &game.external_id,
            previous.clone(),
            next.clone(),
        );
        records.entry(game.external_id.clone()).or_insert(Pending {
            before: previous,
            after: next.clone(),
        });
    }
    write(&records)?;
    if let Err(error) = super::super::steam_runtime::prepare(game, &next.advanced, mods) {
        write(&old).map_err(|rollback| {
            format!("{error}; não foi possível restaurar as pendências: {rollback}")
        })?;
        return Err(error);
    }
    Ok(())
}

fn same_proton(first: Option<&String>, second: Option<&String>) -> bool {
    let canonical =
        |value: &String| fs::canonicalize(value).unwrap_or_else(|_| PathBuf::from(value));
    first.map(canonical) == second.map(canonical)
}

fn selected_proton(game: &ExternalGame) -> Option<String> {
    if game.native_linux {
        return None;
    }
    let home = dirs::home_dir()?;
    steam_proton_state::selected(&home, game.flatpak, game.app_id.as_deref()?)
        .and_then(|selection| selection.proton)
}

fn update_linked(
    records: &mut BTreeMap<String, Pending>,
    id: &str,
    next: &SavedExternalSettings,
    selected: Option<String>,
) {
    if same_proton(selected.as_ref(), next.proton.as_ref()) {
        records.remove(id);
    } else {
        let mut before = next.clone();
        before.proton = selected;
        records.insert(
            id.into(),
            Pending {
                before: Some(before),
                after: next.clone(),
            },
        );
    }
}

fn linked(game: &ExternalGame) -> Result<bool, String> {
    let app = game.app_id.as_deref().ok_or("Steam AppID ausente.")?;
    let expected = format!("{}%command%", super::super::steam_runtime::prefix(game)?);
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let configs = match manual_account_configs(super::super::steam_installation::roots(
        &home,
        game.flatpak,
    )) {
        Ok(configs) => configs,
        // Account discovery failure must not mark an unverified line as linked.
        Err(_) => return Ok(false),
    };
    let text = fs::read_to_string(&configs[0]).map_err(|error| error.to_string())?;
    let actual = super::super::steam_launch_options::read_launch_options(&text, app)?;
    Ok(same_manual_line(&actual, &expected) || universal_line_matches(game, &actual)?)
}
fn update(
    records: &mut BTreeMap<String, Pending>,
    id: &str,
    previous: Option<SavedExternalSettings>,
    next: SavedExternalSettings,
) {
    let before = records
        .get(id)
        .map(|p| p.before.clone())
        .unwrap_or(previous);
    if effective(before.as_ref()) == effective(Some(&next)) {
        records.remove(id);
    } else {
        records.insert(
            id.into(),
            Pending {
                before,
                after: next,
            },
        );
    }
}
fn universal_line_matches(game: &ExternalGame, actual: &str) -> Result<bool, String> {
    Ok(super::super::launch_profiles::prefix(Path::new(&game.path))?
        .is_some_and(|prefix| same_manual_line(actual, &format!("{prefix}%command%"))))
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
#[derive(Serialize)]
pub(super) struct ManualOptions {
    pub line: String,
    pub proton: Option<String>,
}
/// Recognize the previous release's expanded command during one-time migration.
/// New manual exports always reference the stable Rust helper instead.
fn manual_line(settings: &AdvancedSettings) -> Result<String, String> {
    let marker = Path::new("__PELIGAMES_STEAM_COMMAND__");
    let mut command =
        crate::core::advanced_settings::configured_launch_command(marker, Some(settings))?;
    let custom = custom_options(settings)?;
    let mut env = BTreeMap::new();
    crate::core::launch_environment::apply(&mut command, &custom);
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            env.insert(
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            );
        }
    }
    let mut words: Vec<String> = env
        .into_iter()
        .map(|(key, value)| format!("{key}={}", quote(&value)))
        .collect();
    for token in std::iter::once(command.get_program()).chain(command.get_args()) {
        words.push(if token == marker.as_os_str() {
            "%command%".into()
        } else {
            quote(&token.to_string_lossy())
        });
    }
    words.extend(custom.get_args().map(|arg| quote(&arg.to_string_lossy())));
    Ok(words.join(" "))
}
pub(super) fn manual(game: &ExternalGame) -> Result<ManualOptions, String> {
    super::super::steam_runtime::prepare(
        game,
        &game.advanced,
        super::super::steam_runtime::mod_options(game)?,
    )?;
    Ok(ManualOptions {
        line: format!("{}%command%", super::super::steam_runtime::prefix(game)?),
        proton: game.proton.clone(),
    })
}
fn wrap(current: &str, prefix: &str) -> String {
    let clean = current.replacen(prefix, "", 1);
    let current = clean.trim();
    let rest = if current.contains("%command%") {
        current.into()
    } else if current.is_empty() {
        "%command%".into()
    } else {
        format!("%command% {current}")
    };
    rest.replacen("%command%", &format!("{prefix}%command%"), 1)
}
fn apply(game: &ExternalGame) -> Result<(), String> {
    if super::super::steam_installation::running(game.flatpak) {
        return Err(
            "A Steam ainda está aberta. Aguarde o encerramento antes de aplicar as alterações."
                .into(),
        );
    }
    let mut records = read()?;
    let Some(edit) = records.get(&game.external_id).cloned() else {
        return Ok(());
    };
    let app = game.app_id.as_deref().ok_or("Steam AppID ausente.")?;
    super::super::steam_runtime::prepare(
        game,
        &edit.after.advanced,
        super::super::steam_runtime::mod_options(game)?,
    )?;
    let proton_file = external_proton::steam_config(game)?;
    let old_script_dir = proton_file
        .parent()
        .ok_or("Configuração sem diretório.")?
        .join("peligames-game-settings");
    let old_script_path = old_script_dir.join(format!(
        "{}-{app}.sh",
        if game.flatpak { "flatpak" } else { "native" }
    ));
    let old_prefix = format!("/bin/sh {} ", quote(&old_script_path.to_string_lossy()));
    let prefix = super::super::steam_runtime::prefix(game)?;
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let configs =
        manual_account_configs(super::super::steam_installation::roots(&home, game.flatpak))?;
    let mut changes = Vec::new();
    for path in configs {
        let before = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let current = super::super::steam_launch_options::read_launch_options(&before, app)?;
        let mut current = current.replacen(&old_prefix, "", 1);
        if let Some(prefix) = super::super::launch_profiles::prefix(Path::new(&game.path))? {
            if same_manual_line(&current, &format!("{prefix}%command%")) {
                let words = crate::core::advanced_settings::wrapper_args(&current)?;
                let count = crate::core::advanced_settings::wrapper_args(&prefix)?.len();
                current = words[count..].iter().map(|word| if word == "%command%" { word.clone() } else { quote(word) }).collect::<Vec<_>>().join(" ");
            }
        }
        let current = migrate_expanded_line(&current, edit.before.as_ref())?;
        let current = super::super::steam_launch_options::strip_runtime_mod_options(app, &current)?;
        let after = super::super::steam_launch_options::write_launch_options(
            &before,
            app,
            &wrap(&current, &prefix),
        )?;
        changes.push((path, before, after));
    }
    // Keep exact file contents for rollback if applying any one file fails.
    let proton_before = fs::read(&proton_file).map_err(|e| e.to_string())?;
    let result = (|| {
        if let Some(proton) = edit
            .after
            .proton
            .as_ref()
            .filter(|next| edit.before.as_ref().and_then(|s| s.proton.as_ref()) != Some(*next))
        {
            if !game.native_linux {
                external_proton::apply(
                    game,
                    &crate::core::game_installation::validate_proton(proton)?,
                )?;
            }
        }
        for (path, _, after) in &changes {
            super::super::steam_launch_options::write_file(path, after)?;
        }
        records.remove(&game.external_id);
        write(&records)
    })();
    if result.is_err() {
        for (path, before, _) in &changes {
            let _ = super::super::steam_launch_options::write_file(path, before);
        }
        let _ = fs::write(proton_file, proton_before);
    }
    result
}

fn migrate_expanded_line(
    current: &str,
    before: Option<&SavedExternalSettings>,
) -> Result<String, String> {
    if let Some(before) = before {
        // A removed old component need not be installed again just to migrate.
        if let Ok(expected) = manual_line(&before.advanced) {
            if same_manual_line(current, &expected) {
                let words = crate::core::advanced_settings::wrapper_args(current)?;
                let count = crate::core::advanced_settings::wrapper_args(&expected)?.len();
                return Ok(std::iter::once("%command%".into())
                    .chain(words[count..].iter().map(|word| quote(word)))
                    .collect::<Vec<_>>()
                    .join(" "));
            }
        }
    }
    let root = crate::core::paths::app_root()?;
    let words = crate::core::advanced_settings::wrapper_args(current)?;
    let existing_wrapper = words.iter().any(|word| {
        Path::new(word).starts_with(root.join("FrameGeneration/profiles"))
            || word
                .strip_prefix("MANGOHUD_CONFIGFILE=")
                .is_some_and(|path| Path::new(path).starts_with(root.join("MangoHUD")))
            || before.is_some_and(|before| {
                before
                    .advanced
                    .wrappers
                    .iter()
                    .any(|wrapper| wrapper.enabled && wrapper.program == *word)
            })
    });
    if existing_wrapper {
        return Err("As opções atuais usam um perfil antigo do PeliGames. Use “Configurar na Steam” e substitua a linha pela nova para evitar carregar os componentes duas vezes. As opções atuais foram preservadas.".into());
    }
    Ok(current.into())
}
pub(super) fn before_launch(game: &ExternalGame) -> Result<(), String> {
    if !pending(&game.external_id)? {
        return Ok(());
    }
    if super::super::steam_installation::running(game.flatpak) {
        return Err("Conclua “Configurar na Steam” para vincular a linha fixa ou confirmar o Proton. Você pode fazer isso com a Steam aberta.".into());
    }
    apply(game)
}
fn same_manual_line(actual: &str, expected: &str) -> bool {
    let words = |line: &str| crate::core::advanced_settings::wrapper_args(line);
    unquoted_command(actual)
        && matches!((words(actual), words(expected)), (Ok(a),Ok(b)) if a.starts_with(&b) && !a[b.len()..].iter().any(|v| v.chars().any(|c| ";|&<>`\n\r".contains(c)) || v.contains("$(")) && a.iter().filter(|v|v.as_str()=="%command%").count()==1)
}

fn unquoted_command(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.contains(&0) {
        return false;
    }
    let mut quote = 0;
    let mut i = 0;
    let mut count = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if quote != b'\'' => {
                i += 2;
                continue;
            }
            b'\'' | b'"' if quote == 0 => quote = bytes[i],
            value if value == quote => quote = 0,
            _ if quote == 0 && bytes[i..].starts_with(b"%command%") => {
                count += 1;
                i += 9;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    count == 1
}
fn login_field<'a>(value: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    value
        .as_object()?
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))?
        .1
        .as_str()
}
fn login_account(users: &serde_json::Value) -> Result<u32, String> {
    let users = users
        .as_object()
        .and_then(|map| {
            map.iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("users"))
        })
        .and_then(|(_, value)| value.as_object())
        .ok_or("Arquivo de contas Steam inválido.")?;
    let candidates: Vec<_> = users
        .iter()
        .filter_map(|(id, info)| {
            let account = id.parse::<u64>().ok()?.checked_sub(76561197960265728)?;
            let account = u32::try_from(account).ok().filter(|id| *id != 0)?;
            Some((account, info))
        })
        .collect();
    let recent: Vec<_> = candidates
        .iter()
        .filter(|(_, info)| login_field(info, "MostRecent") == Some("1"))
        .collect();
    if recent.len() == 1 {
        return Ok(recent[0].0);
    }
    if recent.len() > 1 {
        return Err("Há mais de uma conta marcada como recente na Steam.".into());
    }
    // Current Steam builds may omit MostRecent entirely. AutoLogin is accepted
    // only when unique and not contradicted by a newer login timestamp.
    let automatic: Vec<_> = candidates
        .iter()
        .filter(|(_, info)| login_field(info, "AutoLogin") == Some("1"))
        .collect();
    if automatic.len() == 1 {
        let selected = automatic[0];
        let timestamp = login_field(selected.1, "timestamp").and_then(|v| v.parse::<u64>().ok());
        let contradicted =
            candidates
                .iter()
                .filter(|(id, _)| *id != selected.0)
                .any(|(_, info)| {
                    let other = login_field(info, "timestamp").and_then(|v| v.parse::<u64>().ok());
                    matches!((timestamp, other), (Some(a), Some(b)) if b >= a)
                });
        if !contradicted {
            return Ok(selected.0);
        }
    }
    if candidates.len() == 1 {
        return Ok(candidates[0].0);
    }
    Err("A Steam não identificou uma única conta recente. Faça login na conta desejada e tente novamente.".into())
}
fn manual_account_configs(
    roots: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<PathBuf>, String> {
    let mut configs = Vec::new();
    let mut visited = Vec::new();
    for root in roots {
        let Ok(root) = fs::canonicalize(root) else {
            continue;
        };
        if visited.contains(&root) {
            continue;
        }
        visited.push(root.clone());
        let login = root.join("config/loginusers.vdf");
        if !login.is_file() {
            continue;
        }
        let users = external_proton::vdf(&fs::read_to_string(login).map_err(|e| e.to_string())?)?;
        let account = login_account(&users)?;
        let path = fs::canonicalize(root.join(format!("userdata/{account}/config/localconfig.vdf"))).map_err(|_| "A configuração da conta identificada não está disponível. Abra a Steam nessa conta e tente novamente.")?;
        if !configs.contains(&path) {
            configs.push(path);
        }
    }
    if configs.len() != 1 {
        return Err("Não foi possível identificar uma única instalação e conta Steam. Abra a Steam desejada e tente novamente.".into());
    }
    Ok(configs)
}
/// Acknowledge only a line actually saved by the active Steam account; retain
/// PeliGames' settings and profile instead of reverting them after manual setup.
pub(super) fn confirm_manual(game: &ExternalGame) -> Result<(), String> {
    let mut records = read()?;
    let Some(edit) = records.get(&game.external_id) else {
        return Ok(());
    };
    let app = game.app_id.as_deref().ok_or("Steam AppID ausente.")?;
    let expected = format!("{}%command%", super::super::steam_runtime::prefix(game)?);
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let configs =
        manual_account_configs(super::super::steam_installation::roots(&home, game.flatpak))?;
    let text = fs::read_to_string(&configs[0]).map_err(|e| e.to_string())?;
    let actual = super::super::steam_launch_options::read_launch_options(&text, app)?;
    if !same_manual_line(&actual, &expected) && !universal_line_matches(game, &actual)? {
        return Err("A Steam ainda não gravou a linha esperada. Cole as opções em Propriedades → Geral → Opções de inicialização, feche essa tela e tente verificar novamente.".into());
    }
    if let Some(proton) = &edit.after.proton {
        if !same_proton(selected_proton(game).as_ref(), Some(proton)) {
            return Err("O Proton selecionado ainda difere na Steam. Selecione-o em Compatibilidade ou use a aplicação automática.".into());
        }
    }
    super::super::steam_runtime::prepare(
        game,
        &edit.after.advanced,
        super::super::steam_runtime::mod_options(game)?,
    )?;
    records.remove(&game.external_id);
    write(&records)
}
pub(super) fn revert(game: &ExternalGame) -> Result<(), String> {
    let mut records = read()?;
    let Some(edit) = records.get(&game.external_id) else {
        return Ok(());
    };
    let mut saved = read_settings()?;
    let directory = saved
        .get(&game.external_id)
        .and_then(|s| s.directory.clone());
    if let Some(mut before) = edit.before.clone() {
        before.directory = directory;
        saved.insert(game.external_id.clone(), before);
    } else {
        saved.insert(
            game.external_id.clone(),
            SavedExternalSettings {
                advanced: AdvancedSettings::default(),
                proton: None,
                directory,
            },
        );
    }
    let restored = saved
        .get(&game.external_id)
        .ok_or("Perfil anterior indisponível.")?;
    super::super::steam_runtime::prepare(
        game,
        &restored.advanced,
        super::super::steam_runtime::mod_options(game)?,
    )?;
    let path = settings_file()?;
    let temporary = path.with_extension("json.part");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())?;
    records.remove(&game.external_id);
    write(&records)
}
pub(super) fn restart(game: &ExternalGame, mut progress: impl FnMut(&str)) -> Result<(), String> {
    if !pending(&game.external_id)? { return Ok(()); }
    let mut client = SteamClient::new(game.flatpak)?;
    super::super::steam_restart::stop(&mut client, &mut progress)?;
    progress("applying");
    apply(game)?;
    let proton = if game.flatpak {
        game.proton
            .as_deref()
            .map(crate::core::game_installation::validate_proton)
            .transpose()?
    } else {
        None
    };
    progress("starting");
    let mut open = super::super::steam_installation::command(game.flatpak, proton.as_deref());
    crate::core::env_utils::host_environment(&mut open);
    let mut child = open
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            format!("Configurações aplicadas, mas não foi possível reabrir a Steam: {e}")
        })?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(12);
    while std::time::Instant::now() < until {
        if super::super::steam_processes::clients(Path::new("/proc"), game.flatpak)?
            .iter().any(|p|p.executable.file_name().is_some_and(|name|name=="steam")) {
            std::thread::spawn(move || { let _ = child.wait(); });
            progress("done");
            return Ok(());
        }
        if child.try_wait().map_err(|e|e.to_string())?.is_some_and(|status|!status.success()) {
            return Err("Configurações aplicadas, mas a Steam encerrou durante a inicialização. Abra a Steam manualmente; o perfil já está salvo.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    std::thread::spawn(move || { let _ = child.wait(); });
    Err("Configurações aplicadas. A Steam está demorando para iniciar e a abertura não pôde ser confirmada. Abra a Steam manualmente; não é preciso reaplicar o perfil.".into())
}

struct SteamClient {
    flatpak: bool,
    original: Vec<super::super::steam_processes::Process>,
    roots: Vec<PathBuf>,
}
impl SteamClient {
    fn new(flatpak: bool) -> Result<Self,String> {
        let home=dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
        Ok(Self {flatpak, original:super::super::steam_processes::clients(Path::new("/proc"),flatpak)?,roots:super::super::steam_installation::roots(&home,flatpak)})
    }
}
impl super::super::steam_restart::Client for SteamClient {
    fn running(&mut self)->Result<bool,String> {
        let current=super::super::steam_processes::clients(Path::new("/proc"),self.flatpak)?;
        if current.iter().any(|p| !self.original.iter().any(|old|old.pid==p.pid && old.start==p.start)) {
            return Err("Uma nova instância da Steam apareceu durante o encerramento. As configurações continuam pendentes; tente novamente.".into());
        }
        Ok(!current.is_empty())
    }
    fn games_running(&mut self)->Result<bool,String> {
        super::super::steam_processes::games_running(Path::new("/proc"),self.flatpak,&self.original.iter().map(|p|p.pid).collect::<Vec<_>>())
    }
    fn request_shutdown(&mut self)->Result<(),String> {
        let mut command=super::super::steam_installation::command(self.flatpak,None);
        crate::core::env_utils::host_environment(&mut command);
        let mut child=command.arg("-shutdown").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
        std::thread::spawn(move || {let _=child.wait();});
        Ok(())
    }
    fn terminate(&mut self,force:bool)->Result<(),String> {
        let current=super::super::steam_processes::clients(Path::new("/proc"),self.flatpak)?;
        let targets:Vec<_>=current.iter().filter(|p|p.executable.file_name().is_some_and(|name|name=="steam")).collect();
        if targets.is_empty() && !current.is_empty() {return Err("O cliente principal encerrou, mas um inicializador da Steam ficou preso. Feche-o e tente novamente; as alterações continuam pendentes.".into());}
        for process in targets {
            let original=self.original.iter().find(|p|p.pid==process.pid && p.start==process.start).ok_or("A instância da Steam mudou; tente novamente.")?;
            super::super::steam_processes::signal(original,self.flatpak,&self.roots,force)?;
        }
        Ok(())
    }
    fn pause(&mut self,duration:std::time::Duration){std::thread::sleep(duration)}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_detection_supports_current_and_legacy_steam_without_guessing() {
        let old = serde_json::json!({"users":{"76561197960265729":{"MostRecent":"1"},"76561197960265730":{"MostRecent":"0"}}});
        assert_eq!(login_account(&old).unwrap(), 1);
        let current = serde_json::json!({"Users":{"76561197960265729":{"AutoLogin":"1","timestamp":"200"},"76561197960265730":{"AutoLogin":"0","timestamp":"100"}}});
        assert_eq!(login_account(&current).unwrap(), 1);
        let mut stale = current.clone();
        stale["Users"]["76561197960265730"]["timestamp"] = "300".into();
        assert!(login_account(&stale).is_err());
        let ambiguous = serde_json::json!({"users":{"76561197960265729":{"MostRecent":"1"},"76561197960265730":{"MostRecent":"1"}}});
        assert!(login_account(&ambiguous).is_err());
        assert!(login_account(&serde_json::json!({"users":{"76561197960265728":{}}})).is_err());
        assert_eq!(
            login_account(&serde_json::json!({"users":{"76561197960265729":{}}})).unwrap(),
            1
        );
    }
    #[test]
    fn account_config_deduplicates_steam_aliases_and_reads_selected_user_only() {
        let root =
            std::env::temp_dir().join(format!("peli-steam-account-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("userdata/1/config")).unwrap();
        fs::write(
            root.join("config/loginusers.vdf"),
            r#""users" { "76561197960265729" { "AutoLogin" "1" } }"#,
        )
        .unwrap();
        fs::write(root.join("userdata/1/config/localconfig.vdf"), "original").unwrap();
        let alias = root.with_extension("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let paths = manual_account_configs(vec![root.clone(), alias.clone()]).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(fs::read_to_string(&paths[0]).unwrap(), "original");
        fs::remove_file(alias).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "read-only inspection of the user's current Steam configuration"]
    fn installed_steam_manual_verification_audit() {
        let home = dirs::home_dir().unwrap();
        let records: BTreeMap<String, Pending> = serde_json::from_slice(
            &fs::read(std::env::var("PELI_STEAM_PENDING_AUDIT").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!records.is_empty());
        for (id, edit) in records {
            let app = id.rsplit(':').next().unwrap();
            let mut game = super::super::tests::game("Steam", app);
            game.flatpak = id.starts_with("Steam:flatpak:");
            let configs = manual_account_configs(super::super::super::steam_installation::roots(
                &home,
                game.flatpak,
            ))
            .unwrap();
            let text = fs::read_to_string(&configs[0]).unwrap();
            let actual =
                super::super::super::steam_launch_options::read_launch_options(&text, app).unwrap();
            let expected = format!(
                "{}%command%",
                super::super::super::steam_runtime::prefix(&game).unwrap()
            );
            let matches = same_manual_line(&actual, &expected);
            println!("Steam app {app}: saved launch options match expected = {matches}");
            if let Some(expected) = &edit.after.proton {
                let selected =
                    steam_proton_state::selected(&home, game.flatpak, app).and_then(|s| s.proton);
                let matches = selected.and_then(|s| fs::canonicalize(s).ok())
                    == fs::canonicalize(expected).ok();
                println!("Steam app {app}: selected Proton matches expected = {matches}");
            }
        }
    }

    #[test]
    fn manual_confirmation_requires_the_complete_command() {
        assert!(same_manual_line(
            "'/profile.sh' %command% '-windowed'",
            "'/profile.sh' %command%"
        ));
        assert!(!same_manual_line(
            "'/profile.sh' %command% ';' 'other-command'",
            "'/profile.sh' %command%"
        ));
        assert!(same_manual_line(
            "MANGOHUD=0 '/path with space/profile.sh' %command%",
            "MANGOHUD='0' \"/path with space/profile.sh\" %command%"
        ));
        assert!(!same_manual_line(
            "MANGOHUD=0 %command%",
            "MANGOHUD=0 '/profile.sh' %command%"
        ));
        assert!(!same_manual_line(
            "'/profile.sh' %command% %command%",
            "'/profile.sh' %command%"
        ));
        assert!(same_manual_line(
            "\"/path with spaces/pelirun\" --app 42 -- %command% '-windowed'",
            "'/path with spaces/pelirun' --app 42 -- %command%"
        ));
        assert!(!same_manual_line(
            "'/path with spaces/pelirun' --app 42 -- '%command%'",
            "'/path with spaces/pelirun' --app 42 -- %command%"
        ));
    }
    #[test]
    fn restoring_proton_clears_only_its_change_in_both_steam_installations() {
        for id in ["Steam:native:42", "Steam:flatpak:42"] {
            let baseline = SavedExternalSettings {
                advanced: AdvancedSettings::default(),
                proton: Some("/proton/Experimental".into()),
                directory: None,
            };
            let mut ge = baseline.clone();
            ge.proton = Some("/proton/GE".into());
            let mut records = BTreeMap::new();
            update(&mut records, id, Some(baseline.clone()), ge.clone());
            assert!(records.contains_key(id));
            update(&mut records, id, Some(ge.clone()), baseline.clone());
            assert!(
                !records.contains_key(id),
                "Returning to Experimental must enable Play"
            );
            update(&mut records, id, Some(baseline.clone()), ge.clone());
            let mut additional = ge.clone();
            additional
                .advanced
                .options
                .insert("optiscaler".into(), true);
            update(&mut records, id, Some(ge), additional.clone());
            additional.proton = baseline.proton.clone();
            update(&mut records, id, None, additional);
            assert!(
                records.contains_key(id),
                "Other changed variables still need applying"
            );
            update(&mut records, id, None, baseline);
            assert!(records.is_empty());
        }
    }
    #[test]
    fn proton_alias_and_canonical_directory_are_the_same_selection() {
        let root =
            std::env::temp_dir().join(format!("peligames-proton-alias-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("Experimental")).unwrap();
        std::os::unix::fs::symlink(root.join("Experimental"), root.join("Current")).unwrap();
        let baseline = SavedExternalSettings {
            advanced: AdvancedSettings::default(),
            proton: Some(root.join("Current").to_string_lossy().into_owned()),
            directory: None,
        };
        let mut restored = baseline.clone();
        restored.proton = Some(root.join("Experimental").to_string_lossy().into_owned());
        assert_eq!(effective(Some(&baseline)), effective(Some(&restored)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_expanded_command_is_available_only_for_migration() {
        let mut settings = AdvancedSettings::default();
        // The migration fixture must not depend on the desktop's global HUD
        // preference or require MangoHUD in the build container.
        settings.mangohud_enabled = Some(false);
        settings.options.insert("optiscaler".into(), true);
        settings.launch_options =
            Some("PG_COPY_TEST='value with spaces' %command% '-windowed'".into());
        let line = manual_line(&settings).unwrap();
        assert!(line.contains("PROTON_USE_OPTISCALER='1'"));
        assert!(line.contains("PG_COPY_TEST='value with spaces'"));
        assert!(line.ends_with("%command% '-windowed'"));
        assert!(!line.contains("steam-game-settings"));
        assert!(!line.contains("peligames-game-settings"));
        let command = line.replace(
            "%command%",
            "/bin/sh -c 'printf \"%s\\n\" \"$PG_COPY_TEST\" \"$PROTON_USE_OPTISCALER\" \"$@\"' --",
        );
        let result = Command::new("/bin/sh")
            .args(["-c", &command])
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "value with spaces\n1\n-windowed\n"
        );
    }
    #[test]
    fn later_edits_keep_the_original_baseline_and_compare_defaults() {
        let mut settings = AdvancedSettings::default();
        settings.schema_version = 2;
        settings.options.insert("fsr4".into(), false);
        let saved = SavedExternalSettings {
            advanced: settings,
            proton: None,
            directory: None,
        };
        assert_eq!(effective(None), effective(Some(&saved)));
        let mut changed = saved.clone();
        changed.proton = Some("/runner/new".into());
        assert_ne!(effective(Some(&saved)), effective(Some(&changed)));
    }
    #[test]
    fn multiple_edits_keep_baseline_and_returning_to_it_clears_pending() {
        let baseline = SavedExternalSettings {
            advanced: AdvancedSettings::default(),
            proton: Some("/proton/old".into()),
            directory: None,
        };
        let mut changed = baseline.clone();
        changed.proton = Some("/proton/new".into());
        let mut records = BTreeMap::new();
        update(
            &mut records,
            "Steam:native:42",
            Some(baseline.clone()),
            changed.clone(),
        );
        let mut again = changed.clone();
        again.advanced.options.insert("optiscaler".into(), true);
        update(&mut records, "Steam:native:42", Some(changed), again);
        let bytes = serde_json::to_vec(&records).unwrap();
        let mut records: BTreeMap<String, Pending> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            records["Steam:native:42"].before.as_ref().unwrap().proton,
            baseline.proton
        );
        update(&mut records, "Steam:native:42", None, baseline);
        assert!(records.is_empty());
    }
    #[test]
    fn linked_profiles_apply_advanced_edits_without_a_steam_restart() {
        let mut next = SavedExternalSettings {
            advanced: AdvancedSettings::default(),
            proton: Some("/proton/current".into()),
            directory: None,
        };
        next.advanced.mangohud_enabled = Some(true);
        next.advanced.mangohud_fps_limit = Some(60);
        let mut records = BTreeMap::new();
        update_linked(&mut records, "Steam:native:42", &next, next.proton.clone());
        assert!(records.is_empty());
        next.proton = Some("/proton/new".into());
        update_linked(
            &mut records,
            "Steam:native:42",
            &next,
            Some("/proton/current".into()),
        );
        let pending = &records["Steam:native:42"];
        assert_eq!(
            pending.before.as_ref().unwrap().advanced.mangohud_enabled,
            Some(true)
        );
        assert_eq!(
            pending.before.as_ref().unwrap().advanced.mangohud_fps_limit,
            Some(60)
        );
        assert_eq!(
            pending.before.as_ref().unwrap().proton.as_deref(),
            Some("/proton/current")
        );
        next.advanced.mangohud_enabled = Some(false);
        next.proton = Some("/proton/current".into());
        update_linked(&mut records, "Steam:native:42", &next, next.proton.clone());
        assert!(records.is_empty());
    }
    #[test]
    fn wrapper_preserves_steam_arguments_and_does_not_duplicate() {
        let prefix = "/bin/sh '/some path/game.sh' ";
        let original = "PROTON_LOG=1 gamemoderun %command% -dx11";
        let next = wrap(original, prefix);
        assert_eq!(wrap(&next, prefix), next);
        assert_eq!(
            next,
            format!("PROTON_LOG=1 gamemoderun {prefix}%command% -dx11")
        );
        assert_eq!(wrap("-dx11", prefix), format!("{prefix}%command% -dx11"));
    }
    #[test]
    fn shell_values_cannot_execute_commands() {
        let dangerous = "a'$(touch /tmp/should-not-exist)";
        let out = Command::new("sh")
            .args(["-c", &format!("printf %s {}", quote(dangerous))])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), dangerous);
    }
}
