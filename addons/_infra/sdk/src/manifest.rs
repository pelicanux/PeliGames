use crate::Result;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT_VERSION: u32 = 1;
pub const HOST_API: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub host_api: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    /// SPDX expression for the integration; bundled dependencies keep their notices.
    pub license: String,
    pub repository: String,
    pub host_version: String,
    pub platforms: Vec<String>,
    pub entrypoints: Entrypoints,
    pub contributions: Vec<Contribution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Component data may keep its existing organized location during extraction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_directory: Option<String>,
    pub dependencies: BTreeMap<String, String>,
    /// Descriptive capabilities for review, not an operating-system sandbox.
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entrypoints {
    pub ui: Option<String>,
    pub service: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub target: Target,
    pub route: String,
    pub title: String,
    /// Module-owned card descriptions, keyed by UI language tag.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub descriptions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Settings,
    GameMods,
    GameFrameGeneration,
    GameCaptures,
    GameOverview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Mods,
    FrameGeneration,
    Captures,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    GameFiles,
    ComponentDownloads,
    LaunchProfiles,
    NativeLaunch,
    BackgroundService,
}

pub fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || value.split('-').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
        || !value.as_bytes()[0].is_ascii_lowercase()
    {
        return Err(format!("Invalid add-on identifier: {value}"));
    }
    Ok(())
}

/// One portable interpretation on Linux and Windows; reject ambiguous ZIP names.
pub fn relative_file(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 240
        || !value.is_ascii()
        || value.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with('.')
                || part.ends_with(' ')
                || part
                    .bytes()
                    .any(|c| c.is_ascii_control() || b"\\:<>\"|?*".contains(&c))
        })
    {
        return Err(format!("Invalid package path: {value}"));
    }
    Ok(())
}

impl Manifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != FORMAT_VERSION || self.host_api != HOST_API {
            return Err("Unsupported add-on format or host API".into());
        }
        identifier(&self.id)?;
        if let Some(group) = &self.group {
            identifier(group)?;
        }
        if let Some(directory) = &self.component_directory {
            relative_file(directory)?;
            if !directory.starts_with("components/") {
                return Err("Add-on component data must stay inside components".into());
            }
        }
        let version = Version::parse(&self.version).map_err(|e| e.to_string())?;
        if self.version.len() > 120
            || version.to_string() != self.version
            || !version.build.is_empty()
        {
            return Err("Add-on versions must be canonical SemVer without build metadata".into());
        }
        if self.host_version.len() > 120 {
            return Err("Host compatibility requirement is too long".into());
        }
        VersionReq::parse(&self.host_version).map_err(|e| e.to_string())?;
        for (name, value, limit) in [
            ("name", &self.name, 80),
            ("summary", &self.summary, 240),
            ("author", &self.author, 120),
            ("license", &self.license, 120),
        ] {
            if value.trim().is_empty() || value.len() > limit || value.chars().any(char::is_control)
            {
                return Err(format!("Invalid add-on {name}"));
            }
        }
        if !self.repository.starts_with("https://")
            || self.repository.len() > 300
            || self.repository.chars().any(char::is_whitespace)
        {
            return Err("An HTTPS project repository is required".into());
        }
        if self.platforms.is_empty()
            || self.platforms.len() > 4
            || self.platforms.iter().any(|p| {
                ![
                    "linux-x86_64",
                    "linux-aarch64",
                    "windows-x86_64",
                    "windows-aarch64",
                ]
                .contains(&p.as_str())
            })
            || self.platforms.iter().collect::<BTreeSet<_>>().len() != self.platforms.len()
        {
            return Err("Invalid add-on platforms".into());
        }
        if self.entrypoints.ui.is_none() && self.entrypoints.service.is_none() {
            return Err("An add-on needs at least one entrypoint".into());
        }
        if self.capabilities.contains(&Capability::NativeLaunch)
            && self.entrypoints.service.is_none()
        {
            return Err("A native launch integration needs a service entrypoint".into());
        }
        for path in [&self.entrypoints.ui, &self.entrypoints.service]
            .into_iter()
            .flatten()
        {
            relative_file(path)?;
        }
        if self
            .entrypoints
            .ui
            .as_ref()
            .is_some_and(|p| !p.ends_with(".js"))
        {
            return Err("The UI entrypoint must be a compiled JavaScript module".into());
        }
        if self.contributions.len() > 16 || self.dependencies.len() > 32 {
            return Err("Too many add-on contributions or dependencies".into());
        }
        let mut routes = BTreeSet::new();
        for c in &self.contributions {
            identifier(&c.route)?;
            if self.entrypoints.ui.is_none()
                || !routes.insert(&c.route)
                || c.title.trim().is_empty()
                || c.title.len() > 80
            {
                return Err("Invalid or duplicate UI contribution".into());
            }
            if c.descriptions.len() > 32 || c.descriptions.iter().any(|(language, text)| {
                language.is_empty() || language.len() > 35
                    || language.split('-').any(|part| part.is_empty() || !part.bytes().all(|c| c.is_ascii_alphanumeric()))
                    || text.trim().is_empty() || text.len() > 320 || text.chars().any(char::is_control)
            }) {
                return Err("Invalid UI contribution descriptions".into());
            }
        }
        for (id, req) in &self.dependencies {
            identifier(id)?;
            if id == &self.id {
                return Err("An add-on cannot depend on itself".into());
            }
            if req.len() > 120 {
                return Err("Dependency requirement is too long".into());
            }
            VersionReq::parse(req).map_err(|e| e.to_string())?;
        }
        if self.capabilities.iter().collect::<BTreeSet<_>>().len() != self.capabilities.len() {
            return Err("Duplicate add-on capability".into());
        }
        Ok(())
    }

    pub fn compatible(&self, host: &str, platform: &str) -> Result<()> {
        self.validate()?;
        if !self.platforms.iter().any(|p| p == platform)
            || !VersionReq::parse(&self.host_version)
                .map_err(|e| e.to_string())?
                .matches(&Version::parse(host).map_err(|e| e.to_string())?)
        {
            return Err(format!(
                "{} is incompatible with this launcher or platform",
                self.id
            ));
        }
        Ok(())
    }
}
