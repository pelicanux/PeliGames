//! Declarative game support. Modules contain data, never executable extension code.
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Definition {
    pub schema: u32,
    pub version: String,
    pub domain: String,
    pub name: String,
    pub platform: String,
    pub engine: Engine,
    pub executable: String,
    #[serde(default)]
    pub launch_executable: Option<String>,
    #[serde(default)]
    pub mod_launch_executable: Option<String>,
    #[serde(default)]
    pub launch_arguments: Vec<String>,
    #[serde(default)]
    pub activation: Option<Activation>,
    pub required: Vec<String>,
    pub header: Vec<u8>,
    #[serde(default)]
    pub steam_id: Option<String>,
    pub destinations: Vec<String>,
    #[serde(default)]
    pub allowed_files: Vec<String>,
    #[serde(default)]
    pub protected_files: Vec<String>,
    #[serde(default)]
    pub root_extensions: Vec<String>,
    pub routes: Vec<Route>,
    #[serde(default)]
    pub tools: Vec<GameTool>,
    #[serde(default)]
    pub frameworks: Vec<Framework>,
    #[serde(default)]
    pub dll_overrides: Vec<DllOverride>,
    #[serde(default)]
    pub config_files: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub credits: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub rejected_prefixes: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GameTool { pub name: String, pub executable: String, #[serde(default)] pub arguments: Vec<String> }
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Framework {
    pub name: String,
    pub mod_id: u64,
    /// Verified Nexus title; the display name may be translated or abbreviated.
    #[serde(default)]
    pub nexus_name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    pub required_files: Vec<String>,
    #[serde(default)]
    pub any_files: Vec<String>,
    pub for_paths: Vec<String>,
    #[serde(default)]
    pub for_extensions: Vec<String>,
}
impl Framework {
    fn verify_nexus_name(&self, actual: &str) -> Result<(), String> {
        let expected = self.nexus_name.as_deref().unwrap_or(&self.name);
        let normalize = |name: &str| name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
        if normalize(expected) != normalize(actual) {
            return Err(format!("O requisito {} aponta para outro mod no Nexus ({actual}). Abertura e download bloqueados; o cadastro do módulo precisa ser corrigido.", self.name));
        }
        Ok(())
    }

    pub(super) fn installed(&self, root: &Path) -> bool {
        let present = |path: &String| root.join(path).is_file()
            && fs::canonicalize(root.join(path)).is_ok_and(|file| file.starts_with(root));
        self.required_files.iter().all(present)
            && (self.any_files.is_empty() || self.any_files.iter().any(present))
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DllOverride { pub name: String, pub file: String }
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Activation {
    pub kind: ActivationKind,
    pub path: String,
    pub data: String,
    #[serde(default)]
    pub starred: bool,
}
#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ActivationKind { Bethesda, Morrowind, Memoria, Elden, BaldursGate3, Reloaded, Sims4 }
#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Engine { Deployment, ValheimProfile }
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Route {
    Prefix { source: String, target: String, #[serde(default)] search: bool },
    File { source: String, target: String, #[serde(default)] search: bool },
    Extension { extension: String, target: String, #[serde(default)] root_only: bool },
    Manifest { marker: String, target: String },
    Patch { target: String },
}
pub(super) fn relative(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':'))
}
impl Definition {
    pub(super) fn verify_framework_identity(&self, mod_id: u64, actual: &str) -> Result<(), String> {
        for framework in self.frameworks.iter().filter(|f| f.mod_id > 0 && f.mod_id == mod_id) {
            framework.verify_nexus_name(actual)?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != 1 || self.version.is_empty() || self.name.trim().is_empty()
            || self.domain.is_empty() || !self.domain.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            || !["native", "proton"].contains(&self.platform.as_str())
            || self.required.is_empty() || self.header.is_empty() || self.header.len() > 16
            || self.routes.is_empty() || self.destinations.is_empty() { return Err("Definição incompleta ou esquema não suportado.".into()); }
        if self.engine == Engine::ValheimProfile && (self.domain != "valheim" || self.platform != "native") {
            return Err("O perfil isolado BepInEx está reservado ao Valheim nativo.".into());
        }
        if self.steam_id.as_ref().is_some_and(|id| id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit())) {
            return Err("Steam ID inválido.".into());
        }
        for path in self.required.iter().chain(&self.destinations).chain(&self.allowed_files)
            .chain(&self.protected_files).chain(std::iter::once(&self.executable)).chain(self.launch_executable.iter()) {
            if !relative(path) || path.contains('{') || path.contains('}') { return Err(format!("Caminho inválido no módulo: {path}")); }
        }
        // Never let a deployment rule replace the game's identity files.
        if self.engine == Engine::Deployment && self.required.iter().chain(std::iter::once(&self.executable))
            .chain(self.launch_executable.iter()).any(|path| self.allowed(path)) {
            return Err("O destino não pode incluir executáveis ou arquivos de identificação do jogo.".into());
        }
        for tool in &self.tools {
            if tool.name.trim().is_empty() || !relative(&tool.executable) || !tool.executable.ends_with(".exe") || tool.arguments.len() > 16 || tool.arguments.iter().any(|a| a.len()>256 || a.chars().any(char::is_control)) {return Err("Ferramenta de mods inválida.".into());}
        }
        for framework in &self.frameworks {
            if framework.name.is_empty() || framework.nexus_name.as_ref().is_some_and(|name| name.trim().is_empty()) || (framework.mod_id == 0 && !framework.url.as_ref().is_some_and(|url| url.starts_with("https://"))) || (framework.required_files.is_empty() && framework.any_files.is_empty()) || framework.for_paths.is_empty()
                || framework.required_files.iter().chain(&framework.any_files).any(|path| !relative(path))
                || framework.for_paths.iter().any(|path| !relative(path.trim_end_matches('/'))) {
                return Err("Definição de carregador inválida.".into());
            }
            if framework.mod_id > 0 && framework.url.as_ref().is_some_and(|url| {
                let path = format!("{}/mods/{}", self.domain, framework.mod_id);
                ![format!("https://www.nexusmods.com/{path}"), format!("https://nexusmods.com/{path}")].contains(&url.trim_end_matches('/').to_string())
            }) {
                return Err("O link do requisito não corresponde ao jogo e ID cadastrados.".into());
            }
            if framework.for_extensions.iter().any(|ext| !ext.starts_with('.') || ext.len() < 2 || !ext[1..].bytes().all(|b| b.is_ascii_alphanumeric())) {
                return Err("Extensão de dependência inválida.".into());
            }
        }
        for item in &self.dll_overrides {
            if item.name.is_empty() || !item.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') || !relative(&item.file) {
                return Err("DLL override inválido.".into());
            }
        }
        if self.config_files.iter().any(|path| !relative(path)) { return Err("Configuração inválida no módulo.".into()); }
        if self.root_extensions.iter().any(|extension| ![".asi", ".ini"].contains(&extension.as_str())) {
            return Err("Extensão de raiz não permitida.".into());
        }
        if self.mod_launch_executable.as_ref().is_some_and(|path| !relative(path))
            || self.launch_arguments.len() > 16 || self.launch_arguments.iter().any(|arg| arg.len() > 256 || arg.chars().any(char::is_control)) {
            return Err("Configuração de execução inválida.".into());
        }
        if let Some(activation) = &self.activation {
            let suffix = match activation.kind { ActivationKind::Bethesda => ".txt", ActivationKind::Morrowind | ActivationKind::Memoria => ".ini", ActivationKind::Elden => ".toml", ActivationKind::BaldursGate3 => ".lsx", ActivationKind::Reloaded => "AppConfig.json", ActivationKind::Sims4 => "Resource.cfg" };
            if !relative(&activation.path) || !relative(&activation.data)
                || !self.allowed_files.contains(&activation.path) || !self.config_files.contains(&activation.path) {
                return Err("Destino de ativação inválido.".into());
            }
            if !activation.path.ends_with(suffix) {return Err("Formato de configuração de ativação inválido.".into());}
        }
        if self.rejected_prefixes.iter().any(|path| !relative(path.trim_end_matches('/'))) { return Err("Filtro de pacote inválido.".into()); }
        for route in &self.routes {
            let (source, target) = match route {
                Route::Prefix { source, target, .. } => {
                    if !source.ends_with('/') || !target.ends_with('/') { return Err("Rotas de pasta devem terminar com /.".into()); }
                    (Some(source.trim_end_matches('/')), target)
                }
                Route::File { source, target, .. } => (Some(source.as_str()), target),
                Route::Extension { extension, target, .. } => {
                    if !extension.starts_with('.') || extension.len() < 2 || !extension[1..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') || (!target.is_empty() && !target.ends_with('/')) {
                        return Err("Rota por extensão inválida.".into());
                    }
                    (None, target)
                }
                Route::Patch { target } => {
                    if self.domain != "helldivers2" || target != "data/" {return Err("Rota de patch fora do jogo autorizado.".into());}
                    (None, target)
                }
                Route::Manifest { marker, target } => {
                    if !relative(marker) || !target.ends_with('/') { return Err("Manifesto inválido.".into()); }
                    (None, target)
                }
            };
            if source.is_some_and(|source| !relative(source)) { return Err("Origem inválida na rota.".into()); }
            if let Route::Extension { extension, target, .. } = route {
                if target.is_empty() {
                    if !self.root_extensions.contains(extension) { return Err("Extensão não autorizada na raiz.".into()); }
                    continue;
                }
            }
            let target = target.replace("{mod_id}", "00000000-0000-0000-0000-000000000000");
            if !relative(target.trim_end_matches('/')) || target.contains('{') || target.contains('}') { return Err("Destino inválido na rota.".into()); }
            let sample = if target.ends_with('/') { format!("{target}module-check.{}", if self.domain == "crimsondesert" {"asi"} else {"bin"}) } else { target };
            if !self.allowed(&sample) { return Err(format!("Rota fora dos destinos permitidos: {sample}")); }
        }
        Ok(())
    }
    pub(super) fn allowed(&self, path: &str) -> bool {
        if self.domain == "crimsondesert" {
            let path = path.to_ascii_lowercase();
            let Some(file) = path.strip_prefix("bin64/") else { return false; };
            return relative(&path) && (matches!(file, "winmm.dll" | "version.dll")
                || (!file.contains('/') || file.starts_with("plugins/"))
                    && (file.ends_with(".asi") || file.ends_with(".ini")));
        }
        relative(path) && !self.protected_files.iter().any(|file|file.eq_ignore_ascii_case(path)) && (self.allowed_files.iter().any(|file| file == path)
            || !path.contains('/') && self.root_extensions.iter().any(|ext| path.to_ascii_lowercase().ends_with(ext))
            || self.destinations.iter().any(|folder| path.strip_prefix(folder).is_some_and(|rest| rest.starts_with('/'))))
    }
    pub(super) fn route(&self, source: &str, mod_id: &str) -> Result<Option<String>, String> {
        if !relative(source) { return Err(format!("Caminho inseguro no pacote: {source}")); }
        uuid::Uuid::parse_str(mod_id).map_err(|_| "Identificador de mod inválido.")?;
        let lower = source.to_ascii_lowercase();
        // REDmod's top-level mods directory is not CET's Lua mods directory.
        // Keep rejecting REDmod, including wrappers before an otherwise valid CET path.
        let cet_mods = (self.domain == "cyberpunk2077").then(|| {
            const CET: &str = "bin/x64/plugins/cyber_engine_tweaks/mods/";
            lower.match_indices(CET)
                .filter(|(index, _)| *index == 0 || lower.as_bytes()[*index - 1] == b'/')
                .map(|(index, _)| index + CET.len() - "mods/".len()).min()
        }).flatten();
        if self.rejected_prefixes.iter().any(|prefix| {
            let prefix = prefix.to_ascii_lowercase();
            lower.match_indices(&prefix).any(|(index, _)| {
                (index == 0 || lower.as_bytes()[index - 1] == b'/')
                    && !(prefix == "mods/" && cet_mods.is_some_and(|root| index >= root))
            })
        }) {
            return Err(format!("Este pacote exige um instalador especializado ainda não disponível neste módulo: {source}"));
        }
        for rule in &self.routes {
            let target = match rule {
                Route::Prefix { source: prefix, target, search } => {
                    let normalized = if self.platform == "proton" { source.to_ascii_lowercase() } else { source.to_string() };
                    let prefix_match = if self.platform == "proton" { prefix.to_ascii_lowercase() } else { prefix.clone() };
                    let indices: Vec<_> = normalized.match_indices(&prefix_match)
                        .filter(|(index, _)| *index == 0 || *search && normalized.as_bytes()[*index - 1] == b'/')
                        .map(|(index, _)| index).collect();
                    if indices.len() > 1 { return Err(format!("Estrutura ambígua no pacote: {source}")); }
                    indices.first().map(|index| format!("{target}{}", &source[index + prefix.len()..]))
                },
                Route::File { source: file, target, search } => {
                    let (path, file) = if self.platform == "proton" { (source.to_ascii_lowercase(), file.to_ascii_lowercase()) } else { (source.to_string(), file.clone()) };
                    (path == file || *search && path.ends_with(&format!("/{file}"))).then(|| target.clone())
                },
                Route::Extension { extension, target, root_only } => {
                    (!(*root_only && source.contains('/')) && source.to_ascii_lowercase().ends_with(&extension.to_ascii_lowercase()))
                        .then(|| format!("{target}{}", source.rsplit('/').next().unwrap_or(source)))
                }
                Route::Manifest { .. } => None,
                Route::Patch { target } => {
                    let name = source.rsplit('/').next().unwrap_or(source);
                    super::nexus_patches::parts(name).map(|_| format!("{target}{name}"))
                },
            };
            if let Some(target) = target {
                let target = target.replace("{mod_id}", mod_id);
                if !self.allowed(&target) { return Err(format!("Destino fora das regras do módulo: {target}")); }
                return Ok(Some(target));
            }
        }
        Ok(None)
    }
    pub(super) fn route_package(&self, source: &str, mod_id: &str, entries: &[String]) -> Result<Option<String>, String> {
        if self.domain == "crimsondesert" {
            let lower = source.to_ascii_lowercase();
            let extension = Path::new(&lower).extension().and_then(|e| e.to_str()).unwrap_or("");
            if ["json", "modpatch", "cdmod", "paz", "pamt", "papgt", "pathc", "pabgb", "xml", "dds", "bnk", "bat", "cmd", "ps1", "py", "bsdiff", "xdelta", "pak", "lua", "addon64", "exe"].contains(&extension) {
                return Err(format!("Este pacote de Crimson Desert exige processamento especializado (CDUMM/DMM ou instalador do autor): {source}. O módulo atual instala apenas plugins ASI e seus INIs; nenhum arquivo foi aplicado."));
            }
            if extension == "ini" && !entries.iter().any(|entry| entry.to_ascii_lowercase() == format!("{}.asi", lower.trim_end_matches(".ini"))) {
                let loader_config = lower.rsplit('/').next() == Some("global.ini") && entries.iter().any(|entry| matches!(entry.to_ascii_lowercase().rsplit('/').next(), Some("winmm.dll" | "version.dll")));
                if !loader_config { return Err(format!("INI sem plugin ASI correspondente: {source}. Presets ReShade e configurações avulsas não são instalados por este módulo.")); }
            }
            // Bare wrapper folders may be flattened only when they contain a
            // single plugin directory, never several variants at once.
            if matches!(extension, "asi" | "ini") && !lower.split('/').any(|part| part == "bin64") {
                let parents: std::collections::BTreeSet<_> = entries.iter().filter(|entry| entry.to_ascii_lowercase().ends_with(".asi"))
                    .map(|entry| entry.rsplit_once('/').map(|(parent, _)| parent.to_ascii_lowercase()).unwrap_or_default()).collect();
                if parents.len() > 1 { return Err("O pacote ASI contém várias pastas/variantes. Importe somente a variante desejada; nenhuma foi aplicada.".into()); }
            }
        }
        if self.domain == "masseffectlegendaryedition" && entries.iter().any(|entry| entry.rsplit('/').next().is_some_and(|name| name.eq_ignore_ascii_case("moddesc.ini") || name.to_ascii_lowercase().ends_with(".mem"))) {return Err("Este pacote requer processamento pelo ME3Tweaks/Mass Effect Modder; use o gerenciador especializado.".into());}
        if let Some(target) = self.route(source, mod_id)? {
            if self.domain == "masseffectlegendaryedition" && target.split("/BioGame/DLC/").nth(1).is_none_or(|rest| !rest.starts_with("DLC_MOD_")) {return Err("Somente DLC novo DLC_MOD_ com destino explícito é instalado; dados originais ficam preservados.".into());}
            return Ok(Some(target));
        }
        for rule in &self.routes {
            let Route::Manifest { marker, target } = rule else { continue; };
            let mut roots: Vec<_> = entries.iter().filter(|entry| entry.eq_ignore_ascii_case(marker) || entry.to_ascii_lowercase().ends_with(&format!("/{}", marker.to_ascii_lowercase())))
                .map(|entry| entry[..entry.len()-marker.len()].to_string())
                .filter(|root| source.starts_with(root)).collect();
            roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
            if let Some(root) = roots.first() {
                let folder = root.trim_end_matches('/').rsplit('/').next().filter(|name| !name.is_empty()).unwrap_or(mod_id);
                let target = format!("{target}{folder}/{}", &source[root.len()..]).replace("{mod_id}", mod_id);
                if !self.allowed(&target) { return Err("Destino de manifesto inválido.".into()); }
                return Ok(Some(target));
            }
        }
        Ok(None)
    }
}
#[derive(Serialize)]
pub struct ModuleRegistry { pub(super) definitions: Vec<Definition>, pub(super) errors: Vec<String> }
fn parse(bytes: &[u8]) -> Result<Definition, String> {
    let definition: Definition = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    definition.validate()?;
    Ok(definition)
}
fn builtins() -> Vec<Definition> {
    static DEFINITIONS: std::sync::OnceLock<Vec<Definition>> = std::sync::OnceLock::new();
    DEFINITIONS.get_or_init(|| [include_bytes!("../../resources/nexus-games/palworld.json").as_slice(),
     include_bytes!("../../resources/nexus-games/valheim.json").as_slice(),
     include_bytes!("../../resources/nexus-games/finalfantasy12.json").as_slice(),
     include_bytes!("../../resources/nexus-games/crimsondesert.json").as_slice()]
        .iter().map(|bytes| parse(bytes).expect("Invalid bundled Nexus module")).chain({
            let modules: Vec<Definition> = serde_json::from_slice(include_bytes!("../../resources/nexus-games/catalog.json")).expect("Invalid Nexus catalog");
            for module in &modules { module.validate().expect("Invalid bundled Nexus module"); }
            modules
        }).collect()).clone()
}
fn external(directory: &Path, mut definitions: Vec<Definition>) -> ModuleRegistry {
    let mut errors = Vec::new();
    let mut entries = match fs::read_dir(directory) {
        Ok(entries) => entries.filter_map(Result::ok).map(|entry| entry.path()).collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => { errors.push(error.to_string()); Vec::new() }
    };
    entries.sort();
    for path in entries.into_iter().filter(|path| path.extension().is_some_and(|ext| ext == "json")) {
        let result = (|| {
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() > 256 * 1024 { return Err("O módulo deve ser um arquivo JSON de até 256 KB.".into()); }
            let definition = parse(&fs::read(&path).map_err(|e| e.to_string())?)?;
            if definitions.iter().any(|existing| existing.domain == definition.domain && existing.platform == definition.platform) {
                return Err("Já existe um módulo para este jogo e plataforma; substituição recusada.".into());
            }
            definitions.push(definition);
            Ok::<_, String>(())
        })();
        if let Err(error) = result { errors.push(format!("{}: {error}", path.display())); }
    }
    ModuleRegistry { definitions, errors }
}
pub(super) fn verify_framework_identity(domain: &str, mod_id: u64, actual: &str) -> Result<(), String> {
    for definition in registry().definitions.iter().filter(|module| module.domain == domain) {
        definition.verify_framework_identity(mod_id, actual)?;
    }
    Ok(())
}
pub(super) fn registry() -> ModuleRegistry {
    match crate::core::paths::app_root() {
        Ok(root) => external(&root.join("Mod/Nexus/modules"), builtins()),
        Err(error) => ModuleRegistry { definitions: builtins(), errors: vec![error.to_string()] },
    }
}
#[tauri::command]
pub fn list_nexus_modules() -> ModuleRegistry { registry() }

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "00000000-0000-4000-8000-000000000001";
    #[test]
    fn cyberpunk_cet_scripts_are_not_redmod_packages() {
        let game = builtins().into_iter().find(|m| m.domain == "cyberpunk2077").unwrap();
        let base = "bin/x64/plugins/cyber_engine_tweaks/mods/Vehicle Customizer/";
        for wrapper in ["", "Vehicle Customizer release/"] {
            for file in ["init.lua", "modules/helpers.lua", "config/settings.json", "assets/mods/data.json"] {
                let source = format!("{wrapper}{base}{file}");
                assert_eq!(game.route_package(&source, ID, &[source.clone()]).unwrap(), Some(format!("{base}{file}")));
            }
        }
        let upper = "Release/BIN/X64/PLUGINS/CYBER_ENGINE_TWEAKS/mods/Vehicle Customizer/init.lua";
        assert_eq!(game.route_package(upper, ID, &[upper.into()]).unwrap(), Some(format!("{base}init.lua")));
        for source in ["mods/Vehicle Customizer/info.json", "Release/mods/Vehicle Customizer/init.lua",
            "Release/mods/REDmod/bin/x64/plugins/cyber_engine_tweaks/mods/Vehicle Customizer/init.lua",
            "fakebin/x64/plugins/cyber_engine_tweaks/mods/Vehicle Customizer/init.lua",
            "bin/x64/plugins/cyber_engine_tweaks/mods/../../../../outside.lua"] {
            assert!(game.route_package(source, ID, &[source.into()]).is_err(), "{source}");
        }
        assert_eq!(game.route_package("init.lua", ID, &["init.lua".into()]).unwrap(), None);
        assert_eq!(game.route_package("archive/pc/mod/example.archive", ID, &[]).unwrap().as_deref(), Some("archive/pc/mod/example.archive"));
    }
    #[test]
    fn crimson_routes_asi_packages_and_blocks_unprocessed_game_data() {
        let game = builtins().into_iter().find(|m|m.domain == "crimsondesert").unwrap();
        for prefix in ["", "AutoLoot/", "Release/bin64/", "Release/bin64/plugins/"] {
            let entries = vec![format!("{prefix}autoloot.asi"), format!("{prefix}autoloot.ini")];
            let target_prefix = if prefix.ends_with("plugins/") {"bin64/plugins/"} else {"bin64/"};
            for extension in ["asi", "ini"] {
                assert_eq!(game.route_package(&format!("{prefix}autoloot.{extension}"), ID, &entries).unwrap(), Some(format!("{target_prefix}autoloot.{extension}")));
            }
        }
        assert_eq!(game.route_package("winmm-x64/winmm.dll",ID,&["winmm-x64/winmm.dll".into()]).unwrap().as_deref(),Some("bin64/winmm.dll"));
        for source in ["data/stamina.json", "Mods/appearance.cdmod", "0008/1.paz", "meta/0.papgt", "meta/1.pamt", "overlay/0.pathc", "install.bat", "patch.py", "skin.dds", "sound.bnk", "main.lua", "skin.pak", "OG_model.xml"] {
            assert!(game.route_package(source,ID,&[source.into()]).is_err(),"{source}");
        }
        assert!(game.route_package("preset.ini",ID,&["preset.ini".into()]).is_err());
        assert!(game.route_package("bin64/CrimsonDesert.exe",ID,&[]).is_err());
        assert!(game.route("bin64/unknown.dll",ID).is_err());
        assert!(!game.allowed("bin64/CrimsonDesert.exe"));
        assert!(!game.allowed("meta/0.papgt"));
        assert!(game.route("../bin64/plugin.asi",ID).is_err());
        let entries: Vec<String> = vec!["Fast/plugin.asi".into(),"Slow/plugin.asi".into()];
        assert!(game.route_package(&entries[0],ID,&entries).is_err());
        assert_eq!(game.steam_id.as_deref(),Some("3321460"));
        assert!(game.dll_overrides.iter().any(|o|o.name == "winmm" && o.file == "bin64/winmm.dll"));
    }
    #[test]
    fn crimson_loader_accepts_alternatives_but_not_empty_requirements() {
        let mut game = builtins().into_iter().find(|m|m.domain == "crimsondesert").unwrap();
        let root = std::env::temp_dir().join(format!("crimson-loader-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("bin64")).unwrap();
        assert!(!game.frameworks[0].installed(&root));
        for file in ["winmm.dll","version.dll"] {
            let path = root.join("bin64").join(file); fs::write(&path,b"fixture").unwrap();
            assert!(game.frameworks[0].installed(&root)); fs::remove_file(path).unwrap();
        }
        game.frameworks[0].any_files.clear();
        assert!(game.validate().is_err());
        fs::remove_dir_all(root).unwrap();
    }
    // Independently checked against the authors' Nexus pages. These IDs must not
    // silently change to another mod when a game module is edited.
    #[test]
    fn bundled_frameworks_have_verified_nexus_identities() {
        let expected = [
            ("finalfantasy12", "FF12 External File Loader", 170, "FF12 External File Loader"),
            ("finalfantasy12", "FF12 LUA Loader", 171, "FF12 LUA Loader"),
            ("cyberpunk2077", "Cyber Engine Tweaks", 107, "Cyber Engine Tweaks"),
            ("cyberpunk2077", "RED4ext", 2380, "RED4ext"),
            ("cyberpunk2077", "Redscript", 1511, "redscript"),
            ("cyberpunk2077", "ArchiveXL", 4198, "ArchiveXL"),
            ("cyberpunk2077", "TweakXL", 4197, "TweakXL"),
            ("stardewvalley", "SMAPI", 2400, "SMAPI - Stardew Modding API"),
            ("monsterhunterworld", "Stracker's Loader", 1982, "Stracker's Loader"),
            ("oblivionremastered", "UE4SS para Oblivion Remastered", 32, "UE4SS for OblivionRemastered"),
        ];
        let modules = builtins();
        assert_eq!(modules.iter().flat_map(|m| &m.frameworks).filter(|f| f.mod_id > 0).count(), expected.len());
        for (domain, name, id, title) in expected {
            let module = modules.iter().find(|m| m.domain == domain).unwrap();
            let framework = module.frameworks.iter().find(|f| f.name == name).unwrap();
            assert_eq!(framework.mod_id, id, "{domain}: {name}");
            assert_eq!(framework.nexus_name.as_deref(), Some(title));
            assert!(module.verify_framework_identity(id, title).is_ok());
            assert!(module.verify_framework_identity(id, "Unrelated mod").is_err());
        }
    }
    #[test]
    fn dependency_identity_blocks_wrong_mod_and_conflicting_link() {
        let mut module = builtins().into_iter().find(|m| m.domain == "cyberpunk2077").unwrap();
        assert!(module.verify_framework_identity(4197, "Classy V").is_err());
        assert!(module.verify_framework_identity(4197, "tweak xl").is_ok());
        // An ordinary catalog mod is not mistaken for a framework.
        assert!(module.verify_framework_identity(34907, "Midas' MaxTac Mantis Blades").is_ok());
        let framework = module.frameworks.iter_mut().find(|f| f.name == "TweakXL").unwrap();
        framework.url = Some("https://www.nexusmods.com/cyberpunk2077/mods/4847".into());
        assert!(module.validate().is_err());
    }
    #[test]
    fn repo_routes_mono_loader_plugins_and_content_without_replacing_game_files() {
        let game = builtins().into_iter().find(|game| game.domain == "repo").unwrap();
        for (source, target) in [
            ("BepInExPack/BepInEx/core/BepInEx.Preloader.dll", "BepInEx/core/BepInEx.Preloader.dll"),
            ("Archive/BepInExPack/winhttp.dll", "winhttp.dll"),
            ("BepInExPack/doorstop_config.ini", "doorstop_config.ini"),
            ("plugins/REPOLib.dll", "BepInEx/plugins/REPOLib.dll"),
            ("REPOLib.dll", "BepInEx/plugins/REPOLib.dll"),
            ("MenuLib/plugins/MenuLib/assets/menu.bundle", "BepInEx/plugins/MenuLib/assets/menu.bundle"),
            ("BepInEx/patchers/Example.dll", "BepInEx/patchers/Example.dll"),
            ("config/headclef.Improve.cfg", "BepInEx/config/headclef.Improve.cfg"),
        ] {
            assert_eq!(game.route(source, ID).unwrap().as_deref(), Some(target));
        }
        assert_eq!(game.route("RepoConfig.dll", ID).unwrap(), Some(format!("BepInEx/plugins/{ID}/RepoConfig.dll")));
        assert_eq!(game.route("items.repobundle", ID).unwrap(), Some(format!("BepInEx/plugins/{ID}/items.repobundle")));
        let entries: Vec<String> = vec!["Custom/manifest.json".into(), "Custom/assets/items.repobundle".into()];
        assert_eq!(game.route_package(&entries[1], ID, &entries).unwrap().as_deref(), Some("BepInEx/plugins/Custom/assets/items.repobundle"));
        assert!(game.route("REPO.exe", ID).unwrap().is_none());
        assert!(!game.allowed("REPO.exe"));
        assert!(game.route("REPO_Data/Managed/Assembly-CSharp.dll", ID).is_err());
        assert!(game.route("Mods/MelonMod.dll", ID).is_err());
        assert!(game.route("../plugins/escape.dll", ID).is_err());
        assert_eq!(game.dll_overrides[0].name, "winhttp");
        assert_eq!(game.steam_id.as_deref(), Some("3241660"));
    }
    #[test]
    fn bundled_rules_route_loader_paks_and_native_plugins() {
        let definitions = builtins();
        let pal = &definitions[0];
        assert_eq!(pal.route("skin.pak", ID).unwrap().as_deref(), Some("Pal/Content/Paks/~mods/skin.pak"));
        assert_eq!(pal.route("LogicMods/menu.pak", ID).unwrap().as_deref(), Some("Pal/Content/Paks/LogicMods/menu.pak"));
        assert_eq!(pal.route("ue4ss/Mods/Menu/Scripts/main.lua", ID).unwrap().as_deref(), Some("Pal/Binaries/Win64/ue4ss/Mods/Menu/Scripts/main.lua"));
        assert!(pal.route("Palworld.exe", ID).unwrap().is_none());
        assert!(pal.route("../skin.pak", ID).is_err());
        assert!(pal.route("C:/skin.pak", ID).is_err());
        assert!(!pal.allowed("Pal/Content/Paks/~mods/../Palworld.exe"));
        let val = &definitions[1];
        assert_eq!(val.route("plugin.dll", ID).unwrap(), Some(format!("BepInEx/plugins/{ID}/plugin.dll")));
        assert_eq!(val.route("BepInExPack_Valheim/doorstop_libs/libdoorstop_x64.so", ID).unwrap().as_deref(), Some("doorstop_libs/libdoorstop_x64.so"));
    }
    #[test]
    fn ff12_routes_data_scripts_loaders_and_rejects_game_archive() {
        let definitions = builtins(); let game = &definitions[2];
        assert_eq!(game.route("Mod Version/ff12data/gamedata/battle.bin", ID).unwrap().as_deref(),Some("mods/deploy/ff12data/gamedata/battle.bin"));
        assert_eq!(game.route("archive/x64/Scripts/init/sample.lua", ID).unwrap().as_deref(),Some("x64/scripts/init/sample.lua"));
        assert_eq!(game.route("loader-multi/x64/modules/ff12-file-loader.dll", ID).unwrap().as_deref(),Some("x64/modules/ff12-file-loader.dll"));
        assert_eq!(game.route("dinput/dinput8.dll", ID).unwrap().as_deref(),Some("x64/dinput8.dll"));
        assert!(game.route("FFXII_TZA.vbf", ID).unwrap().is_none());
        assert!(game.route("unknown.bin", ID).unwrap().is_none());
        assert!(game.route("scripts/../x64/FFXII_TZA.exe", ID).is_err());
    }
    #[test]
    fn rejects_invalid_schema_paths_and_executable_destinations() {
        let mut definition = builtins().remove(0);
        definition.schema = 2; assert!(definition.validate().is_err());
        definition.schema = 1; definition.destinations.push("../escape".into()); assert!(definition.validate().is_err());
        definition.destinations.pop(); definition.allowed_files.push("Palworld.exe".into()); assert!(definition.validate().is_err());
    }
    #[test]
    fn external_modules_do_not_replace_bundled_support() {
        let folder = std::env::temp_dir().join(format!("nexus-modules-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("duplicate.json"), include_bytes!("../../resources/nexus-games/palworld.json")).unwrap();
        fs::write(folder.join("invalid.json"), b"{}").unwrap();
        let registry = external(&folder, builtins());
        assert_eq!(registry.definitions.len(), builtins().len()); assert_eq!(registry.errors.len(), 2);
        fs::remove_dir_all(folder).unwrap();
    }
}
