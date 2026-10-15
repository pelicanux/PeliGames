//! The real system behind the automount logic: UDisks2 on the system D-Bus
//! (through `zbus`, no Python), the local filesystem, and fixed `pkexec`
//! commands. UDisks2 asks polkit for authorization when a change needs it.
use super::automount::{Call, Entry, EntryValue, Objects, PathState, Reply, System, B, F, P, T, U};
use serde_json::{json, Map, Value as Json};
use std::{collections::HashMap, fs, path::Path, process::Command, time::Duration};
use zbus::{
    blocking::{connection::Builder, Connection, Proxy},
    proxy::MethodFlags,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

const MANAGER: &str = "org.freedesktop.UDisks2.Manager";
type Options = HashMap<&'static str, Value<'static>>;

pub(crate) struct Udisks {
    connection: Connection,
}
impl Udisks {
    pub(crate) fn connect() -> Result<Self, String> {
        // Formatting a large disk takes minutes; the default would give up first.
        let connection = Builder::system()
            .and_then(|builder| builder.method_timeout(Duration::from_secs(600)).build())
            .map_err(|_| "O serviço de discos do sistema (UDisks2) não está disponível.".to_string())?;
        Ok(Udisks { connection })
    }
    fn proxy<'a>(&'a self, path: &'a str, interface: &'a str) -> Result<Proxy<'a>, String> {
        Proxy::new(&self.connection, U, path, interface).map_err(|error| error.to_string())
    }
    /// A call that may ask for the administrator password (polkit).
    fn authorized<B, R>(&self, path: &str, interface: &str, method: &str, body: &B) -> Result<R, String>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
    {
        self.proxy(path, interface)?
            .call_with_flags(method, MethodFlags::AllowInteractiveAuth.into(), body)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("{method}: sem resposta do UDisks2."))
    }
}

/// D-Bus values as JSON; byte strings (`ay`) stay arrays of numbers.
fn to_json(value: &Value) -> Json {
    match value {
        Value::U8(v) => json!(v),
        Value::Bool(v) => json!(v),
        Value::I16(v) => json!(v),
        Value::U16(v) => json!(v),
        Value::I32(v) => json!(v),
        Value::U32(v) => json!(v),
        Value::I64(v) => json!(v),
        Value::U64(v) => json!(v),
        Value::F64(v) => json!(v),
        Value::Str(v) => json!(v.as_str()),
        Value::Signature(v) => json!(v.to_string()),
        Value::ObjectPath(v) => json!(v.as_str()),
        Value::Value(v) => to_json(v),
        Value::Array(v) => Json::Array(v.inner().iter().map(to_json).collect()),
        Value::Dict(v) => Json::Object(v.iter().map(|(key, value)| {
            let key = match to_json(key) { Json::String(s) => s, other => other.to_string() };
            (key, to_json(value))
        }).collect()),
        Value::Structure(v) => Json::Array(v.fields().iter().map(to_json).collect()),
        _ => Json::Null,
    }
}
fn entry(entry: &Entry) -> (String, HashMap<String, Value<'static>>) {
    let details = entry.1.iter().map(|(key, value)| (key.clone(), match value {
        EntryValue::Bytes(bytes) => Value::new(bytes.clone()),
        EntryValue::Int(number) => Value::I32(*number),
    })).collect();
    (entry.0.clone(), details)
}

impl System for Udisks {
    fn objects(&mut self) -> Result<Objects, String> {
        let tree: HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> = self
            .proxy("/org/freedesktop/UDisks2", "org.freedesktop.DBus.ObjectManager")?
            .call("GetManagedObjects", &())
            .map_err(|error| error.to_string())?;
        let mut objects = Map::new();
        for (path, interfaces) in tree {
            let interfaces: Map<String, Json> = interfaces.into_iter()
                .map(|(name, properties)| (name, Json::Object(properties.iter().map(|(key, value)| (key.clone(), to_json(value))).collect())))
                .collect();
            objects.insert(path.as_str().to_string(), Json::Object(interfaces));
        }
        Ok(objects)
    }
    fn call(&mut self, path: &str, call: Call) -> Result<Reply, String> {
        let none = Options::new;
        match call {
            Call::Unmount => self.authorized::<_, ()>(path, F, "Unmount", &(none(),)).map(|_| Reply::Unit),
            Call::Mount => self.authorized::<_, String>(path, F, "Mount", &(none(),)).map(Reply::Path),
            Call::SetLabel(label) => self.authorized::<_, ()>(path, F, "SetLabel", &(label, none())).map(|_| Reply::Unit),
            Call::SetName(name) => self.authorized::<_, ()>(path, P, "SetName", &(name, none())).map(|_| Reply::Unit),
            Call::AddConfiguration(new) => self.authorized::<_, ()>(path, B, "AddConfigurationItem", &(entry(&new), none())).map(|_| Reply::Unit),
            Call::UpdateConfiguration(old, new) => self.authorized::<_, ()>(path, B, "UpdateConfigurationItem", &(entry(&old), entry(&new), none())).map(|_| Reply::Unit),
            Call::RemoveConfiguration(old) => self.authorized::<_, ()>(path, B, "RemoveConfigurationItem", &(entry(&old), none())).map(|_| Reply::Unit),
            Call::Format { kind, label } => {
                let mut options = none();
                if let Some(label) = label {
                    options.insert("label", Value::new(label));
                    options.insert("take-ownership", Value::Bool(true));
                    options.insert("update-partition-type", Value::Bool(true));
                }
                self.authorized::<_, ()>(path, B, "Format", &(kind, options)).map(|_| Reply::Unit)
            }
            Call::CanFormat(kind) => self.authorized::<_, ((bool, String),)>(path, MANAGER, "CanFormat", &(kind,))
                .map(|((available, missing),)| Reply::CanFormat(available, missing)),
            Call::CreatePartition { offset, size, kind, name, uuid } => {
                let mut options = none();
                options.insert("partition-uuid", Value::new(uuid));
                self.authorized::<_, OwnedObjectPath>(path, T, "CreatePartition", &(offset, size, kind, name, options))
                    .map(|created| Reply::Path(created.as_str().to_string()))
            }
        }
    }
    fn make_mount_dir(&mut self, target: &str) -> Result<(), String> {
        let status = Command::new("/usr/bin/pkexec").args(["/usr/bin/mkdir", "-m", "0755", "--", target]).status()
            .map_err(|_| "pkexec não está disponível para criar a pasta de montagem.".to_string())?;
        if !status.success() {
            return Err(format!("Não foi possível criar {target} (autorização cancelada ou negada)."));
        }
        Ok(())
    }
    fn reload_systemd(&mut self) -> Result<(), String> {
        let output = Command::new("/usr/bin/pkexec").args(["/usr/bin/systemctl", "daemon-reload"]).output()
            .map_err(|_| "Falha ao recarregar o systemd (daemon-reload). pkexec indisponível.".to_string())?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(format!("Falha ao recarregar o systemd (daemon-reload). {}", if detail.is_empty() { "Autorização cancelada ou comando indisponível." } else { &detail }));
        }
        Ok(())
    }
    fn fstab(&mut self) -> Result<String, String> {
        fs::read_to_string("/etc/fstab").map_err(|_| "Não foi possível ler /etc/fstab.".to_string())
    }
    fn path(&mut self, path: &str) -> PathState {
        let link = fs::symlink_metadata(path);
        let dir = fs::metadata(path).is_ok_and(|meta| meta.is_dir());
        PathState {
            present: link.is_ok(),
            symlink: link.is_ok_and(|meta| meta.file_type().is_symlink()),
            dir,
            empty: dir && fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none()),
            canonical: fs::canonicalize(path).is_ok_and(|real| real == Path::new(path)),
        }
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Read-only: lists this machine's drives through the real UDisks2.
    #[test]
    #[ignore = "reads the real UDisks2; run by hand"]
    fn lists_real_drives() {
        let mut system = Udisks::connect().unwrap();
        let objects = system.objects().unwrap();
        println!("{}", serde_json::to_string(&super::super::automount::storage_listing(&objects)).unwrap());
    }
    #[test]
    fn dbus_values_become_json_with_byte_strings_kept() {
        let bytes = Value::new(b"/mnt/Games\0".to_vec());
        assert_eq!(to_json(&bytes), json!([47, 109, 110, 116, 47, 71, 97, 109, 101, 115, 0]));
        assert_eq!(super::super::automount::text(&to_json(&bytes)), "/mnt/Games");
        assert_eq!(to_json(&Value::new("ext4")), json!("ext4"));
        assert_eq!(to_json(&Value::U64(256)), json!(256));
        let (kind, details) = entry(&("fstab".into(), vec![("dir".into(), EntryValue::Bytes(b"/mnt/A\0".to_vec())), ("passno".into(), EntryValue::Int(2))]));
        assert_eq!(kind, "fstab");
        assert_eq!(details["passno"], Value::I32(2));
        assert_eq!(to_json(&details["dir"]), json!([47, 109, 110, 116, 47, 65, 0]));
    }
}
