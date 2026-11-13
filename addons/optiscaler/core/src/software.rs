//! File inventory, deliberately separate from in-game activation or GPU support.
use super::installation::safe_target;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct Component {
    pub id: &'static str,
    pub present: bool,
    pub integrated: bool,
    pub version: Option<String>,
}

pub fn inventory(root: &Path, proxy: Option<&str>, version: Option<String>) -> Vec<Component> {
    let present = |names: &[&str]| {
        names
            .iter()
            .any(|name| safe_target(root, name).is_ok_and(|path| path.is_file()))
    };
    let optiscaler = proxy.is_some_and(|name| present(&[name]));
    let text = super::config_path(root)
        .and_then(|path| super::read_file(&path))
        .unwrap_or_default();
    let parsed = peligames_mod_config::ini::sections(&text);
    let integrated = optiscaler
        && peligames_mod_config::ini::get(&parsed, "fakenvapi", "UseFakenvapi").is_some();
    let mut items = vec![Component {
        id: "optiscaler",
        present: optiscaler,
        integrated: false,
        version: optiscaler.then_some(version).flatten(),
    }];
    for (id, names) in [
        ("dlss", &["nvngx_dlss.dll", "OptiScaler/nvngx_dlss.dll"][..]),
        (
            "fsr",
            &[
                "amd_fidelityfx_dx12.dll",
                "amd_fidelityfx_upscaler_dx12.dll",
                "amd_fidelityfx_upscaler_vk.dll",
                "OptiScaler/amd_fidelityfx_dx12.dll",
                "OptiScaler/amd_fidelityfx_upscaler_dx12.dll",
                "OptiScaler/amd_fidelityfx_upscaler_vk.dll",
            ][..],
        ),
        (
            "xess",
            &[
                "libxess.dll",
                "libxess_dx11.dll",
                "OptiScaler/libxess.dll",
                "OptiScaler/libxess_dx11.dll",
            ][..],
        ),
        (
            "fakenvapi",
            &["fakenvapi.dll", "OptiScaler/fakenvapi.dll"][..],
        ),
        (
            "streamline",
            &[
                "sl.interposer.dll",
                "OptiScaler/streamline/sl.interposer.dll",
            ][..],
        ),
        (
            "optipatcher",
            &["OptiPatcher.asi", "OptiScaler/plugins/OptiPatcher.asi"][..],
        ),
    ] {
        items.push(Component {
            id,
            present: present(names),
            integrated: id == "fakenvapi" && integrated,
            version: None,
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_uses_local_files_never_the_other_channel_or_game_activity() {
        let root = std::env::temp_dir().join(format!(
            "peligames-software-status-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("OptiScaler/plugins")).unwrap();
        std::fs::write(root.join("dxgi.dll"), "proxy").unwrap();
        std::fs::write(
            root.join("OptiScaler.ini"),
            "[fakenvapi]\nUseFakenvapi=auto\n",
        )
        .unwrap();
        std::fs::write(root.join("OptiScaler/libxess.dll"), "library").unwrap();
        std::fs::write(root.join("OptiScaler/plugins/OptiPatcher.asi"), "plugin").unwrap();
        let inventory = inventory(&root, Some("dxgi.dll"), Some("0.9.4".into()));
        let item = |id| inventory.iter().find(|item| item.id == id).unwrap();
        assert_eq!(item("optiscaler").version.as_deref(), Some("0.9.4"));
        assert!(item("xess").present && item("xess").version.is_none());
        assert!(item("optipatcher").present);
        assert!(item("fakenvapi").integrated);
        assert!(!item("dlss").present && !item("fsr").present);
        std::os::unix::fs::symlink(root.join("dxgi.dll"), root.join("nvngx_dlss.dll")).unwrap();
        assert!(!super::inventory(&root, None, Some("10.0".into()))
            .iter()
            .any(|item| item.id == "dlss" && item.present));
        assert!(super::inventory(&root, None, Some("10.0".into()))[0]
            .version
            .is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
