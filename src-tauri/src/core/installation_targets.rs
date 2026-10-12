//! Read installation hints without executing shortcuts, registry commands or installers.
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct Target {
    pub source: String,
    pub arguments: Vec<String>,
    /// Shortcut name ("Battlefield 2042.lnk" → "Battlefield 2042").
    pub label: Option<String>,
}
fn read(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > limit {
        return None;
    }
    let mut data = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut data)
        .ok()?;
    (data.len() as u64 <= limit).then_some(data)
}
fn u16le(b: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(p..p.checked_add(2)?)?.try_into().ok()?,
    ))
}
fn u32le(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(p..p.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn string(b: &[u8], p: usize, unicode: bool) -> Option<String> {
    let tail = b.get(p..)?;
    if unicode {
        let mut chars = Vec::new();
        for pair in tail.chunks_exact(2) {
            let c = u16::from_le_bytes([pair[0], pair[1]]);
            if c == 0 {
                return String::from_utf16(&chars).ok();
            }
            chars.push(c);
        }
        None
    } else {
        let end = tail.iter().position(|c| *c == 0)?;
        Some(tail[..end].iter().map(|c| char::from(*c)).collect())
    }
}
// CommandLineToArgvW quoting rules; no shell is involved when these arguments are used.
fn arguments(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let mut count = 1;
            while chars.peek() == Some(&'\\') {
                chars.next();
                count += 1;
            }
            if chars.peek() == Some(&'"') {
                for _ in 0..count / 2 {
                    word.push('\\');
                }
                chars.next();
                if count % 2 == 1 {
                    word.push('"');
                } else {
                    quoted = !quoted;
                }
            } else {
                for _ in 0..count {
                    word.push('\\');
                }
            }
            started = true;
        } else if c == '"' {
            quoted = !quoted;
            started = true;
        } else if c.is_whitespace() && !quoted {
            if started {
                result.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(c);
            started = true;
        }
    }
    if started {
        result.push(word);
    }
    result
}
fn link(b: &[u8]) -> Option<(String, Vec<String>)> {
    const CLSID: [u8; 16] = [1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70];
    if u32le(b, 0)? != 76 || b.get(4..20)? != CLSID {
        return None;
    }
    let flags = u32le(b, 20)?;
    let mut p = 76usize;
    let mut target = None;
    if flags & 1 != 0 {
        p = p.checked_add(2 + u16le(b, p)? as usize)?;
    }
    if flags & 2 != 0 {
        let size = u32le(b, p)? as usize;
        if size < 28 {
            return None;
        }
        let block = b.get(p..p.checked_add(size)?)?;
        let header = u32le(block, 4)?;
        if header < 28 || header as usize > size {
            return None;
        }
        if u32le(block, 8)? & 1 != 0 {
            let wide = if header >= 36 {
                u32le(block, 28)? as usize
            } else {
                0
            };
            let offset = if wide != 0 {
                wide
            } else {
                u32le(block, 16)? as usize
            };
            if offset < header as usize {
                return None;
            }
            let base = string(block, offset, wide != 0)?;
            let suffix_wide = if header >= 36 {
                u32le(block, 32)? as usize
            } else {
                0
            };
            let suffix_offset = if suffix_wide != 0 {
                suffix_wide
            } else {
                u32le(block, 24)? as usize
            };
            let suffix = string(block, suffix_offset, suffix_wide != 0)?;
            target = Some(
                if suffix.is_empty() || base.to_lowercase().ends_with(&suffix.to_lowercase()) {
                    base
                } else {
                    format!(
                        "{}\\{}",
                        base.trim_end_matches('\\'),
                        suffix.trim_start_matches('\\')
                    )
                },
            );
        }
        p += size;
    }
    let mut relative = None;
    let mut working = None;
    let mut args = Vec::new();
    for flag in [4, 8, 16, 32, 64] {
        if flags & flag == 0 {
            continue;
        }
        let count = u16le(b, p)? as usize;
        p += 2;
        let wide = flags & 128 != 0;
        let bytes = count.checked_mul(if wide { 2 } else { 1 })?;
        let content = b.get(p..p.checked_add(bytes)?)?;
        p += bytes;
        let value = if wide {
            String::from_utf16(
                &content
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .ok()?
        } else {
            content.iter().map(|c| char::from(*c)).collect()
        };
        let value = value.trim_end_matches('\0').to_owned();
        match flag {
            8 => relative = Some(value),
            16 => working = Some(value),
            32 => args = arguments(&value),
            _ => (),
        }
    }
    if target.is_none() {
        if let (Some(relative), Some(working)) = (relative, working) {
            target = Some(
                if relative
                    .get(..3)
                    .is_some_and(|s| s.eq_ignore_ascii_case("c:\\"))
                {
                    relative
                } else {
                    format!("{working}\\{relative}")
                },
            );
        }
    }
    // EnvironmentVariableDataBlock covers many Wine-created shortcuts.
    while p + 8 <= b.len() {
        let size = u32le(b, p)? as usize;
        if size == 0 {
            break;
        }
        if size < 8 {
            return None;
        }
        let block = b.get(p..p.checked_add(size)?)?;
        if target.is_none() && u32le(block, 4)? == 0xa0000001 && size >= 788 {
            target = string(block, 268, true)
                .filter(|s| !s.is_empty())
                .or_else(|| string(block, 8, false));
        }
        p += size;
    }
    Some((target?, args))
}
fn resolve(prefix: &Path, value: &str) -> Option<PathBuf> {
    let value = value.trim();
    let windows = value.replace('/', "\\");
    if !windows.get(..3)?.eq_ignore_ascii_case("c:\\") || windows.contains('\0') {
        return None;
    }
    let root = prefix.join("drive_c").canonicalize().ok()?;
    let mut path = root.clone();
    for part in windows[3..].split('\\') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        let exact = path.join(part);
        path = if exact.exists() {
            exact
        } else {
            fs::read_dir(&path)
                .ok()?
                .filter_map(Result::ok)
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?
                .path()
        };
    }
    let path = path.canonicalize().ok()?;
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.contains("installer")
        || name.contains("updater")
        || name.contains("errorreport")
        || path
            .components()
            .any(|c| c.as_os_str().eq_ignore_ascii_case("Package Cache"))
        || !path.starts_with(&root)
        || path
            .strip_prefix(&root)
            .ok()?
            .components()
            .next()
            .is_some_and(|c| c.as_os_str().eq_ignore_ascii_case("windows"))
        || !name.ends_with(".exe")
        || [
            "unins",
            "uninstall",
            "setup",
            "crash",
            "update",
            "vcredist",
            "vc_redist",
            "dxsetup",
        ]
        .iter()
        .any(|s| name.starts_with(s))
    {
        return None;
    }
    let mut signature = [0; 2];
    fs::File::open(&path)
        .ok()?
        .read_exact(&mut signature)
        .ok()?;
    (signature == *b"MZ").then_some(path)
}
// Some self-updating launchers leave shortcuts pointing at a previous directory.
// Accept a relocated target only when a single version folder contains that exact suffix.
fn resolve_hint(prefix: &Path, value: &str) -> Option<PathBuf> {
    if let Some(path) = resolve(prefix, value) {
        return Some(path);
    }
    let windows = value.trim().replace('/', "\\");
    if !windows.get(..3)?.eq_ignore_ascii_case("c:\\") {
        return None;
    }
    let parts: Vec<_> = windows[3..].split('\\').collect();
    if parts.iter().any(|p| *p == "..") {
        return None;
    }
    let mut matches = std::collections::BTreeSet::new();
    // Restrict relocation to an application's directory under Program Files.
    if !parts
        .first()?
        .to_ascii_lowercase()
        .starts_with("program files")
    {
        return None;
    }
    for i in 2..parts.len().saturating_sub(1) {
        let parent = prefix.join("drive_c").join(parts[..i].join("/"));
        let Ok(entries) = fs::read_dir(parent) else {
            continue;
        };
        for entry in entries.take(64).flatten() {
            let version = entry.file_name().to_string_lossy().into_owned();
            if !version.starts_with(|c: char| c.is_ascii_digit())
                || !entry.file_type().is_ok_and(|t| t.is_dir())
            {
                continue;
            }
            let candidate = format!(
                "C:\\{}\\{}\\{}",
                parts[..i].join("\\"),
                version,
                parts[i..].join("\\")
            );
            if let Some(path) = resolve(prefix, &candidate) {
                matches.insert(path);
            }
        }
    }
    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    }
}

fn shortcuts(
    root: &Path,
    prefix: &Path,
    found: &mut BTreeMap<PathBuf, Target>,
    depth: usize,
    visited: &mut usize,
) {
    if depth > 12 || *visited >= 10_000 || !fs::symlink_metadata(root).is_ok_and(|m| m.is_dir()) {
        return;
    }
    let Some(canonical) = root.canonicalize().ok() else {
        return;
    };
    let Some(drive) = prefix.join("drive_c").canonicalize().ok() else {
        return;
    };
    if !canonical.starts_with(&drive) {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        *visited += 1;
        if *visited > 10_000 {
            break;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            shortcuts(&entry.path(), prefix, found, depth + 1, visited);
        } else if kind.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("lnk"))
        {
            if let Some((value, args)) = read(&entry.path(), 1_048_576).and_then(|b| link(&b)) {
                if let Some(path) = resolve_hint(prefix, &value) {
                    let label = entry.path().file_stem().map(|stem| stem.to_string_lossy().into_owned());
                    found.entry(path).or_insert(Target {
                        source: "Atalho do Windows".into(),
                        arguments: args,
                        label,
                    });
                }
            }
        }
    }
}
fn reg_string(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.starts_with('"') || !value.ends_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = value[1..value.len() - 1].chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.push(match chars.next()? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                c => c,
            });
        } else {
            out.push(c);
        }
    }
    Some(out)
}
/// A Windows shortcut of the prefix, as the shell would open it.
#[derive(Clone, Debug)]
pub struct Shortcut {
    /// The shortcut file itself (identity: a launcher adds one per game).
    pub file: PathBuf,
    /// Shortcut name ("Battlefield 2042").
    pub label: String,
    /// Program and arguments of a `.lnk` (Battle.net: its launcher with `--exec`).
    pub target: Option<PathBuf>,
    pub arguments: Vec<String>,
    /// Address of a `.url` handled by a launcher (`uplay://…`, `origin2://…`).
    pub url: Option<String>,
    /// Program whose icon represents the shortcut.
    pub icon: Option<PathBuf>,
}
/// `.url` shortcut: launcher address and icon (web pages are not games).
fn internet_shortcut(prefix: &Path, path: &Path) -> Option<(String, Option<PathBuf>)> {
    let text = String::from_utf8_lossy(&read(path, 65_536)?).into_owned();
    let field = |key: &str| text.lines().find_map(|line| line.trim().strip_prefix(key).map(|value| value.trim().to_string()));
    let url = field("URL=")?;
    let scheme = url.split_once(':')?.0.to_ascii_lowercase();
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
        || ["http", "https", "ftp", "mailto", "file", "javascript", "data"].contains(&scheme.as_str()) || url.len() > 2048 {
        return None;
    }
    Some((url, field("IconFile=").and_then(|icon| resolve(prefix, &icon))))
}
fn shortcut_files_in(root: &Path, prefix: &Path, found: &mut Vec<Shortcut>, depth: usize, visited: &mut usize) {
    if depth > 12 || *visited >= 10_000 || !fs::symlink_metadata(root).is_ok_and(|m| m.is_dir()) { return; }
    let (Some(canonical), Some(drive)) = (root.canonicalize().ok(), prefix.join("drive_c").canonicalize().ok()) else { return; };
    if !canonical.starts_with(&drive) { return; }
    let Ok(entries) = fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        *visited += 1;
        if *visited > 10_000 { break; }
        let Ok(kind) = entry.file_type() else { continue; };
        let path = entry.path();
        if kind.is_dir() { shortcut_files_in(&path, prefix, found, depth + 1, visited); continue; }
        if !kind.is_file() { continue; }
        let label = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let lower = label.to_lowercase();
        // Uninstallers, manuals and web pages are not something to play.
        if ["uninstall", "desinstalar", "readme", "manual", "website", "support", "suporte"].iter().any(|word| lower.contains(word)) { continue; }
        let extension = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if extension == "lnk" {
            let Some((value, arguments)) = read(&path, 1_048_576).and_then(|b| link(&b)) else { continue; };
            let Some(target) = resolve_hint(prefix, &value) else { continue; };
            found.push(Shortcut { file: path, label, icon: Some(target.clone()), target: Some(target), arguments, url: None });
        } else if extension == "url" {
            let Some((url, icon)) = internet_shortcut(prefix, &path) else { continue; };
            found.push(Shortcut { file: path, label, target: None, arguments: Vec::new(), url: Some(url), icon });
        }
    }
}
/// Games the Epic Games Launcher installed, from its install manifests (it
/// creates no Start Menu entry, and a desktop shortcut only on request). They
/// open through the launcher's address, as its own shortcuts do.
pub fn epic_games(prefix: &Path) -> Vec<Shortcut> {
    let folder = prefix.join("drive_c/ProgramData/Epic/EpicGamesLauncher/Data/Manifests");
    let Ok(entries) = fs::read_dir(&folder) else { return Vec::new(); };
    let encode = |text: &str| text.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)).then(|| text.to_string());
    let mut games = Vec::new();
    for entry in entries.flatten().take(2000) {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("item")) { continue; }
        let Some(value) = read(&path, 1_048_576).and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()) else { continue; };
        let text = |key: &str| value[key].as_str().map(str::trim).filter(|s| !s.is_empty());
        if value["bIsIncompleteInstall"].as_bool() == Some(true) { continue; }
        // Add-ons and tools without a program of their own are not games.
        let (Some(name), Some(app), Some(location), Some(program)) = (text("DisplayName"), text("AppName"), text("InstallLocation"), text("LaunchExecutable")) else { continue; };
        let Some(app) = encode(app) else { continue; };
        let id = match (text("CatalogNamespace").and_then(encode), text("CatalogItemId").and_then(encode)) {
            (Some(namespace), Some(item)) => format!("{namespace}%3A{item}%3A{app}"),
            _ => app,
        };
        let icon = resolve(prefix, &format!("{}\\{}", location.trim_end_matches(['\\', '/']), program.replace('/', "\\")));
        games.push(Shortcut {
            file: path, label: name.chars().take(120).collect(), target: None, arguments: Vec::new(),
            url: Some(format!("com.epicgames.launcher://apps/{id}?action=launch&silent=true")), icon,
        });
    }
    games
}
/// Desktop and Start Menu shortcuts of the prefix (all users).
pub fn shortcut_files(prefix: &Path) -> Vec<Shortcut> {
    let mut found = Vec::new();
    let mut visited = 0;
    if let Ok(entries) = fs::read_dir(prefix.join("drive_c/users")) {
        for user in entries.flatten().filter(|user| user.file_type().is_ok_and(|t| t.is_dir())) {
            for folder in ["Desktop", "Start Menu", "AppData/Roaming/Microsoft/Windows/Start Menu"] {
                shortcut_files_in(&user.path().join(folder), prefix, &mut found, 0, &mut visited);
            }
        }
    }
    shortcut_files_in(&prefix.join("drive_c/ProgramData/Microsoft/Windows/Start Menu"), prefix, &mut found, 0, &mut visited);
    found
}
pub fn discover(prefix: &Path) -> BTreeMap<PathBuf, Target> {
    let mut found = BTreeMap::new();
    let mut visited = 0;
    // Do not traverse Wine's links to the Linux desktop/home.
    let users = prefix.join("drive_c/users");
    if let Ok(entries) = fs::read_dir(&users) {
        for user in entries.flatten() {
            if !user.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            for folder in [
                "Desktop",
                "Start Menu",
                "AppData/Roaming/Microsoft/Windows/Start Menu",
            ] {
                shortcuts(
                    &user.path().join(folder),
                    prefix,
                    &mut found,
                    0,
                    &mut visited,
                );
            }
        }
    }
    shortcuts(
        &prefix.join("drive_c/ProgramData/Microsoft/Windows/Start Menu"),
        prefix,
        &mut found,
        0,
        &mut visited,
    );
    for file in ["system.reg", "user.reg"] {
        let Some(data) = read(&prefix.join(file), 16 * 1024 * 1024) else {
            continue;
        };
        let mut section = String::new();
        for line in String::from_utf8_lossy(&data).lines() {
            if line.starts_with('[') {
                section = line.to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let app_path = section.contains("\\\\app paths\\\\") && key == "@";
            let icon = section.contains("\\\\uninstall\\\\") && key == "\"displayicon\"";
            let launcher =
                key == "\"launcherappPath\"".to_ascii_lowercase() || key == "\"desktopapppath\"";
            if !app_path && !icon && !launcher {
                continue;
            }
            if let Some(mut value) = reg_string(value) {
                if icon {
                    if let Some((path, index)) = value.rsplit_once(',') {
                        if index.trim().parse::<i32>().is_ok() {
                            value = path.into();
                        }
                    }
                }
                let value = value.trim_matches('"');
                if let Some(path) = resolve_hint(prefix, value) {
                    found.entry(path).or_insert(Target {
                        source: if app_path {
                            "Registro: App Paths"
                        } else if launcher {
                            "Registro: launcher"
                        } else {
                            "Registro: ícone da instalação"
                        }
                        .into(),
                        arguments: Vec::new(),
                        label: None,
                    });
                }
            }
        }
    }
    found
}
