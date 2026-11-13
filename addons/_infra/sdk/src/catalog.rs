use crate::{
    manifest::{Manifest, FORMAT_VERSION},
    package::MAX_ARCHIVE_BYTES,
    Result,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Bundled/reviewed catalog is the trust root. A digest from an arbitrary download
/// is not an endorsement; callers must obtain entries from this catalog only.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub addons: Vec<Entry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub manifest: Manifest,
    pub artifacts: BTreeMap<String, Artifact>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
}
impl Artifact {
    pub fn validate(&self) -> Result<()> {
        if self.bytes == 0
            || self.bytes > MAX_ARCHIVE_BYTES
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("Invalid add-on artifact size or SHA-256 digest".into());
        }
        // Deliberately fixed publisher; recipes cannot add arbitrary download origins.
        let suffix = self
            .url
            .strip_prefix("https://github.com/pelicanux/PeliGames/releases/download/")
            .ok_or("Add-on artifacts must come from the official release repository")?;
        let parts: Vec<_> = suffix.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|part| {
                part.is_empty()
                    || part.contains("..")
                    || !part
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            })
            || !parts[1].ends_with(".zip")
        {
            return Err("Invalid add-on artifact URL".into());
        }
        Ok(())
    }
}
impl Catalog {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != FORMAT_VERSION || self.addons.len() > 128 {
            return Err("Unsupported or oversized add-on catalog".into());
        }
        let mut ids = BTreeSet::new();
        for entry in &self.addons {
            entry.manifest.validate()?;
            if !ids.insert(&entry.manifest.id) {
                return Err("Duplicate catalog identifier".into());
            }
            for (platform, artifact) in &entry.artifacts {
                if !entry.manifest.platforms.contains(platform) {
                    return Err("Artifact platform is not declared by the add-on".into());
                }
                artifact.validate()?;
            }
        }
        // Check dependency graph even for entries that do not have artifacts yet.
        let mut visiting = BTreeSet::new();
        let mut ordered = Vec::new();
        for entry in &self.addons {
            self.visit(&entry.manifest.id, &mut visiting, &mut ordered)?;
        }
        Ok(())
    }
    pub fn entry(&self, id: &str) -> Result<&Entry> {
        self.addons
            .iter()
            .find(|e| e.manifest.id == id)
            .ok_or_else(|| format!("Unknown add-on: {id}"))
    }
    fn visit<'a>(
        &'a self,
        id: &str,
        visiting: &mut BTreeSet<&'a str>,
        ordered: &mut Vec<&'a Entry>,
    ) -> Result<()> {
        if ordered.iter().any(|e| e.manifest.id == id) {
            return Ok(());
        }
        let entry = self.entry(id)?;
        let id = entry.manifest.id.as_str();
        if !visiting.insert(id) {
            return Err("Cyclic add-on dependency".into());
        }
        for (dependency, requirement) in &entry.manifest.dependencies {
            let candidate = self.entry(dependency)?;
            if !VersionReq::parse(requirement)
                .map_err(|e| e.to_string())?
                .matches(&Version::parse(&candidate.manifest.version).map_err(|e| e.to_string())?)
            {
                return Err(format!("Incompatible dependency: {dependency}"));
            }
            self.visit(dependency, visiting, ordered)?;
        }
        visiting.remove(id);
        ordered.push(entry);
        Ok(())
    }
    /// Resolve the complete selection before downloading or changing any state.
    pub fn plan(&self, ids: &[String], host: &str, platform: &str) -> Result<Vec<&Entry>> {
        self.validate()?;
        if ids.is_empty() || ids.len() > self.addons.len() {
            return Err("Invalid add-on selection".into());
        }
        let mut visiting = BTreeSet::new();
        let mut ordered = Vec::new();
        for id in ids {
            self.visit(id, &mut visiting, &mut ordered)?;
        }
        for entry in &ordered {
            entry.manifest.compatible(host, platform)?;
            entry.artifacts.get(platform).ok_or_else(|| {
                format!(
                    "No published package for {} on {platform}",
                    entry.manifest.id
                )
            })?;
        }
        Ok(ordered)
    }
}
