//! Per-entry launch options. Environment values and wrapper arguments never use a shell.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Deserialize)]
struct OptionSpec {
    id: String,
    key: String,
    value: String,
    enabled: bool,
    kind: String,
}
fn catalog() -> Vec<OptionSpec> {
    serde_json::from_str(include_str!("../../../src/config/advancedOptions.json"))
        .expect("invalid bundled advanced options")
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "SavedSettings")]
pub struct AdvancedSettings {
    pub schema_version: u32,
    pub launch_options: Option<String>,
    pub mangohud_fps_limit: Option<u32>,
    pub mangohud_enabled: Option<bool>,
    pub gamescope: GamescopeSettings,
    pub options: BTreeMap<String, bool>,
    pub dll_overrides: String,
    pub wrappers: Vec<LaunchWrapper>,
    pub launch_backend: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct LaunchWrapper { pub program: String, pub arguments: String, pub enabled: bool }
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GamescopeSettings {
    pub model_version: u32,
    pub enable_upscaling: bool,
    pub enable_limiter: bool,
    pub force_grab_cursor: bool,
    pub width: u32,
    pub height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub fps: u32,
    pub fps_unfocused: u32,
    pub filter: String,
    pub window_type: String,
    pub additional_options: String,
}
impl Default for GamescopeSettings {
    fn default() -> Self {
        Self {
            model_version: 0,
            enable_upscaling: false,
            enable_limiter: false,
            force_grab_cursor: false,
            width: 0,
            height: 0,
            output_width: 0,
            output_height: 0,
            fps: 0,
            fps_unfocused: 0,
            filter: "fsr".into(),
            window_type: "fullscreen".into(),
            additional_options: String::new(),
        }
    }
}
impl GamescopeSettings {
    pub fn enabled(&self) -> bool {
        self.enable_upscaling
            || self.enable_limiter
            || self.force_grab_cursor
            || !self.additional_options.trim().is_empty()
    }
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct SavedSettings {
    schema_version: u32,
    launch_options: Option<String>,
    mangohud_fps_limit: Option<u32>,
    mangohud_enabled: Option<bool>,
    options: BTreeMap<String, bool>,
    dll_overrides: Option<String>,
    wrappers: Vec<LaunchWrapper>,
    launch_backend: Option<String>,
    gamescope: GamescopeSettings,
}
impl From<SavedSettings> for AdvancedSettings {
    fn from(saved: SavedSettings) -> Self {
        let mut settings = Self::default();
        if saved.schema_version == 2 {
            settings.launch_options = saved.launch_options;
            settings.mangohud_fps_limit = saved.mangohud_fps_limit;
            settings.mangohud_enabled = saved.mangohud_enabled;
            settings.options = saved.options;
            settings.wrappers = saved.wrappers;
            settings.dll_overrides = if settings.options.get("dll_overrides") == Some(&true) { saved.dll_overrides.unwrap_or_default() } else { String::new() };
            settings.launch_backend = saved.launch_backend.unwrap_or_else(|| "system".into());
            if saved.gamescope.model_version == 2 {
                settings.gamescope = saved.gamescope;
            }
        }
        settings
    }
}
#[derive(Deserialize)]
struct Rules {
    dependencies: Vec<[String; 3]>,
    conflicts: Vec<[String; 3]>,
}
impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            launch_options: None,
            mangohud_fps_limit: None,
            mangohud_enabled: None,
            gamescope: GamescopeSettings {
                model_version: 2,
                ..Default::default()
            },
            options: catalog().into_iter().map(|s| (s.id, s.enabled)).collect(),
            dll_overrides: String::new(),
            wrappers: Vec::new(),
            launch_backend: "system".into(),
        }
    }
}
impl AdvancedSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.mangohud_fps_limit.is_some_and(|fps| fps > 1000) { return Err("Informe um limite de FPS entre 0 e 1000.".into()); }
        if super::release_features::enabled() && self.wrappers.iter().any(|w|w.enabled && w.program.contains("/FrameGeneration/profiles/") && w.program.ends_with(".sh")) {
            crate::commands::frame_generation::conflicts(self)?;
        }
        if let Some(value) = &self.launch_options { crate::commands::nexus_launch_options::validate_custom(value)?; }
        if self.wrappers.len() > 16 { return Err("Use até 16 wrappers.".into()); }
        for wrapper in &self.wrappers {
            if wrapper.program.trim().is_empty() || wrapper.program.len() > 4096 || wrapper.program.chars().any(|c| c.is_control()) { return Err("Informe o executável do wrapper.".into()); }
            wrapper_args(&wrapper.arguments)?;
        }
        let known = catalog();
        if self
            .options
            .keys()
            .any(|id| !known.iter().any(|s| &s.id == id))
        {
            return Err("Opção avançada desconhecida.".into());
        }
        if !["system", "gamescope"].contains(&self.launch_backend.as_str()) {
            return Err("Modo de exibição inválido.".into());
        }
        if self.dll_overrides.len() > 2048
            || self
                .dll_overrides
                .chars()
                .any(|c| c == '\0' || c == '\n' || c == '\r')
        {
            return Err("Configuração de DLL inválida.".into());
        }
        let rules: Rules =
            serde_json::from_str(include_str!("../../../src/config/advancedRules.json")).unwrap();
        for [id, dependency, message] in rules.dependencies {
            if self.on(&id) && !self.on(&dependency) {
                return Err(message);
            }
        }
        for [a, b, message] in rules.conflicts {
            if self.on(&a) && self.on(&b) {
                return Err(message);
            }
        }
        if self.on("dll_overrides") && !valid_dlls(&self.dll_overrides) {
            return Err("Use DLLs no formato dxgi=n,b;dinput8=n,b.".into());
        }
        let g = &self.gamescope;
        if [g.width, g.height, g.output_width, g.output_height]
            .iter()
            .any(|v| *v > 16384)
            || g.fps > 1000
            || g.fps_unfocused > 1000
        {
            return Err("Dimensões ou FPS do Gamescope fora do limite.".into());
        }
        if !["fsr", "nis", "integer", "stretch"].contains(&g.filter.as_str())
            || !["fullscreen", "borderless", "windowed"].contains(&g.window_type.as_str())
        {
            return Err("Método de escala ou tipo de janela inválido.".into());
        }
        additional_args(&g.additional_options)?;
        if g.enabled() && (self.on("mangohud") || self.on("mangohud_env") || self.mangohud_fps_limit.unwrap_or(0) > 0) {
            return Err("Desative o MangoHUD e o limitador individual antes de usar o Gamescope.".into());
        }
        Ok(())
    }
    pub fn validate_runner(&self, proton: &Path) -> Result<(), String> {
        self.validate()?;
        if !catalog()
            .iter()
            .any(|spec| (spec.key.starts_with("PROTON_") || spec.id == "steam_client") && self.on(&spec.id))
        {
            return Ok(());
        }
        let script = std::fs::read_to_string(proton.join("proton")).map_err(|_| {
            "Não foi possível verificar os recursos do Proton selecionado.".to_string()
        })?;
        for spec in catalog() {
            if (spec.key.starts_with("PROTON_") || spec.id == "steam_client") && self.on(&spec.id) && !script.contains(&spec.key) {
                return Err(format!("O Proton selecionado não declara suporte a {}. Escolha um runner compatível ou desative essa opção.", spec.key));
            }
        }
        Ok(())
    }
    fn on(&self, id: &str) -> bool {
        self.options
            .get(id)
            .copied()
            .unwrap_or_else(|| catalog().iter().any(|s| s.id == id && s.enabled))
    }
}
fn valid_dlls(value: &str) -> bool {
    !value.is_empty()
        && value.split(';').all(|entry| {
            let Some((names, modes)) = entry.split_once('=') else {
                return false;
            };
            names.split(',').all(|n| {
                !n.is_empty()
                    && n.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_.*-".contains(c))
            }) && ["", "n", "b", "d", "n,b", "b,n"].contains(&modes)
        })
}
pub fn wrapper_args(value: &str) -> Result<Vec<String>, String> {
    if value.len() > 4096 || value.chars().any(|c| c.is_control()) { return Err("Argumentos do wrapper inválidos.".into()); }
    let mut args = Vec::new(); let mut token = String::new(); let mut quote = None;
    let mut escaped = false; let mut started = false;
    for c in value.chars() {
        if escaped { token.push(c); escaped = false; started = true; }
        else if c == '\\' && quote != Some('\'') { escaped = true; started = true; }
        else if let Some(q) = quote { if c == q { quote = None; } else { token.push(c); } }
        else if c == '\'' || c == '"' { quote = Some(c); started = true; }
        else if c.is_whitespace() { if started { args.push(std::mem::take(&mut token)); started = false; } }
        else { token.push(c); started = true; }
    }
    if quote.is_some() || escaped { return Err("Feche as aspas dos argumentos do wrapper.".into()); }
    if started { args.push(token); }
    Ok(args)
}
fn additional_args(value: &str) -> Result<Vec<String>, String> {
    if value.len() > 4096
        || value
            .chars()
            .any(|c| c.is_control() || "$`;&|<>".contains(c))
    {
        return Err("Informe apenas argumentos do Gamescope, sem comandos de shell.".into());
    }
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escape = false;
    for c in value.chars() {
        if escape {
            token.push(c);
            escape = false;
        } else if c == '\\' && quote != Some('\'') {
            escape = true;
        } else if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                token.push(c);
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c.is_whitespace() {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else {
            token.push(c);
        }
    }
    if quote.is_some() || escape {
        return Err("Feche as aspas das opções adicionais.".into());
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    if tokens.first().is_some_and(|t| !t.starts_with('-')) {
        return Err("As opções adicionais precisam começar por uma opção do Gamescope.".into());
    }
    let reserved = [
        "-w",
        "-h",
        "-W",
        "-H",
        "-r",
        "-o",
        "-F",
        "-S",
        "-f",
        "-b",
        "--force-grab-cursor",
        "--nested-width",
        "--nested-height",
        "--output-width",
        "--output-height",
        "--nested-refresh",
        "--nested-unfocused-refresh",
        "--filter",
        "--scaler",
        "--fullscreen",
        "--borderless",
        "--expose-wayland",
    ];
    if tokens
        .iter()
        .any(|t| t == "--" || reserved.contains(&t.split('=').next().unwrap_or("")))
    {
        return Err("Configure resolução, escala, FPS, janela e cursor nos campos próprios; não use -- nas opções adicionais.".into());
    }
    Ok(tokens)
}
fn gamescope_args(s: &AdvancedSettings) -> Vec<String> {
    let g = &s.gamescope;
    let mut args = Vec::new();
    if g.enable_upscaling {
        for (flag, value) in [
            ("-w", g.width),
            ("-h", g.height),
            ("-W", g.output_width),
            ("-H", g.output_height),
        ] {
            if value > 0 {
                args.extend([flag.into(), value.to_string()]);
            }
        }
        match g.filter.as_str() {
            "fsr" | "nis" => args.extend(["-F".into(), g.filter.clone()]),
            "integer" | "stretch" => args.extend(["-S".into(), g.filter.clone()]),
            _ => {}
        }
        if g.window_type == "fullscreen" {
            args.push("-f".into());
        } else if g.window_type == "borderless" {
            args.push("-b".into());
        }
    }
    if g.enable_limiter {
        if g.fps > 0 {
            args.extend(["-r".into(), g.fps.to_string()]);
        }
        if g.fps_unfocused > 0 {
            args.extend(["-o".into(), g.fps_unfocused.to_string()]);
        }
    }
    if g.force_grab_cursor {
        args.push("--force-grab-cursor".into());
    }
    if s.on("wayland") {
        args.push("--expose-wayland".into());
    }
    args.extend(additional_args(&g.additional_options).expect("validated additional options"));
    args
}
fn executable(name: &str) -> Result<PathBuf, String> {
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let file = dir.join(name);
        if file.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if file
                    .metadata()
                    .map(|m| m.permissions().mode() & 0o111 == 0)
                    .unwrap_or(true)
                {
                    continue;
                }
            }
            return Ok(file);
        }
    }
    Err(format!("{name} não está instalado ou não está no PATH. Instale a ferramenta ou desative sua opção avançada."))
}
/// The command that runs umu-run (and the game) with the game's settings, in
/// the host's environment rather than the AppImage's.
pub fn launch_command(umu: &Path, settings: Option<&AdvancedSettings>) -> Result<Command, String> {
    let mut command = configured_launch_command(umu, settings)?;
    super::env_utils::host_environment(&mut command);
    Ok(command)
}
pub(crate) fn configured_launch_command(umu: &Path, settings: Option<&AdvancedSettings>) -> Result<Command, String> {
    let global = super::mangohud_config::global_enabled()?;
    if settings.is_none() && !global { return Ok(Command::new(umu)); }
    let effective = with_global_monitoring(settings, global);
    if global && effective.mangohud_enabled != Some(false) && effective.gamescope.enabled() {
        return Err("O MangoHUD global não é compatível com este modo de Gamescope. Desative a chave global de MangoHUD para executar com Gamescope.".into());
    }
    let mut command = launch_command_with_tools(umu, Some(&effective), executable)?;
    super::mangohud_config::apply_with_override(&mut command, settings.is_some_and(|s| s.on("mangohud") || s.on("mangohud_env")), effective.mangohud_fps_limit, effective.mangohud_enabled)?;
    if super::release_features::enabled() {
        crate::commands::frame_generation::configure_monitoring(&mut command, &effective)?;
    }
    if !super::release_features::enabled() {
        let mut words: Vec<String> = std::iter::once(command.get_program()).chain(command.get_args()).map(|s|s.to_string_lossy().into_owned()).collect();
        let mut environment = command.get_envs().map(|(k,v)|(k.to_string_lossy().into_owned(),v.map(|s|s.to_string_lossy().into_owned()))).collect();
        super::release_features::restrict(&mut words, &mut environment, false);
        let mut restricted = Command::new(words.first().ok_or("Comando do jogo ausente")?);
        restricted.args(&words[1..]);
        for (key,value) in environment { match value {Some(v)=>{restricted.env(key,v);},None=>{restricted.env_remove(key);}} }
        if let Some(dir)=command.get_current_dir(){restricted.current_dir(dir);}
        command=restricted;
    }
    Ok(command)
}
fn with_global_monitoring(settings: Option<&AdvancedSettings>, global: bool) -> AdvancedSettings {
    let mut effective = settings.cloned().unwrap_or_default();
    if effective.mangohud_enabled == Some(false) {
        effective.options.insert("mangohud".into(), false);
        effective.options.insert("mangohud_env".into(), false);
    }
    let display = effective.mangohud_enabled.unwrap_or(global);
    if (display || effective.mangohud_fps_limit.unwrap_or(0) > 0) && !effective.on("mangohud") && !effective.on("mangohud_env") {
        effective.options.insert("mangohud".into(), true);
    }
    effective
}
fn launch_command_with_tools(
    umu: &Path,
    settings: Option<&AdvancedSettings>,
    resolve: impl Fn(&str) -> Result<PathBuf, String>,
) -> Result<Command, String> {
    let Some(settings) = settings else {
        return Ok(Command::new(umu));
    };
    settings.validate()?;
    let mut wrappers: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for wrapper in settings.wrappers.iter().filter(|wrapper| wrapper.enabled) {
        if !super::release_features::enabled() && super::release_features::experimental_program(&wrapper.program) { continue; }
        wrappers.push((resolve(wrapper.program.trim())?, wrapper_args(&wrapper.arguments)?));
    }
    if settings.gamescope.enabled() {
        let mut args = gamescope_args(settings); args.push("--".into());
        wrappers.push((resolve("gamescope")?, args));
    }
    for spec in catalog().into_iter().filter(|s| s.kind == "wrapper") {
        if settings.on(&spec.id) {
            wrappers.push((resolve(&spec.key)?, Vec::new()));
        }
    }
    let mut command = if let Some((first, args)) = wrappers.first() {
        let mut c = Command::new(first); c.args(args);
        for (program, args) in wrappers.iter().skip(1) { c.arg(program).args(args); }
        c.arg(umu); c
    } else { Command::new(umu) };
    for spec in catalog().into_iter().filter(|s| s.kind == "env") {
        if settings.on(&spec.id) {
            command.env(
                &spec.key,
                if spec.id == "dll_overrides" {
                    &settings.dll_overrides
                } else {
                    &spec.value
                },
            );
        } else {
            command.env_remove(&spec.key);
        }
    }
    if settings.on("hdr") {
        command.env("DXVK_HDR", "1");
    } else {
        command.env_remove("DXVK_HDR");
    }
    Ok(command)
}

#[cfg(test)]
mod steam_bridge_tests {
    use super::*;
    #[test]
    fn steam_bridge_is_opt_in_and_checks_runner_support() {
        let mut settings = AdvancedSettings::default();
        let env = |settings: &AdvancedSettings| {
            launch_command_with_tools(Path::new("umu"), Some(settings), |name| Ok(PathBuf::from(name))).unwrap()
                .get_envs().find(|(key, _)| *key == "UMU_USE_STEAM").map(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
        };
        assert_eq!(env(&settings), Some(None));
        settings.options.insert("steam_client".into(), true);
        assert_eq!(env(&settings), Some(Some("1".into())));
        let root = std::env::temp_dir().join(format!("peligames-steam-bridge-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("proton"), "unsupported runner").unwrap();
        assert!(settings.validate_runner(&root).unwrap_err().contains("UMU_USE_STEAM"));
        std::fs::write(root.join("proton"), "UMU_USE_STEAM").unwrap();
        assert!(settings.validate_runner(&root).is_ok());
        let restored: AdvancedSettings = serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(restored.on("steam_client"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod global_monitoring_tests {
    use super::*;
    #[test] fn default_enables_wrapper_without_changing_the_saved_game() {
        let game=AdvancedSettings::default();let effective=with_global_monitoring(Some(&game),true);
        assert!(!game.on("mangohud"));assert!(effective.on("mangohud"));
        let cmd=launch_command_with_tools(Path::new("umu"),Some(&effective),|n|Ok(PathBuf::from(n))).unwrap();
        assert_eq!(cmd.get_program(),"mangohud");assert!(cmd.get_args().any(|a|a=="umu"));
    }
    #[test] fn disabling_default_preserves_explicit_game_monitoring() {
        let mut game=AdvancedSettings::default();game.options.insert("mangohud_env".into(),true);
        let effective=with_global_monitoring(Some(&game),true);assert!(!effective.on("mangohud"));assert!(effective.on("mangohud_env"));
        assert!(with_global_monitoring(Some(&game),false).on("mangohud_env"));assert!(!with_global_monitoring(None,false).on("mangohud"));
    }
}

#[cfg(test)]
mod individual_fps_tests {
    use super::*;
    #[test]
    fn saved_limits_round_trip_and_old_profiles_keep_general_default() {
        let old: AdvancedSettings = serde_json::from_str(r#"{"schema_version":2}"#).unwrap();
        assert_eq!(old.mangohud_fps_limit, None);
        for limit in [None, Some(0), Some(60), Some(1000)] {
            let mut game = AdvancedSettings::default(); game.mangohud_fps_limit = limit;
            let restored: AdvancedSettings = serde_json::from_value(serde_json::to_value(&game).unwrap()).unwrap();
            assert_eq!(restored.mangohud_fps_limit, limit); assert!(restored.validate().is_ok());
        }
        let mut invalid = AdvancedSettings::default(); invalid.mangohud_fps_limit = Some(1001);
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn limiter_enables_wrapper_without_enabling_saved_hud() {
        let mut game = AdvancedSettings::default(); game.mangohud_fps_limit = Some(60);
        let effective = with_global_monitoring(Some(&game), false);
        assert!(!game.on("mangohud")); assert!(effective.on("mangohud"));
        let cmd = launch_command_with_tools(Path::new("umu"), Some(&effective), |n| Ok(PathBuf::from(n))).unwrap();
        assert_eq!(cmd.get_program(), "mangohud");
        game.mangohud_fps_limit = Some(0);
        assert!(!with_global_monitoring(Some(&game), false).on("mangohud"));
    }
    #[test]
    fn gamescope_rejects_individual_mangohud_limit() {
        let mut game = AdvancedSettings::default(); game.mangohud_fps_limit = Some(60);
        game.gamescope.enable_limiter = true;
        assert!(game.validate().is_err());
    }
}

#[cfg(test)]
mod per_game_monitoring_tests {
    use super::*;
    #[test]
    fn performance_game_override_wins_over_global_monitoring() {
        let mut game = AdvancedSettings::default();
        game.mangohud_enabled = Some(false);
        let hidden = with_global_monitoring(Some(&game), true);
        assert!(!hidden.on("mangohud") && !hidden.on("mangohud_env"));
        game.mangohud_enabled = Some(true);
        assert!(with_global_monitoring(Some(&game), false).on("mangohud"));
        game.mangohud_enabled = Some(false);
        game.mangohud_fps_limit = Some(60);
        let limited = with_global_monitoring(Some(&game), true);
        assert!(limited.on("mangohud")); // Loaded for the limiter; display stays explicitly hidden.
        assert_eq!(limited.mangohud_enabled, Some(false));
    }
    #[test]
    fn performance_override_roundtrip_preserves_independent_game_settings() {
        let mut first = AdvancedSettings::default();
        first.mangohud_enabled = Some(false);
        first.mangohud_fps_limit = Some(144);
        let restored: AdvancedSettings = serde_json::from_str(&serde_json::to_string(&first).unwrap()).unwrap();
        assert_eq!(restored.mangohud_enabled, Some(false));
        assert_eq!(restored.mangohud_fps_limit, Some(144));
        let second = AdvancedSettings::default();
        assert_eq!(second.mangohud_enabled, None);
        assert_eq!(second.mangohud_fps_limit, None);
    }
}
