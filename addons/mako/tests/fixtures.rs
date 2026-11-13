use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
pub struct Fixture(pub PathBuf);
impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "peligames-mako-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub fn runtime(&self) -> PathBuf {
        let path = self.0.join("mako/versions/4.5.0");
        for file in [
            "bin/mako-launch",
            "bin/mako-cli",
            "lib/libmako-render.so",
            "lib32/libmako-render.so",
            "share/mako-render/vulkan/implicit_layer.d/VkLayer_MAKO_render.json",
        ] {
            let file = path.join(file);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, b"fixture").unwrap();
        }
        let launcher = path.join("bin/mako-launch");
        fs::write(&launcher,b"#!/bin/sh\nexport VK_INSTANCE_LAYERS=\"VK_LAYER_MAKO_render${VK_INSTANCE_LAYERS:+:$VK_INSTANCE_LAYERS}\"\nexec \"$@\"\n").unwrap();
        fs::set_permissions(launcher, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    pub fn dll(&self) -> PathBuf {
        let path = self.0.join("Lossless.dll");
        fs::write(&path, b"MZfixture").unwrap();
        path
    }
    pub fn profile(&self) -> PathBuf {
        self.runtime();
        peligames_mako::profiles::prepare(
            &self.0,
            "fixture-game",
            "Fixture Game",
            &self.0,
            peligames_mako::Settings {
                enabled: true,
                dll: self.dll().to_string_lossy().into(),
                ..Default::default()
            },
            None,
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
