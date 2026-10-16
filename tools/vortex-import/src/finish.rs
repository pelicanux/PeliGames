//! Final touches on the generated modules before `--emit` writes them:
//! manual adjustments that static analysis cannot derive (`overrides.json`) and
//! the module version, which changes only when the module itself changes.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// Manual adjustments per Nexus domain:
/// `{ "<domain>": { "reason": "…", "merge": { … }, "append": { "<list>": [ … ] } } }`.
/// `merge` merges objects key by key (other values are replaced); `append` adds
/// list items the generated module does not already have.
pub type Overrides = BTreeMap<String, Value>;

pub fn load_overrides(path: &Path) -> Result<Overrides, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let Value::Object(map) = value else { return Err(format!("{}: esperado um objeto por domínio", path.display())) };
    let mut overrides = Overrides::new();
    for (domain, entry) in map {
        if domain.starts_with('$') { continue; } // "$comment" and other notes
        let Value::Object(fields) = &entry else { return Err(format!("{domain}: ajuste inválido")) };
        if let Some(unknown) = fields.keys().find(|key| !["reason", "merge", "append"].contains(&key.as_str())) {
            return Err(format!("{domain}: campo desconhecido no ajuste: {unknown}"));
        }
        if fields.get("reason").and_then(Value::as_str).is_none_or(|reason| reason.trim().is_empty()) {
            return Err(format!("{domain}: todo ajuste manual precisa de \"reason\""));
        }
        overrides.insert(domain, entry);
    }
    Ok(overrides)
}

fn merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                match target.get_mut(key) {
                    Some(current) => merge(current, value),
                    None => { target.insert(key.clone(), value.clone()); }
                }
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

/// Applies the adjustment of `module`'s domain; returns whether there was one.
pub fn apply_override(module: &mut Value, overrides: &Overrides) -> Result<bool, String> {
    let Some(domain) = module["domain"].as_str().map(str::to_string) else { return Ok(false) };
    let Some(entry) = overrides.get(&domain) else { return Ok(false) };
    if let Some(patch) = entry.get("merge") { merge(module, patch); }
    if let Some(Value::Object(lists)) = entry.get("append") {
        for (key, items) in lists {
            let Value::Array(items) = items else { return Err(format!("{domain}: append.{key} precisa ser uma lista")) };
            let list = module.as_object_mut().ok_or("módulo inválido")?.entry(key.clone()).or_insert_with(|| Value::Array(Vec::new()));
            let Value::Array(list) = list else { return Err(format!("{domain}: {key} não é uma lista no módulo gerado")) };
            for item in items {
                if !list.contains(item) { list.push(item.clone()); }
            }
        }
    }
    Ok(true)
}

/// Pinned revisions in evidence URLs change on every import without changing the module.
fn without_revisions(value: &Value) -> Value {
    match value {
        Value::String(text) => {
            let mut out = String::with_capacity(text.len());
            let mut rest = text.as_str();
            while let Some(at) = rest.find("/blob/") {
                out.push_str(&rest[..at + 6]);
                rest = &rest[at + 6..];
                let hex = rest.bytes().take_while(u8::is_ascii_hexdigit).count();
                if hex == 40 { out.push_str("REV"); rest = &rest[40..]; }
            }
            out.push_str(rest);
            Value::String(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(without_revisions).collect()),
        Value::Object(map) => Value::Object(map.iter().filter(|(key, _)| key.as_str() != "version").map(|(key, value)| (key.clone(), without_revisions(value))).collect::<Map<_, _>>()),
        other => other.clone(),
    }
}

fn bump_patch(version: &str) -> String {
    let mut parts: Vec<u64> = version.split('.').map(|part| part.parse().unwrap_or(0)).collect();
    parts.resize(3, 0);
    parts[2] += 1;
    format!("{}.{}.{}", parts[0], parts[1], parts[2])
}

/// Keeps the previous version when the module is unchanged (revision hashes
/// aside) and raises the patch number when it changed. New modules keep theirs.
pub fn carry_version(module: &mut Value, previous: Option<&Value>) {
    let Some(previous) = previous else { return };
    let Some(old) = previous["version"].as_str() else { return };
    let version = if without_revisions(module) == without_revisions(previous) { old.to_string() } else { bump_patch(old) };
    module["version"] = Value::String(version);
}

/// Modules of a previously written `<family>-vortex.json`, by domain.
pub fn previous_modules(path: &Path) -> BTreeMap<String, Value> {
    let Ok(text) = std::fs::read_to_string(path) else { return BTreeMap::new() };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else { return BTreeMap::new() };
    items.into_iter().filter_map(|item| item["domain"].as_str().map(|domain| (domain.to_string(), item.clone()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overrides_merge_fields_and_append_missing_items_once() {
        let overrides: Overrides = [("sz".to_string(), json!({ "reason": "plugins", "merge": { "family": { "plugin_mods": "Mods" } }, "append": { "notes": ["SZ"], "downloads": [{ "name": "B" }] } }))].into();
        let mut module = json!({ "domain": "sz", "family": { "kind": "unreal", "project": "SparkingZERO" }, "notes": ["base"], "downloads": [{ "name": "A" }] });
        assert!(apply_override(&mut module, &overrides).unwrap());
        assert!(apply_override(&mut module, &overrides).unwrap());
        assert_eq!(module, json!({ "domain": "sz", "family": { "kind": "unreal", "project": "SparkingZERO", "plugin_mods": "Mods" }, "notes": ["base", "SZ"], "downloads": [{ "name": "A" }, { "name": "B" }] }));
        let mut other = json!({ "domain": "x" });
        assert!(!apply_override(&mut other, &overrides).unwrap());
    }

    #[test]
    fn overrides_need_a_reason_and_known_fields() {
        let dir = std::env::temp_dir().join(format!("vortex-overrides-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("o.json");
        std::fs::write(&file, r#"{ "$comment": "ok", "a": { "merge": {} } }"#).unwrap();
        assert!(load_overrides(&file).unwrap_err().contains("reason"));
        std::fs::write(&file, r#"{ "a": { "reason": "r", "replace": {} } }"#).unwrap();
        assert!(load_overrides(&file).unwrap_err().contains("replace"));
        std::fs::write(&file, r#"{ "$comment": "ok", "a": { "reason": "r", "merge": {} } }"#).unwrap();
        assert_eq!(load_overrides(&file).unwrap().len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn version_changes_only_when_the_module_changes() {
        let previous = json!({ "domain": "g", "version": "1.1.0", "sources": ["https://github.com/a/b/blob/0123456789abcdef0123456789abcdef01234567/x.js"], "required": ["A.exe"] });
        let mut same = json!({ "domain": "g", "version": "1.0.0", "sources": ["https://github.com/a/b/blob/fedcba9876543210fedcba9876543210fedcba98/x.js"], "required": ["A.exe"] });
        carry_version(&mut same, Some(&previous));
        assert_eq!(same["version"], "1.1.0");
        let mut changed = json!({ "domain": "g", "version": "1.0.0", "sources": [], "required": ["B.exe"] });
        carry_version(&mut changed, Some(&previous));
        assert_eq!(changed["version"], "1.1.1");
        let mut new = json!({ "domain": "n", "version": "1.0.0" });
        carry_version(&mut new, None);
        assert_eq!(new["version"], "1.0.0");
    }
}
