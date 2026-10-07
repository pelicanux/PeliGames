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
                    found.entry(path).or_insert(Target {
                        source: "Atalho do Windows".into(),
                        arguments: args,
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
                    });
                }
            }
        }
    }
    found
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_broken_links_and_parses_unicode_target_and_arguments() {
        assert!(link(&[0; 76]).is_none());
        let mut b = vec![0u8; 76];
        b[..4].copy_from_slice(&76u32.to_le_bytes());
        b[4..20].copy_from_slice(&[1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70]);
        b[20..24].copy_from_slice(&(8u32 | 16 | 32 | 128).to_le_bytes());
        for s in [
            "app.exe",
            "C:\\Games\\Example",
            "--name \"two words\" --literal $(touch /tmp/no)",
        ] {
            let s: Vec<_> = s.encode_utf16().collect();
            b.extend((s.len() as u16).to_le_bytes());
            for c in s {
                b.extend(c.to_le_bytes());
            }
        }
        let (target, args) = link(&b).unwrap();
        assert_eq!(target, "C:\\Games\\Example\\app.exe");
        assert_eq!(args[..3], ["--name", "two words", "--literal"]);
        for n in 0..b.len() {
            assert!(link(&b[..n]).is_none());
        }
    }
    fn link_info(target: &str) -> Vec<u8> {
        let mut bytes = vec![0; 76];
        bytes[..4].copy_from_slice(&76u32.to_le_bytes());
        bytes[4..20].copy_from_slice(&[1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70]);
        bytes[20..24].copy_from_slice(&2u32.to_le_bytes());
        let size = 28 + target.len() + 2;
        let mut block = vec![0; size];
        block[..4].copy_from_slice(&(size as u32).to_le_bytes());
        block[4..8].copy_from_slice(&28u32.to_le_bytes());
        block[8..12].copy_from_slice(&1u32.to_le_bytes());
        block[16..20].copy_from_slice(&28u32.to_le_bytes());
        block[24..28].copy_from_slice(&((size - 1) as u32).to_le_bytes());
        block[28..28 + target.len()].copy_from_slice(target.as_bytes());
        bytes.extend(block);
        bytes
    }
    #[test]
    fn prioritizes_real_shortcuts_and_recovers_only_unambiguous_versioned_targets() {
        let root = std::env::temp_dir().join(format!("peli-links-{}", uuid::Uuid::new_v4()));
        let app = root.join("drive_c/Program Files/Vendor/App/2.0/App");
        fs::create_dir_all(&app).unwrap();
        fs::write(app.join("Main.exe"), b"MZfixture").unwrap();
        let desktop = root.join("drive_c/users/Public/Desktop");
        fs::create_dir_all(&desktop).unwrap();
        let target = "C:\\Program Files\\Vendor\\App\\App\\Main.exe";
        fs::write(desktop.join("App.lnk"), link_info(target)).unwrap();
        let hints = discover(&root);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints.values().next().unwrap().source, "Atalho do Windows");
        assert_eq!(hints.keys().next().unwrap(), &app.join("Main.exe"));
        assert!(resolve(&root, target).is_none());
        let other = root.join("drive_c/Program Files/Vendor/App/3.0/App");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("Main.exe"), b"MZfixture").unwrap();
        assert!(resolve_hint(&root, target).is_none());
        fs::write(app.join("VC_redist.exe"), b"MZfixture").unwrap();
        assert!(resolve(
            &root,
            "C:\\Program Files\\Vendor\\App\\2.0\\App\\VC_redist.exe"
        )
        .is_none());
        let bytes = link_info(target);
        assert_eq!(link(&bytes).unwrap().0, target);
        let mut broken = bytes;
        broken[92..96].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(link(&broken).is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn discovers_registry_and_shortcuts_without_following_external_paths() {
        let root = std::env::temp_dir().join(format!("peli-targets-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("drive_c/Games")).unwrap();
        fs::write(root.join("drive_c/Games/App.exe"), b"MZtest").unwrap();
        fs::write(root.join("system.reg"),"[Software\\\\Microsoft\\\\Windows\\\\CurrentVersion\\\\App Paths\\\\App.exe]\n@=\"C:\\\\Games\\\\app.exe\"\n").unwrap();
        let found = discover(&root);
        assert_eq!(found.len(), 1);
        assert!(found.values().next().unwrap().source.contains("App Paths"));
        assert!(resolve(&root, "C:\\..\\outside.exe").is_none());
        assert!(resolve(&root, "Z:\\tmp\\app.exe").is_none());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/bin/ls", root.join("drive_c/Games/escaped.exe")).unwrap();
            assert!(resolve(&root, "C:\\Games\\escaped.exe").is_none());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
