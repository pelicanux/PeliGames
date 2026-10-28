//! Stable on-disk Epic routes shared by the launcher and its standalone helper.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const MARKER: &str = "# PeliGames Epic runtime adapter v1";
pub const SENTINEL: &str = "__PELIGAMES_EPIC_PROTON__";

#[derive(Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub compat: String,
    pub prefix: String,
    pub executable: Option<String>,
    pub proton: Option<String>,
    pub command: Vec<String>,
    pub environment: BTreeMap<String, Option<String>>,
    pub arguments: Vec<String>,
    pub legacy_adapter: String,
}
#[derive(Serialize, Deserialize)]
pub struct Runtime {
    pub backup: String,
    pub original_sha256: String,
    pub shim_sha256: String,
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
