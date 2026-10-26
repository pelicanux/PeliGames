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
    #[serde(default)]
    title: String,
    value: String,
    enabled: bool,
    kind: String,
    #[serde(default)] min: Option<u32>,
    #[serde(default)] max: Option<u32>,
    #[serde(default)] choices: Vec<OptionChoice>,
    #[serde(default)] extra_env: BTreeMap<String,String>,
    #[serde(default)] runner_probe: bool,
}
#[derive(Deserialize)]
struct OptionChoice { value:String }

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
    pub mangohud_preset: Option<super::mangohud_presets::Preset>,
    pub overlay_enabled: Option<bool>,
    pub background_fps: Option<u32>,
    pub mangohud_enabled: Option<bool>,
    pub gamescope: GamescopeSettings,
    pub options: BTreeMap<String, bool>,
    pub option_values: BTreeMap<String, String>,
    pub dll_overrides: String,
    pub wrappers: Vec<LaunchWrapper>,
    pub launch_backend: String,
    /// AFMF frame generation for this game (a Vulkan layer driven by environment variables).
    pub afmf: Option<AfmfSettings>,
}
/// afmf-linux options. Read once at game start; there is no live control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AfmfSettings {
    pub enabled: bool,
    /// auto, quality or performance.
    pub performance_mode: String,
    /// auto, standard or high.
    pub search_mode: String,
    /// blend or repeat.
    pub fast_motion: String,
    /// Real frame rate below which no frame is generated; 0 always generates.
    pub min_fps: u32,
    pub hud_detect: bool,
    pub pacing: bool,
}
impl Default for AfmfSettings {
    fn default() -> Self {
        Self { enabled: false, performance_mode: "auto".into(), search_mode: "auto".into(), fast_motion: "blend".into(), min_fps: 30, hud_detect: true, pacing: true }
    }
}
impl AfmfSettings {
    fn validate(&self) -> Result<(), String> {
        if !["auto", "quality", "performance"].contains(&self.performance_mode.as_str())
            || !["auto", "standard", "high"].contains(&self.search_mode.as_str())
            || !["blend", "repeat"].contains(&self.fast_motion.as_str())
        {
            return Err("Opção do AFMF inválida.".into());
        }
        if self.min_fps > 240 { return Err("Informe o FPS mínimo do AFMF entre 0 e 240.".into()); }
        Ok(())
    }
    /// The variables afmf-linux reads, without the layer path.
    pub fn environment(&self, gamescope: bool) -> Vec<(&'static str, String)> {
        let flag = |on: bool| if on { "1" } else { "0" }.to_string();
        let mut env = vec![
            ("AFMF_ENABLE", "1".to_string()),
            ("AFMF_PERFORMANCE_MODE", self.performance_mode.clone()),
            ("AFMF_SEARCH_MODE", self.search_mode.clone()),
            ("AFMF_FAST_MOTION_RESPONSE", self.fast_motion.clone()),
            ("AFMF_MIN_FPS", self.min_fps.to_string()),
            ("AFMF_HUD_DETECT", flag(self.hud_detect)),
            ("AFMF_PACING", flag(self.pacing)),
            // Never stack another frame generator layer.
            ("DISABLE_LSFGVK", "1".to_string()),
        ];
        if gamescope { env.push(("AFMF_GAMESCOPE", "1".to_string())); }
        env
    }
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
    pub hdr: bool,
    pub adaptive_sync: bool,
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
            hdr: false,
            adaptive_sync: false,
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
            || self.hdr || self.adaptive_sync
            || !self.additional_options.trim().is_empty()
    }
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct SavedSettings {
    schema_version: u32,
    launch_options: Option<String>,
    mangohud_fps_limit: Option<u32>,
    mangohud_preset: Option<super::mangohud_presets::Preset>,
    overlay_enabled: Option<bool>,
    background_fps: Option<u32>,
    mangohud_enabled: Option<bool>,
    options: BTreeMap<String, bool>,
    option_values: BTreeMap<String, String>,
    dll_overrides: Option<String>,
    wrappers: Vec<LaunchWrapper>,
    launch_backend: Option<String>,
    gamescope: GamescopeSettings,
    afmf: Option<AfmfSettings>,
}
impl From<SavedSettings> for AdvancedSettings {
    fn from(saved: SavedSettings) -> Self {
        let mut settings = Self::default();
        if saved.schema_version == 2 {
            settings.launch_options = saved.launch_options;
            settings.mangohud_fps_limit = saved.mangohud_fps_limit;
            settings.mangohud_preset = saved.mangohud_preset;
            settings.overlay_enabled = saved.overlay_enabled;
            settings.background_fps = saved.background_fps;
            settings.mangohud_enabled = saved.mangohud_enabled;
            settings.options = saved.options;
            settings.option_values = saved.option_values;
            settings.wrappers = saved.wrappers;
            settings.dll_overrides = if settings.options.get("dll_overrides") == Some(&true) { saved.dll_overrides.unwrap_or_default() } else { String::new() };
            settings.launch_backend = saved.launch_backend.unwrap_or_else(|| "system".into());
            settings.afmf = saved.afmf;
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
            mangohud_preset: None,
            overlay_enabled: None,
            background_fps: None,
            mangohud_enabled: None,
            gamescope: GamescopeSettings {
                model_version: 2,
                ..Default::default()
            },
            options: catalog().into_iter().map(|s| (s.id, s.enabled)).collect(),
            option_values: BTreeMap::new(),
            dll_overrides: String::new(),
            wrappers: Vec::new(),
            launch_backend: "system".into(),
            afmf: None,
        }
    }
}
impl AdvancedSettings {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(preset) = &self.mangohud_preset { super::mangohud_config::selected_settings(&super::mangohud_config::defaults(), Some(preset))?; }
        if self.background_fps.is_some_and(|fps| fps > 1000) { return Err("Informe FPS sem foco entre 1 e 1000, ou use Automático.".into()); }
        if self.background_fps.is_some() && self.overlay_enabled != Some(true) { return Err("Ative o Overlay PeliGames para limitar FPS sem foco.".into()); }
        if self.mangohud_fps_limit.is_some_and(|fps| fps > 1000) { return Err("Informe um limite de FPS entre 0 e 1000.".into()); }
        if super::release_features::enabled() && self.wrappers.iter().any(|w|w.enabled && w.program.contains("/FrameGeneration/profiles/") && w.program.ends_with(".sh")) {
            crate::commands::frame_generation::conflicts(self)?;
        }
        if let Some(value) = &self.launch_options { crate::commands::nexus_launch_options::validate_custom(value)?; }
        if let Some(afmf) = self.afmf.as_ref().filter(|a| a.enabled) {
            afmf.validate()?;
            if self.wrappers.iter().any(|w| w.enabled && w.program.contains("/FrameGeneration/profiles/") && w.program.ends_with(".sh")) {
                return Err("MAKO e AFMF não podem ser usados juntos. Desative um deles na aba Geração de quadros.".into());
            }
            if self.on("mlfg") || self.on("optiscaler") {
                return Err("Desative a geração de quadros por ML e o OptiScaler nas opções avançadas antes de usar o AFMF.".into());
            }
        }
        if self.wrappers.len() > 16 { return Err("Use até 16 wrappers.".into()); }
        for wrapper in &self.wrappers {
            if wrapper.program.trim().is_empty() || wrapper.program.len() > 4096 || wrapper.program.chars().any(|c| c.is_control()) { return Err("Informe o executável do wrapper.".into()); }
            wrapper_args(&wrapper.arguments)?;
        }
        let known = catalog();
        for (id,value) in &self.option_values {
            let spec=known.iter().find(|spec| &spec.id==id).ok_or("Campo avançado desconhecido")?;
            if spec.min.is_none() && spec.choices.is_empty() { return Err("Campo avançado não configurável".into()); }
            if !spec.choices.is_empty() && !spec.choices.iter().any(|choice| &choice.value==value) {
                return Err(format!("Valor inválido para {}",spec.title));
            }
            if let Some(min)=spec.min {
                let parsed=value.parse::<u32>().ok().filter(|_|!value.is_empty()&&value.bytes().all(|c|c.is_ascii_digit()));
                if !parsed.is_some_and(|n|n>=min&&n<=spec.max.unwrap_or(min)) { return Err(format!("Informe {} entre {} e {}",spec.title,min,spec.max.unwrap_or(min))); }
            }
        }
        if self.on("dxvk_limit") && (self.mangohud_fps_limit.unwrap_or(0)>0 || self.gamescope.enable_limiter) {
            return Err("Escolha apenas um limitador de FPS: DXVK, MangoHUD ou Gamescope.".into());
        }
        if self.on("no_hdr") && self.gamescope.hdr { return Err("HDR do Gamescope conflita com a opção de desligar HDR.".into()); }

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
        if g.enabled() && (self.on("mangohud") || self.on("mangohud_env") || self.mangohud_fps_limit.unwrap_or(0) > 0 || self.background_fps.is_some()) {
            return Err("Desative o MangoHUD e o limitador individual antes de usar o Gamescope.".into());
        }
        Ok(())
    }
    pub fn validate_runner(&self, proton: &Path) -> Result<(), String> {
        self.validate()?;
        if !catalog()
            .iter()
            .any(|spec| (spec.key.starts_with("PROTON_") || spec.id == "steam_client" || spec.runner_probe) && self.on(&spec.id))
        {
            return Ok(());
        }
        let proton_only = || catalog().into_iter().filter(|spec| (spec.key.starts_with("PROTON_") || spec.id == "steam_client" || spec.runner_probe) && self.on(&spec.id));
        let Some(script) = read_runner_text(&proton.join("proton"), 2*1024*1024) else {
            // A plain Wine runner has no Proton features at all; say so instead of "cannot verify".
            if let Some(spec) = proton_only().next().filter(|_| proton.join("bin/wine").is_file()) {
                return Err(format!("“{}” é um recurso do Proton e não funciona com Wine. Escolha um Proton compatível, como GE-Proton ou Proton-CachyOS, ou desative essa opção.", spec.title));
            }
            return Err("Não foi possível verificar os recursos do Proton selecionado.".into());
        };
        let docs=runner_documentation(proton);
        if let Some(spec) = proton_only().find(|spec| !runner_advertises(&script, &docs, &spec.key) && !ships_feature(proton, &spec.id)) {
            return Err(format!("O Proton selecionado não tem suporte a “{}” ({}). Escolha um Proton compatível, como GE-Proton ou Proton-CachyOS, ou desative essa opção.", spec.title, spec.key));
        }
        Ok(())
    }
    pub fn validate_native(&self)->Result<(),String> {
        self.validate()?;
        if let Some(spec)=catalog().into_iter().find(|spec|(spec.key.starts_with("PROTON_")||spec.id=="steam_client"||spec.runner_probe)&&self.on(&spec.id)) {
            return Err(format!("“{}” requer Proton e não se aplica a um jogo Linux nativo.",spec.title));
        }
        Ok(())
    }
    pub fn runner_issues(root:&Path)->BTreeMap<String,String> {
        let mut result=BTreeMap::new();
        let script=read_runner_text(&root.join("proton"),2*1024*1024).unwrap_or_default();
        let docs=runner_documentation(root);
        for spec in catalog().into_iter().filter(|spec|spec.key.starts_with("PROTON_")||spec.id=="steam_client"||spec.runner_probe) {
            // Check capability evidence independently of dependencies/conflicts.
            if !runner_advertises(&script,&docs,&spec.key)&&!ships_feature(root,&spec.id) {
                result.insert(spec.id,format!("O runner selecionado não anuncia {}. Escolha um runner compatível ou mantenha a opção desligada.",spec.key));
            }
        }
        result
    }
    fn on(&self, id: &str) -> bool {
        self.options
            .get(id)
            .copied()
            .unwrap_or_else(|| catalog().iter().any(|s| s.id == id && s.enabled))
    }
}
/// Bounded source/documentation probes; never infer support from version names.
fn read_runner_text(path:&Path,limit:u64)->Option<String> {
    use std::io::Read;
    let mut text=String::new();std::fs::File::open(path).ok()?.take(limit).read_to_string(&mut text).ok()?;Some(text)
}
fn runner_documentation(root:&Path)->String {
    ["README.md","CHANGELOG.md","user_settings.sample.py","docs/FSR4.md","docs/EM-ADDITIONS.md"].iter()
        .filter_map(|name|read_runner_text(&root.join(name),512*1024)).collect::<Vec<_>>().join("\n")
}
fn runner_advertises(script:&str,docs:&str,key:&str)->bool {
    script.contains(key)||docs.contains(&format!("{key}=1"))
}
/// Features a runner reads outside its `proton` script. GE-Proton and
/// Proton-CachyOS check PROTON_DISCORD_BRIDGE inside steam.exe; the script only
/// copies the bridge into the prefix, so the bridge itself is the evidence.
fn ships_feature(proton: &Path, id: &str) -> bool {
    match id {
        "discord" => ["files", "dist"].iter().any(|dir| proton.join(dir).join("lib/wine/discord-rpc-bridge/bridge.exe").is_file()),
        _ => false,
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
        "--expose-wayland", "--hdr-enabled", "--adaptive-sync",
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
    if g.hdr { args.push("--hdr-enabled".into()); }
    if g.adaptive_sync { args.push("--adaptive-sync".into()); }
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
    // Launch time, not profile time: a Flatpak/Snap Discord needs a top-level
    // socket link in this session for Proton's bridge. Steam games get it from PeliRun.
    if settings.is_some_and(|s| s.on("discord")) {
        if let Err(error) = super::discord_socket::link_sandboxed(&super::discord_socket::runtime_dir()) {
            super::logger::event("WARN", &error);
        }
    }
    super::env_utils::host_environment(&mut command);
    Ok(command)
}
pub(crate) fn configured_launch_command(umu: &Path, settings: Option<&AdvancedSettings>) -> Result<Command, String> {
    if let Some(settings)=settings { super::advanced_capabilities::validate(settings)?; }
    let global = super::mangohud_config::global_enabled()?;
    if settings.is_none() && !global { return Ok(Command::new(umu)); }
    let effective = with_global_monitoring(settings, global);
    if global && effective.mangohud_enabled != Some(false) && effective.gamescope.enabled() {
        return Err("O MangoHUD global não é compatível com este modo de Gamescope. Desative a chave global de MangoHUD para executar com Gamescope.".into());
    }
    if effective.overlay_enabled == Some(true) { crate::commands::frame_overlay_runtime::install_standalone()?; }
    let mut command = launch_command_with_tools(umu, Some(&effective), executable)?;
    super::mangohud_config::apply_with_preset(&mut command, settings.is_some_and(|s| s.on("mangohud") || s.on("mangohud_env")), effective.mangohud_fps_limit, effective.mangohud_enabled, effective.mangohud_preset.as_ref())?;
    // DXVK owns the cap for this invocation; retain MangoHUD's overlay but
    // prevent a global MangoHUD profile from silently applying a second cap.
    if effective.on("dxvk_limit") {
        command.env("MANGOHUD_CONFIG", "read_cfg,fps_limit=0");
    }
    if super::release_features::enabled() {
        crate::commands::frame_generation::configure_monitoring(&mut command, &effective)?;
        super::afmf_compatibility::apply(&mut command);
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
    if (display || effective.mangohud_fps_limit.unwrap_or(0) > 0 || effective.background_fps.is_some()) && !effective.on("mangohud") && !effective.on("mangohud_env") {
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
    if settings.overlay_enabled == Some(true) { wrappers.push((resolve(&crate::commands::frame_overlay_runtime::standalone_runner()?.to_string_lossy())?, vec!["--".into()])); }
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
                    settings.option_values.get(&spec.id).unwrap_or(&spec.value)
                },
            );
            for (key,value) in &spec.extra_env { command.env(key,value); }
        } else {
            command.env_remove(&spec.key);
            for key in spec.extra_env.keys() { command.env_remove(key); }
        }
    }
    if let Some(enabled) = settings.overlay_enabled {
        command.env("PELI_OVERLAY_GENERAL", if enabled {"1"} else {"0"}).env("PELI_OVERLAY_DISABLED",if enabled {"0"} else {"1"});
        if enabled {command.env("PELI_OVERLAY_CONFIG_ROOT",super::paths::app_root()?);}
    }
    command.env_remove("PELI_OVERLAY_BACKGROUND_FPS");
    if let Some(fps)=settings.background_fps {command.env("PELI_OVERLAY_BACKGROUND_FPS",fps.to_string());}
    command.env_remove("AFMF_ENABLE").env_remove("AFMF_GAMESCOPE").env_remove("AFMF_ASYNC");
    if let Some(afmf) = settings.afmf.as_ref().filter(|a| a.enabled) {
        // The layer stays in PeliGames' folder and is offered only to this game.
        super::afmf_compatibility::layer(&mut command,
            &crate::commands::frame_generation::afmf_layer_dir()?,
            std::env::var_os("VK_ADD_LAYER_PATH"), std::env::var_os("VK_INSTANCE_LAYERS"),
            std::env::var_os("VK_LAYER_PATH"))?;
        for (key, value) in afmf.environment(settings.gamescope.enabled()) { command.env(key, value); }
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
    #[test]
    fn discord_bridge_is_recognized_by_the_bridge_the_runner_ships() {
        // GE-Proton/CachyOS read PROTON_DISCORD_BRIDGE in steam.exe, not in the script.
        let mut settings = AdvancedSettings::default();
        settings.options.insert("discord".into(), true);
        let root = std::env::temp_dir().join(format!("peligames-discord-bridge-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("proton"), "copies wine/discord-rpc-bridge/bridge.exe").unwrap();
        assert!(settings.validate_runner(&root).unwrap_err().contains("PROTON_DISCORD_BRIDGE"));
        let bridge = root.join("files/lib/wine/discord-rpc-bridge");
        std::fs::create_dir_all(&bridge).unwrap();
        std::fs::write(bridge.join("bridge.exe"), b"MZ").unwrap();
        assert!(settings.validate_runner(&root).is_ok());
        // A plain Wine runner says the option belongs to Proton, not "cannot verify".
        let wine = root.join("wine-runner");
        std::fs::create_dir_all(wine.join("bin")).unwrap();
        std::fs::write(wine.join("bin/wine"), "#!/bin/sh\n").unwrap();
        assert!(settings.validate_runner(&wine).unwrap_err().contains("não funciona com Wine"));
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

#[cfg(test)]
mod overlay_monitoring_tests {
    use super::*;
    #[test]
    fn background_cap_needs_overlay_and_rejects_incompatible_compositor() {
        let mut settings=AdvancedSettings::default();settings.background_fps=Some(0);
        assert!(settings.validate().is_err());settings.overlay_enabled=Some(true);assert!(settings.validate().is_ok());
        settings.background_fps=Some(1001);assert!(settings.validate().is_err());
        settings.background_fps=Some(30);settings.gamescope.enable_limiter=true;assert!(settings.validate().is_err());
    }
    #[test]
    fn overlay_and_hidden_limiter_run_without_frame_generation() {
        let mut saved=AdvancedSettings::default();saved.overlay_enabled=Some(true);saved.background_fps=Some(0);saved.mangohud_enabled=Some(false);
        let original=serde_json::to_value(&saved).unwrap();let effective=with_global_monitoring(Some(&saved),false);
        let command=launch_command_with_tools(Path::new("umu"),Some(&effective),|name|Ok(PathBuf::from(name))).unwrap();
        let words:Vec<_>=std::iter::once(command.get_program()).chain(command.get_args()).map(|v|v.to_string_lossy().into_owned()).collect();
        assert!(words.iter().any(|word|word.ends_with("peligames-overlay-run")));assert!(!words.iter().any(|word|word=="gamescope"||word.contains("mako")));
        assert!(command.get_envs().any(|(key,value)|key=="PELI_OVERLAY_BACKGROUND_FPS"&&value==Some(std::ffi::OsStr::new("0"))));
        assert_eq!(original,serde_json::to_value(saved).unwrap());
    }
    #[test]
    fn old_profiles_keep_legacy_overlay_and_new_profiles_remember_explicit_off() {
        let old:AdvancedSettings=serde_json::from_str(r#"{"schema_version":2}"#).unwrap();assert_eq!(old.overlay_enabled,None);assert_eq!(old.background_fps,None);
        let mut next=old;next.overlay_enabled=Some(false);let restored:AdvancedSettings=serde_json::from_value(serde_json::to_value(next).unwrap()).unwrap();assert_eq!(restored.overlay_enabled,Some(false));
    }
}

#[cfg(test)]
mod afmf_tests {
    use super::*;
    #[test]
    fn afmf_variables_follow_the_game_settings() {
        let afmf = AfmfSettings { enabled: true, performance_mode: "quality".into(), min_fps: 45, hud_detect: false, ..Default::default() };
        let env: std::collections::BTreeMap<_, _> = afmf.environment(true).into_iter().collect();
        assert_eq!(env["AFMF_ENABLE"], "1");
        assert_eq!(env["AFMF_PERFORMANCE_MODE"], "quality");
        assert_eq!(env["AFMF_MIN_FPS"], "45");
        assert_eq!(env["AFMF_HUD_DETECT"], "0");
        assert_eq!(env["AFMF_PACING"], "1");
        assert_eq!(env["AFMF_GAMESCOPE"], "1");
        assert_eq!(env["DISABLE_LSFGVK"], "1");
        assert!(!afmf.environment(false).iter().any(|(k, _)| *k == "AFMF_GAMESCOPE"));
    }
    #[test]
    fn afmf_never_runs_with_another_frame_generator() {
        let mut settings = AdvancedSettings { afmf: Some(AfmfSettings { enabled: true, ..Default::default() }), ..Default::default() };
        settings.validate().unwrap();
        settings.wrappers.push(LaunchWrapper { program: "/home/u/.config/PeliGames/FrameGeneration/profiles/game.sh".into(), arguments: String::new(), enabled: true });
        assert!(settings.validate().unwrap_err().contains("MAKO e AFMF"));
        settings.wrappers.clear();
        settings.options.insert("optiscaler".into(), true);
        assert!(settings.validate().unwrap_err().contains("OptiScaler"));
        settings.options.insert("optiscaler".into(), false);
        settings.afmf.as_mut().unwrap().performance_mode = "turbo".into();
        assert!(settings.validate().is_err());
        // A disabled AFMF entry is kept but never checked or applied.
        settings.afmf.as_mut().unwrap().enabled = false;
        settings.validate().unwrap();
    }
    #[test]
    fn saved_settings_keep_the_afmf_choice() {
        let mut settings = AdvancedSettings::default();
        settings.afmf = Some(AfmfSettings { enabled: true, min_fps: 0, ..Default::default() });
        let restored: AdvancedSettings = serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(restored.afmf, settings.afmf);
    }
    #[test]
    fn configurable_values_are_bounded_and_survive_roundtrip() {
        let mut settings=AdvancedSettings::default();
        settings.options.insert("dxvk_limit".into(),true);
        settings.option_values.insert("dxvk_limit".into(),"144".into());
        let restored:AdvancedSettings=serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        let command=launch_command_with_tools(Path::new("umu"),Some(&restored),|name|Ok(PathBuf::from(name))).unwrap();
        assert_eq!(command.get_envs().find(|(k,_)|*k=="DXVK_FRAME_RATE").unwrap().1.unwrap(),"144");
        for value in ["0","1001","-1","60;echo bad",""] {
            settings.option_values.insert("dxvk_limit".into(),value.into());assert!(settings.validate().is_err());
        }
        settings.option_values.clear();settings.option_values.insert("dxvk_log".into(),"trace".into());assert!(settings.validate().is_err());
        settings.option_values.insert("dxvk_log".into(),"warn".into());settings.validate().unwrap();
    }
    #[test]
    fn competing_limiters_and_hdr_paths_are_rejected() {
        let mut settings=AdvancedSettings::default();settings.options.insert("dxvk_limit".into(),true);
        settings.mangohud_fps_limit=Some(60);assert!(settings.validate().is_err());
        settings.mangohud_fps_limit=None;settings.gamescope.enable_limiter=true;assert!(settings.validate().is_err());
        settings.gamescope.enable_limiter=false;settings.options.insert("dxvk_limit".into(),false);
        settings.gamescope.hdr=true;settings.options.insert("no_hdr".into(),true);assert!(settings.validate().is_err());
        settings.options.insert("no_hdr".into(),false);settings.gamescope.adaptive_sync=true;settings.validate().unwrap();
        assert!(gamescope_args(&settings).contains(&"--hdr-enabled".into()));
        assert!(gamescope_args(&settings).contains(&"--adaptive-sync".into()));
    }
    #[test]
    fn composite_gpu_preset_is_removed_completely_when_disabled() {
        let mut settings=AdvancedSettings::default();settings.options.insert("nvidia_offload".into(),true);
        let command=launch_command_with_tools(Path::new("umu"),Some(&settings),|name|Ok(PathBuf::from(name))).unwrap();
        assert!(command.get_envs().any(|(key,value)|key=="__GLX_VENDOR_LIBRARY_NAME"&&value==Some(std::ffi::OsStr::new("nvidia"))));
        settings.options.insert("nvidia_offload".into(),false);
        let command=launch_command_with_tools(Path::new("umu"),Some(&settings),|name|Ok(PathBuf::from(name))).unwrap();
        assert!(command.get_envs().any(|(key,value)|key=="__GLX_VENDOR_LIBRARY_NAME"&&value.is_none()));
    }
    #[test]
    fn custom_runner_features_require_evidence_not_names() {
        let root=std::env::temp_dir().join(format!("GE-Proton-999-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join("proton"),"unsupported").unwrap();
        let mut settings=AdvancedSettings::default();settings.options.insert("d7vk".into(),true);
        assert!(settings.validate_runner(&root).unwrap_err().contains("PROTON_USE_D7VK"));
        std::fs::write(root.join("README.md"),"Supported: PROTON_USE_D7VK=1").unwrap();settings.validate_runner(&root).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn editor_capabilities_and_native_validation_match_runner_checks() {
        let root=std::env::temp_dir().join(format!("advanced-runner-status-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join("proton"),"PROTON_USE_D7VK").unwrap();
        let issues=AdvancedSettings::runner_issues(&root);
        assert!(!issues.contains_key("d7vk"));assert!(issues.contains_key("prefer_sdl"));
        let mut settings=AdvancedSettings::default();settings.options.insert("d7vk".into(),true);
        assert!(settings.validate_native().unwrap_err().contains("Linux nativo"));
        settings.options.insert("d7vk".into(),false);settings.options.insert("zink".into(),true);settings.validate_native().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

}
