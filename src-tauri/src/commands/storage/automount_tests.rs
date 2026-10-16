//! Automount and storage preparation checks against a simulated UDisks2:
//! nothing here touches a real disk, polkit or /etc/fstab.
use super::*;
use std::collections::HashMap;

const PATH: &str = "/org/freedesktop/UDisks2/block_devices/sdb1";
const MIB_U: u64 = 1024 * 1024;

/// A NUL-terminated byte string (`ay`), as UDisks2 sends it.
fn ay(text: &str) -> Value {
    let mut bytes: Vec<Value> = text.bytes().map(|b| json!(b)).collect();
    bytes.push(json!(0));
    Value::Array(bytes)
}
fn fstab_entry(dir: &str, opts: &str) -> Value {
    json!([["fstab", {"dir": ay(dir), "opts": ay(opts), "fsname": ay("UUID=test-uuid"), "type": ay("ext4"), "freq": 0, "passno": 2}]])
}

type Hook = Box<dyn FnMut(&mut Objects, usize)>;
struct Fake {
    objects: Objects,
    calls: Vec<(String, Call)>,
    objects_reads: usize,
    forbid_bus: bool,
    fail: Option<(&'static str, String)>,
    reload: Result<(), String>,
    reloads: usize,
    mount_reply: String,
    can_format: (bool, String),
    fstab: String,
    paths: HashMap<String, PathState>,
    /// Changes the tree before a given `objects()` read (1-based), as a device
    /// replaced or changed meanwhile.
    before_read: Option<Hook>,
    wrong_partition: bool,
    /// UDisks did not (yet) apply the requested partition UUID.
    stale_partition_uuid: bool,
}
impl Fake {
    fn new(objects: Value) -> Self {
        let mut paths = HashMap::new();
        paths.insert("/mnt".into(), PathState { present: true, dir: true, canonical: true, ..Default::default() });
        Fake {
            objects: objects.as_object().unwrap().clone(), calls: Vec::new(), objects_reads: 0, forbid_bus: false, fail: None,
            reload: Ok(()), reloads: 0, mount_reply: "/mnt/Games".into(), can_format: (true, String::new()),
            fstab: String::new(), paths, before_read: None, wrong_partition: false, stale_partition_uuid: false,
        }
    }
    fn methods(&self) -> Vec<&'static str> {
        self.calls.iter().map(|(_, call)| call.method()).collect()
    }
    fn folder(&mut self, path: &str, state: PathState) {
        self.paths.insert(path.into(), state);
    }
}
fn empty_dir() -> PathState {
    PathState { present: true, dir: true, empty: true, canonical: true, ..Default::default() }
}
impl System for Fake {
    fn objects(&mut self) -> Result<Objects, String> {
        assert!(!self.forbid_bus, "unexpected bus call");
        self.objects_reads += 1;
        if let Some(hook) = self.before_read.as_mut() {
            hook(&mut self.objects, self.objects_reads);
        }
        Ok(self.objects.clone())
    }
    fn call(&mut self, path: &str, call: Call) -> Result<Reply, String> {
        assert!(!self.forbid_bus, "unexpected mutation");
        self.calls.push((path.into(), call.clone()));
        if let Some((method, error)) = &self.fail {
            if *method == call.method() {
                return Err(error.clone());
            }
        }
        match call {
            Call::CanFormat(_) => return Ok(Reply::CanFormat(self.can_format.0, self.can_format.1.clone())),
            Call::Mount => return Ok(Reply::Path(self.mount_reply.clone())),
            Call::Unmount => {
                self.objects[path][F]["MountPoints"] = json!([]);
            }
            Call::Format { kind, .. } if kind == "gpt" => {
                self.objects[path][T] = json!({"Type": "gpt"});
                self.objects[path][B]["IdUsage"] = json!("partitiontable");
                self.objects[path][B]["IdType"] = json!("gpt");
            }
            Call::Format { kind, .. } => {
                self.objects[path][B]["IdUsage"] = json!("filesystem");
                self.objects[path][B]["IdType"] = json!(kind);
                self.objects[path][B]["IdUUID"] = json!("new-uuid");
                self.objects[path][F] = json!({"MountPoints": []});
            }
            Call::CreatePartition { offset, size, uuid, kind, .. } => {
                let offset = if self.wrong_partition { 0 } else { offset };
                let uuid = if self.stale_partition_uuid { "8c2f0f3e-0000-4000-8000-000000000000".to_string() } else { uuid };
                self.objects.insert("/created".into(), json!({
                    B: {"Device": ay("/dev/testdisk2"), "DeviceNumber": 10, "Drive": "/drive", "Size": size, "IdType": "", "IdUsage": "", "IdUUID": ""},
                    P: {"UUID": uuid, "Table": path, "Offset": offset, "Size": size, "Type": kind},
                }));
                return Ok(Reply::Path("/created".into()));
            }
            _ => {}
        }
        Ok(Reply::Unit)
    }
    fn make_mount_dir(&mut self, target: &str) -> Result<(), String> {
        self.paths.insert(target.into(), empty_dir());
        Ok(())
    }
    fn reload_systemd(&mut self) -> Result<(), String> {
        self.reloads += 1;
        self.reload.clone()
    }
    fn fstab(&mut self) -> Result<String, String> {
        Ok(self.fstab.clone())
    }
    fn path(&mut self, path: &str) -> PathState {
        self.paths.get(path).copied().unwrap_or_default()
    }
    fn pause(&mut self) {}
}

// --- Automount -------------------------------------------------------------

fn partition() -> Value {
    json!({
        PATH: {
            B: {"Device": ay("/dev/sdb1"), "IdType": "ext4", "IdUUID": "test-uuid", "IdLabel": "Old", "Drive": "/drive", "Size": 1000},
            F: {"MountPoints": []},
            P: {"Name": "Old", "Table": "/table"},
        },
        "/table": {T: {"Type": "gpt"}},
    })
}
fn objects_with(change: impl FnOnce(&mut Value)) -> Objects {
    let mut all = partition();
    change(&mut all);
    all.as_object().unwrap().clone()
}
fn request() -> Value {
    json!({"id": PATH, "uuid": "test-uuid", "name": "Games", "mount_at_startup": true})
}
fn with(request: &Value, changes: Value) -> Value {
    let mut next = request.clone();
    for (key, value) in changes.as_object().unwrap() {
        next[key] = value.clone();
    }
    next
}
fn ready_fake() -> Fake {
    let mut fake = Fake::new(partition());
    fake.folder("/mnt/Games", empty_dir());
    fake
}

#[test]
fn critical_mounts_and_fstab_are_blocked() {
    for mount in ["/", "/home", "/boot", "/usr", "/var", "/etc"] {
        let all = objects_with(|all| all[PATH][F]["MountPoints"] = json!([ay(mount)]));
        assert!(!reason(PATH, &all, false).is_empty(), "{mount}");
        let all = objects_with(|all| all[PATH][B]["Configuration"] = json!([["fstab", {"dir": ay(mount), "opts": ay("defaults")}]]));
        assert!(!reason(PATH, &all, false).is_empty(), "{mount}");
    }
}

#[test]
fn ntfs_mbr_readonly_encrypted_blocked() {
    for (interface, key, value) in [(B, "IdType", json!("ntfs")), (B, "ReadOnly", json!(true)), (B, "CryptoBackingDevice", json!("/crypto")), (P, "Flags", json!(1))] {
        let all = objects_with(|all| all[PATH][interface][key] = value);
        assert!(!reason(PATH, &all, false).is_empty(), "{key}");
    }
    let all = objects_with(|all| all["/table"][T]["Type"] = json!("dos"));
    assert!(!reason(PATH, &all, false).is_empty());
}

#[test]
fn listing_hides_system_mounts_and_fstab_even_when_unmounted() {
    for mount in ["/", "/home", "/home/user", "/boot", "/boot/efi", "/efi", "/usr", "/var"] {
        let all = objects_with(|all| { all[PATH][B]["IdUsage"] = json!("filesystem"); all[PATH][F]["MountPoints"] = json!([ay(mount)]); });
        assert!(!storage_candidate(PATH, &all), "{mount}");
        let all = objects_with(|all| { all[PATH][B]["IdUsage"] = json!("filesystem"); all[PATH][B]["Configuration"] = json!([["fstab", {"dir": ay(mount), "opts": ay("defaults")}]]); });
        assert!(!storage_candidate(PATH, &all), "{mount}");
    }
}

#[test]
fn listing_hides_unmounted_efi_and_recovery_but_keeps_game_data() {
    for filesystem in ["ext4", "btrfs", "ntfs", "ntfs3"] {
        let kind = if filesystem.starts_with("ntfs") { "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" } else { LINUX_DATA };
        let all = objects_with(|all| { all[PATH][B]["IdUsage"] = json!("filesystem"); all[PATH][B]["IdType"] = json!(filesystem); all[PATH][P]["Type"] = json!(kind); });
        assert!(storage_candidate(PATH, &all), "{filesystem}");
    }
    for (kind, filesystem) in [("c12a7328-f81f-11d2-ba4b-00a0c93ec93b", "vfat"), ("de94bba4-06d1-4d40-a16a-bfd50179d6ac", "ntfs"), ("933ac7e1-2eb4-4f13-b844-0e14e2aef915", "ext4")] {
        let all = objects_with(|all| { all[PATH][B]["IdUsage"] = json!("filesystem"); all[PATH][B]["IdType"] = json!(filesystem); all[PATH][P]["Type"] = json!(kind); });
        assert!(!storage_candidate(PATH, &all), "{kind}");
        assert!(!reason(PATH, &all, true).is_empty(), "{kind}");
    }
}

#[test]
fn listing_protects_shared_btrfs_root_uuid() {
    let all = objects_with(|all| {
        all[PATH][B]["IdUsage"] = json!("filesystem");
        all[PATH][B]["IdType"] = json!("btrfs");
        let mut peer = all[PATH].clone();
        peer[F]["MountPoints"] = json!([ay("/"), ay("/home")]);
        all["/root-volume"] = peer;
    });
    assert!(!storage_candidate(PATH, &all));
    assert!(!reason(PATH, &all, true).is_empty());
}

#[test]
fn unformatted_partition_is_listed_but_operations_stay_blocked() {
    let mut all = objects_with(|all| {
        all[PATH].as_object_mut().unwrap().remove(F);
        for key in ["IdUsage", "IdType", "IdUUID"] { all[PATH][B][key] = json!(""); }
        all[PATH][P]["Type"] = json!(LINUX_DATA);
    });
    assert!(storage_candidate(PATH, &all));
    assert!(reason(PATH, &all, true).contains("Nenhum sistema de arquivos"));
    assert_eq!(describe(PATH, &all)["filesystem"], "");
    all[PATH][P]["Type"] = json!("c12a7328-f81f-11d2-ba4b-00a0c93ec93b");
    assert!(!storage_candidate(PATH, &all));
}

#[test]
fn empty_physical_disk_is_listed_without_duplicate_partition_parent() {
    let mut all = json!({"/disk": {B: {"Device": ay("/dev/sdz"), "Drive": "/drive", "IdUsage": "", "IdType": ""}}}).as_object().unwrap().clone();
    assert!(storage_candidate("/disk", &all));
    all["/disk"][T] = json!({"Type": "gpt"});
    all["/disk"][B]["IdUsage"] = json!("partitiontable");
    all["/disk"][B]["IdType"] = json!("gpt");
    assert!(storage_candidate("/disk", &all));
    all.insert("/partition".into(), json!({P: {"Table": "/disk"}}));
    assert!(!storage_candidate("/disk", &all));
    all.remove("/partition");
    all["/disk"][B]["Drive"] = json!("/");
    assert!(!storage_candidate("/disk", &all));
}

#[test]
fn bad_names_rejected_before_bus_or_mutation() {
    for name in ["../root", "a/b", "", "-abc", &"a".repeat(17), "a;id", "a\nroot", "ação"] {
        let mut fake = ready_fake();
        fake.forbid_bus = true;
        assert!(save(&with(&request(), json!({"name": name})), &mut fake).is_err(), "{name:?}");
    }
}

#[test]
fn changed_device_and_mount_conflicts() {
    let mut fake = ready_fake();
    assert!(save(&with(&request(), json!({"uuid": "stale"})), &mut fake).is_err());
    fake.objects.insert("/other".into(), json!({B: {"IdUUID": "other"}, F: {"MountPoints": [ay("/mnt/Games")]}}));
    assert!(save(&request(), &mut fake).unwrap_err().contains("ponto de montagem"));
    assert!(fake.calls.is_empty());
}

#[test]
fn destination_reserved_in_fstab_for_another_device_is_refused() {
    let mut fake = ready_fake();
    fake.fstab = "# comment\nUUID=someone-else /mnt/Games ext4 defaults 0 2\n".into();
    assert!(save(&request(), &mut fake).unwrap_err().contains("/etc/fstab"));
    fake.fstab = "UUID=test-uuid /mnt/Games ext4 defaults 0 2\n".into();
    assert!(save(&request(), &mut fake).is_ok(), "a entrada do próprio dispositivo não bloqueia");
}

#[test]
fn symlink_destination_rejected() {
    let mut fake = ready_fake();
    fake.folder("/mnt/Games", PathState { present: true, symlink: true, dir: true, ..Default::default() });
    assert!(save(&request(), &mut fake).is_err());
    let mut fake = ready_fake();
    fake.folder("/mnt/Games", PathState { present: true, dir: true, empty: false, canonical: true, ..Default::default() });
    assert!(save(&request(), &mut fake).unwrap_err().contains("não está vazia"));
    assert!(fake.calls.is_empty());
}

#[test]
fn save_uses_uuid_nofail_and_never_formats() {
    let mut fake = ready_fake();
    save(&with(&request(), json!({"mount_at_startup": false})), &mut fake).unwrap();
    assert_eq!(fake.methods(), ["SetLabel", "SetName", "AddConfigurationItem", "Mount"]);
    assert_eq!(fake.reloads, 1, "daemon-reload antes de montar");
    let Call::AddConfiguration((kind, details)) = &fake.calls[2].1 else { panic!("expected AddConfigurationItem") };
    assert_eq!(kind, "fstab");
    let get = |key: &str| details.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap();
    assert_eq!(get("fsname"), EntryValue::Bytes(b"UUID=test-uuid\0".to_vec()));
    assert_eq!(get("passno"), EntryValue::Int(2));
    let EntryValue::Bytes(opts) = get("opts") else { panic!() };
    let opts = String::from_utf8(opts).unwrap();
    assert!(opts.contains("noauto") && opts.contains("nofail") && opts.contains("x-systemd.device-timeout=10s"), "{opts}");
}

#[test]
fn existing_entry_is_updated_keeping_its_own_options() {
    let mut fake = ready_fake();
    fake.objects[PATH][B]["Configuration"] = fstab_entry("/mnt/Old", "defaults,noatime,noauto,nofail");
    save(&request(), &mut fake).unwrap();
    let Some((_, Call::UpdateConfiguration(old, new))) = fake.calls.iter().find(|(_, call)| call.method() == "UpdateConfigurationItem") else { panic!("expected UpdateConfigurationItem") };
    assert!(old.1.iter().any(|(key, value)| key == "dir" && *value == EntryValue::Bytes(b"/mnt/Old\0".to_vec())));
    let EntryValue::Bytes(opts) = &new.1.iter().find(|(key, _)| key == "opts").unwrap().1 else { panic!() };
    assert_eq!(String::from_utf8_lossy(opts), "defaults,noatime,nofail,x-systemd.device-timeout=10s\0");
}

#[test]
fn partial_failure_reports_completed_step() {
    let mut fake = ready_fake();
    fake.fail = Some(("SetName", "authorization cancelled".into()));
    let error = save(&request(), &mut fake).unwrap_err();
    assert!(error.contains("label alterado") && error.contains("authorization cancelled"), "{error}");
}

#[test]
fn reload_denial_reports_saved_config_and_does_not_mount() {
    let mut fake = ready_fake();
    fake.reload = Err("Falha ao recarregar o systemd (daemon-reload). Authorization cancelled".into());
    let error = save(&request(), &mut fake).unwrap_err();
    assert!(error.contains("automontagem salva") && error.contains("daemon-reload"), "{error}");
    assert!(!fake.methods().contains(&"Mount"));
}

#[test]
fn mount_at_another_path_is_reported() {
    let mut fake = ready_fake();
    fake.mount_reply = "/media/user/Games".into();
    assert!(save(&request(), &mut fake).unwrap_err().contains("caminho diferente"));
}

fn format_fake() -> (Fake, Value) {
    let mut fake = ready_fake();
    fake.objects[PATH][B]["IdType"] = json!("ntfs");
    fake.objects[PATH][P]["Type"] = json!("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7");
    fake.objects[PATH][P]["UUID"] = json!("stable-partuuid");
    let request = with(&request(), json!({"filesystem": "ext4", "backup_confirmed": true, "confirmation_device": "/dev/sdb1", "confirmation_word": "confirmar"}));
    (fake, request)
}

#[test]
fn format_requires_backup_and_exact_device() {
    for change in [json!({"backup_confirmed": false}), json!({"confirmation_device": "/dev/sda1"}), json!({"filesystem": "ntfs"}), json!({"confirmation_word": ""}), json!({"confirmation_word": "apagar"})] {
        let (mut fake, request) = format_fake();
        assert!(format_ntfs(&with(&request, change.clone()), &mut fake).is_err(), "{change}");
        assert!(fake.calls.is_empty(), "{change}");
    }
}

#[test]
fn format_blocks_recovery_and_non_ntfs() {
    let (mut fake, request) = format_fake();
    fake.objects[PATH][P]["Type"] = json!("de94bba4-06d1-4d40-a16a-bfd50179d6ac");
    assert!(format_ntfs(&request, &mut fake).is_err());
    let (mut fake, request) = format_fake();
    fake.objects[PATH][B]["IdType"] = json!("ext4");
    assert!(format_ntfs(&request, &mut fake).is_err());
    assert!(fake.calls.is_empty());
}

#[test]
fn missing_format_utility_leaves_data_untouched() {
    let (mut fake, request) = format_fake();
    fake.can_format = (false, "mkfs.ext4".into());
    assert!(format_ntfs(&request, &mut fake).unwrap_err().contains("mkfs.ext4"));
    assert_eq!(fake.methods(), ["CanFormat"]);
}

#[test]
fn format_success_uses_new_uuid_and_mounts() {
    let (mut fake, request) = format_fake();
    let result = format_ntfs(&request, &mut fake).unwrap();
    assert_eq!(result["formatted"], true);
    assert_eq!(result["mount_path"], "/mnt/Games");
    let methods = fake.methods();
    assert_eq!(&methods[..2], ["CanFormat", "Format"]);
    assert_eq!(methods.last(), Some(&"Mount"));
    let Call::Format { kind, label } = &fake.calls[1].1 else { panic!() };
    assert_eq!((kind.as_str(), label.as_deref()), ("ext4", Some("Games")));
    let Some((_, Call::AddConfiguration((_, details)))) = fake.calls.iter().find(|(_, c)| c.method() == "AddConfigurationItem") else { panic!() };
    assert!(details.contains(&("fsname".into(), EntryValue::Bytes(b"UUID=new-uuid\0".to_vec()))), "a nova UUID vai para o fstab");
}

#[test]
fn format_auth_denial_reports_possible_data_loss() {
    let (mut fake, request) = format_fake();
    fake.fail = Some(("Format", "Authorization cancelled".into()));
    let error = format_ntfs(&request, &mut fake).unwrap_err();
    assert!(error.contains("dados podem já ter sido apagados"), "{error}");
    assert!(!fake.methods().contains(&"Mount"));
}

#[test]
fn device_changed_during_preparation_is_not_formatted() {
    let (mut fake, request) = format_fake();
    fake.before_read = Some(Box::new(|objects, read| if read == 2 { objects[PATH][P]["UUID"] = json!("different-device"); }));
    let error = format_ntfs(&request, &mut fake).unwrap_err();
    assert!(error.contains("formatação não foi iniciada"), "{error}");
    assert_eq!(fake.methods(), ["CanFormat"]);
}

#[test]
fn active_disk_job_blocks_another_operation() {
    let all = objects_with(|all| all["/job"] = json!({"org.freedesktop.UDisks2.Job": {"Objects": [PATH]}}));
    assert!(!reason(PATH, &all, false).is_empty());
    assert!(!reason(PATH, &all, true).is_empty());
}

fn operation(name: &str) -> Value {
    with(&request(), json!({"operation": name}))
}

#[test]
fn eject_only_unmounts_selected_partition_without_force() {
    let mut fake = ready_fake();
    fake.objects[PATH][F]["MountPoints"] = json!([ay("/mnt/Games")]);
    fake.objects.insert("/sibling".into(), json!({B: {"IdUUID": "sibling"}, F: {"MountPoints": [ay("/mnt/Other")]}}));
    storage_operation(&operation("unmount"), &mut fake).unwrap();
    assert_eq!(fake.calls, [(PATH.to_string(), Call::Unmount)]);
}

#[test]
fn remove_only_selected_fstab_entry_and_reload() {
    let mut fake = ready_fake();
    fake.objects[PATH][B]["Configuration"] = fstab_entry("/mnt/Games", "defaults");
    storage_operation(&operation("remove_automount"), &mut fake).unwrap();
    assert_eq!(fake.methods(), ["RemoveConfigurationItem"]);
    assert_eq!(fake.calls[0].0, PATH);
    let Call::RemoveConfiguration((kind, details)) = &fake.calls[0].1 else { panic!() };
    assert_eq!(kind, "fstab");
    assert!(details.contains(&("passno".into(), EntryValue::Int(2))));
    assert_eq!(fake.reloads, 1);
}

#[test]
fn remount_unmounts_before_mounting_configured_path() {
    let mut fake = ready_fake();
    fake.objects[PATH][F]["MountPoints"] = json!([ay("/mnt/Games")]);
    fake.objects[PATH][B]["Configuration"] = fstab_entry("/mnt/Games", "defaults");
    let result = storage_operation(&operation("remount"), &mut fake).unwrap();
    assert_eq!(fake.methods(), ["Unmount", "Mount"]);
    assert_eq!(fake.reloads, 1);
    assert_eq!(result["mount_path"], "/mnt/Games");
}

#[test]
fn operations_reject_system_stale_and_missing_configuration() {
    for request in [operation("bad"), with(&operation("unmount"), json!({"uuid": "stale"})), operation("remount"), operation("remove_automount")] {
        let mut fake = ready_fake();
        assert!(storage_operation(&request, &mut fake).is_err(), "{request}");
        assert!(fake.calls.is_empty(), "{request}");
    }
    let mut fake = ready_fake();
    fake.objects[PATH][F]["MountPoints"] = json!([ay("/")]);
    assert!(storage_operation(&operation("unmount"), &mut fake).is_err());
    assert!(fake.calls.is_empty());
}

#[test]
fn busy_eject_does_not_force_or_touch_configuration() {
    let mut fake = ready_fake();
    fake.objects[PATH][F]["MountPoints"] = json!([ay("/mnt/Games")]);
    fake.fail = Some(("Unmount", "DeviceBusy".into()));
    assert!(storage_operation(&operation("unmount"), &mut fake).unwrap_err().contains("DeviceBusy"));
    assert_eq!(fake.calls, [(PATH.to_string(), Call::Unmount)]);
}

#[test]
fn mount_does_not_unmount_and_requires_configuration() {
    let mut fake = ready_fake();
    fake.objects[PATH][B]["Configuration"] = fstab_entry("/mnt/Games", "noauto,nofail");
    storage_operation(&operation("mount"), &mut fake).unwrap();
    assert_eq!(fake.methods(), ["Mount"]);
    fake.objects[PATH][F]["MountPoints"] = json!([ay("/mnt/Games")]);
    assert!(storage_operation(&operation("mount"), &mut fake).unwrap_err().contains("já está montada"));
    assert_eq!(fake.calls.len(), 1);
}

// --- Preparation of empty disks, partitions and free space ------------------

const DISK: &str = "/disk";
fn disk_fake() -> Fake {
    let mut fake = Fake::new(json!({
        DISK: {B: {"Device": ay("/dev/testdisk"), "DeviceNumber": 8, "Drive": "/drive", "Size": 256 * MIB_U, "IdUsage": "", "IdType": "", "IdUUID": ""}},
        "/drive": {"org.freedesktop.UDisks2.Drive": {"Serial": "test-serial"}},
    }));
    fake.folder("/mnt/Games", empty_dir());
    fake
}
fn add_partition(fake: &mut Fake, filesystem: &str) {
    fake.objects[DISK][T] = json!({"Type": "gpt"});
    fake.objects[DISK][B]["IdUsage"] = json!("partitiontable");
    fake.objects[DISK][B]["IdType"] = json!("gpt");
    let mut part = json!({
        B: {"Device": ay("/dev/testdisk1"), "DeviceNumber": 9, "Drive": "/drive", "Size": 100 * MIB_U, "IdType": filesystem,
            "IdUsage": if filesystem.is_empty() { "" } else { "filesystem" }, "IdUUID": if filesystem.is_empty() { "" } else { "known" }},
        P: {"Type": LINUX_DATA, "UUID": "part-uuid", "Table": DISK, "Offset": MIB_U, "Size": 100 * MIB_U},
    });
    if !filesystem.is_empty() {
        part[F] = json!({"MountPoints": []});
    }
    fake.objects.insert("/part".into(), part);
}
fn preparation(fake: &Fake, kind: &str, path: &str) -> Value {
    json!({
        "id": path, "preparation_kind": kind, "preparation_identity": preparation_identity(path, &fake.objects),
        "name": "Games", "filesystem": "ext4", "backup_confirmed": true, "confirmation_word": "confirmar",
        "confirmation_device": text(&fake.objects[path][B]["Device"]), "mount_at_startup": true,
    })
}
fn free_request(fake: &mut Fake) -> Value {
    add_partition(fake, "ext4");
    let listing = storage_listing(&fake.objects);
    let item = listing.iter().find(|item| item["preparation_kind"] == "free_space").expect("free space listed");
    let mut request = preparation(fake, "disk", DISK);
    for key in ["id", "parent_id", "preparation_kind", "preparation_identity", "offset", "extent_size"] {
        request[key] = item[key].clone();
    }
    request
}

#[test]
fn free_space_listing_excludes_partition_ranges_and_small_alignment_gaps() {
    let mut fake = disk_fake();
    let request = free_request(&mut fake);
    let (offset, size) = (request["offset"].as_u64().unwrap(), request["extent_size"].as_u64().unwrap());
    assert!(offset >= 101 * MIB_U);
    assert!(offset + size <= 255 * MIB_U);
    assert!(!storage_listing(&fake.objects).iter().any(|item| item["id"] == DISK), "o disco é representado pelas partições");
}

#[test]
fn changed_identity_overlap_and_protected_partition_are_rejected() {
    let mut fake = disk_fake();
    let request = free_request(&mut fake);
    for change in [json!({"offset": MIB_U}), json!({"extent_size": 300 * MIB_U}), json!({"preparation_identity": {}}), json!({"confirmation_device": "/dev/other"})] {
        assert!(preparation_preflight(&with(&request, change.clone()), &mut fake).is_err(), "{change}");
    }
    fake.objects["/part"][P]["Type"] = json!("c12a7328-f81f-11d2-ba4b-00a0c93ec93b");
    let request = preparation(&fake, "partition", "/part");
    assert!(preparation_preflight(&request, &mut fake).is_err());
}

#[test]
fn no_confirmation_or_missing_utility_never_mutates() {
    for change in [json!({"backup_confirmed": false}), json!({"confirmation_word": ""}), json!({"filesystem": "ntfs"}), json!({"name": "../root"})] {
        let mut fake = disk_fake();
        let request = preparation(&fake, "disk", DISK);
        assert!(prepare_storage(&with(&request, change.clone()), &mut fake).is_err(), "{change}");
        assert!(fake.calls.is_empty(), "{change}");
    }
    let mut fake = disk_fake();
    fake.can_format = (false, "mkfs.ext4".into());
    let request = preparation(&fake, "disk", DISK);
    assert!(prepare_storage(&request, &mut fake).unwrap_err().contains("mkfs.ext4"));
    assert_eq!(fake.methods(), ["CanFormat"]);
}

/// Runs a preparation and returns the destructive calls (before mounting).
fn run_success(kind: &str, wrong_partition: bool) -> Vec<(String, &'static str)> {
    let mut fake = disk_fake();
    let request = match kind {
        "free_space" => free_request(&mut fake),
        "partition" => { add_partition(&mut fake, ""); preparation(&fake, "partition", "/part") }
        _ => preparation(&fake, "disk", DISK),
    };
    fake.wrong_partition = wrong_partition;
    let sibling = fake.objects.get("/part").cloned();
    let result = prepare_storage(&request, &mut fake);
    if wrong_partition {
        assert!(result.unwrap_err().contains("não corresponde"));
        assert!(!fake.methods().contains(&"Mount"));
    } else {
        let result = result.unwrap();
        assert_eq!(result["formatted"], true);
        let Some((_, Call::AddConfiguration((_, details)))) = fake.calls.iter().find(|(_, c)| c.method() == "AddConfigurationItem") else { panic!() };
        assert!(details.contains(&("fsname".into(), EntryValue::Bytes(b"UUID=new-uuid\0".to_vec()))));
    }
    if kind == "free_space" {
        assert_eq!(fake.objects.get("/part").cloned(), sibling, "a partição existente fica intacta");
    }
    fake.calls.iter().map(|(path, call)| (path.clone(), call.method()))
        .take_while(|(_, method)| !matches!(*method, "SetLabel" | "SetName" | "AddConfigurationItem" | "Mount"))
        .collect()
}

#[test]
fn empty_disk_creates_gpt_partition_then_formats() {
    let methods: Vec<_> = run_success("disk", false).into_iter().map(|(_, m)| m).collect();
    assert_eq!(methods, ["CanFormat", "Format", "CreatePartition", "Format"]);
}

#[test]
fn existing_empty_partition_formats_only_selected_device() {
    assert_eq!(run_success("partition", false), [("/org/freedesktop/UDisks2/Manager".to_string(), "CanFormat"), ("/part".to_string(), "Format")]);
}

#[test]
fn free_space_formats_when_udisks_keeps_its_own_partition_uuid() {
    let mut fake = disk_fake();
    let request = free_request(&mut fake);
    fake.stale_partition_uuid = true;
    assert_eq!(prepare_storage(&request, &mut fake).unwrap()["formatted"], true);
    assert!(fake.methods().contains(&"Format"));
}

#[test]
fn free_space_preserves_existing_partitions_and_does_not_replace_table() {
    let methods: Vec<_> = run_success("free_space", false).into_iter().map(|(_, m)| m).collect();
    assert_eq!(methods, ["CanFormat", "CreatePartition", "Format"]);
}

#[test]
fn wrong_created_partition_is_not_formatted() {
    let methods: Vec<_> = run_success("free_space", true).into_iter().map(|(_, m)| m).collect();
    assert!(!methods.contains(&"Format"), "{methods:?}");
}

#[test]
fn entire_disk_with_partitions_cannot_be_reinitialized() {
    let mut fake = disk_fake();
    add_partition(&mut fake, "ext4");
    let request = preparation(&fake, "disk", DISK);
    assert!(prepare_storage(&request, &mut fake).is_err());
    assert!(fake.calls.is_empty());
}

#[test]
fn serial_change_blocks_request() {
    let mut fake = disk_fake();
    let request = preparation(&fake, "disk", DISK);
    fake.objects["/drive"]["org.freedesktop.UDisks2.Drive"]["Serial"] = json!("changed");
    assert!(preparation_preflight(&request, &mut fake).unwrap_err().contains("mudaram"));
}

#[test]
fn incomplete_partition_metadata_never_offers_free_space_or_disk_reset() {
    let mut fake = disk_fake();
    add_partition(&mut fake, "ext4");
    fake.objects[DISK][T]["Partitions"] = json!(["/part", "/missing-partition"]);
    assert!(free_extents(DISK, &fake.objects).is_empty());
    fake.objects.remove("/part");
    assert!(!empty_storage_reason(DISK, &fake.objects, true).is_empty());
}
