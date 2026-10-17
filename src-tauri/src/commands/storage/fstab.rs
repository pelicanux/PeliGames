//! Only supported data mounts are editable. System entries and user comments
//! stay byte-for-byte intact. The privileged destination is always /etc/fstab.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

pub(crate) const MARKER: &str = "x-peligames";
pub(crate) const HEADER: &str = "# PeliGames - montagens gerenciadas pelo launcher";
const WRITER: &str = include_str!("fstab_write.sh");
#[derive(Serialize)]
pub(crate) struct Row {
    key: String,
    pub source: String,
    pub mount_path: String,
    pub line: String,
    managed: bool,
    automatic: bool,
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn fields(line: &str) -> Option<Vec<&str>> {
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() < 6 || fields[0].starts_with('#') {
        return None;
    }
    Some(fields)
}
fn decode(value: &str) -> String {
    // fstab escaping, not shell escaping. Decode backslash last.
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\134", "\\")
}
fn data_mount(fields: &[&str]) -> bool {
    let target = decode(fields[1]);
    let source = fields[0];
    target.starts_with("/mnt/")
        && target.len() > 5
        && !target.split('/').any(|part| matches!(part, "." | ".."))
        && matches!(fields[2], "ext4" | "btrfs" | "ntfs" | "ntfs3" | "auto")
        && ["UUID=", "LABEL=", "PARTUUID=", "/dev/"]
            .iter()
            .any(|prefix| source.starts_with(prefix))
        && !fields[3].split(',').any(|option| {
            matches!(option, "bind" | "rbind" | "swap") || option.starts_with("x-systemd.automount")
        })
}
pub(crate) fn rows(text: &str) -> Vec<Row> {
    let revision = digest(text);
    text.split_inclusive('\n')
        .enumerate()
        .filter_map(|(index, line)| {
            let parts = fields(line)?;
            if !data_mount(&parts) {
                return None;
            }
            Some(Row {
                key: digest(&format!("{revision}:{index}:{line}")),
                source: decode(parts[0]),
                mount_path: decode(parts[1]),
                line: line.trim_end_matches(['\r', '\n']).into(),
                managed: parts[3].split(',').any(|o| o == MARKER),
                automatic: !parts[3].split(',').any(|o| o == "noauto"),
            })
        })
        .collect()
}
pub(crate) fn headers(text: &str) -> String {
    let mut lines: Vec<&str> = text
        .split_inclusive('\n')
        .filter(|line| line.trim_end_matches(['\r', '\n']) != HEADER)
        .collect();
    if let Some(index) = lines
        .iter()
        .position(|line| fields(line).is_some_and(|f| f[3].split(',').any(|o| o == MARKER)))
    {
        lines.insert(
            index,
            concat!("# PeliGames - montagens gerenciadas pelo launcher", "\n"),
        );
    }
    lines.concat()
}
pub(crate) fn without(text: &str, key: &str) -> Result<String, String> {
    if !rows(text).iter().any(|row| row.key == key) {
        return Err(
            "A entrada do fstab mudou ou não é uma montagem de dados suportada. Atualize a lista."
                .into(),
        );
    }
    let revision = digest(text);
    let next: String = text
        .split_inclusive('\n')
        .enumerate()
        .filter(|(index, line)| digest(&format!("{revision}:{index}:{line}")) != key)
        .map(|(_, line)| line)
        .collect();
    Ok(headers(&next))
}
pub(crate) fn read() -> Result<String, String> {
    fs::read_to_string("/etc/fstab").map_err(|e| format!("Não foi possível ler /etc/fstab: {e}"))
}
fn write_authorized(previous: &str, next: &str, reload: bool) -> Result<(), String> {
    if previous == next {
        return Ok(());
    }
    let root = unsafe { libc::geteuid() } == 0;
    let mut command = Command::new(if root { "/bin/sh" } else { "/usr/bin/pkexec" });
    crate::core::env_utils::host_environment(&mut command);
    if !root {
        command.arg("/bin/sh");
    }
    let mut child = command
        .args([
            "-c",
            WRITER,
            "peligames-fstab",
            if reload { "reload" } else { "write" },
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Não foi possível solicitar autorização de administrador: {e}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("Canal do assistente indisponível.")?;
    let sent = writeln!(stdin, "{}\n{}", digest(previous), digest(next))
        .and_then(|_| stdin.write_all(next.as_bytes()));
    drop(stdin);
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.code() == Some(126) {
        return Err("Autorização cancelada. Nada foi alterado no fstab.".into());
    }
    if !output.status.success() {
        return Err(format!(
            "Não foi possível atualizar /etc/fstab: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    sent.map_err(|e| e.to_string())
}
pub(crate) fn sync_headers() -> Result<(), String> {
    let previous = read()?;
    write_authorized(&previous, &headers(&previous), false)
}
pub(crate) fn remove(key: &str) -> Result<(), String> {
    let previous = read()?;
    write_authorized(&previous, &without(&previous, key)?, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SYSTEM: &str =
        "# user comment\nUUID=root / btrfs defaults 0 0\nUUID=home /home ext4 defaults 0 2\n";
    const A: &str = "UUID=aaa /mnt/Games ext4 defaults,nofail,x-peligames 0 2\n";
    const B: &str = "UUID=bbb /mnt/Other btrfs noauto,nofail,x-peligames 0 0\n";
    #[test]
    fn only_data_mounts_are_listed_even_when_the_disk_is_disconnected() {
        let text = format!("{SYSTEM}{A}{B}LABEL=Manual /mnt/Manual ext4 defaults 0 2\n");
        let listed = rows(&text);
        assert_eq!(listed.len(), 3);
        assert!(listed[0].managed && listed[0].automatic);
        assert!(listed[1].managed && !listed[1].automatic);
        assert!(!listed[2].managed);
        assert!(without(&text, &digest("1:UUID=root / btrfs defaults 0 0\n")).is_err());
    }
    #[test]
    fn comment_is_unique_and_removed_only_after_the_last_managed_entry() {
        let text = headers(&format!("{SYSTEM}{A}{B}"));
        assert_eq!(headers(&text), text);
        assert_eq!(text.matches(HEADER).count(), 1);
        let next = without(&text, &rows(&text)[0].key).unwrap();
        assert!(next.starts_with(SYSTEM));
        assert!(next.contains(HEADER) && next.contains(B));
        let next = without(&next, &rows(&next)[0].key).unwrap();
        assert_eq!(next, SYSTEM);
    }
    #[test]
    fn stale_selection_is_rejected_and_duplicates_are_removed_one_at_a_time() {
        let text = format!("{SYSTEM}{A}{A}");
        let listed = rows(&text);
        assert_ne!(listed[0].key, listed[1].key);
        let next = without(&text, &listed[1].key).unwrap();
        assert_eq!(next.matches(A).count(), 1);
        assert!(without(&format!("# edited\n{text}"), &listed[1].key).is_err());
    }
    #[test]
    fn preserves_escaped_paths_and_does_not_list_bind_or_system_paths() {
        let text = "UUID=a /mnt/My\\040Games ext4 defaults 0 2\nUUID=b /mnt/../home ext4 defaults 0 2\n/dev/sdb1 /mnt/Bind ext4 bind 0 0\n";
        let listed = rows(text);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].mount_path, "/mnt/My Games");
        assert_eq!(listed[0].line, text.lines().next().unwrap());
    }
    #[test]
    fn atomic_writer_keeps_a_backup_and_rejects_concurrent_edits() {
        let dir = std::env::temp_dir().join(format!("peligames-fstab-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("fstab");
        fs::write(&path, SYSTEM).unwrap();
        let script = WRITER.replace("fstab='/etc/fstab'", &format!("fstab='{}'", path.display()));
        let write = |expected: &str, complete: &str, transmitted: &str| {
            let mut child = Command::new("/bin/sh")
                .args(["-c", &script, "test", "write"])
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let mut input = child.stdin.take().unwrap();
            writeln!(input, "{}\n{}", digest(expected), digest(complete)).unwrap();
            input.write_all(transmitted.as_bytes()).unwrap();
            drop(input);
            child.wait_with_output().unwrap()
        };
        let next = format!("{SYSTEM}{A}");
        assert_eq!(write(SYSTEM, &next, "incomplete").status.code(), Some(21));
        assert_eq!(fs::read_to_string(&path).unwrap(), SYSTEM);
        assert!(write(SYSTEM, &next, &next).status.success());
        assert_eq!(fs::read_to_string(&path).unwrap(), next);
        assert_eq!(
            fs::read_to_string(dir.join("fstab.peligames.bak")).unwrap(),
            SYSTEM
        );
        assert_eq!(write(SYSTEM, "bad", "bad").status.code(), Some(20));
        assert_eq!(fs::read_to_string(&path).unwrap(), next);
        fs::remove_dir_all(dir).unwrap();
    }
}
