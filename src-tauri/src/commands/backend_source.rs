//! Release providers for DLSSNR components. The installer consumes one shared catalogue.
use super::backend_release;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug)]
pub(super) struct Asset {
    pub name: String,
    pub url: String,
    pub sha256: Option<String>,
}

#[derive(Debug)]
pub(super) struct Release {
    pub tag_name: String,
    name: Option<String>,
    description: Option<String>,
    unavailable: bool,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn linux_assets(&self) -> Vec<(&Asset, &'static str)> {
        let windows = backend_release::windows_release(
            &self.tag_name,
            self.name.as_deref().unwrap_or(""),
            self.description.as_deref().unwrap_or(""),
        );
        self.assets
            .iter()
            .filter_map(|asset| {
                backend_release::linux_bitness(&asset.name, windows).map(|bits| (asset, bits))
            })
            .collect()
    }
    fn compatible(&self) -> bool {
        !self.unavailable && !self.tag_name.trim().is_empty() && !self.linux_assets().is_empty()
    }
}

#[derive(Clone, Copy)]
enum Source {
    GitHub,
    GitLab,
}

impl Source {
    fn for_arch(arch: &str) -> Result<Self, String> {
        match arch {
            "rdna3" => Ok(Self::GitHub),
            "rdna4" => Ok(Self::GitLab),
            _ => Err("Arquitetura de backend inválida.".into()),
        }
    }
    fn page(self, page: usize) -> String {
        match self {
            Self::GitHub => format!("https://api.github.com/repos/mauri870/DLSSNR-RDNA3/releases?per_page=30&page={page}"),
            Self::GitLab => format!("https://gitlab.com/api/v4/projects/88319462/releases?per_page=30&page={page}&order_by=released_at&sort=desc"),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
        }
    }
    fn decode(self, body: &[u8]) -> Result<Vec<Release>, String> {
        match self {
            Self::GitHub => serde_json::from_slice::<Vec<GitHubRelease>>(body)
                .map(|items| items.into_iter().map(Release::from).collect()),
            Self::GitLab => serde_json::from_slice::<Vec<GitLabRelease>>(body)
                .map(|items| items.into_iter().map(Release::from).collect()),
        }.map_err(|_| format!("O {} não retornou uma lista válida de releases do backend. Os arquivos instalados foram preservados.", self.label()))
    }
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}
#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    assets: Vec<GitHubAsset>,
}
impl From<GitHubRelease> for Release {
    fn from(r: GitHubRelease) -> Self {
        Self {
            tag_name: r.tag_name,
            name: r.name,
            description: r.body,
            unavailable: r.draft,
            assets: r
                .assets
                .into_iter()
                .map(|a| Asset {
                    name: a.name,
                    url: a.browser_download_url,
                    sha256: a
                        .digest
                        .and_then(|d| d.strip_prefix("sha256:").map(str::to_owned)),
                })
                .collect(),
        }
    }
}

#[derive(Deserialize)]
struct GitLabLink {
    name: String,
    url: String,
}
#[derive(Deserialize)]
struct GitLabAssets {
    #[serde(default)]
    links: Vec<GitLabLink>,
}
#[derive(Deserialize)]
struct GitLabRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    upcoming_release: bool,
    assets: GitLabAssets,
}
impl From<GitLabRelease> for Release {
    fn from(r: GitLabRelease) -> Self {
        Self {
            tag_name: r.tag_name,
            name: r.name,
            description: r.description,
            unavailable: r.upcoming_release,
            // GitLab sources are repository snapshots, not installable release packages.
            // Use each publisher-supplied link directly, including Generic Package URLs.
            assets: r
                .assets
                .links
                .into_iter()
                .map(|a| Asset {
                    name: a.name,
                    url: a.url,
                    sha256: None,
                })
                .collect(),
        }
    }
}

/// Version checks and downloads select the same newest compatible Linux release,
/// including previews. A Windows-only release never replaces a Linux backend.
pub(super) async fn linux_release(client: &Client, arch: &str) -> Result<Release, String> {
    let source = Source::for_arch(arch)?;
    for page in 1..=5 {
        let response = client
            .get(source.page(page))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| format!("Falha ao consultar o backend no {}: {e}", source.label()))?;
        let releases = source.decode(&response.bytes().await.map_err(|e| e.to_string())?)?;
        let count = releases.len();
        if let Some(release) = releases.into_iter().find(Release::compatible) {
            let assets = release.linux_assets();
            for (asset, _) in &assets {
                let url = reqwest::Url::parse(&asset.url)
                    .map_err(|_| "URL inválida no pacote do backend.")?;
                if url.scheme() != "https" || url.host_str().is_none() {
                    return Err("O pacote do backend deve ter uma URL HTTPS válida.".into());
                }
            }
            for bits in ["32", "64"] {
                if assets.iter().filter(|(_, b)| *b == bits).count() > 1 {
                    return Err(format!("A release tem mais de um pacote Linux de {bits} bits; não foi possível escolher com segurança."));
                }
            }
            return Ok(release);
        }
        if count < 30 {
            break;
        }
    }
    Err("Nenhuma release com pacotes Linux compatíveis foi encontrada. O backend instalado foi preservado.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gitlab_mixed_release_uses_package_links_not_source_snapshots() {
        let body = json!([{"tag_name":"v0.0.8.1", "name":"DLSSNR-AMD 0.0.8.1",
        "description":"This release is for Linux and Windows.", "upcoming_release":false,
        "assets":{"sources":[{"format":"tar.gz","url":"https://gitlab.com/source.tar.gz"}],"links":[
            {"name":"DLSSNR-AMD-lang-CJK-0.0.8.1.zip","url":"https://gitlab.com/lang.zip"},
            {"name":"DLSSNR-AMD-Vulkan-Windows-0.0.8.1-preview-x86_64.zip","url":"https://gitlab.com/windows.zip"},
            {"name":"DLSSNR-AMD-Vulkan-Linux-0.0.8.1-x86_64.tar.gz","url":"https://gitlab.com/api/v4/projects/88319462/packages/generic/dlssnr-amd/0.0.8.1/linux64.tar.gz"},
            {"name":"DLSSNR-AMD-Vulkan-Linux-0.0.8.1-i686.tar.gz","url":"https://gitlab.com/linux32.tar.gz"}
        ]}}]);
        let releases = Source::GitLab
            .decode(&serde_json::to_vec(&body).unwrap())
            .unwrap();
        let r = &releases[0];
        assert!(r.compatible());
        assert_eq!(r.tag_name, "v0.0.8.1");
        let assets = r.linux_assets();
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].1, "64");
        assert!(assets[0].0.url.contains("/packages/generic/"));
        assert_eq!(assets[1].1, "32");
    }

    #[test]
    fn github_rdna3_still_accepts_previews_and_reads_checksums() {
        let body = json!([{"tag_name":"v0.0.6-preview","prerelease":true,"draft":false,"assets":[{
            "name":"DLSSNR-AMD-Vulkan-Linux-0.0.6-x86_64-rdna3.tar.gz",
            "browser_download_url":"https://github.com/mauri870/archive.tar.gz", "digest":format!("sha256:{}","a".repeat(64))
        }]}]);
        let releases = Source::GitHub
            .decode(&serde_json::to_vec(&body).unwrap())
            .unwrap();
        assert!(releases[0].compatible());
        assert_eq!(
            releases[0].assets[0].sha256.as_deref(),
            Some("a".repeat(64).as_str())
        );
        let mut r = Source::GitHub
            .decode(&serde_json::to_vec(&body).unwrap())
            .unwrap()
            .remove(0);
        r.unavailable = true;
        assert!(!r.compatible());
    }

    #[test]
    fn gitlab_future_windows_and_empty_releases_are_skipped() {
        for (upcoming, name) in [
            (true, "DLSSNR-AMD-Vulkan-Linux-0.0.8.1-x86_64.tar.gz"),
            (false, "DLSSNR-AMD-Vulkan-Windows-0.0.8.1-x86_64.zip"),
        ] {
            let body = json!([{"tag_name":"v0.0.8.1","upcoming_release":upcoming,"assets":{"links":[{"name":name,"url":"https://gitlab.com/file"}]}}]);
            assert!(!Source::GitLab
                .decode(&serde_json::to_vec(&body).unwrap())
                .unwrap()[0]
                .compatible());
        }
        assert!(Source::GitLab.decode(br#"[]"#).unwrap().is_empty());
        for body in [
            b"<html>Sign in</html>".as_slice(),
            br#"{"message":"404 Project Not Found"}"#,
        ] {
            assert!(Source::GitLab.decode(body).is_err());
        }
    }

    #[test]
    fn architecture_selects_only_its_official_provider() {
        assert!(Source::for_arch("rdna4")
            .unwrap()
            .page(2)
            .contains("gitlab.com/api/v4/projects/88319462/releases?per_page=30&page=2"));
        assert!(Source::for_arch("rdna3")
            .unwrap()
            .page(1)
            .contains("api.github.com/repos/mauri870/"));
        assert!(Source::for_arch("../rdna4").is_err());
    }
}
