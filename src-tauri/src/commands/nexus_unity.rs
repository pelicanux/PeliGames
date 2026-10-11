//! Unity family: classifies loose managed DLLs by the loader they reference,
//! as the Vortex Unity template does, and keeps a package on a single loader.
use std::path::Path;

use super::nexus_modules::{Definition, UnityLayout, UnityLoader};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DllKind { BepInExPlugin, BepInExPatcher, MelonMod, MelonPlugin, Unknown }

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

/// .NET metadata stores referenced assembly and type names as UTF-8 strings.
pub(super) fn dll_kind(bytes: &[u8]) -> DllKind {
    if contains(bytes, b"BepInEx.Preloader.Core.Patching") || contains(bytes, b"BepInEx.Preloader.Patching") {
        DllKind::BepInExPatcher
    } else if contains(bytes, b"BepInEx") {
        DllKind::BepInExPlugin
    } else if contains(bytes, b"MelonPlugin") {
        DllKind::MelonPlugin
    } else if contains(bytes, b"MelonLoader") {
        DllKind::MelonMod
    } else {
        DllKind::Unknown
    }
}

fn loader_of(kind: DllKind) -> Option<UnityLoader> {
    match kind {
        DllKind::BepInExPlugin | DllKind::BepInExPatcher => Some(UnityLoader::Bepinex),
        DllKind::MelonMod | DllKind::MelonPlugin => Some(UnityLoader::Melonloader),
        DllKind::Unknown => None,
    }
}

/// Loader currently installed in the game folder; both at once is ambiguous.
pub(super) fn installed_loader(root: &Path) -> Result<Option<UnityLoader>, String> {
    let bepinex = root.join("BepInEx/core").is_dir();
    let melon = root.join("MelonLoader").is_dir();
    match (bepinex, melon) {
        (true, true) => Err("BepInEx e MelonLoader estão instalados no jogo. Mantenha somente um carregador antes de instalar mods; nenhum arquivo foi aplicado.".into()),
        (true, false) => Ok(Some(UnityLoader::Bepinex)),
        (false, true) => Ok(Some(UnityLoader::Melonloader)),
        (false, false) => Ok(None),
    }
}

fn name(path: &str) -> &str { path.rsplit('/').next().unwrap_or(path) }

/// Adjusts canonical routes for one package. `read` returns the bytes of a
/// package entry; only DLLs placed in mod folders are inspected.
pub(super) fn retarget(
    module: &Definition,
    layout: &UnityLayout,
    files: &mut [(String, String)],
    mod_id: &str,
    installed: Option<UnityLoader>,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let lower = |s: &str| s.to_ascii_lowercase();
    let provides_bepinex = files.iter().any(|(_, t)| lower(t) == "winhttp.dll" || lower(t).starts_with("bepinex/core/"));
    let provides_melon = files.iter().any(|(_, t)| lower(t) == "version.dll" || lower(t).starts_with("melonloader/"));
    if provides_bepinex && provides_melon {
        return Err("O pacote contém BepInEx e MelonLoader juntos. Instale somente um carregador; nenhum arquivo foi aplicado.".into());
    }
    let mod_area = |t: &str| {
        let t = lower(t);
        ["bepinex/plugins/", "bepinex/patchers/", "mods/", "plugins/"].iter().any(|p| t.starts_with(p))
    };
    let mut kinds = Vec::with_capacity(files.len());
    for (source, target) in files.iter() {
        let kind = (mod_area(target) && lower(source).ends_with(".dll")).then(|| read(source).map(|bytes| dll_kind(&bytes))).transpose()?;
        kinds.push(kind);
    }
    let loaders: std::collections::BTreeSet<_> = kinds.iter().flatten().filter_map(|k| loader_of(*k)).collect();
    // Helper libraries (e.g. Newtonsoft.Json) reference no loader and follow the
    // package's plugins; a package where no DLL names a loader is unidentifiable.
    if loaders.is_empty() {
        if let Some((source, _)) = files.iter().zip(&kinds).find(|(_, kind)| **kind == Some(DllKind::Unknown)).map(|(file, _)| file) {
            return Err(format!("Não foi possível identificar o carregador da DLL {source}: ela não referencia BepInEx nem MelonLoader. Nenhum arquivo foi aplicado."));
        }
    }
    if loaders.len() > 1 {
        return Err("O pacote mistura mods de BepInEx e MelonLoader. Importe somente a versão do carregador usado; nenhum arquivo foi aplicado.".into());
    }
    // Packages without managed DLLs (configs, assets in a plugin folder) follow their folder.
    let package_loader = loaders.into_iter().next().or_else(|| {
        let melon = files.iter().any(|(_, t)| { let t = lower(t); t.starts_with("mods/") || t.starts_with("userlibs/") || t.starts_with("userdata/") });
        let bepinex = files.iter().any(|(_, t)| lower(t).starts_with("bepinex/plugins/") || lower(t).starts_with("bepinex/patchers/") || lower(t).starts_with("bepinex/config/"));
        match (bepinex, melon) { (true, false) => Some(UnityLoader::Bepinex), (false, true) => Some(UnityLoader::Melonloader), _ => None }
    });
    let Some(loader) = package_loader else { return Ok(()); };
    if !layout.loaders.contains(&loader) {
        return Err(format!("Este jogo não usa {} no módulo atual; nenhum arquivo foi aplicado.", loader.label()));
    }
    let provided = if provides_bepinex { Some(UnityLoader::Bepinex) } else if provides_melon { Some(UnityLoader::Melonloader) } else { None };
    if let Some(current) = provided.or(installed) {
        if current != loader {
            return Err(format!("Mod para {}, mas o jogo usa {}. Instale a versão do mod para o carregador atual; nenhum arquivo foi aplicado.", loader.label(), current.label()));
        }
    }
    let loose = format!("bepinex/plugins/{}/", lower(mod_id));
    for ((source, target), kind) in files.iter_mut().zip(kinds) {
        let t = lower(target);
        let next = match (loader, kind) {
            (UnityLoader::Bepinex, Some(DllKind::BepInExPatcher)) if t.starts_with(&loose) => format!("BepInEx/patchers/{mod_id}/{}", name(target)),
            (UnityLoader::Bepinex, _) if t.starts_with("mods/") || t.starts_with("userlibs/") || t.starts_with("userdata/") => {
                return Err(format!("Estrutura MelonLoader em um mod BepInEx: {source}. Nenhum arquivo foi aplicado."));
            }
            (UnityLoader::Melonloader, Some(DllKind::MelonPlugin)) if t.starts_with(&loose) || t.starts_with("bepinex/plugins/") => format!("Plugins/{}", name(target)),
            (UnityLoader::Melonloader, _) if t.starts_with(&loose) => format!("Mods/{}", &target[loose.len()..]),
            (UnityLoader::Melonloader, _) if t.starts_with("bepinex/plugins/") => format!("Plugins/{}", &target["bepinex/plugins/".len()..]),
            (UnityLoader::Melonloader, _) if t.starts_with("bepinex/") => {
                return Err(format!("Estrutura BepInEx em um mod MelonLoader: {source}. Nenhum arquivo foi aplicado."));
            }
            _ => continue,
        };
        if !module.allowed(&next) {
            return Err(format!("Destino fora das regras do módulo: {next}"));
        }
        *target = next;
    }
    Ok(())
}

/// Routes a whole package and applies `retarget`, reading DLLs from the archive.
pub(super) fn plan_package(
    module: &Definition,
    layout: &UnityLayout,
    archive: &mut zip::ZipArchive<std::fs::File>,
    entries: &[String],
    mod_id: &str,
    game_root: &Path,
) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    for entry in entries.iter().filter(|e| !e.ends_with('/')) {
        if let Some(target) = module.route_package(entry, mod_id, entries)? {
            files.push((entry.clone(), target));
        }
    }
    retarget_from_archive(module, layout, archive, &mut files, mod_id, game_root)?;
    Ok(files)
}

/// `retarget` reading the DLL bytes from the archive.
pub(super) fn retarget_from_archive(
    module: &Definition,
    layout: &UnityLayout,
    archive: &mut zip::ZipArchive<std::fs::File>,
    files: &mut [(String, String)],
    mod_id: &str,
    game_root: &Path,
) -> Result<(), String> {
    let installed = installed_loader(game_root)?;
    retarget(module, layout, files, mod_id, installed, |source| {
        use std::io::Read;
        let index = (0..archive.len())
            .find(|i| archive.by_index(*i).is_ok_and(|e| e.name().replace('\\', "/") == source))
            .ok_or("DLL não encontrada no pacote.")?;
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        // Managed plugins are small; refuse oversized inputs instead of buffering them.
        if entry.size() > 64 * 1024 * 1024 { return Err(format!("DLL grande demais para análise: {source}")); }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.take(64 * 1024 * 1024).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "00000000-0000-4000-8000-000000000001";

    fn module(loaders: &[UnityLoader]) -> (Definition, UnityLayout) {
        let loaders: Vec<_> = loaders.iter().map(|l| format!("{:?}", l.label().to_ascii_lowercase())).collect();
        let json = format!(r#"{{"schema":1,"version":"1.0.0","domain":"unitytest","name":"Unity Test","platform":"proton","engine":"deployment",
            "family":{{"kind":"unity","data":"Test_Data","loaders":[{}]}},"executable":"Test.exe","required":["Test.exe","UnityPlayer.dll"],
            "header":[77,90],"destinations":[],"routes":[]}}"#, loaders.join(","));
        let module = super::super::nexus_modules::parse(json.as_bytes()).unwrap();
        let layout = module.unity().unwrap();
        (module, layout)
    }

    fn plan(module: &Definition, entries: &[&str]) -> Vec<(String, String)> {
        let entries: Vec<String> = entries.iter().map(|e| e.to_string()).collect();
        entries.iter().filter_map(|e| module.route_package(e, ID, &entries).unwrap().map(|t| (e.clone(), t))).collect()
    }

    fn bytes(kind: &str) -> impl FnMut(&str) -> Result<Vec<u8>, String> + '_ {
        move |_| Ok(format!("MZ....{kind}....").into_bytes())
    }

    #[test]
    fn classifies_dlls_by_referenced_loader() {
        assert_eq!(dll_kind(b"..BepInEx.Preloader.Core.Patching.."), DllKind::BepInExPatcher);
        assert_eq!(dll_kind(b"..BepInEx.Unity.Mono.."), DllKind::BepInExPlugin);
        assert_eq!(dll_kind(b"..MelonLoader..MelonPlugin.."), DllKind::MelonPlugin);
        assert_eq!(dll_kind(b"..MelonLoader..MelonMod.."), DllKind::MelonMod);
        assert_eq!(dll_kind(b"..UnityEngine.CoreModule.."), DllKind::Unknown);
    }

    #[test]
    fn loose_plugins_follow_their_loader() {
        let (m, layout) = module(&[UnityLoader::Bepinex, UnityLoader::Melonloader]);
        let mut files = plan(&m, &["Release/Cheats.dll"]);
        retarget(&m, &layout, &mut files, ID, None, bytes("BepInEx")).unwrap();
        assert_eq!(files[0].1, format!("BepInEx/plugins/{ID}/Cheats.dll"));
        let mut files = plan(&m, &["Release/Cheats.dll"]);
        retarget(&m, &layout, &mut files, ID, None, bytes("BepInEx.Preloader.Core.Patching")).unwrap();
        assert_eq!(files[0].1, format!("BepInEx/patchers/{ID}/Cheats.dll"));
        let mut files = plan(&m, &["Release/Cheats.dll"]);
        retarget(&m, &layout, &mut files, ID, None, bytes("MelonLoader")).unwrap();
        assert_eq!(files[0].1, "Mods/Cheats.dll");
        let mut files = plan(&m, &["Plugins/Helper.dll"]);
        retarget(&m, &layout, &mut files, ID, None, bytes("MelonLoader MelonPlugin")).unwrap();
        assert_eq!(files[0].1, "Plugins/Helper.dll");
        let mut files = plan(&m, &["Release/Cheats.dll"]);
        assert!(retarget(&m, &layout, &mut files, ID, None, bytes("UnityEngine")).is_err());
        // A helper library follows the plugins of its package.
        let mut files = plan(&m, &["Pack/Mods/Cheats.dll", "Pack/Mods/Newtonsoft.Json.dll"]);
        retarget(&m, &layout, &mut files, ID, None, |s| Ok(if s.contains("Cheats") { b"MelonLoader".to_vec() } else { b"System.Runtime".to_vec() })).unwrap();
        assert_eq!(files.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["Mods/Cheats.dll", "Mods/Newtonsoft.Json.dll"]);
    }

    #[test]
    fn wrong_or_mixed_loaders_are_refused() {
        let (m, layout) = module(&[UnityLoader::Bepinex, UnityLoader::Melonloader]);
        let mut files = plan(&m, &["Mods/Melon.dll"]);
        assert!(retarget(&m, &layout, &mut files, ID, Some(UnityLoader::Bepinex), bytes("MelonLoader")).unwrap_err().contains("BepInEx"));
        let mut files = plan(&m, &["plugins/Bep.dll"]);
        assert!(retarget(&m, &layout, &mut files, ID, Some(UnityLoader::Melonloader), bytes("BepInEx")).is_err());
        let mut files = plan(&m, &["BepInEx/core/BepInEx.dll", "winhttp.dll", "MelonLoader/net6/MelonLoader.dll", "version.dll"]);
        assert!(retarget(&m, &layout, &mut files, ID, None, bytes("x")).unwrap_err().contains("juntos"));
        let mut files = plan(&m, &["Bep.dll", "Melon.dll"]);
        let mut n = 0;
        assert!(retarget(&m, &layout, &mut files, ID, None, |_| { n += 1; Ok(if n == 1 { b"BepInEx".to_vec() } else { b"MelonLoader".to_vec() }) }).unwrap_err().contains("mistura"));
        // A mod that ships its own loader is checked against that loader, not the installed one.
        let mut files = plan(&m, &["BepInExPack/BepInEx/core/BepInEx.dll", "BepInExPack/winhttp.dll", "plugins/Bep.dll"]);
        retarget(&m, &layout, &mut files, ID, None, bytes("BepInEx")).unwrap();
        let (m, layout) = module(&[UnityLoader::Melonloader]);
        let mut files = plan(&m, &["Release/Bep.dll"]);
        assert!(retarget(&m, &layout, &mut files, ID, None, bytes("BepInEx")).unwrap_err().contains("não usa"));
    }

    #[test]
    fn detects_installed_loader_and_ambiguity() {
        let root = std::env::temp_dir().join(format!("unity-loader-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(installed_loader(&root).unwrap(), None);
        std::fs::create_dir_all(root.join("BepInEx/core")).unwrap();
        assert_eq!(installed_loader(&root).unwrap(), Some(UnityLoader::Bepinex));
        std::fs::create_dir_all(root.join("MelonLoader")).unwrap();
        assert!(installed_loader(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
