//! Read-only preparation checks. Native MAKO binaries must never wrap a Flatpak sandbox.
use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const EXTENSION: &str = "org.freedesktop.Platform.VulkanLayer.makorender";
const LAYERS: &str = "/usr/lib/extensions/vulkan/makorender/share/vulkan/implicit_layer.d";

pub(super) fn app_id(game_key: &str) -> Result<&'static str, String> {
    let mut parts = game_key.split(':');
    let launcher = parts.next().unwrap_or_default();
    if parts.next() != Some("flatpak") {
        return Err("Identificação Flatpak inválida.".into());
    }
    match launcher {
        "Steam" => Ok("com.valvesoftware.Steam"),
        "Heroic" => Ok("com.heroicgameslauncher.hgl"),
        "Lutris" => Ok("net.lutris.Lutris"),
        "Faugus" => Ok("io.github.Faugus.faugus-launcher"),
        "Bottles" => Ok("com.usebottles.bottles"),
        _ => Err("Este launcher Flatpak ainda não tem integração MAKO.".into()),
    }
}

fn command(args: &[String]) -> Result<String, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path =
        std::env::temp_dir().join(format!("peligames-flatpak-check-{}", uuid::Uuid::new_v4()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let _cleanup = Cleanup(path.clone());
    let mut cmd = Command::new("flatpak");
    crate::core::env_utils::host_environment(&mut cmd);
    let mut child = cmd
        .args(args)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "O comando Flatpak não está disponível.")?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => break,
            Some(_) => {
                return Err(
                    "O Flatpak não confirmou o aplicativo, a extensão ou o acesso aos arquivos."
                        .into(),
                )
            }
            None if started.elapsed() > Duration::from_secs(12) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("A verificação do Flatpak expirou.".into());
            }
            None => std::thread::sleep(Duration::from_millis(40)),
        }
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(128 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "Resposta Flatpak inválida.".into())
}

fn runtime_branch(runtime: &str, metadata: &str) -> Option<String> {
    let parts: Vec<_> = runtime.trim().split('/').collect();
    if parts.len() != 3 || parts[1] != "x86_64" {
        return None;
    }
    let supported = |v: &str| ["23.08", "24.08", "25.08"].contains(&v);
    let mut section = false;
    for line in metadata.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line == "[Extension org.freedesktop.Platform.VulkanLayer]";
            continue;
        }
        if section {
            if let Some(values) = line
                .strip_prefix("version=")
                .or_else(|| line.strip_prefix("versions="))
            {
                if let Some(branch) = values.split(';').find(|v| supported(v)) {
                    return Some(branch.into());
                }
            }
        }
    }
    // Freedesktop itself names the extension ABI; KDE/GNOME version numbers do not.
    (parts[0] == "org.freedesktop.Platform" && supported(parts[2])).then(|| parts[2].into())
}

#[derive(Serialize)]
pub struct Status {
    pub app_id: String,
    pub runtime: String,
    pub branch: Option<String>,
    pub extension_installed: bool,
    pub ready: bool,
    pub message: String,
    pub commands: Vec<String>,
    pub release_url: String,
}

pub(super) fn inspect(game_key: &str, dll: &str, version: &str) -> Result<Status, String> {
    let app = app_id(game_key)?;
    let runtime = command(&["info".into(), "--show-runtime".into(), app.into()])?
        .trim()
        .to_string();
    let metadata = command(&["info".into(), "--show-metadata".into(), runtime.clone()])?;
    let branch = runtime_branch(&runtime, &metadata);
    let extension_installed = branch.as_ref().is_some_and(|branch| {
        command(&[
            "info".into(),
            "--show-commit".into(),
            format!("{EXTENSION}//{branch}"),
        ])
        .is_ok()
    });
    let profiles = super::root()?.join("profiles");
    fs::create_dir_all(&profiles).map_err(|e| e.to_string())?;
    let valid_dll = Path::new(dll).is_absolute() && super::dll_valid(Path::new(dll));
    let ready = extension_installed && valid_dll && command(&[
        "run".into(),"--command=sh".into(),app.into(),"-c".into(),
        "test -r \"$1/VkLayer_MAKO_render.json\" && test -d \"$2\" && test -r \"$2\" && test -w \"$2\" && test -r \"$3\"".into(),
        "peligames-check".into(),LAYERS.into(),profiles.to_string_lossy().into(),dll.into(),
    ]).is_ok();
    let mut commands = Vec::new();
    if let Some(branch) = &branch {
        commands.push(format!("flatpak install --user ./org.freedesktop.Platform.VulkanLayer.makorender-{branch}.flatpak"));
        commands.push(format!(
            "flatpak override --user --filesystem={} {app}",
            super::quote(&format!("{}:rw", profiles.display()))
        ));
        if valid_dll {
            commands.push(format!(
                "flatpak override --user --filesystem={} {app}",
                super::quote(&format!(
                    "{}:ro",
                    Path::new(dll).parent().unwrap().display()
                ))
            ));
        }
    }
    Ok(Status { app_id:app.into(), runtime, branch:branch.clone(), extension_installed, ready,
        message: if ready {"Extensão e acesso ao perfil e à DLL confirmados no sandbox."}
        else if branch.is_none() {"Runtime ou arquitetura sem extensão MAKO compatível. Não selecione uma ramificação por aproximação."}
        else if !valid_dll {"Selecione primeiro a Lossless.dll da sua instalação legítima do Lossless Scaling."}
        else {"Instale a extensão da mesma versão do MAKO e conceda acesso às duas pastas indicadas. Feche o launcher e seus jogos antes do preparo."}.into(),
        commands, release_url:format!("https://github.com/eugeniosegala/MAKO/releases/tag/render-v{version}"),
    })
}

#[tauri::command]
pub async fn frame_generation_flatpak_status(
    game_key: String,
    dll: String,
    version: String,
) -> Result<Status, String> {
    if !super::versions::compatible("mako", &version) {
        return Err("Versão MAKO inválida.".into());
    }
    tauri::async_runtime::spawn_blocking(move || inspect(&game_key, &dll, &version))
        .await
        .map_err(|e| e.to_string())?
}

pub(super) fn wrapper(config: &Path) -> String {
    // The same script runs inside Steam's sandbox, or adds per-invocation flags to a
    // fully closed external Flatpak launcher. Never invoke host mako-launch here.
    let environment = [
        ("MAKO_CONFIG", config.to_string_lossy().into_owned()),
        ("MAKO_PROFILE", "PeliGames".into()),
        ("ENABLE_MAKO", "1".into()),
        ("DISABLE_MAKO", "0".into()),
        ("DISABLE_LSFG", "1".into()),
        ("DISABLE_LSFGVK", "1".into()),
        ("DISABLE_GAMESCOPE_WSI", "1".into()),
        ("DISABLE_VKBASALT", "1".into()),
        ("MAKO_DISABLE_HDR_EXPOSURE", "1".into()),
        ("VK_IMPLICIT_LAYER_PATH", LAYERS.into()),
    ];
    let mut script = "#!/bin/sh\n# PeliGames MAKO profile for a prepared Flatpak sandbox.\nunset MAKO_ENV ENABLE_GAMESCOPE_WSI ENABLE_VKBASALT DXVK_HDR VK_ADD_IMPLICIT_LAYER_PATH\n".to_string();
    for (key, value) in &environment {
        script.push_str(&format!("export {key}={}\n", super::quote(value)));
    }
    script.push_str("case \"$1\" in flatpak|*/flatpak) program=$1; shift; if [ \"$1\" != run ]; then echo 'MAKO requires flatpak run' >&2; exit 2; fi; shift\nexec \"$program\" run ");
    for (key, value) in &environment {
        script.push_str(&format!(
            "{} ",
            super::quote(&format!("--env={key}={value}"))
        ));
    }
    for key in [
        "MAKO_ENV",
        "ENABLE_GAMESCOPE_WSI",
        "ENABLE_VKBASALT",
        "DXVK_HDR",
        "VK_ADD_IMPLICIT_LAYER_PATH",
    ] {
        script.push_str(&format!("--unset-env={key} "));
    }
    script.push_str("\"$@\";;\n*) exec \"$@\";;\nesac\n");
    script
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_extension_abi_instead_of_desktop_runtime_version() {
        assert_eq!(
            runtime_branch(
                "org.kde.Platform/x86_64/6.9",
                "[Extension org.freedesktop.Platform.VulkanLayer]\nversions=25.08;24.08;\n"
            ),
            Some("25.08".into())
        );
        assert_eq!(
            runtime_branch("org.freedesktop.Platform/x86_64/24.08", ""),
            Some("24.08".into())
        );
        assert!(runtime_branch(
            "org.gnome.Platform/aarch64/49",
            "[Extension org.freedesktop.Platform.VulkanLayer]\nversion=25.08"
        )
        .is_none());
        assert!(
            runtime_branch("org.kde.Platform/x86_64/6.9", "[Runtime]\nversion=25.08").is_none()
        );
        assert!(app_id("Steam:native:42").is_err());
    }
    #[test]
    fn sandbox_wrapper_forwards_paths_and_arguments_without_using_host_layers() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("peligames-mako-sandbox-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("profile.sh");
        let config = root.join("config with ' quote.toml");
        fs::write(&path, wrapper(&config)).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let output=Command::new(&path).args(["/usr/bin/python3","-c","import os,sys,json; print(json.dumps([sys.argv[1:],os.environ['MAKO_CONFIG'],os.environ['VK_IMPLICIT_LAYER_PATH']]))","arg with spaces","$(not-a-command)"]).output().unwrap();
        assert!(output.status.success());
        let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            data[0],
            serde_json::json!(["arg with spaces", "$(not-a-command)"])
        );
        assert_eq!(data[1], config.to_string_lossy().as_ref());
        assert_eq!(data[2], LAYERS);
        let fake = root.join("flatpak");
        fs::write(&fake, b"#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(&path)
            .arg(fake)
            .args(["run", "com.valvesoftware.Steam", "arg with spaces"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let output = String::from_utf8(output.stdout).unwrap();
        assert!(output
            .lines()
            .any(|l| l == format!("--env=MAKO_CONFIG={}", config.display())));
        assert!(output.lines().any(|l| l == "--unset-env=MAKO_ENV"));
        assert!(output.ends_with("com.valvesoftware.Steam\narg with spaces\n"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sandbox_wrapper_does_not_reference_host_runtime() {
        let text = wrapper(Path::new("/home/test/config with ' quote.toml"));
        assert!(!text.contains("mako-launch"));
        assert!(text.contains("VK_IMPLICIT_LAYER_PATH"));
        assert!(text.contains("--unset-env=MAKO_ENV"));
        assert!(!text.contains("flatpak override"));
    }
}
