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
    fn enabled(&self) -> bool {
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
            settings.options = saved.options;
            settings.wrappers = saved.wrappers;
            settings.dll_overrides = saved.dll_overrides.unwrap_or_else(|| "dxgi=n,b".into());
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
            gamescope: GamescopeSettings {
                model_version: 2,
                ..Default::default()
            },
            options: catalog().into_iter().map(|s| (s.id, s.enabled)).collect(),
            dll_overrides: "dxgi=n,b".into(),
            wrappers: Vec::new(),
            launch_backend: "system".into(),
        }
    }
}
impl AdvancedSettings {
    pub fn validate(&self) -> Result<(), String> {
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
        if g.enabled() && (self.on("mangohud") || self.on("mangohud_env")) {
            return Err("Desative os modos de MangoHud antes de usar o Gamescope.".into());
        }
        Ok(())
    }
    pub fn validate_runner(&self, proton: &Path) -> Result<(), String> {
        self.validate()?;
        if !catalog()
            .iter()
            .any(|spec| spec.key.starts_with("PROTON_") && self.on(&spec.id))
        {
            return Ok(());
        }
        let script = std::fs::read_to_string(proton.join("proton")).map_err(|_| {
            "Não foi possível verificar os recursos do Proton selecionado.".to_string()
        })?;
        for spec in catalog() {
            if spec.key.starts_with("PROTON_") && self.on(&spec.id) && !script.contains(&spec.key) {
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
pub fn launch_command(umu: &Path, settings: Option<&AdvancedSettings>) -> Result<Command, String> {
    launch_command_with_tools(umu, settings, executable)
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
