//! Stable release discovery and immutable runtime history. Updating never rewrites
//! a runtime referenced by a game's launch profile.
use super::{release, root, validate_runtime_at, Settings, INSTALLING};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::atomic::Ordering};

#[derive(Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub hash: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Cached {
    checked: u64,
    release: Release,
}
#[derive(Serialize)]
pub struct Installed {
    pub version: String,
    pub current: bool,
    pub in_use: bool,
    pub directory: String,
}
fn semver(s: &str) -> Option<[u32; 3]> {
    let parts = s
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()
        .ok()?;
    (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
}
pub fn builtin(backend: &str) -> Result<Release, String> {
    let (version, url, hash) = release(backend)?;
    Ok(Release {
        version: version.into(),
        url: url.into(),
        hash: Some(hash.into()),
    })
}
fn base(backend: &str) -> Result<PathBuf, String> {
    release(backend)?;
    Ok(root()?.join(backend))
}
fn valid(backend: &str, r: &Release) -> bool {
    if semver(&r.version).is_none() {
        return false;
    }
    let expected = match backend {
        "mako" => format!("https://github.com/eugeniosegala/MAKO/releases/download/render-v{0}/MAKO-Renderer-v{0}-linux.tar.xz", r.version),
        "afmf" => format!("https://github.com/serialexperimentslainnnn/afmf-linux/releases/download/v{0}/afmf-linux-{0}-x86_64.tar.xz", r.version),
        _ => return false,
    };
    r.url == expected
        && r.hash
            .as_ref()
            .is_none_or(|h| h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit()))
}
pub fn newer(a: &str, b: &str) -> bool {
    semver(a).zip(semver(b)).is_some_and(|(a, b)| a > b)
}
pub fn compatible(backend: &str, version: &str) -> bool {
    semver(version)
        .zip(release(backend).ok().and_then(|r| semver(r.0)))
        .is_some_and(|(v, min)| v[0] == min[0] && v >= min)
}
pub fn location(backend: &str, version: &str) -> Result<PathBuf, String> {
    location_at(&root()?, backend, version)
}
fn location_at(storage: &std::path::Path, backend: &str, version: &str) -> Result<PathBuf, String> {
    release(backend)?;
    if semver(version).is_none() {
        return Err("Versão de runtime inválida.".into());
    }
    let legacy = storage.join(format!("{backend}-{version}"));
    if legacy.is_dir() && !legacy.is_symlink() {
        return Ok(legacy);
    }
    Ok(storage.join(backend).join("versions").join(version))
}
pub fn current(backend: &str) -> Result<String, String> {
    if let Ok(bytes) = fs::read(base(backend)?.join("current.json")) {
        if let Ok(version) = serde_json::from_slice::<String>(&bytes) {
            if compatible(backend, &version)
                && validate_runtime_at(&location(backend, &version)?, backend).is_ok()
            {
                return Ok(version);
            }
        }
    }
    Ok(release(backend)?.0.into())
}
pub fn latest(backend: &str) -> Result<Release, String> {
    if let Ok(bytes) = fs::read(base(backend)?.join("release.json")) {
        if let Ok(cache) = serde_json::from_slice::<Cached>(&bytes) {
            if valid(backend, &cache.release)
                && semver(&cache.release.version) >= semver(release(backend)?.0)
            {
                return Ok(cache.release);
            }
        }
    }
    builtin(backend)
}
pub fn publish(backend: &str, r: &Release) -> Result<(), String> {
    let path = base(backend)?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    super::write(
        &path.join("current.json"),
        &serde_json::to_vec(&r.version).map_err(|e| e.to_string())?,
        0o600,
    )?;
    super::write(
        &path.join(format!("{}.release.json", r.version)),
        &serde_json::to_vec(r).map_err(|e| e.to_string())?,
        0o600,
    )
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
/// Each backend's official catalog and how its stable packages are named.
fn catalog(backend: &str) -> Option<(&'static str, &'static str, fn(&str) -> String)> {
    match backend {
        "mako" => Some(("https://api.github.com/repos/eugeniosegala/MAKO/releases?per_page=50", "render-v", |v| format!("MAKO-Renderer-v{v}-linux.tar.xz"))),
        "afmf" => Some(("https://api.github.com/repos/serialexperimentslainnnn/afmf-linux/releases?per_page=30", "v", |v| format!("afmf-linux-{v}-x86_64.tar.xz"))),
        _ => None,
    }
}
fn parse_release(backend: &str, json: &serde_json::Value) -> Option<Release> {
    let (_, prefix, package) = catalog(backend)?;
    json.as_array()?
        .iter()
        .filter(|r| r["prerelease"] == false && r["draft"] == false)
        .filter_map(|r| {
            let version = r["tag_name"].as_str()?.strip_prefix(prefix)?;
            let v = semver(version)?;
            let name = package(version);
            let a = r["assets"]
                .as_array()?
                .iter()
                .find(|a| a["name"].as_str() == Some(name.as_str()))?;
            Some((
                v,
                Release {
                    version: version.into(),
                    url: a["browser_download_url"].as_str()?.into(),
                    hash: a["digest"]
                        .as_str()
                        .and_then(|s| s.strip_prefix("sha256:"))
                        .map(str::to_owned),
                },
            ))
        })
        .max_by_key(|(v, _)| *v)
        .map(|(_, r)| r)
}
#[derive(Serialize)]
pub struct Update {
    pub version: String,
    pub available: bool,
    pub compatible: bool,
    pub download_url: String,
    pub install_directory: String,
    pub package_name: String,
}
#[tauri::command]
pub async fn frame_generation_check_updates(
    backend: String,
    force: bool,
) -> Result<Update, String> {
    crate::core::release_features::require()?;
    let builtin = builtin(&backend)?;
    let cache_path = base(&backend)?.join("release.json");
    let cached = fs::read(&cache_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Cached>(&b).ok());
    let r = if !force
        && cached
            .as_ref()
            .is_some_and(|c| now().saturating_sub(c.checked) < 21600 && valid(&backend, &c.release))
    {
        cached.unwrap().release
    } else {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("PeliGames-frame-generation")
            .build()
            .map_err(|e| e.to_string())?;
        let (url, _, _) = catalog(&backend).ok_or("Tecnologia de geração de quadros inválida.")?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let mut response = response;
        let mut data = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if data.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err("Catálogo de versões excedeu o limite.".into());
            }
            data.extend_from_slice(&chunk);
        }
        let found = serde_json::from_slice(&data)
            .ok()
            .and_then(|j| parse_release(&backend, &j))
            .ok_or("Não foi possível identificar uma versão estável oficial.")?;
        if !valid(&backend, &found) {
            return Err("O endereço do pacote oficial não corresponde ao esperado.".into());
        }
        let found = if semver(&found.version) <= semver(&builtin.version) {
            builtin
        } else {
            found
        };
        fs::create_dir_all(base(&backend)?).map_err(|e| e.to_string())?;
        super::write(
            &cache_path,
            &serde_json::to_vec(&Cached {
                checked: now(),
                release: found.clone(),
            })
            .map_err(|e| e.to_string())?,
            0o600,
        )?;
        found
    };
    let installed = current(&backend)?;
    Ok(Update {
        download_url: r.url.clone(),
        install_directory: location(&backend, &r.version)?.to_string_lossy().into(),
        package_name: r.url.rsplit('/').next().unwrap_or_default().into(),
        available: semver(&r.version) > semver(&installed),
        compatible: compatible(&backend, &r.version),
        version: r.version,
    })
}
/// Only profiles in this application's private directory are considered. Old
/// snapshot files count too: pending Steam changes may still reference them.
fn referenced(backend: &str, version: &str) -> Result<bool, String> {
    referenced_at(&root()?.join("profiles"), backend, version)
}
fn referenced_at(profiles: &std::path::Path, backend: &str, version: &str) -> Result<bool, String> {
    if !profiles.exists() {
        return Ok(false);
    }
    for entry in fs::read_dir(profiles).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.extension().is_some_and(|e| e == "json") {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            let settings: Settings = serde_json::from_slice(&bytes)
                .map_err(|_| "Há um perfil ilegível; o backup foi preservado.")?;
            let used = if settings.runtime_version.is_empty() {
                release(&settings.backend)?.0
            } else {
                &settings.runtime_version
            };
            if settings.backend == backend && used == version && settings.enabled {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
pub fn installed(backend: &str) -> Result<Vec<Installed>, String> {
    let mut names = std::collections::BTreeSet::new();
    let path = base(backend)?.join("versions");
    if path.is_dir() {
        for e in fs::read_dir(path).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let n = e.file_name().to_string_lossy().into_owned();
            if semver(&n).is_some() && e.file_type().map_err(|e| e.to_string())?.is_dir() {
                names.insert(n);
            }
        }
    }
    if location(backend, release(backend)?.0)?.is_dir() {
        names.insert(release(backend)?.0.into());
    }
    let current = current(backend)?;
    let mut all = Vec::new();
    for version in names {
        if validate_runtime_at(&location(backend, &version)?, backend).is_ok() {
            all.push(Installed {
                current: version == current,
                in_use: referenced(backend, &version)?,
                directory: location(backend, &version)?.to_string_lossy().into(),
                version,
            });
        }
    }
    all.sort_by_key(|r| std::cmp::Reverse(semver(&r.version)));
    Ok(all)
}
#[tauri::command]
pub async fn frame_generation_remove_backup(
    backend: String,
    version: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move||{
        if INSTALLING.swap(true,Ordering::AcqRel){return Err("Aguarde a instalação em andamento.".into())}
        struct Guard;impl Drop for Guard{fn drop(&mut self){INSTALLING.store(false,Ordering::Release);}}let _guard=Guard;
        if current(&backend)?==version{return Err("A versão atual não pode ser removida.".into())}
        if referenced(&backend,&version)?{return Err("Este backup está vinculado a um perfil de jogo ou a uma alteração pendente. Ele foi preservado.".into())}
        let p=location(&backend,&version)?;
        if p.is_symlink(){return Err("Backup inválido; remoção recusada.".into())}
        fs::remove_dir_all(p).map_err(|e|e.to_string())
    }).await.map_err(|e|e.to_string())?
}
/// Explicit uninstall includes the current runtime, unlike backup cleanup.
#[tauri::command]
pub async fn frame_generation_uninstall(backend:String)->Result<(),String>{
 crate::core::release_features::require()?;
 tauri::async_runtime::spawn_blocking(move||{
  release(&backend)?;
  if INSTALLING.swap(true,Ordering::AcqRel){return Err("Aguarde a operação no componente em andamento.".into());}
  struct Guard;impl Drop for Guard{fn drop(&mut self){INSTALLING.store(false,Ordering::Release);}}let _guard=Guard;
  for runtime in installed(&backend)?{if referenced(&backend,&runtime.version)?{return Err("Este componente ainda está ativado em um perfil de jogo ou alteração pendente. Desative-o nos jogos e aplique as alterações antes de desinstalar.".into());}}
  crate::core::component_uninstall::remove_runtime(&root()?,&backend)
 }).await.map_err(|e|e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_runtime_paths_are_preserved_and_new_versions_are_separate() {
        let path =
            std::env::temp_dir().join(format!("peligames-fg-history-{}", uuid::Uuid::new_v4()));
        let legacy = path.join("mako-4.5.0");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("original"), b"unchanged").unwrap();
        assert_eq!(location_at(&path, "mako", "4.5.0").unwrap(), legacy);
        assert_eq!(
            location_at(&path, "mako", "4.6.0").unwrap(),
            path.join("mako/versions/4.6.0")
        );
        assert_eq!(fs::read(legacy.join("original")).unwrap(), b"unchanged");
        assert!(location_at(&path, "mako", "../../other").is_err());
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn backups_linked_to_profiles_and_unreadable_profiles_are_preserved() {
        let path =
            std::env::temp_dir().join(format!("peligames-fg-references-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("old.json"),
            br#"{"backend":"mako","enabled":true}"#,
        )
        .unwrap();
        assert!(referenced_at(&path, "mako", "4.5.0").unwrap());
        assert!(!referenced_at(&path, "mako", "4.6.0").unwrap());
        fs::write(
            path.join("old.json"),
            br#"{"backend":"mako","runtime_version":"4.6.0","enabled":true}"#,
        )
        .unwrap();
        assert!(!referenced_at(&path, "mako", "4.5.0").unwrap());
        assert!(referenced_at(&path, "mako", "4.6.0").unwrap());
        fs::write(path.join("old.json"), b"broken").unwrap();
        assert!(referenced_at(&path, "mako", "4.5.0").is_err());
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn profile_version_is_persisted_and_legacy_profiles_remain_readable() {
        let old: Settings = serde_json::from_str(r#"{"backend":"mako","enabled":true}"#).unwrap();
        assert!(old.runtime_version.is_empty());
        let mut current = old.clone();
        current.runtime_version = "4.5.0".into();
        let old_snapshot = super::super::profile_key(&serde_json::to_string(&current).unwrap());
        current.runtime_version = "4.6.0".into();
        assert_ne!(
            old_snapshot,
            super::super::profile_key(&serde_json::to_string(&current).unwrap())
        );
        assert_eq!(
            serde_json::from_slice::<Settings>(&serde_json::to_vec(&current).unwrap())
                .unwrap()
                .runtime_version,
            "4.6.0"
        );
    }
    #[tokio::test]
    #[ignore = "Reads official release feeds; run only with isolated XDG directories"]
    async fn official_stable_feeds_are_recognized() {
        assert!(std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_default()
            .starts_with("/tmp/peligames-fg-"));
        for backend in ["mako"] {
            let update = frame_generation_check_updates(backend.into(), true)
                .await
                .unwrap();
            assert!(semver(&update.version) >= semver(release(backend).unwrap().0));
            assert!(!update.download_url.is_empty());
        }
    }
    #[test]
    fn only_stable_and_known_sources() {
        assert!(semver("4.5.0-rc1").is_none());
        assert!(semver("../../4.5.0").is_none());
        assert!(compatible("mako", "4.6.0"));
        assert!(!compatible("mako", "5.0.0"));
        assert!(!compatible("mako", "4.4.0"));
        let mut r = builtin("mako").unwrap();
        r.url = "https://example.com/a".into();
        assert!(!valid("mako", &r));
    }
    #[test]
    fn mako_releases_filter_prereleases_and_assets() {
        let j = serde_json::json!([{"tag_name":"render-v4.6.0","prerelease":true,"draft":false,"assets":[]},{"tag_name":"v9.0.0","prerelease":false,"draft":false,"assets":[]},{"tag_name":"render-v4.5.0","prerelease":false,"draft":false,"assets":[{"name":"MAKO-Renderer-v4.5.0-linux.tar.xz","browser_download_url":builtin("mako").unwrap().url}]}]);
        assert_eq!(parse_release("mako", &j).unwrap().version, "4.5.0");
    }
    #[test]
    fn afmf_releases_use_their_own_tags_package_and_checks() {
        // A MAKO-style tag must not be read as an AFMF release, nor the other way around.
        let j = serde_json::json!([{"tag_name":"v1.4.0","prerelease":true,"draft":false,"assets":[]},{"tag_name":"render-v9.0.0","prerelease":false,"draft":false,"assets":[]},{"tag_name":"v1.3.1","prerelease":false,"draft":false,"assets":[{"name":"afmf-linux-1.3.1-1.fc44.x86_64.rpm","browser_download_url":"x"},{"name":"afmf-linux-1.3.1-x86_64.tar.xz","browser_download_url":builtin("afmf").unwrap().url,"digest":"sha256:7fafa09021ee94dc5ec8da694cb02fe298b54c114b486611bb24b1e1500edb18"}]}]);
        let found = parse_release("afmf", &j).unwrap();
        assert_eq!(found.version, "1.3.1");
        assert!(valid("afmf", &found) && !valid("mako", &found));
        assert!(parse_release("mako", &j).is_none());
        assert!(compatible("afmf", "1.4.2") && !compatible("afmf", "2.0.0") && !compatible("afmf", "1.2.0"));
    }
}
