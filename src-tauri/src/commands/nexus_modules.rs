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
    /// Strip only directories shared by every archive file, after explicit rules fail.
    #[serde(default)]
    pub unwrap_common_root: bool,
    #[serde(default)]
    pub bundle_plugin_resources: bool,
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
    #[serde(default)]
    pub rejected_extensions: Vec<String>,
    /// Shared engine profile expanded into routes when the module is loaded.
    #[serde(default)]
    pub family: Option<Family>,
    /// Vortex mod type id → module destination, used for Nexus Collections.
    #[serde(default)]
    pub vortex_mod_types: std::collections::BTreeMap<String, String>,
    /// Bethesda script extender (SKSE64, F4SE, xNVSE...): installed at the game
    /// root, used to launch the game and required by its plugin folder.
    #[serde(default)]
    pub script_extender: Option<ScriptExtender>,
    /// INI values applied in the prefix while mods are installed (archive
    /// invalidation for loose files, as the Vortex Bethesda extensions do).
    #[serde(default)]
    pub ini_settings: Vec<IniSetting>,
    /// Store editions using other `AppData/Local` and `My Games` folders (GOG,
    /// Epic), recognized by a file only that store installs (libloadorder rules).
    #[serde(default)]
    pub store_variants: Vec<StoreVariant>,
    /// Vortex support confirmed for this game. Automatic library discovery only
    /// adds such games; others can still be added by hand.
    #[serde(default = "default_true")]
    pub vortex_supported: bool,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoreVariant {
    /// File in the game folder, e.g. `Galaxy64.dll` (GOG).
    pub marker: String,
    /// Folder name used by the Steam edition, e.g. `Skyrim Special Edition`.
    pub folder: String,
    /// Folder name of this store edition, e.g. `Skyrim Special Edition GOG`.
    pub replacement: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IniSetting {
    /// `@documents/...` INI file; managed as configuration once written.
    pub file: String,
    pub section: String,
    pub key: String,
    pub value: String,
    /// Create the file when absent (`*Custom.ini`); game-written INIs must exist.
    #[serde(default)]
    pub create: bool,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScriptExtender {
    pub name: String,
    /// Loader executable at the game root, e.g. `skse64_loader.exe`.
    pub loader: String,
    /// Prefix of the versioned runtime DLLs at the game root, e.g. `skse64_`.
    pub dll_prefix: String,
    /// Plugin folder, e.g. `Data/SKSE/Plugins`.
    pub plugins: String,
    /// Nexus page of the extender (identity verified by name).
    pub mod_id: u64,
    pub nexus_name: String,
    /// Nexus game hosting the extender when it is not the module's own (Enderal
    /// uses the Skyrim extenders). Such a page is linked, never identity-checked.
    #[serde(default)]
    pub domain: Option<String>,
    /// Log files or folders (`@documents/...` inside the prefix).
    #[serde(default)]
    pub logs: Vec<String>,
}
impl ScriptExtender {
    /// Loader or runtime DLL, by file name.
    fn owns(&self, file: &str) -> bool {
        let file = file.to_ascii_lowercase();
        file == self.loader.to_ascii_lowercase() || (file.starts_with(&self.dll_prefix.to_ascii_lowercase()) && file.ends_with(".dll"))
    }
}
/// Engine families mirror the Vortex game templates: one profile, many games.
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Family {
    Unreal {
        /// Project folder below the game root (`Pal`, `SB`, `MarvelGame/Marvel`).
        project: String,
        #[serde(default = "default_unreal_binaries")]
        binaries: String,
        /// UE4SS scripts and LogicMods; disabled for games where the loader is unsafe.
        #[serde(default = "default_true")]
        ue4ss: bool,
    },
    /// RE Engine with REFramework (scripts, plugins, data and its dinput8 loader).
    /// `natives/` and Fluffy packages need specialized processing and are refused.
    ReEngine {},
    Unity {
        /// `<Game>_Data`; replacing game data is refused, as in the existing modules.
        data: String,
        /// Loaders offered by the Vortex extension for this game.
        #[serde(default = "default_unity_loaders")]
        loaders: Vec<UnityLoader>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum UnityLoader { Bepinex, Melonloader }
impl UnityLoader {
    pub fn label(self) -> &'static str { match self { UnityLoader::Bepinex => "BepInEx", UnityLoader::Melonloader => "MelonLoader" } }
}
fn default_unity_loaders() -> Vec<UnityLoader> { vec![UnityLoader::Bepinex, UnityLoader::Melonloader] }
pub(super) struct UnityLayout { pub loaders: Vec<UnityLoader> }
fn default_unreal_binaries() -> String { "Win64".into() }
fn default_true() -> bool { true }
/// Resolved UE4SS locations of an Unreal module.
pub(super) struct Ue4ssLayout { pub binaries: String, pub logic_mods: String, pub paks: String }
impl Ue4ssLayout {
    /// Nested (`ue4ss/Mods`, current releases) and flat (`Mods`, older releases) script folders.
    pub fn mod_folders(&self) -> [String; 2] { [format!("{}/ue4ss/Mods", self.binaries), format!("{}/Mods", self.binaries)] }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GameTool { pub name: String, pub executable: String, #[serde(default)] pub arguments: Vec<String> }
#[derive(Clone, PartialEq, Deserialize, Serialize)]
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
#[derive(Clone, PartialEq, Deserialize, Serialize)]
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
#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Route {
    Prefix { source: String, target: String, #[serde(default)] search: bool },
    File { source: String, target: String, #[serde(default)] search: bool },
    Extension { extension: String, target: String, #[serde(default)] root_only: bool },
    Manifest { marker: String, target: String },
    Patch { target: String },
}
fn push_new<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) { items.push(item); }
}
pub(super) fn relative(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':'))
}
impl Definition {
    /// Paths of the store edition installed at `root` (first matching marker).
    pub(super) fn for_store(mut self, root: &Path) -> Self {
        let Some(variant) = self.store_variants.iter().find(|v| super::nexus_discovery::locate(root, &v.marker).is_some_and(|p| p.is_file())).cloned() else { return self; };
        let rewrite = |path: &mut String| {
            for base in ["@local/", "@documents/My Games/"] {
                if let Some(rest) = path.strip_prefix(&format!("{base}{}/", variant.folder)) {
                    *path = format!("{base}{}/{rest}", variant.replacement);
                }
            }
        };
        if let Some(activation) = &mut self.activation { rewrite(&mut activation.path); }
        self.config_files.iter_mut().chain(self.allowed_files.iter_mut()).for_each(rewrite);
        self.ini_settings.iter_mut().for_each(|setting| rewrite(&mut setting.file));
        if let Some(extender) = &mut self.script_extender { extender.logs.iter_mut().for_each(rewrite); }
        self
    }
    pub(super) fn ue4ss(&self) -> Option<Ue4ssLayout> {
        match &self.family {
            Some(Family::Unreal { project, binaries, ue4ss: true }) => Some(Ue4ssLayout {
                binaries: format!("{project}/Binaries/{binaries}"),
                logic_mods: format!("{project}/Content/Paks/LogicMods"),
                paks: format!("{project}/Content/Paks/~mods"),
            }),
            _ => None,
        }
    }

    /// Destination for a Vortex mod type, only when it lies inside the module's destinations.
    pub(super) fn vortex_destination(&self, mod_type: &str) -> Option<String> {
        self.vortex_mod_types.get(mod_type).filter(|d| self.allowed(&format!("{d}/module-check.bin"))).cloned()
    }

    pub(super) fn unity(&self) -> Option<UnityLayout> {
        match &self.family {
            Some(Family::Unity { loaders, .. }) => Some(UnityLayout { loaders: loaders.clone() }),
            _ => None,
        }
    }

    /// Expands the family profile. Explicit family paths come first, then the
    /// module's own rules, then the family's extension fallbacks, following the
    /// installer priorities of the Vortex Unreal template (LogicMods and UE4SS
    /// before loose paks). Unknown loose files still have no route.
    fn apply_family(&mut self) -> Result<(), String> {
        self.apply_script_extender()?;
        for file in self.ini_settings.iter().map(|setting| setting.file.clone()).collect::<Vec<_>>() {
            push_new(&mut self.allowed_files, file.clone());
            push_new(&mut self.config_files, file);
        }
        if let Some(Family::Unity { data, loaders }) = self.family.clone() {
            return self.apply_unity(&data, &loaders);
        }
        if let Some(Family::ReEngine {}) = self.family {
            self.apply_re_engine();
            return Ok(());
        }
        let Some(Family::Unreal { project, binaries, ue4ss }) = self.family.clone() else { return Ok(()); };
        if !relative(&project) || !relative(&binaries) || binaries.contains('/') {
            return Err("Família Unreal com pastas inválidas.".into());
        }
        let paks = format!("{project}/Content/Paks/~mods");
        let logic = format!("{project}/Content/Paks/LogicMods");
        let bin = format!("{project}/Binaries/{binaries}");
        let prefix = |source: &str, target: &str, search: bool| Route::Prefix { source: format!("{source}/"), target: format!("{target}/"), search };
        let mut explicit = vec![prefix(&paks, &paks, true)];
        let mut destinations = vec![paks.clone()];
        if ue4ss {
            explicit.extend([
                prefix(&logic, &logic, true),
                prefix(&format!("{bin}/ue4ss"), &format!("{bin}/ue4ss"), true),
                Route::File { source: format!("{bin}/dwmapi.dll"), target: format!("{bin}/dwmapi.dll"), search: true },
                prefix(&format!("{bin}/Mods"), &format!("{bin}/ue4ss/Mods"), true),
                prefix("LogicMods", &logic, true),
                prefix("ue4ss", &format!("{bin}/ue4ss"), true),
                Route::File { source: "dwmapi.dll".into(), target: format!("{bin}/dwmapi.dll"), search: true },
            ]);
            destinations.extend([logic.clone(), format!("{bin}/ue4ss"), format!("{bin}/Mods")]);
            push_new(&mut self.allowed_files, format!("{bin}/dwmapi.dll"));
            push_new(&mut self.dll_overrides, DllOverride { name: "dwmapi".into(), file: format!("{bin}/dwmapi.dll") });
            push_new(&mut self.frameworks, Framework {
                name: "UE4SS".into(), mod_id: 0, nexus_name: None,
                url: Some("https://github.com/UE4SS-RE/RE-UE4SS/releases".into()),
                required_files: vec![format!("{bin}/dwmapi.dll")],
                any_files: vec![format!("{bin}/ue4ss/UE4SS.dll"), format!("{bin}/UE4SS.dll")],
                for_paths: vec![format!("{bin}/ue4ss/Mods/"), format!("{bin}/Mods/"), format!("{logic}/")],
                for_extensions: Vec::new(),
            });
        } else {
            for rejected in ["ue4ss/", "LogicMods/"] { push_new(&mut self.rejected_prefixes, rejected.into()); }
        }
        explicit.push(prefix("~mods", &paks, true));
        let mut generic: Vec<Route> = [".pak", ".utoc", ".ucas"].into_iter()
            .map(|extension| Route::Extension { extension: extension.into(), target: format!("{paks}/"), root_only: false }).collect();
        if ue4ss {
            // A script folder is recognized by its entry point, as in the Vortex template.
            generic.push(Route::Manifest { marker: "Scripts/main.lua".into(), target: format!("{bin}/ue4ss/Mods/") });
            generic.push(Route::Manifest { marker: "dlls/main.dll".into(), target: format!("{bin}/ue4ss/Mods/") });
        }
        let own = std::mem::take(&mut self.routes);
        for route in explicit.into_iter().chain(own).chain(generic) { push_new(&mut self.routes, route); }
        for destination in destinations { push_new(&mut self.destinations, destination); }
        // Xbox/GDK builds use another binaries layout that this profile does not target.
        push_new(&mut self.rejected_prefixes, format!("{project}/Binaries/WinGDK/"));
        Ok(())
    }

    /// Launch through the extender when it is installed and require it for its plugins.
    fn apply_script_extender(&mut self) -> Result<(), String> {
        let Some(extender) = self.script_extender.clone() else { return Ok(()); };
        let loader_ok = relative(&extender.loader) && !extender.loader.contains('/') && extender.loader.to_ascii_lowercase().ends_with(".exe");
        let prefix_ok = extender.dll_prefix.len() >= 3 && extender.dll_prefix.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if !loader_ok || !prefix_ok || !relative(&extender.plugins) || extender.mod_id == 0 || extender.nexus_name.trim().is_empty()
            || extender.logs.iter().any(|log| !relative(log.trim_start_matches('@'))) {
            return Err("Script extender inválido no módulo.".into());
        }
        let domain = extender.domain.clone().unwrap_or_else(|| self.domain.clone());
        if domain.is_empty() || !domain.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
            return Err("Script extender inválido no módulo.".into());
        }
        if self.mod_launch_executable.is_none() { self.mod_launch_executable = Some(extender.loader.clone()); }
        // Framework ids belong to the module's domain; another game's page is only a link.
        let own = domain == self.domain;
        push_new(&mut self.frameworks, Framework {
            name: extender.name.clone(), mod_id: if own { extender.mod_id } else { 0 }, nexus_name: own.then(|| extender.nexus_name.clone()),
            url: Some(format!("https://www.nexusmods.com/{domain}/mods/{}", extender.mod_id)),
            required_files: vec![extender.loader.clone()], any_files: Vec::new(),
            for_paths: vec![format!("{}/", extender.plugins)], for_extensions: Vec::new(),
        });
        Ok(())
    }

    /// REFramework layout shared by the RE Engine games (Vortex "Fluffy +
    /// REFramework" template, REFramework part only).
    fn apply_re_engine(&mut self) {
        let prefix = |folder: &str| Route::Prefix { source: format!("reframework/{folder}/"), target: format!("reframework/{folder}/"), search: true };
        let explicit = [prefix("autorun"), prefix("plugins"), prefix("data"), Route::File { source: "dinput8.dll".into(), target: "dinput8.dll".into(), search: false }];
        let generic = [Route::Extension { extension: ".lua".into(), target: "reframework/autorun/".into(), root_only: true }];
        let own = std::mem::take(&mut self.routes);
        for route in explicit.into_iter().chain(own).chain(generic) { push_new(&mut self.routes, route); }
        for folder in ["autorun", "plugins", "data"] { push_new(&mut self.destinations, format!("reframework/{folder}")); }
        push_new(&mut self.allowed_files, "dinput8.dll".into());
        for rejected in ["natives/", "pak_mods/"] { push_new(&mut self.rejected_prefixes, rejected.into()); }
        push_new(&mut self.dll_overrides, DllOverride { name: "dinput8".into(), file: "dinput8.dll".into() });
        if !self.frameworks.iter().any(|f| f.name.starts_with("REFramework")) {
            push_new(&mut self.frameworks, Framework {
                name: "REFramework".into(), mod_id: 0, nexus_name: None, url: Some("https://github.com/praydog/REFramework/releases".into()),
                required_files: vec!["dinput8.dll".into()], any_files: Vec::new(), for_paths: vec!["reframework/".into()], for_extensions: Vec::new(),
            });
        }
    }

    /// Routes of the Vortex Unity template (BepInEx/MelonLoader hybrid). Loose
    /// DLLs get a canonical BepInEx route here; `nexus_unity::retarget` moves
    /// them after reading which loader each DLL references.
    fn apply_unity(&mut self, data: &str, loaders: &[UnityLoader]) -> Result<(), String> {
        if !relative(data) || data.contains('/') || !data.ends_with("_Data") || loaders.is_empty() {
            return Err("Família Unity com pasta de dados ou carregadores inválidos.".into());
        }
        let prefix = |source: &str, target: &str| Route::Prefix { source: format!("{source}/"), target: format!("{target}/"), search: true };
        let file = |name: &str| Route::File { source: name.into(), target: name.into(), search: true };
        let bepinex = loaders.contains(&UnityLoader::Bepinex);
        let melon = loaders.contains(&UnityLoader::Melonloader);
        let mut explicit = Vec::new();
        if bepinex {
            explicit.extend([prefix("BepInEx", "BepInEx"), file("winhttp.dll"), file("doorstop_config.ini"), file(".doorstop_version"),
                prefix("plugins", "BepInEx/plugins"), prefix("patchers", "BepInEx/patchers"), prefix("config", "BepInEx/config")]);
            push_new(&mut self.destinations, "BepInEx".into());
            for name in ["winhttp.dll", "doorstop_config.ini", ".doorstop_version"] { push_new(&mut self.allowed_files, name.into()); }
            push_new(&mut self.dll_overrides, DllOverride { name: "winhttp".into(), file: "winhttp.dll".into() });
            // A module may declare its own, more specific BepInEx requirement (e.g. a game pack).
            if !self.frameworks.iter().any(|f| f.name.starts_with("BepInEx")) { push_new(&mut self.frameworks, Framework {
                name: "BepInEx".into(), mod_id: 0, nexus_name: None, url: Some("https://github.com/BepInEx/BepInEx/releases".into()),
                required_files: vec!["winhttp.dll".into()],
                any_files: ["BepInEx.dll", "BepInEx.Core.dll", "BepInEx.Unity.Mono.dll", "BepInEx.Unity.IL2CPP.dll"].iter().map(|f| format!("BepInEx/core/{f}")).collect(),
                for_paths: vec!["BepInEx/plugins/".into(), "BepInEx/patchers/".into()], for_extensions: Vec::new(),
            }); }
        }
        if melon {
            explicit.extend([prefix("MelonLoader", "MelonLoader"), file("version.dll"), file("dobby.dll"),
                prefix("Mods", "Mods"), prefix("UserLibs", "UserLibs"), prefix("UserData", "UserData"), prefix("Plugins", "Plugins")]);
            for folder in ["MelonLoader", "Mods", "Plugins", "UserLibs", "UserData"] { push_new(&mut self.destinations, folder.into()); }
            for name in ["version.dll", "dobby.dll"] { push_new(&mut self.allowed_files, name.into()); }
            push_new(&mut self.dll_overrides, DllOverride { name: "version".into(), file: "version.dll".into() });
            push_new(&mut self.frameworks, Framework {
                name: "MelonLoader".into(), mod_id: 0, nexus_name: None, url: Some("https://github.com/LavaGang/MelonLoader/releases".into()),
                required_files: vec!["version.dll".into()],
                any_files: ["net6/MelonLoader.dll", "net35/MelonLoader.dll", "MelonLoader.dll"].iter().map(|f| format!("MelonLoader/{f}")).collect(),
                for_paths: vec!["Mods/".into(), "Plugins/".into()], for_extensions: Vec::new(),
            });
        }
        let loose = if bepinex { "BepInEx/plugins/{mod_id}/" } else { "Mods/" };
        // Only a DLL at the package root (after removing a shared wrapper) is loose;
        // sibling variant folders stay ambiguous and are refused.
        let generic = [Route::Extension { extension: ".dll".into(), target: loose.into(), root_only: true }];
        let own = std::mem::take(&mut self.routes);
        for route in explicit.into_iter().chain(own).chain(generic) { push_new(&mut self.routes, route); }
        // Game data, managed assemblies and asset replacements modify original files.
        push_new(&mut self.rejected_prefixes, format!("{data}/"));
        // A loader this game does not use is refused before any wrapper is removed.
        if !bepinex { push_new(&mut self.rejected_prefixes, "BepInEx/".into()); }
        if !melon { push_new(&mut self.rejected_prefixes, "MelonLoader/".into()); }
        for extension in [".assets", ".resource", ".ress"] { push_new(&mut self.rejected_extensions, extension.into()); }
        self.unwrap_common_root = true;
        self.bundle_plugin_resources = bepinex;
        Ok(())
    }

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
        for variant in &self.store_variants {
            let folder = |text: &str| relative(text) && !text.contains('/');
            if !relative(&variant.marker) || !folder(&variant.folder) || !folder(&variant.replacement) || variant.folder == variant.replacement {
                return Err("Variante de loja inválida no módulo.".into());
            }
        }
        let name = |text: &str| !text.trim().is_empty() && text == text.trim() && !text.contains(['=', '[', ']', ';', '#']) && !text.chars().any(char::is_control);
        for setting in &self.ini_settings {
            if !relative(&setting.file) || !setting.file.to_ascii_lowercase().ends_with(".ini") || !name(&setting.section) || !name(&setting.key)
                || setting.value.chars().any(char::is_control) || setting.value.len() > 256 {
                return Err("Ajuste de INI inválido no módulo.".into());
            }
            if self.ini_settings.iter().any(|other| other.file == setting.file && other.create != setting.create) {
                return Err("Ajuste de INI inválido no módulo.".into());
            }
        }
        if self.vortex_mod_types.iter().any(|(kind, path)| kind.is_empty() || kind.len() > 100 || !relative(path)) { return Err("Tipo de mod Vortex inválido.".into()); }
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
        if self.rejected_extensions.iter().any(|extension| !extension.starts_with('.') || extension.len() < 2 || !extension[1..].bytes().all(|b| b.is_ascii_alphanumeric())) { return Err("Filtro de extensão inválido.".into()); }
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
        if !path.contains('/') && self.script_extender.as_ref().is_some_and(|extender| extender.owns(path)) && relative(path) {
            return true;
        }
        relative(path) && !self.protected_files.iter().any(|file|file.eq_ignore_ascii_case(path)) && (self.allowed_files.iter().any(|file| file == path)
            || !path.contains('/') && self.root_extensions.iter().any(|ext| path.to_ascii_lowercase().ends_with(ext))
            || self.destinations.iter().any(|folder| path.strip_prefix(folder).is_some_and(|rest| rest.starts_with('/'))))
    }
    /// Destination for a file of a manually rooted package. The module's
    /// rejections still apply and the target must be an allowed destination.
    pub(super) fn manual_target(&self, source: &str, destination: &str, mod_id: &str) -> Result<String, String> {
        if !self.destinations.iter().any(|d| d == destination) {
            return Err(format!("Destino fora das regras do módulo: {destination}"));
        }
        self.route(source, mod_id)?;
        let target = format!("{destination}/{source}");
        if !self.allowed(&target) { return Err(format!("Destino fora das regras do módulo: {target}")); }
        Ok(target)
    }
    pub(super) fn route(&self, source: &str, mod_id: &str) -> Result<Option<String>, String> {
        if !relative(source) { return Err(format!("Caminho inseguro no pacote: {source}")); }
        uuid::Uuid::parse_str(mod_id).map_err(|_| "Identificador de mod inválido.")?;
        let lower = source.to_ascii_lowercase();
        if self.rejected_extensions.iter().any(|extension| lower.ends_with(&extension.to_ascii_lowercase())) {
            return Err(format!("Formato incompatível com o módulo de {}: {source}", self.name));
        }
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
        // The extender's loader and runtime DLLs sit at the archive root or inside
        // its single versioned folder (`skse64_2_02_06/`), and go to the game root.
        if let Some(extender) = &self.script_extender {
            let name = source.rsplit('/').next().unwrap_or(source);
            if source.matches('/').count() <= 1 && extender.owns(name) {
                return Ok(Some(name.to_string()));
            }
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
        if self.unwrap_common_root {
            for prefix in common_package_roots(entries)? {
                if let Some(relative) = source.strip_prefix(&prefix) {
                    if let Some(target) = self.route(relative, mod_id)? { return Ok(Some(target)); }
                }
            }
        }
        if self.bundle_plugin_resources {
            let mut roots = vec![String::new()];
            roots.extend(common_package_roots(entries)?);
            for root in roots {
                let plugins: Vec<_> = entries.iter().filter_map(|entry| entry.strip_prefix(&root))
                    .filter(|entry| !entry.contains('/') && entry.to_ascii_lowercase().ends_with(".dll")).collect();
                if plugins.len() != 1 { continue; }
                let Some(plugin_target) = self.route(plugins[0], mod_id)? else { continue; };
                // Only an explicit per-mod plugin directory may accept accompanying resources.
                let Some((parent, _)) = plugin_target.rsplit_once('/') else { continue; };
                if !parent.split('/').any(|part| part == mod_id) { continue; }
                let Some(tail) = source.strip_prefix(&root) else { continue; };
                let extension = Path::new(tail).extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                if !["json", "cfg", "ini", "xml", "txt", "png", "jpg", "dds", "assetbundle", "repobundle", "bundle", "bytes", "dat"].contains(&extension.as_str()) { continue; }
                let target = format!("{parent}/{tail}");
                if self.allowed(&target) { return Ok(Some(target)); }
            }
        }
        Ok(None)
    }
}

// A wrapper is removed as a package-wide decision, never separately per file.
// Sibling variant folders therefore remain distinct and cannot be flattened together.
fn common_package_roots(entries: &[String]) -> Result<Vec<String>, String> {
    let files: Vec<_> = entries.iter().filter(|entry| !entry.ends_with('/')).collect();
    if files.iter().any(|entry| !relative(entry)) { return Err("Caminho inseguro no pacote.".into()); }
    let Some(first) = files.first() else { return Ok(Vec::new()); };
    let mut roots = Vec::new();
    for (index, _) in first.match_indices('/').take(32) {
        let prefix = &first[..index + 1];
        if !files.iter().all(|entry| entry.starts_with(prefix)) { break; }
        roots.push(prefix.to_string());
    }
    Ok(roots)
}
#[derive(Serialize)]
pub struct ModuleRegistry { pub(super) definitions: Vec<Definition>, pub(super) errors: Vec<String> }
pub(super) fn parse(bytes: &[u8]) -> Result<Definition, String> {
    let mut definition: Definition = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    definition.apply_family()?;
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
            let mut modules: Vec<Definition> = serde_json::from_slice(include_bytes!("../../resources/nexus-games/catalog.json")).expect("Invalid Nexus catalog");
            // Games generated from the Vortex templates by tools/vortex-import.
            for bytes in [include_bytes!("../../resources/nexus-games/unreal-vortex.json").as_slice(), include_bytes!("../../resources/nexus-games/unity-vortex.json").as_slice(), include_bytes!("../../resources/nexus-games/re_engine-vortex.json").as_slice()] {
                let generated: Vec<Definition> = serde_json::from_slice(bytes).expect("Invalid generated Nexus catalog");
                modules.extend(generated);
            }
            for module in &mut modules {
                module.apply_family().expect("Invalid bundled Nexus family");
                module.validate().expect("Invalid bundled Nexus module");
            }
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
pub async fn list_nexus_modules() -> Result<ModuleRegistry, String> {
    tauri::async_runtime::spawn_blocking(registry).await.map_err(|error|error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn plugin_packages_keep_resources_and_refuse_ambiguous_variants() {
        let id = "00000000-0000-4000-8000-000000000001";
        for module in super::registry().definitions.into_iter().filter(|m| m.bundle_plugin_resources) {
            let expected = module.route("Example.dll", id).unwrap().unwrap();
            let parent = expected.rsplit_once('/').unwrap().0;
            let entries: Vec<String> = ["Author/Release/Example.dll", "Author/Release/settings.json", "Author/Release/assets/items.assetbundle"].map(String::from).to_vec();
            assert_eq!(module.route_package(&entries[0], id, &entries).unwrap().as_deref(), Some(expected.as_str()), "{}", module.domain);
            assert_eq!(module.route_package(&entries[1], id, &entries).unwrap(), Some(format!("{parent}/settings.json")), "{}", module.domain);
            assert_eq!(module.route_package(&entries[2], id, &entries).unwrap(), Some(format!("{parent}/assets/items.assetbundle")), "{}", module.domain);
            let variants: Vec<String> = ["Package/A/Example.dll", "Package/B/Example.dll", "Package/settings.json"].map(String::from).to_vec();
            assert!(module.route_package(&variants[0], id, &variants).unwrap().is_none(), "{}", module.domain);
            assert!(module.route_package(&variants[2], id, &variants).unwrap().is_none(), "{}", module.domain);
            let mixed: Vec<String> = ["Example.dll", "Other.dll", "settings.json"].map(String::from).to_vec();
            assert!(module.route_package("settings.json", id, &mixed).unwrap().is_none(), "{}", module.domain);
            assert!(module.route_package("Author/Release/../../evil.ini", id, &entries).is_err());
            assert!(module.route_package("Author/Release/install.exe", id, &entries).unwrap().is_none());
            let mut disabled = module.clone();
            disabled.unwrap_common_root = false;
            disabled.bundle_plugin_resources = false;
            assert!(disabled.route_package(&entries[0], id, &entries).unwrap().is_none());
        }
    }
    use super::*;
    const ID: &str = "00000000-0000-4000-8000-000000000001";
    #[test]
    fn documented_package_matrix_covers_every_bundled_game_without_losing_companions() {
        let audit: serde_json::Value = serde_json::from_slice(include_bytes!("../../resources/nexus-games/support-audit.json")).unwrap();
        let generated: Vec<serde_json::Value> = [include_bytes!("../../resources/nexus-games/unreal-vortex-audit.json").as_slice(), include_bytes!("../../resources/nexus-games/unity-vortex-audit.json").as_slice(), include_bytes!("../../resources/nexus-games/re_engine-vortex-audit.json").as_slice()]
            .iter().map(|bytes| serde_json::from_slice(bytes).unwrap()).collect();
        let definitions = builtins();
        let cases: Vec<_> = audit["modules"].as_array().unwrap().iter().chain(generated.iter().flat_map(|g| g["modules"].as_array().unwrap())).collect();
        assert_eq!(cases.len(), definitions.len());
        let mut seen = std::collections::BTreeSet::new();
        for case in cases {
            let domain = case["domain"].as_str().unwrap();
            let name = case["name"].as_str().unwrap();
            assert!(seen.insert((domain, name)));
            // Editions share a Nexus game (Skyrim VR uses skyrimspecialedition): match by name too.
            let game = definitions.iter().find(|m| m.domain == domain && m.name == name).unwrap_or_else(|| panic!("{domain}: {name}"));
            assert_eq!(case["module_version"], game.version);
            // Automatic discovery relies on this flag; it must follow the audit.
            assert_eq!(game.vortex_supported, case["vortex_status"] == "confirmed", "{domain}: {name}");
            assert!(!case["vortex_evidence"].as_array().unwrap().is_empty());
            let samples = case["route_cases"].as_array().unwrap();
            let entries: Vec<String> = samples.iter().map(|sample| sample["source"].as_str().unwrap().into()).collect();
            for sample in samples {
                let source = sample["source"].as_str().unwrap();
                assert_eq!(game.route_package(source, ID, &entries).unwrap().as_deref(), sample["target"].as_str(), "{domain}: {source}");
            }
            for source in ["../outside.dll", "/absolute/file.dll", "C:/outside.dll"] {
                assert!(game.route_package(source, ID, &entries).is_err(), "{domain}: {source}");
            }
            for original in game.required.iter().chain(std::iter::once(&game.executable)) {
                assert!(!game.allowed(original), "{domain} permits replacing {original}");
            }
        }
    }
    #[test]
    fn bethesda_modules_do_not_accept_other_games_extenders_or_archive_formats() {
        let definitions = builtins();
        for (domain, own, unsupported) in [
            ("skyrimspecialedition", "SKSE", vec![".ba2"]),
            ("skyrim", "SKSE", vec![".esl", ".ba2"]),
            ("fallout4", "F4SE", vec![".bsa"]),
            ("newvegas", "NVSE", vec![".esl", ".ba2"]),
            ("fallout3", "FOSE", vec![".esl", ".ba2"]),
            ("oblivion", "OBSE", vec![".esl", ".ba2"]),
            ("starfield", "SFSE", vec![".bsa"]),
        ] {
            let game = definitions.iter().find(|m| m.domain == domain).unwrap();
            let script = format!("Data/{own}/Plugins/plugin.dll");
            assert_eq!(game.route_package(&script, ID, &[script.clone()]).unwrap(), Some(script));
            for other in ["SKSE", "F4SE", "NVSE", "FOSE", "OBSE", "SFSE"].into_iter().filter(|name| *name != own) {
                for source in [format!("{other}/Plugins/plugin.dll"), format!("Release/Data/{other}/Plugins/plugin.dll")] {
                    assert!(game.route_package(&source, ID, &[source.clone()]).is_err(), "{domain}: {source}");
                }
            }
            for extension in unsupported {
                for source in [format!("plugin{extension}"), format!("Data/plugin{extension}")] {
                    assert!(game.route_package(&source, ID, &[source.clone()]).is_err(), "{domain}: {source}");
                }
            }
            assert_eq!(game.route_package("textures/armor/skin.dds", ID, &[]).unwrap().as_deref(), Some("Data/textures/armor/skin.dds"));
        }
    }
    #[test]
    fn palworld_mixed_packages_preserve_scripts_activation_and_blueprints() {
        let game = builtins().into_iter().find(|m| m.domain == "palworld").unwrap();
        let sources: Vec<String> = [
            "Pal/Binaries/Win64/Mods/DekModConfigMenu/Scripts/main.lua",
            "Pal/Binaries/Win64/Mods/DekModConfigMenu/enabled.txt",
            "Pal/Content/Paks/LogicMods/DekModConfigMenu_P.pak",
        ].into_iter().map(str::to_string).collect();
        for wrapper in ["", "DekMCM release/"] {
            let entries: Vec<_> = sources.iter().map(|p| format!("{wrapper}{p}")).collect();
            let targets: Vec<_> = entries.iter().map(|p| game.route_package(p, ID, &entries).unwrap().unwrap()).collect();
            assert_eq!(targets, ["Pal/Binaries/Win64/ue4ss/Mods/DekModConfigMenu/Scripts/main.lua", "Pal/Binaries/Win64/ue4ss/Mods/DekModConfigMenu/enabled.txt", "Pal/Content/Paks/LogicMods/DekModConfigMenu_P.pak"]);
        }
        for source in ["Mods/Menu/Scripts/main.lua", "Menu/Scripts/main.lua", "ue4ss/Mods/Menu/Scripts/main.lua"] {
            assert_eq!(game.route_package(source, ID, &[source.into()]).unwrap().as_deref(), Some("Pal/Binaries/Win64/ue4ss/Mods/Menu/Scripts/main.lua"));
        }
        assert_eq!(game.route_package("main.lua", ID, &["main.lua".into()]).unwrap(), None);
        assert!(game.route_package("Mods/Menu/../../outside.lua", ID, &[]).is_err());
        assert!(!game.allowed("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"));
    }
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
            // Script extenders, checked against the public Nexus API on 2026-10-10.
            ("skyrimspecialedition", "SKSE64", 30379, "Skyrim Script Extender (SKSE64)"),
            ("skyrim", "SKSE", 100216, "Skyrim Script Extender (SKSE)"),
            ("fallout4", "F4SE", 42147, "Fallout 4 Script Extender (F4SE)"),
            ("newvegas", "xNVSE", 67883, "New Vegas Script Extender (NVSE xNVSE)"),
            ("fallout3", "FOSE", 8606, "Fallout Script Extender (FOSE)"),
            ("oblivion", "xOBSE", 37952, "Oblivion Script Extender (OBSE xOBSE)"),
            ("starfield", "SFSE", 106, "Starfield Script Extender (SFSE)"),
            // VR editions share the flat game's Nexus page (2026-10-10).
            ("skyrimspecialedition", "SKSEVR", 30457, "Skyrim Script Extender for VR (SKSEVR)"),
            ("skyrimspecialedition", "Skyrim VR ESL Support", 106712, "Skyrim VR ESL Support"),
            ("fallout4", "F4SEVR", 42159, "Fallout 4 Script Extender VR (F4SEVR)"),
            ("fallout4", "Daytripper 4", 91141, "Daytripper 4"),
        ];
        let modules = builtins();
        assert_eq!(modules.iter().flat_map(|m| &m.frameworks).filter(|f| f.mod_id > 0).count(), expected.len());
        for (domain, name, id, title) in expected {
            let module = modules.iter().find(|m| m.domain == domain && m.frameworks.iter().any(|f| f.name == name)).unwrap();
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
    fn unreal_family_routes_vortex_template_layouts_for_every_member() {
        let modules: Vec<_> = builtins().into_iter().filter(|m| matches!(m.family, Some(Family::Unreal { .. }))).collect();
        assert!(modules.len() >= 10);
        for game in modules {
            let Some(Family::Unreal { project, ue4ss, .. }) = game.family.clone() else { unreachable!() };
            let d = &game.domain;
            let paks = format!("{project}/Content/Paks/~mods");
            for source in ["Outfit_P.pak", "Release/Outfit_P.utoc", "Release/Variant/Outfit_P.ucas", "~mods/Outfit_P.pak"] {
                let name = source.rsplit('/').next().unwrap();
                assert_eq!(game.route_package(source, ID, &[source.into()]).unwrap(), Some(format!("{paks}/{name}")), "{d}: {source}");
            }
            let explicit = format!("Wrapper/{paks}/Sub/Outfit_P.pak");
            assert_eq!(game.route_package(&explicit, ID, &[explicit.clone()]).unwrap(), Some(format!("{paks}/Sub/Outfit_P.pak")), "{d}");
            // Identity files, unknown loose files and Xbox layouts are never accepted.
            for original in game.required.iter().chain(std::iter::once(&game.executable)) { assert!(!game.allowed(original), "{d}: {original}"); }
            assert_eq!(game.route_package("readme.txt", ID, &["readme.txt".into()]).unwrap(), None, "{d}");
            let gdk = format!("{project}/Binaries/WinGDK/dwmapi.dll");
            assert!(game.route_package(&gdk, ID, &[gdk.clone()]).is_err(), "{d}");
            assert!(game.route_package("../escape.pak", ID, &[]).is_err(), "{d}");
            let layout = game.ue4ss();
            assert_eq!(layout.is_some(), ue4ss, "{d}");
            let Some(layout) = layout else {
                for source in ["ue4ss/UE4SS.dll", "LogicMods/Menu.pak"] {
                    assert!(game.route_package(source, ID, &[source.into()]).is_err(), "{d}: {source}");
                }
                assert!(game.frameworks.iter().all(|f| f.name != "UE4SS"), "{d}");
                continue;
            };
            let bin = &layout.binaries;
            // LogicMods win over the generic pak route, as in the Vortex priorities.
            for (source, target) in [
                ("Release/LogicMods/Menu.pak".to_string(), format!("{project}/Content/Paks/LogicMods/Menu.pak")),
                ("UE4SS_v3/ue4ss/UE4SS.dll".into(), format!("{bin}/ue4ss/UE4SS.dll")),
                ("UE4SS_v3/dwmapi.dll".into(), format!("{bin}/dwmapi.dll")),
                (format!("{bin}/Mods/Menu/Scripts/main.lua"), format!("{bin}/ue4ss/Mods/Menu/Scripts/main.lua")),
            ] {
                assert_eq!(game.route_package(&source, ID, &[source.clone()]).unwrap(), Some(target), "{d}: {source}");
            }
            let script: Vec<String> = ["Pack/Menu/Scripts/main.lua", "Pack/Menu/enabled.txt", "Pack/Native/dlls/main.dll"].map(String::from).to_vec();
            assert_eq!(game.route_package(&script[0], ID, &script).unwrap(), Some(format!("{bin}/ue4ss/Mods/Menu/Scripts/main.lua")), "{d}");
            assert_eq!(game.route_package(&script[1], ID, &script).unwrap(), Some(format!("{bin}/ue4ss/Mods/Menu/enabled.txt")), "{d}");
            assert_eq!(game.route_package(&script[2], ID, &script).unwrap(), Some(format!("{bin}/ue4ss/Mods/Native/dlls/main.dll")), "{d}");
            let ue4ss = game.frameworks.iter().find(|f| f.name == "UE4SS").expect("UE4SS requirement");
            assert_eq!(ue4ss.required_files, [format!("{bin}/dwmapi.dll")]);
            assert!(ue4ss.for_paths.contains(&format!("{project}/Content/Paks/LogicMods/")));
            assert!(game.dll_overrides.iter().any(|o| o.name == "dwmapi" && o.file == format!("{bin}/dwmapi.dll")), "{d}");
            assert!(!game.allowed(&format!("{bin}/{project}-Win64-Shipping.exe")), "{d}");
        }
    }
    #[test]
    fn vortex_mod_types_resolve_only_inside_module_destinations() {
        let mut game = builtins().into_iter().find(|m| m.domain == "stellarblade").unwrap();
        game.vortex_mod_types.insert("stellarblade-logicmods".into(), "SB/Content/Paks/LogicMods".into());
        game.vortex_mod_types.insert("stellarblade-scripts".into(), "SB/Binaries/Win64/ue4ss/Mods".into());
        game.vortex_mod_types.insert("stellarblade-binaries".into(), "SB/Binaries/Win64".into());
        assert_eq!(game.vortex_destination("stellarblade-logicmods").as_deref(), Some("SB/Content/Paks/LogicMods"));
        assert_eq!(game.vortex_destination("stellarblade-scripts").as_deref(), Some("SB/Binaries/Win64/ue4ss/Mods"));
        assert_eq!(game.vortex_destination("stellarblade-binaries"), None, "pasta dos executáveis fora dos destinos");
        assert_eq!(game.vortex_destination("desconhecido"), None);
        game.vortex_mod_types.insert("bad".into(), "../escape".into());
        assert!(game.validate().is_err());
    }
    #[test]
    fn no_module_identity_is_contained_in_another() {
        // Discovery needs exactly one match. Equal identities can never be told
        // apart; a strict subset is allowed only for a derived game whose base
        // game is also bundled (Enderal over Skyrim), where the larger one wins.
        let modules = builtins();
        let key = super::super::nexus_discovery::identity;
        let mut names = std::collections::BTreeSet::new();
        let derived = [("Enderal", "Skyrim"), ("Enderal Special Edition", "Skyrim Special Edition")];
        for (i, a) in modules.iter().enumerate() {
            assert!(names.insert((&a.domain, &a.platform, &a.name)), "módulo duplicado: {}", a.domain);
            for b in modules.iter().skip(i + 1).filter(|b| b.platform == a.platform) {
                let (ka, kb) = (key(a), key(b));
                assert_ne!(ka, kb, "{} e {} disputam a mesma pasta", a.domain, b.domain);
                if ka.is_subset(&kb) || kb.is_subset(&ka) {
                    assert!(derived.iter().any(|pair| [*pair, (pair.1, pair.0)].contains(&(a.name.as_str(), b.name.as_str()))), "{} e {} disputam a mesma pasta", a.name, b.name);
                }
            }
        }
    }
    #[test]
    fn unity_family_routes_loader_layouts_and_protects_game_data_for_every_member() {
        let modules: Vec<_> = builtins().into_iter().filter(|m| matches!(m.family, Some(Family::Unity { .. }))).collect();
        assert!(modules.len() >= 20);
        for game in modules {
            let Some(Family::Unity { data, loaders }) = game.family.clone() else { unreachable!() };
            let d = &game.domain;
            let route = |source: &str| game.route_package(source, ID, &[source.to_string()]);
            for original in game.required.iter().chain(std::iter::once(&game.executable)) { assert!(!game.allowed(original), "{d}: {original}"); }
            for source in [format!("{data}/Managed/Assembly-CSharp.dll"), format!("Mod/{data}/sharedassets0.assets"), "level0.assets".into(), "../escape.dll".into()] {
                assert!(route(&source).is_err(), "{d}: {source}");
            }
            assert_eq!(route("readme.txt").unwrap(), None, "{d}");
            let bepinex = loaders.contains(&UnityLoader::Bepinex);
            let melon = loaders.contains(&UnityLoader::Melonloader);
            assert_eq!(game.frameworks.iter().any(|f| f.name.starts_with("BepInEx")), bepinex, "{d}");
            assert_eq!(game.frameworks.iter().any(|f| f.name == "MelonLoader"), melon, "{d}");
            assert_eq!(game.dll_overrides.iter().any(|o| o.name == "winhttp"), bepinex, "{d}");
            assert_eq!(game.dll_overrides.iter().any(|o| o.name == "version"), melon, "{d}");
            if bepinex {
                for (source, target) in [
                    ("BepInExPack/BepInEx/core/BepInEx.dll", "BepInEx/core/BepInEx.dll"),
                    ("BepInExPack/winhttp.dll", "winhttp.dll"),
                    ("BepInExPack/doorstop_config.ini", "doorstop_config.ini"),
                    ("Author-Mod/plugins/Mod/Mod.dll", "BepInEx/plugins/Mod/Mod.dll"),
                    ("Author-Mod/config/mod.cfg", "BepInEx/config/mod.cfg"),
                ] {
                    assert_eq!(route(source).unwrap().as_deref(), Some(target), "{d}: {source}");
                }
                assert_eq!(route("Mod.dll").unwrap(), Some(format!("BepInEx/plugins/{ID}/Mod.dll")), "{d}");
            }
            if melon {
                for (source, target) in [("MelonLoader/net6/MelonLoader.dll", "MelonLoader/net6/MelonLoader.dll"), ("version.dll", "version.dll"), ("Pack/Mods/Mod.dll", "Mods/Mod.dll"), ("Pack/UserLibs/Lib.dll", "UserLibs/Lib.dll")] {
                    assert_eq!(route(source).unwrap().as_deref(), Some(target), "{d}: {source}");
                }
                if !bepinex { assert_eq!(route("Mod.dll").unwrap().as_deref(), Some("Mods/Mod.dll"), "{d}"); }
            } else {
                assert!(route("MelonLoader/net6/MelonLoader.dll").is_err(), "{d}");
            }
            if !bepinex { assert!(route("BepInExPack/BepInEx/core/BepInEx.dll").is_err(), "{d}"); }
        }
    }
    #[test]
    fn script_extenders_install_at_root_launch_and_are_required() {
        let modules: Vec<_> = builtins().into_iter().filter(|m| m.script_extender.is_some()).collect();
        assert_eq!(modules.len(), 11);
        for game in modules {
            let extender = game.script_extender.clone().unwrap();
            let (d, loader, prefix) = (&game.domain, extender.loader.clone(), extender.dll_prefix.clone());
            let package: Vec<String> = [format!("{prefix}2_02_06/{loader}"), format!("{prefix}2_02_06/{prefix}1_6_1170.dll"), format!("{prefix}2_02_06/Data/Scripts/Game.pex"), format!("{prefix}2_02_06/src/common/IDebugLog.cpp")].to_vec();
            assert_eq!(game.route_package(&package[0], ID, &package).unwrap(), Some(loader.clone()), "{d}");
            assert_eq!(game.route_package(&package[1], ID, &package).unwrap(), Some(format!("{prefix}1_6_1170.dll")), "{d}");
            assert_eq!(game.route_package(&package[2], ID, &package).unwrap().as_deref(), Some("Data/Scripts/Game.pex"), "{d}");
            assert_eq!(game.route_package(&package[3], ID, &package).unwrap(), None, "{d}: código-fonte não é implantado");
            // A plugin DLL with the extender prefix stays in the plugin folder.
            let plugin = format!("{}/{prefix}helper.dll", extender.plugins);
            assert_eq!(game.route_package(&plugin, ID, &[plugin.clone()]).unwrap().as_deref(), Some(plugin.as_str()), "{d}");
            assert!(!game.allowed("other.dll") && !game.allowed("dinput8.dll"), "{d}: só o loader e as DLLs do extender vão para a raiz");
            assert!(!game.allowed(&game.executable), "{d}");
            assert_eq!(game.mod_launch_executable.as_deref(), Some(loader.as_str()), "{d}");
            let framework = game.frameworks.iter().find(|f| f.name == extender.name).unwrap();
            assert_eq!(framework.required_files, [loader.clone()]);
            assert_eq!(framework.for_paths, [format!("{}/", extender.plugins)]);
            let page = extender.domain.clone().unwrap_or_else(|| d.clone());
            assert_eq!(framework.url.as_deref(), Some(format!("https://www.nexusmods.com/{page}/mods/{}", extender.mod_id).as_str()));
            // Another game's page is linked but cannot vouch for a download of this game.
            assert_eq!(framework.mod_id > 0, page == *d, "{d}");
        }
    }
    #[test]
    fn bethesda_editions_are_told_apart_by_executable_or_steam_id() {
        let modules = builtins();
        let find = |name: &str| modules.iter().find(|m| m.name == name).unwrap();
        for (edition, flat, domain) in [("Skyrim VR", "Skyrim Special Edition", "skyrimspecialedition"), ("Fallout 4 VR", "Fallout 4", "fallout4")] {
            let (vr, flat) = (find(edition), find(flat));
            // Same Nexus game, different installation and plugin list.
            assert_eq!((vr.domain.as_str(), flat.domain.as_str()), (domain, domain));
            assert_ne!(vr.executable, flat.executable);
            assert_ne!(vr.activation.as_ref().unwrap().path, flat.activation.as_ref().unwrap().path);
        }
        // An .esl needs the VR ESL enabler; other plugins do not.
        let vr = find("Skyrim VR");
        let esl = vr.frameworks.iter().find(|f| f.for_extensions == [".esl"]).unwrap();
        assert_eq!(esl.required_files, ["Data/SKSE/Plugins/skyrimvresl.dll"]);
        for (edition, flat, extra) in [("Enderal", "Skyrim", "Enderal Launcher.exe"), ("Enderal Special Edition", "Skyrim Special Edition", "Enderal Launcher.exe")] {
            let (enderal, skyrim) = (find(edition), find(flat));
            assert_eq!(enderal.executable, skyrim.executable);
            assert!(enderal.required.iter().any(|path| path == extra) && !skyrim.required.iter().any(|path| path == extra));
            assert_ne!(enderal.steam_id, skyrim.steam_id);
        }
    }
    #[test]
    fn re_engine_family_routes_reframework_and_refuses_unprocessed_data() {
        let modules: Vec<_> = builtins().into_iter().filter(|m| matches!(m.family, Some(Family::ReEngine {}))).collect();
        assert!(modules.len() >= 10);
        for game in modules {
            let d = &game.domain;
            let route = |source: &str| game.route_package(source, ID, &[source.to_string()]);
            for (source, target) in [
                ("REFramework/dinput8.dll", None),
                ("dinput8.dll", Some("dinput8.dll")),
                ("Mod/reframework/autorun/script.lua", Some("reframework/autorun/script.lua")),
                ("reframework/plugins/plugin.dll", Some("reframework/plugins/plugin.dll")),
                ("Pack/reframework/data/config.json", Some("reframework/data/config.json")),
                ("script.lua", Some("reframework/autorun/script.lua")),
            ] {
                assert_eq!(route(source).unwrap().as_deref(), target, "{d}: {source}");
            }
            for source in ["natives/STM/streaming/x.tex", "Pack/pak_mods/mod.pak", "../dinput8.dll"] { assert!(route(source).is_err(), "{d}: {source}"); }
            for original in game.required.iter().chain(std::iter::once(&game.executable)) { assert!(!game.allowed(original), "{d}: {original}"); }
            assert!(game.frameworks.iter().any(|f| f.name == "REFramework" && f.required_files == ["dinput8.dll"]), "{d}");
            assert!(game.dll_overrides.iter().any(|o| o.name == "dinput8"), "{d}");
        }
    }
    #[test]
    fn unreal_family_rejects_unsafe_folders_and_keeps_expansion_idempotent() {
        let mut game = builtins().into_iter().find(|m| m.domain == "stellarblade").unwrap();
        let routes = game.routes.len();
        game.apply_family().unwrap();
        assert_eq!(game.routes.len(), routes);
        game.family = Some(Family::Unreal { project: "../SB".into(), binaries: "Win64".into(), ue4ss: true });
        assert!(game.apply_family().is_err());
        game.family = Some(Family::Unreal { project: "SB".into(), binaries: "Win64/../..".into(), ue4ss: true });
        assert!(game.apply_family().is_err());
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
