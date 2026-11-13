use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Fixture(pub PathBuf);
impl Fixture {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "peligames-afmf-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub fn runtime(&self, version: &str) -> PathBuf {
        let path = self.0.join("afmf/versions").join(version);
        fs::create_dir_all(&path).unwrap();
        let mut elf = [0u8; 64];
        elf[..5].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2]);
        elf[18] = 62;
        fs::write(path.join("libafmf-linux.so"), elf).unwrap();
        fs::write(path.join("afmf-linux.json"), Self::MANIFEST).unwrap();
        path
    }
    pub const MANIFEST: &'static str = r#"{"layer":{"name":"VK_LAYER_AFMF","library_path":"libafmf-linux.so","enable_environment":{"AFMF_ENABLE":"1"}}}"#;
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
