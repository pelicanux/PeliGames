//! Per-service credential storage. Temporary sessions never call the disk-backed provider.
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
#[derive(Default)]
struct Store {
    temporary: bool,
    used: BTreeSet<String>,
    values: BTreeMap<String, String>,
}
static STORES: Mutex<BTreeMap<String, Arc<Mutex<Store>>>> = Mutex::new(BTreeMap::new());
fn store_for(service: &str) -> Result<Arc<Mutex<Store>>, String> {
    valid_group(service)?;
    Ok(STORES
        .lock()
        .map_err(|_| "Armazenamento de sessões indisponível.")?
        .entry(service.into())
        .or_default()
        .clone())
}
fn group(service: &str) -> Result<&'static str, String> {
    match service {
        "nexusmods-personal-api" | "nexusmods-oauth" => Ok("nexus"),
        "discord-oauth-session" => Ok("discord"),
        "matrix-community-chat" | "matrix-rtc-crypto-store" => Ok("matrix"),
        _ => Err("Serviço de credenciais desconhecido.".into()),
    }
}
fn valid_group(service: &str) -> Result<(), String> {
    if matches!(service, "nexus" | "discord" | "matrix") {
        Ok(())
    } else {
        Err("Serviço de conexão desconhecido.".into())
    }
}
#[derive(Serialize)]
pub struct StorageStatus {
    pub temporary: bool,
    pub in_use: bool,
}
fn change(store: &mut Store, temporary: bool) -> Result<(), String> {
    if store.temporary != temporary && !store.used.is_empty() {
        return Err("Desconecte esta conta antes de alterar como a sessão é guardada.".into());
    }
    if store.temporary != temporary {
        store.values.clear();
    }
    store.temporary = temporary;
    Ok(())
}
#[tauri::command]
pub async fn connection_storage_status(service: String) -> Result<StorageStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let slot = store_for(&service)?;
        let store = slot
            .lock()
            .map_err(|_| "Armazenamento de sessões indisponível.")?;
        Ok(StorageStatus {
            temporary: store.temporary,
            in_use: !store.used.is_empty(),
        })
    })
    .await
    .map_err(|_| "Não foi possível consultar o modo da conexão.".to_string())?
}
#[tauri::command]
pub async fn set_connection_temporary(
    service: String,
    temporary: bool,
) -> Result<StorageStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let slot = store_for(&service)?;
        let mut store = slot
            .lock()
            .map_err(|_| "Armazenamento de sessões indisponível.")?;
        change(&mut store, temporary)?;
        Ok(StorageStatus {
            temporary: store.temporary,
            in_use: !store.used.is_empty(),
        })
    })
    .await
    .map_err(|_| "Não foi possível alterar o modo da conexão.".to_string())?
}
fn access(
    store: &mut Store,
    action: &str,
    key: Option<&str>,
    service: &str,
    system: impl FnOnce() -> Result<Option<String>, String>,
) -> Result<Option<String>, String> {
    if !matches!(action, "lookup" | "store" | "clear") {
        return Err("Operação de credencial inválida.".into());
    }
    if action == "store" && key.is_none_or(|value| value.len() > 65536) {
        return Err("Credencial inválida ou muito longa.".into());
    }
    let result = if store.temporary {
        match action {
            "lookup" => store.values.get(service).cloned(),
            "store" => {
                store.values.insert(service.into(), key.unwrap().into());
                None
            }
            "clear" => {
                store.values.remove(service);
                None
            }
            _ => unreachable!(),
        }
    } else {
        system()?
    };
    if service != "matrix-rtc-crypto-store"
        && (action == "store" || (action == "lookup" && result.is_some()))
    {
        store.used.insert(service.into());
    }
    if action == "clear" {
        store.used.remove(service);
    }
    Ok(result)
}
pub(super) fn temporary(service: &str) -> Result<bool, String> {
    let slot = store_for(service)?;
    let result = slot
        .lock()
        .map_err(|_| "Armazenamento de sessões indisponível.")?
        .temporary;
    Ok(result)
}
pub(super) fn credential(
    action: &str,
    key: Option<&str>,
    service: &str,
    system: impl FnOnce() -> Result<Option<String>, String>,
) -> Result<Option<String>, String> {
    let group = group(service)?;
    let slot = store_for(group)?;
    let mut store = slot
        .lock()
        .map_err(|_| "Armazenamento de sessões indisponível.")?;
    access(&mut store, action, key, service, system)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn temporary_secrets_rotate_clear_and_never_reach_persistent_provider() {
        let mut store = Store::default();
        change(&mut store, true).unwrap();
        let no_disk = || -> Result<Option<String>, String> {
            panic!("temporary credentials must never access persistent storage")
        };
        assert_eq!(
            access(&mut store, "lookup", None, "nexusmods-oauth", no_disk).unwrap(),
            None
        );
        access(
            &mut store,
            "store",
            Some("first"),
            "nexusmods-oauth",
            no_disk,
        )
        .unwrap();
        access(
            &mut store,
            "store",
            Some("rotated"),
            "nexusmods-oauth",
            no_disk,
        )
        .unwrap();
        assert_eq!(
            access(&mut store, "lookup", None, "nexusmods-oauth", no_disk)
                .unwrap()
                .as_deref(),
            Some("rotated")
        );
        assert!(change(&mut store, false).is_err());
        access(&mut store, "clear", None, "nexusmods-oauth", no_disk).unwrap();
        assert!(
            access(&mut store, "lookup", None, "nexusmods-oauth", no_disk)
                .unwrap()
                .is_none()
        );
        change(&mut store, false).unwrap();
        assert!(Store::default().values.is_empty());
    }
    #[test]
    fn temporary_services_are_isolated_and_crypto_is_cleared_on_mode_change() {
        let mut store = Store::default();
        change(&mut store, true).unwrap();
        let no_disk =
            || -> Result<Option<String>, String> { panic!("persistent storage accessed") };
        access(
            &mut store,
            "store",
            Some("crypto"),
            "matrix-rtc-crypto-store",
            no_disk,
        )
        .unwrap();
        assert!(
            access(&mut store, "lookup", None, "matrix-community-chat", no_disk)
                .unwrap()
                .is_none()
        );
        change(&mut store, false).unwrap();
        assert!(store.values.is_empty());
        change(&mut store, true).unwrap();
        assert!(access(
            &mut store,
            "lookup",
            None,
            "matrix-rtc-crypto-store",
            no_disk
        )
        .unwrap()
        .is_none());
    }
    #[test]
    fn persistent_errors_remain_errors_and_services_do_not_mix() {
        let mut store = Store::default();
        assert!(access(
            &mut store,
            "store",
            Some("secret"),
            "discord-oauth-session",
            || Err("locked".into())
        )
        .is_err());
        assert!(store.values.is_empty());
        assert!(store.used.is_empty());
        access(&mut store, "lookup", None, "discord-oauth-session", || {
            Ok(Some("saved".into()))
        })
        .unwrap();
        assert!(change(&mut store, true).is_err());
        assert_eq!(
            group("nexusmods-oauth").unwrap(),
            group("nexusmods-personal-api").unwrap()
        );
        assert_ne!(
            group("discord-oauth-session").unwrap(),
            group("matrix-community-chat").unwrap()
        );
        assert!(group("unknown").is_err());
    }
}
