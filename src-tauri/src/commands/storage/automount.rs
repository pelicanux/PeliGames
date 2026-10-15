//! Original PeliGames implementation using the public UDisks2 D-Bus API.
//!
//! The UDisks2 object tree is read as JSON (`Objects`: object path →
//! interface → property) and every change goes through UDisks2 itself, which
//! asks polkit for authorization. Only fixed programs run through `pkexec`
//! (`mkdir` for the mount folder, `systemctl daemon-reload`). The checks are
//! deliberately strict: system, recovery and encrypted volumes are refused,
//! and every destructive step re-reads the device and stops if it changed.
use serde_json::{json, Map, Value};

pub(crate) const U: &str = "org.freedesktop.UDisks2";
pub(crate) const B: &str = "org.freedesktop.UDisks2.Block";
pub(crate) const F: &str = "org.freedesktop.UDisks2.Filesystem";
pub(crate) const P: &str = "org.freedesktop.UDisks2.Partition";
pub(crate) const T: &str = "org.freedesktop.UDisks2.PartitionTable";
const JOB: &str = "org.freedesktop.UDisks2.Job";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
const MIB: i128 = 1024 * 1024;
const MIN_FREE_SPACE: i128 = 64 * MIB;
/// Generic data partitions only; EFI, recovery, reserved and discoverable
/// root/home/boot partition types must stay protected even while unmounted.
const DATA_PARTITION_TYPES: [&str; 2] = ["ebd0a0a2-b9e5-4433-87c0-68b6b72699c7", "0fc63daf-8483-4772-8e79-3d69d8477de4"];
const LINUX_DATA: &str = "0fc63daf-8483-4772-8e79-3d69d8477de4";

pub(crate) type Objects = Map<String, Value>;

/// One fstab configuration item of a block device: `(type, details)`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EntryValue {
    Bytes(Vec<u8>),
    Int(i32),
}
pub(crate) type Entry = (String, Vec<(String, EntryValue)>);

/// A UDisks2 method call on one object.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Call {
    Unmount,
    Mount,
    SetLabel(String),
    SetName(String),
    AddConfiguration(Entry),
    UpdateConfiguration(Entry, Entry),
    RemoveConfiguration(Entry),
    /// `label` is set when creating a filesystem; a partition table has none.
    Format { kind: String, label: Option<String> },
    CanFormat(String),
    CreatePartition { offset: u64, size: u64, kind: String, name: String, uuid: String },
}
#[cfg(test)]
impl Call {
    pub(crate) fn method(&self) -> &'static str {
        match self {
            Call::Unmount => "Unmount",
            Call::Mount => "Mount",
            Call::SetLabel(_) => "SetLabel",
            Call::SetName(_) => "SetName",
            Call::AddConfiguration(_) => "AddConfigurationItem",
            Call::UpdateConfiguration(..) => "UpdateConfigurationItem",
            Call::RemoveConfiguration(_) => "RemoveConfigurationItem",
            Call::Format { .. } => "Format",
            Call::CanFormat(_) => "CanFormat",
            Call::CreatePartition { .. } => "CreatePartition",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Reply {
    Unit,
    /// Mount point (Mount) or object path (CreatePartition).
    Path(String),
    CanFormat(bool, String),
}

/// State of a path on the local filesystem.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PathState {
    /// The path itself exists, even as a dangling link (`lexists`).
    pub present: bool,
    pub symlink: bool,
    /// It is a directory (following links).
    pub dir: bool,
    /// A directory without entries.
    pub empty: bool,
    /// Resolving it yields the same path (no links on the way).
    pub canonical: bool,
}

/// Everything the automount logic needs from the system; tests replace it.
pub(crate) trait System {
    fn objects(&mut self) -> Result<Objects, String>;
    fn call(&mut self, path: &str, call: Call) -> Result<Reply, String>;
    /// `pkexec mkdir -m 0755 -- target`.
    fn make_mount_dir(&mut self, target: &str) -> Result<(), String>;
    /// `pkexec systemctl daemon-reload`.
    fn reload_systemd(&mut self) -> Result<(), String>;
    fn fstab(&mut self) -> Result<String, String>;
    fn path(&mut self, path: &str) -> PathState;
    /// Waits briefly for udev/UDisks2 to publish new metadata.
    fn pause(&mut self);
}

fn iface<'a>(obj: &'a Value, name: &str) -> &'a Value {
    obj.get(name).unwrap_or(&Value::Null)
}
fn has(obj: &Value, name: &str) -> bool {
    obj.get(name).is_some_and(Value::is_object)
}
fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
/// A D-Bus object path property, with UDisks2's "/" meaning none.
fn object_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("/")
}
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}
/// Byte strings (`ay`) end with a NUL; JSON arrays of bytes or plain strings.
pub(crate) fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.trim_end_matches('\0').to_string(),
        Value::Array(items) => {
            let mut bytes: Vec<u8> = items.iter().filter_map(Value::as_u64).map(|b| b as u8).collect();
            while bytes.last() == Some(&0) {
                bytes.pop();
            }
            String::from_utf8_lossy(&bytes).into_owned()
        }
        _ => String::new(),
    }
}
fn texts(value: &Value) -> Vec<String> {
    value.as_array().map(|items| items.iter().map(text).collect()).unwrap_or_default()
}
fn mounts(obj: &Value) -> Vec<String> {
    texts(&iface(obj, F)["MountPoints"])
}
fn fstab_items(block: &Value) -> Vec<&Value> {
    block["Configuration"].as_array().map(|items| items.iter().filter(|entry| entry[0] == "fstab").collect()).unwrap_or_default()
}
fn entry_text(entry: &Value, key: &str) -> String {
    text(&entry[1][key])
}
fn removable_place(path: &str) -> bool {
    ["/mnt/", "/media/", "/run/media/"].iter().any(|prefix| path.starts_with(prefix))
}
fn partition_type(p: &Value) -> String {
    str_of(p, "Type").to_lowercase()
}
fn table_type<'a>(all: &'a Objects, p: &Value) -> &'a str {
    all.get(object_of(p, "Table")).map(|table| str_of(iface(table, T), "Type")).unwrap_or("")
}
fn flags(p: &Value) -> u64 {
    p["Flags"].as_u64().unwrap_or(0)
}
fn obj<'a>(all: &'a Objects, path: &str) -> &'a Value {
    all.get(path).unwrap_or(&Value::Null)
}
fn jobs_touch(all: &Objects, paths: &[&str]) -> bool {
    all.values().any(|value| iface(value, JOB)["Objects"].as_array().is_some_and(|objects| objects.iter().any(|o| o.as_str().is_some_and(|o| paths.contains(&o)))))
}

fn system_partition(path: &str, all: &Objects) -> bool {
    let o = obj(all, path);
    let (b, p) = (iface(o, B), iface(o, P));
    let uuid = str_of(b, "IdUUID");
    // Btrfs subvolumes can expose the same filesystem through multiple objects.
    let peers = std::iter::once(o).chain(all.iter().filter(|(key, other)| !uuid.is_empty() && key.as_str() != path && str_of(iface(other, B), "IdUUID") == uuid).map(|(_, other)| other));
    for other in peers {
        let configured = fstab_items(iface(other, B)).into_iter().map(|entry| entry_text(entry, "dir"));
        if mounts(other).into_iter().chain(configured).any(|place| !removable_place(&place)) {
            return true;
        }
    }
    if table_type(all, p) == "gpt" && !str_of(p, "Type").is_empty() && !DATA_PARTITION_TYPES.contains(&partition_type(p).as_str()) {
        return true;
    }
    truthy(b.get("HintIgnore")) || truthy(p.get("IsContainer")) || flags(p) & 1 != 0
}

fn storage_candidate(path: &str, all: &Objects) -> bool {
    let o = obj(all, path);
    let b = iface(o, B);
    if !has(o, B) || system_partition(path, all) {
        return false;
    }
    let (usage, kind) = (str_of(b, "IdUsage"), str_of(b, "IdType"));
    if has(o, F) && usage == "filesystem" && !kind.is_empty() {
        return matches!(kind, "ext4" | "btrfs" | "ntfs" | "ntfs3");
    }
    // Unformatted physical partitions/disks have no Filesystem interface or UUID.
    // A whole disk with children is represented by its partitions, never twice.
    if !matches!(usage, "" | "filesystem" | "partitiontable") || !matches!(kind, "" | "gpt" | "dos") {
        return false;
    }
    if object_of(b, "Drive") == "/" || object_of(b, "CryptoBackingDevice") != "/" {
        return false;
    }
    !all.values().any(|other| object_of(iface(other, P), "Table") == path && has(other, P))
}

fn reason(path: &str, all: &Objects, allow_ntfs: bool) -> String {
    let o = obj(all, path);
    let (b, p) = (iface(o, B), iface(o, P));
    if system_partition(path, all) {
        return "Partição usada ou reservada pelo sistema; alterações bloqueadas.".into();
    }
    let uuid = str_of(b, "IdUUID");
    let same: Vec<&str> = if uuid.is_empty() { Vec::new() } else { all.iter().filter(|(_, value)| str_of(iface(value, B), "IdUUID") == uuid).map(|(key, _)| key.as_str()).collect() };
    if jobs_touch(all, &same) {
        return "Há uma operação em andamento neste dispositivo. Aguarde a conclusão e atualize a lista.".into();
    }
    let configured = fstab_items(b).into_iter().map(|entry| entry_text(entry, "dir"));
    if mounts(o).into_iter().chain(configured).any(|place| !removable_place(&place)) {
        return "Partição usada pelo sistema; alterações bloqueadas.".into();
    }
    if truthy(b.get("ReadOnly")) || truthy(b.get("HintIgnore")) || truthy(p.get("IsContainer")) || flags(p) & 1 != 0 {
        return "Partição protegida ou somente leitura.".into();
    }
    let kind = str_of(b, "IdType");
    if kind.is_empty() || str_of(b, "IdUsage") == "partitiontable" {
        return if has(o, P) {
            "Nenhum sistema de arquivos detectado. Formate a partição em ext4 ou Btrfs antes de configurar a automontagem."
        } else {
            "Disco sem partições utilizáveis. Crie uma partição e um sistema de arquivos antes de configurar a automontagem."
        }
        .into();
    }
    let supported = if allow_ntfs { &["ext4", "btrfs", "ntfs", "ntfs3"][..] } else { &["ext4", "btrfs"][..] };
    if !supported.contains(&kind) {
        return "Nesta versão, a automontagem está disponível apenas para ext4 e Btrfs.".into();
    }
    if !has(o, P) || table_type(all, p) != "gpt" {
        return "É necessária uma partição GPT para aplicar o mesmo nome à partição e ao label.".into();
    }
    if object_of(b, "CryptoBackingDevice") != "/" || object_of(b, "Drive") == "/" {
        return "Volumes criptografados, virtuais ou compostos não são suportados nesta versão.".into();
    }
    let special = fstab_items(b).into_iter().any(|entry| entry_text(entry, "opts").split(',').any(|option| option == "bind" || option == "rbind" || option.starts_with("x-systemd.automount")));
    if special {
        return "Montagem especial; use o gerenciador de discos do sistema.".into();
    }
    if uuid.is_empty() || fstab_items(b).len() > 1 {
        return "Configuração complexa ou UUID ausente; use o gerenciador de discos do sistema.".into();
    }
    String::new()
}

fn describe(path: &str, all: &Objects) -> Value {
    let o = obj(all, path);
    let (b, p) = (iface(o, B), iface(o, P));
    let entries = fstab_items(b);
    let opts: Vec<String> = entries.first().map(|entry| entry_text(entry, "opts").split(',').map(str::to_string).collect()).unwrap_or_default();
    let operations = reason(path, all, true);
    let format_blocked = if !operations.is_empty() {
        operations.clone()
    } else if DATA_PARTITION_TYPES.contains(&partition_type(p).as_str()) {
        String::new()
    } else {
        "Partição de sistema, recuperação ou tipo não suportado para formatação.".into()
    };
    json!({
        "id": path, "uuid": str_of(b, "IdUUID"), "device": text(&b["Device"]),
        "label": str_of(b, "IdLabel"), "partition_name": str_of(p, "Name"),
        "filesystem": if str_of(b, "IdUsage") == "filesystem" { str_of(b, "IdType") } else { "" },
        "size": b["Size"].as_u64().unwrap_or(0), "mounts": mounts(o),
        "mount_at_startup": !entries.is_empty() && !opts.iter().any(|o| o == "noauto"),
        "configured_path": entries.first().map(|entry| entry_text(entry, "dir")).unwrap_or_default(),
        "blocked_reason": reason(path, all, false),
        "operations_blocked_reason": operations,
        "format_blocked_reason": format_blocked,
    })
}

/// The fstab item in the form UDisks2 expects (`ay` strings, `i` numbers).
fn entry_of(entry: &Value) -> Entry {
    let mut details = Vec::new();
    for key in ["fsname", "dir", "type", "opts", "freq", "passno"] {
        let Some(value) = entry[1].get(key) else { continue };
        if matches!(key, "freq" | "passno") {
            details.push((key.to_string(), EntryValue::Int(value.as_i64().unwrap_or(0) as i32)));
        } else {
            let mut bytes: Vec<u8> = match value {
                Value::String(s) => s.as_bytes().to_vec(),
                _ => value.as_array().map(|items| items.iter().filter_map(Value::as_u64).map(|b| b as u8).collect()).unwrap_or_default(),
            };
            if bytes.last() != Some(&0) {
                bytes.push(0);
            }
            details.push((key.to_string(), EntryValue::Bytes(bytes)));
        }
    }
    (entry[0].as_str().unwrap_or("fstab").to_string(), details)
}
fn bytes(value: &str) -> EntryValue {
    let mut bytes = value.as_bytes().to_vec();
    bytes.push(0);
    EntryValue::Bytes(bytes)
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && name.len() <= 16
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}
fn request_str<'a>(request: &'a Value, key: &str) -> &'a str {
    str_of(request, key)
}
fn fail(message: impl Into<String>) -> Result<Value, String> {
    Err(message.into())
}

fn fstab_targets(fstab: &str) -> impl Iterator<Item = (String, String)> + '_ {
    fstab.lines().filter_map(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        (fields.len() >= 2 && !fields[0].starts_with('#')).then(|| (fields[0].to_string(), fields[1].to_string()))
    })
}

struct Prepared {
    path: String,
    obj: Value,
    target: String,
    old_mounts: Vec<String>,
}
fn preflight(request: &Value, allow_ntfs: bool, sys: &mut dyn System) -> Result<Prepared, String> {
    let name = request_str(request, "name");
    if !valid_name(name) {
        return Err("Use de 1 a 16 caracteres: letras sem acentos, números, ponto, hífen ou sublinhado.".into());
    }
    let all = sys.objects()?;
    let path = request_str(request, "id").to_string();
    let o = obj(&all, &path).clone();
    if o.is_null() || str_of(iface(&o, B), "IdUUID") != request_str(request, "uuid") {
        return Err("O dispositivo mudou. Atualize a lista e selecione novamente.".into());
    }
    let blocked = reason(&path, &all, allow_ntfs);
    if !blocked.is_empty() {
        return Err(blocked);
    }
    let b = iface(&o, B);
    let uuid = str_of(b, "IdUUID");
    let target = format!("/mnt/{name}");
    // Refuse collisions before any authorized operation; UUID avoids duplicate-label ambiguity.
    for (other_path, other) in &all {
        if *other_path == path {
            continue;
        }
        if str_of(iface(other, B), "IdUUID") == uuid {
            return Err("UUID duplicado; configuração bloqueada.".into());
        }
        if mounts(other).contains(&target) || fstab_items(iface(other, B)).into_iter().any(|entry| entry_text(entry, "dir") == target) {
            return Err("O ponto de montagem já pertence a outro dispositivo.".into());
        }
    }
    let own = [format!("UUID={uuid}"), text(&b["Device"]), format!("LABEL={}", str_of(b, "IdLabel")), format!("PARTUUID={}", str_of(iface(&o, P), "UUID")), format!("/dev/disk/by-uuid/{uuid}")];
    if fstab_targets(&sys.fstab()?).any(|(source, dir)| dir == target && !own.contains(&source)) {
        return Err("Destino já reservado em /etc/fstab para outro dispositivo.".into());
    }
    let mnt = sys.path("/mnt");
    if mnt.symlink || !mnt.dir {
        return Err("/mnt precisa ser um diretório real do sistema.".into());
    }
    let place = sys.path(&target);
    if place.present && (place.symlink || !place.dir) {
        return Err("Destino inválido: link simbólico ou arquivo existente.".into());
    }
    let old_mounts = mounts(&o);
    if place.dir && !old_mounts.contains(&target) && !place.empty {
        return Err("A pasta de destino não está vazia.".into());
    }
    Ok(Prepared { path, obj: o, target, old_mounts })
}

fn steps(stages: &[&str]) -> String {
    if stages.is_empty() { "nenhuma".into() } else { stages.join(", ") }
}

pub(crate) fn save(request: &Value, sys: &mut dyn System) -> Result<Value, String> {
    let Prepared { path, obj: o, target, old_mounts } = preflight(request, false, sys)?;
    let name = request_str(request, "name").to_string();
    let b = iface(&o, B).clone();
    let mut stages: Vec<&str> = Vec::new();
    let result = (|| -> Result<Value, String> {
        if !sys.path(&target).present {
            sys.make_mount_dir(&target)?;
            stages.push("pasta criada");
        }
        if !old_mounts.is_empty() {
            sys.call(&path, Call::Unmount)?;
            stages.push("volume desmontado");
        }
        let refreshed = sys.objects()?;
        if str_of(iface(obj(&refreshed, &path), B), "IdUUID") != request_str(request, "uuid") || !reason(&path, &refreshed, false).is_empty() {
            return Err("O estado do dispositivo mudou; operação interrompida.".into());
        }
        if str_of(&b, "IdLabel") != name {
            sys.call(&path, Call::SetLabel(name.clone()))?;
            stages.push("label alterado");
        }
        if str_of(iface(&o, P), "Name") != name {
            sys.call(&path, Call::SetName(name.clone()))?;
            stages.push("nome da partição alterado");
        }
        let entries = fstab_items(&b);
        let old: Vec<String> = entries.first().map(|entry| entry_text(entry, "opts").split(',').map(str::to_string).collect()).unwrap_or_else(|| vec!["defaults".into()]);
        let mut opts: Vec<String> = old.into_iter().filter(|x| !x.is_empty() && !matches!(x.as_str(), "auto" | "noauto" | "nofail") && !x.starts_with("x-systemd.device-timeout=")).collect();
        opts.extend(["nofail".to_string(), "x-systemd.device-timeout=10s".to_string()]);
        if !request["mount_at_startup"].as_bool().unwrap_or(false) {
            opts.push("noauto".into());
        }
        let kind = str_of(&b, "IdType");
        let new: Entry = ("fstab".into(), vec![
            ("fsname".into(), bytes(&format!("UUID={}", str_of(&b, "IdUUID")))),
            ("dir".into(), bytes(&target)),
            ("type".into(), bytes(kind)),
            ("opts".into(), bytes(&opts.join(","))),
            ("freq".into(), EntryValue::Int(0)),
            ("passno".into(), EntryValue::Int(if kind == "ext4" { 2 } else { 0 })),
        ]);
        match entries.first() {
            Some(entry) => sys.call(&path, Call::UpdateConfiguration(entry_of(entry), new))?,
            None => sys.call(&path, Call::AddConfiguration(new))?,
        };
        stages.push("automontagem salva");
        // Reload generated mount units before mounting with the new fstab configuration.
        // Fixed argv and polkit authorization; no service restart and no mount -a.
        sys.reload_systemd()?;
        stages.push("configuração do systemd recarregada");
        let mounted = match sys.call(&path, Call::Mount)? { Reply::Path(path) => path, _ => String::new() };
        if mounted != target {
            return Err("Montagem retornou um caminho diferente do solicitado.".into());
        }
        Ok(json!({"mount_path": mounted}))
    })();
    result.map_err(|error| format!("Não foi possível concluir. Etapas concluídas: {}. {error}", steps(&stages)))
}

/// Stable identification of a device: changes mean another device took its place.
fn identity(o: &Value) -> Value {
    let (b, p) = (iface(o, B), iface(o, P));
    json!([text(&b["Device"]), b.get("DeviceNumber"), b.get("Drive"), b.get("Size"), p.get("UUID"), p.get("Table")])
}

pub(crate) fn storage_operation(request: &Value, sys: &mut dyn System) -> Result<Value, String> {
    let action = request_str(request, "operation").to_string();
    if !matches!(action.as_str(), "unmount" | "remove_automount" | "remount" | "mount") {
        return fail("Operação de armazenamento inválida.");
    }
    let all = sys.objects()?;
    let path = request_str(request, "id").to_string();
    let uuid = request_str(request, "uuid").to_string();
    let o = obj(&all, &path).clone();
    if str_of(iface(&o, B), "IdUUID") != uuid {
        return fail("O dispositivo mudou. Atualize a lista antes de continuar.");
    }
    let blocked = reason(&path, &all, true);
    if !blocked.is_empty() {
        return Err(blocked);
    }
    if all.values().filter(|value| str_of(iface(value, B), "IdUUID") == uuid).count() != 1 {
        return fail("UUID duplicado; operação bloqueada.");
    }
    let original = identity(&o);
    let entries: Vec<Value> = fstab_items(iface(&o, B)).into_iter().cloned().collect();
    let mut stages: Vec<&str> = Vec::new();
    let revalidate = |sys: &mut dyn System| -> Result<Value, String> {
        let current_objects = sys.objects()?;
        let current = obj(&current_objects, &path).clone();
        if str_of(iface(&current, B), "IdUUID") != uuid || identity(&current) != original {
            return Err("A identificação do dispositivo mudou; operação interrompida.".into());
        }
        let blocked = reason(&path, &current_objects, true);
        if !blocked.is_empty() {
            return Err(blocked);
        }
        Ok(current)
    };
    let current_entries = |current: &Value| -> Vec<Value> { fstab_items(iface(current, B)).into_iter().cloned().collect() };
    let result = (|| -> Result<Value, String> {
        if action == "remove_automount" {
            if entries.is_empty() {
                return Err("Esta partição não possui uma entrada de automontagem em /etc/fstab.".into());
            }
            let current = revalidate(sys)?;
            if current_entries(&current) != entries {
                return Err("A configuração de montagem mudou; atualize a lista.".into());
            }
            sys.call(&path, Call::RemoveConfiguration(entry_of(&entries[0])))?;
            stages.push("entrada de automontagem removida");
            sys.reload_systemd()?;
            stages.push("configuração do systemd recarregada");
            return Ok(json!({"operation": action}));
        }
        let mut target = String::new();
        if action == "remount" || action == "mount" {
            if entries.len() != 1 || !entry_text(&entries[0], "dir").starts_with("/mnt/") {
                return Err("Configure primeiro a automontagem desta partição em /mnt.".into());
            }
            target = entry_text(&entries[0], "dir");
            let place = sys.path(&target);
            if !place.canonical || !place.dir {
                return Err("Destino de montagem inválido ou pasta ausente. Configure novamente a automontagem.".into());
            }
        }
        let current = revalidate(sys)?;
        let mounted_now = !mounts(&current).is_empty();
        if action == "mount" && mounted_now {
            return Err("A partição já está montada. Atualize a lista.".into());
        }
        if mounted_now {
            sys.call(&path, Call::Unmount)?;
            stages.push("partição desmontada sem força");
        } else if action == "unmount" {
            return Err("A partição já está desmontada.".into());
        }
        if action == "remount" || action == "mount" {
            let current = revalidate(sys)?;
            if current_entries(&current) != entries || !mounts(&current).is_empty() {
                return Err("A configuração ou o estado de montagem mudou; operação interrompida.".into());
            }
            sys.reload_systemd()?;
            revalidate(sys)?;
            let mounted = match sys.call(&path, Call::Mount)? { Reply::Path(path) => path, _ => String::new() };
            if mounted != target {
                return Err("O caminho montado difere do configurado.".into());
            }
            return Ok(json!({"operation": action, "mount_path": mounted}));
        }
        Ok(json!({"operation": action}))
    })();
    result.map_err(|error| format!("Não foi possível concluir. Etapas concluídas: {}. {error}", steps(&stages)))
}

fn can_format(sys: &mut dyn System, fs: &str) -> Result<(), String> {
    match sys.call("/org/freedesktop/UDisks2/Manager", Call::CanFormat(fs.into()))? {
        Reply::CanFormat(true, _) => Ok(()),
        Reply::CanFormat(false, missing) => Err(missing),
        _ => Err(String::new()),
    }
}
fn confirmed(request: &Value) -> bool {
    request["backup_confirmed"] == true && matches!(request_str(request, "confirmation_word"), "confirmar" | "confirm")
}

pub(crate) fn format_ntfs(request: &Value, sys: &mut dyn System) -> Result<Value, String> {
    let fs = request_str(request, "filesystem").to_string();
    if !matches!(fs.as_str(), "ext4" | "btrfs") || request["backup_confirmed"] != true {
        return fail("Escolha ext4 ou Btrfs e confirme que o backup foi verificado.");
    }
    if !matches!(request_str(request, "confirmation_word"), "confirmar" | "confirm") {
        return fail("Digite confirmar para autorizar a formatação.");
    }
    let Prepared { path, obj: o, target, .. } = preflight(request, true, sys)?;
    let (b, p) = (iface(&o, B), iface(&o, P));
    if !matches!(str_of(b, "IdType"), "ntfs" | "ntfs3") {
        return fail("O assistente de formatação aceita somente uma partição NTFS.");
    }
    if request_str(request, "confirmation_device") != text(&b["Device"]) {
        return fail("A confirmação precisa corresponder exatamente ao dispositivo selecionado.");
    }
    if !DATA_PARTITION_TYPES.contains(&partition_type(p).as_str()) {
        return fail("Partições de sistema, recuperação e tipos especiais não podem ser formatados aqui.");
    }
    if str_of(p, "UUID").is_empty() {
        return fail("Identificação estável da partição ausente; formatação bloqueada.");
    }
    can_format(sys, &fs).map_err(|missing| format!("O sistema não tem suporte para criar {fs}. Instale o utilitário: {missing}"))?;
    let original = identity(&o);
    let mut stages: Vec<&str> = Vec::new();
    let mut format_started = false;
    let result = (|| -> Result<Value, String> {
        // Preparation/authorization failure happens before the destructive call.
        if !sys.path(&target).present {
            sys.make_mount_dir(&target)?;
            stages.push("pasta criada");
        }
        let before = preflight(request, true, sys)?.obj;
        if identity(&before) != original {
            return Err("A identificação da partição mudou; operação interrompida.".into());
        }
        if !mounts(&before).is_empty() {
            sys.call(&path, Call::Unmount)?;
            stages.push("partição desmontada");
        }
        let current = preflight(request, true, sys)?.obj;
        if identity(&current) != original || !mounts(&current).is_empty() {
            return Err("A partição mudou ou voltou a ser montada; formatação interrompida.".into());
        }
        // Explicitly remove only this partition's old fstab entry; never tear down other devices.
        for entry in fstab_items(iface(&current, B)) {
            sys.call(&path, Call::RemoveConfiguration(entry_of(entry)))?;
            stages.push("entrada antiga de automontagem removida");
        }
        let current = preflight(request, true, sys)?.obj;
        if identity(&current) != original || !mounts(&current).is_empty() {
            return Err("A partição mudou; formatação interrompida.".into());
        }
        format_started = true;
        sys.call(&path, Call::Format { kind: fs.clone(), label: Some(request_str(request, "name").into()) })?;
        stages.push("partição formatada; dados anteriores apagados");
        // Wait for udev/UDisks metadata, checking stable GPT identity throughout.
        let mut fresh = None;
        for _ in 0..40 {
            let objects = sys.objects()?;
            let candidate = obj(&objects, &path).clone();
            if identity(&candidate) != original {
                return Err("A identificação da partição mudou após a formatação.".into());
            }
            let b = iface(&candidate, B);
            let new_uuid = str_of(b, "IdUUID");
            if str_of(b, "IdType") == fs && !new_uuid.is_empty() && new_uuid != request_str(request, "uuid") && has(&candidate, F) {
                fresh = Some(new_uuid.to_string());
                break;
            }
            sys.pause();
        }
        let uuid = fresh.ok_or("Formatação concluída, mas os metadados ainda não foram atualizados. Atualize a lista antes de configurar a montagem.")?;
        let mut next = request.clone();
        next["uuid"] = json!(uuid);
        let mut result = save(&next, sys)?;
        result["formatted"] = json!(true);
        Ok(result)
    })();
    result.map_err(|error| {
        let warning = if format_started { " A formatação foi iniciada: os dados podem já ter sido apagados." } else { " A formatação não foi iniciada." };
        format!("Não foi possível concluir.{warning} Etapas concluídas: {}. {error}", steps(&stages))
    })
}

fn child_partitions<'a>(path: &str, all: &'a Objects) -> Vec<(&'a String, &'a Value)> {
    all.iter().filter(|(_, o)| has(o, P) && object_of(iface(o, P), "Table") == path).collect()
}

fn preparation_identity(path: &str, all: &Objects) -> Value {
    let o = obj(all, path);
    let (b, p) = (iface(o, B), iface(o, P));
    let drive = iface(obj(all, object_of(b, "Drive")), DRIVE);
    let mut children: Vec<Value> = child_partitions(path, all).into_iter()
        .map(|(key, value)| { let p = iface(value, P); json!([key, identity(value), p.get("Offset"), p.get("Size"), p.get("Type")]) })
        .collect();
    children.sort_by(|a, b| a[0].as_str().cmp(&b[0].as_str()));
    json!({
        "device": identity(o), "serial": str_of(drive, "Serial"), "usage": str_of(b, "IdUsage"),
        "filesystem": str_of(b, "IdType"), "uuid": str_of(b, "IdUUID"), "partition_type": str_of(p, "Type"),
        "offset": p.get("Offset"), "partition_size": p.get("Size"), "table_type": iface(o, T).get("Type"),
        "children": children,
    })
}

fn empty_storage_reason(path: &str, all: &Objects, whole: bool) -> String {
    let Some(o) = all.get(path) else { return "Dispositivo não encontrado.".into() };
    let b = iface(o, B);
    if !has(o, B) || truthy(b.get("ReadOnly")) || object_of(b, "Drive") == "/" || object_of(b, "CryptoBackingDevice") != "/" {
        return "Dispositivo protegido, virtual ou criptografado.".into();
    }
    if system_partition(path, all) || !mounts(o).is_empty() || !fstab_items(b).is_empty() {
        return "Dispositivo usado pelo sistema, montado ou com configuração existente.".into();
    }
    if !matches!(str_of(b, "IdUsage"), "" | "partitiontable") || !matches!(str_of(b, "IdType"), "" | "gpt" | "dos") {
        return "O dispositivo já tem um sistema de arquivos ou outra assinatura. Atualize a lista.".into();
    }
    if jobs_touch(all, &[path]) {
        return "Há uma operação em andamento neste dispositivo.".into();
    }
    if whole {
        if has(o, P) || !child_partitions(path, all).is_empty() || truthy(iface(o, T).get("Partitions")) {
            return "O disco já possui partições; não é permitido substituir sua tabela.".into();
        }
    } else if !has(o, P) || table_type(all, iface(o, P)) != "gpt" {
        return "Esta operação requer uma partição de dados em tabela GPT.".into();
    }
    String::new()
}

fn free_extents(path: &str, all: &Objects) -> Vec<(u64, u64)> {
    let o = obj(all, path);
    let b = iface(o, B);
    if str_of(iface(o, T), "Type") != "gpt" || has(o, P) || truthy(b.get("ReadOnly")) || truthy(b.get("HintIgnore")) || object_of(b, "Drive") == "/" {
        return Vec::new();
    }
    if object_of(b, "CryptoBackingDevice") != "/" || !mounts(o).is_empty() || !fstab_items(b).is_empty() {
        return Vec::new();
    }
    let children = child_partitions(path, all);
    let mut involved: Vec<&str> = children.iter().map(|(key, _)| key.as_str()).collect();
    involved.push(path);
    if jobs_touch(all, &involved) {
        return Vec::new();
    }
    if let Some(listed) = iface(o, T).get("Partitions") {
        let mut listed: Vec<&str> = listed.as_array().map(|items| items.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let mut known: Vec<&str> = children.iter().map(|(key, _)| key.as_str()).collect();
        listed.sort_unstable();
        listed.dedup();
        known.sort_unstable();
        if listed != known {
            return Vec::new();
        }
    }
    let mut ranges = Vec::new();
    for (_, child) in &children {
        let p = iface(child, P);
        let (Some(offset), Some(size)) = (p["Offset"].as_i64(), p["Size"].as_i64()) else { return Vec::new() };
        if offset < 0 || size <= 0 {
            return Vec::new();
        }
        ranges.push((offset as i128, offset as i128 + size as i128));
    }
    if ranges.is_empty() {
        return Vec::new(); // Empty disks use the whole-disk flow.
    }
    ranges.sort_unstable();
    let size = b["Size"].as_u64().unwrap_or(0) as i128;
    if ranges.iter().any(|(_, end)| *end > size) || ranges.windows(2).any(|pair| pair[1].0 < pair[0].1) {
        return Vec::new();
    }
    let limit = (size.div_euclid(MIB) - 1) * MIB;
    let mut cursor = MIB;
    let mut result = Vec::new();
    for (start, end) in ranges.into_iter().chain([(limit, limit)]) {
        let first = (cursor + MIB - 1).div_euclid(MIB) * MIB;
        let last = start.min(limit).div_euclid(MIB) * MIB;
        if last - first >= MIN_FREE_SPACE {
            result.push((first as u64, (last - first) as u64));
        }
        cursor = cursor.max(end);
    }
    result
}

pub(crate) fn storage_listing(all: &Objects) -> Vec<Value> {
    let mut result = Vec::new();
    for path in all.keys() {
        let o = obj(all, path);
        if storage_candidate(path, all) {
            let mut item = describe(path, all);
            if item["filesystem"] == "" {
                let whole = !has(o, P);
                item["preparation_kind"] = json!(if whole { "disk" } else { "partition" });
                item["preparation_identity"] = preparation_identity(path, all);
                item["format_blocked_reason"] = json!(empty_storage_reason(path, all, whole));
            }
            result.push(item);
        }
        for (offset, size) in free_extents(path, all) {
            let mut item = describe(path, all);
            let fields = json!({
                "id": format!("{path}#free:{offset}:{size}"), "label": "", "partition_name": "", "filesystem": "",
                "size": size, "uuid": "", "mounts": [], "preparation_kind": "free_space",
                "preparation_identity": preparation_identity(path, all), "parent_id": path,
                "offset": offset, "extent_size": size, "mount_at_startup": false, "configured_path": "",
                "blocked_reason": "Espaço não particionado. Crie uma partição antes de configurar a automontagem.",
                "operations_blocked_reason": "Espaço não particionado.", "format_blocked_reason": "",
            });
            for (key, value) in fields.as_object().into_iter().flatten() {
                item[key] = value.clone();
            }
            result.push(item);
        }
    }
    result
}

fn preparation_preflight(request: &Value, sys: &mut dyn System) -> Result<(String, Objects), String> {
    let all = sys.objects()?;
    let kind = request_str(request, "preparation_kind");
    if !matches!(kind, "disk" | "partition" | "free_space") {
        return Err("Tipo de preparação inválido.".into());
    }
    let path = request_str(request, if kind == "free_space" { "parent_id" } else { "id" }).to_string();
    if !all.contains_key(&path) || preparation_identity(&path, &all) != request["preparation_identity"] {
        return Err("O dispositivo ou suas partições mudaram. Atualize a lista antes de continuar.".into());
    }
    if text(&iface(obj(&all, &path), B)["Device"]) != request_str(request, "confirmation_device") {
        return Err("Dispositivo de confirmação diferente.".into());
    }
    if kind == "free_space" {
        let (Some(offset), Some(size)) = (request["offset"].as_u64(), request["extent_size"].as_u64()) else {
            return Err("O espaço livre mudou ou se sobrepõe a uma partição existente.".into());
        };
        if !free_extents(&path, &all).contains(&(offset, size)) {
            return Err("O espaço livre mudou ou se sobrepõe a uma partição existente.".into());
        }
        if request_str(request, "id") != format!("{path}#free:{offset}:{size}") {
            return Err("Identificação do espaço livre inválida.".into());
        }
    } else {
        let blocked = empty_storage_reason(&path, &all, kind == "disk");
        if !blocked.is_empty() {
            return Err(blocked);
        }
    }
    Ok((path, all))
}

fn preparation_target(name: &str, all: &Objects, sys: &mut dyn System) -> Result<String, String> {
    let target = format!("/mnt/{name}");
    let mnt = sys.path("/mnt");
    if mnt.symlink || !mnt.dir {
        return Err("/mnt precisa ser um diretório real.".into());
    }
    let place = sys.path(&target);
    if place.present && (place.symlink || !place.dir || !place.empty) {
        return Err("A pasta de destino está ocupada ou é um link simbólico.".into());
    }
    for o in all.values() {
        if mounts(o).contains(&target) || fstab_items(iface(o, B)).into_iter().any(|entry| entry_text(entry, "dir") == target) {
            return Err("Destino já usado por outro armazenamento.".into());
        }
    }
    if fstab_targets(&sys.fstab()?).any(|(_, dir)| dir == target) {
        return Err("Destino já reservado em /etc/fstab.".into());
    }
    Ok(target)
}

pub(crate) fn prepare_storage(request: &Value, sys: &mut dyn System) -> Result<Value, String> {
    let fs = request_str(request, "filesystem").to_string();
    let name = request_str(request, "name").to_string();
    if !matches!(fs.as_str(), "ext4" | "btrfs") || !valid_name(&name) {
        return fail("Nome ou formato inválido.");
    }
    if !confirmed(request) {
        return fail("Confirme a revisão dos dados e digite confirmar antes de continuar.");
    }
    let (first_path, first_all) = preparation_preflight(request, sys)?;
    let target = preparation_target(&name, &first_all, sys)?;
    can_format(sys, &fs).map_err(|missing| format!("Instale o utilitário para criar {fs}: {missing}"))?;
    let original = identity(obj(&first_all, &first_path));
    let original_serial = preparation_identity(&first_path, &first_all)["serial"].clone();
    let kind = request_str(request, "preparation_kind").to_string();
    let mut stages: Vec<&str> = Vec::new();
    let mut destructive = false;
    let result = (|| -> Result<Value, String> {
        if !sys.path(&target).present {
            sys.make_mount_dir(&target)?;
        }
        let (mut path, mut all) = preparation_preflight(request, sys)?;
        preparation_target(&name, &all, sys)?;
        let disk_changed = |all: &Objects, path: &str| identity(obj(all, path)) != original || preparation_identity(path, all)["serial"] != original_serial;
        if kind == "disk" && str_of(iface(obj(&all, &path), T), "Type") != "gpt" {
            destructive = true;
            sys.call(&path, Call::Format { kind: "gpt".into(), label: None })?;
            stages.push("tabela GPT criada");
            let mut ready = false;
            for _ in 0..40 {
                all = sys.objects()?;
                if disk_changed(&all, &path) {
                    return Err("A identificação do disco mudou.".into());
                }
                if str_of(iface(obj(&all, &path), T), "Type") == "gpt" {
                    ready = true;
                    break;
                }
                sys.pause();
            }
            if !ready {
                return Err("Tabela criada, mas ainda não reconhecida pelo sistema.".into());
            }
        }
        if kind == "disk" || kind == "free_space" {
            if kind == "free_space" {
                (path, all) = preparation_preflight(request, sys)?;
            } else if identity(obj(&all, &path)) != original || !empty_storage_reason(&path, &all, true).is_empty() {
                return Err("O disco deixou de estar vazio.".into());
            }
            let before: Vec<(String, Value)> = child_partitions(&path, &all).into_iter().map(|(key, value)| (key.clone(), value.clone())).collect();
            let disk_size = iface(obj(&all, &path), B)["Size"].as_u64().unwrap_or(0) as i128;
            let (offset, size) = if kind == "free_space" {
                (request["offset"].as_u64().unwrap_or(0) as i128, request["extent_size"].as_u64().unwrap_or(0) as i128)
            } else {
                (MIB, (disk_size.div_euclid(MIB) - 2) * MIB)
            };
            if size < MIN_FREE_SPACE {
                return Err("Espaço insuficiente para criar a partição.".into());
            }
            let partition_uuid = uuid::Uuid::new_v4().to_string();
            let parent = path.clone();
            destructive = true;
            let created_path = match sys.call(&parent, Call::CreatePartition { offset: offset as u64, size: size as u64, kind: LINUX_DATA.into(), name: name.clone(), uuid: partition_uuid.clone() })? {
                Reply::Path(path) => path,
                _ => return Err("A nova partição não foi informada pelo sistema.".into()),
            };
            stages.push("nova partição criada");
            let mut ready = false;
            for _ in 0..40 {
                all = sys.objects()?;
                if disk_changed(&all, &parent) {
                    return Err("A identificação do disco mudou.".into());
                }
                if has(obj(&all, &created_path), P) {
                    ready = true;
                    break;
                }
                sys.pause();
            }
            if !ready {
                return Err("A nova partição ainda não foi reconhecida.".into());
            }
            let p = iface(obj(&all, &created_path), P);
            let (p_offset, p_size) = (p["Offset"].as_i64().unwrap_or(-1) as i128, p["Size"].as_i64().unwrap_or(0) as i128);
            if object_of(p, "Table") != parent || str_of(p, "UUID").to_lowercase() != partition_uuid || p_offset < offset || p_offset + p_size > offset + size {
                return Err("A partição criada não corresponde ao espaço confirmado; formatação interrompida.".into());
            }
            if before.iter().any(|(key, value)| all.get(key) != Some(value)) {
                return Err("Outra partição mudou; formatação interrompida.".into());
            }
            path = created_path;
        } else {
            (path, all) = preparation_preflight(request, sys)?;
        }
        if !empty_storage_reason(&path, &all, false).is_empty() {
            return Err("A partição não está mais disponível para formatação.".into());
        }
        let partition_identity = identity(obj(&all, &path));
        destructive = true;
        sys.call(&path, Call::Format { kind: fs.clone(), label: Some(name.clone()) })?;
        stages.push("partição formatada");
        let mut fresh = None;
        for _ in 0..40 {
            all = sys.objects()?;
            let candidate = obj(&all, &path);
            if identity(candidate) != partition_identity {
                return Err("A identificação da partição mudou após formatar.".into());
            }
            let b = iface(candidate, B);
            if has(candidate, F) && str_of(b, "IdType") == fs && !str_of(b, "IdUUID").is_empty() {
                fresh = Some(str_of(b, "IdUUID").to_string());
                break;
            }
            sys.pause();
        }
        let uuid = fresh.ok_or("Formatação concluída, mas os metadados ainda não foram atualizados.")?;
        let mut next = request.clone();
        next["id"] = json!(path);
        next["uuid"] = json!(uuid);
        let mut result = save(&next, sys)?;
        result["formatted"] = json!(true);
        Ok(result)
    })();
    result.map_err(|error| {
        let state = if destructive { "A preparação foi iniciada; a tabela ou os dados podem ter sido alterados. " } else { "A formatação não foi iniciada. " };
        format!("Não foi possível concluir. {state}Etapas concluídas: {}. {error}", steps(&stages))
    })
}

#[cfg(test)]
#[path = "automount_tests.rs"]
mod tests;
