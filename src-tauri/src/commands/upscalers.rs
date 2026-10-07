use std::{collections::BTreeSet, fs, io::{Read, Seek, SeekFrom}, path::Path};

fn filename_signals(name: &str, found: &mut BTreeSet<String>) {
    match name {
        "nvngx_dlss.dll" | "sl.dlss.dll" => { found.insert("DLSS".into()); }
        "libxess.dll" | "libxess_dx11.dll" => { found.insert("XeSS".into()); }
        _ if name.starts_with("ffx_fsr2") && name.ends_with(".dll") => { found.insert("FSR 2".into()); }
        _ if name.starts_with("ffx_fsr3upscaler") && name.ends_with(".dll") => { found.insert("FSR 3".into()); }
        _ if name.starts_with("amd_fidelityfx_upscaler") && name.ends_with(".dll") => { found.insert("FSR".into()); }
        _ => {}
    }
}

fn binary_signals(bytes: &[u8], found: &mut BTreeSet<String>) {
    for (marker, label) in [
        ("nvngx_dlss.dll", "DLSS"), ("nvngx_dlss_evaluatefeature", "DLSS"),
        ("xessd3d12init", "XeSS"), ("xessd3d11init", "XeSS"), ("libxess.dll", "XeSS"),
        ("ffxfsr2contextcreate", "FSR 2"), ("ffxfsr3upscalercontextcreate", "FSR 3"),
    ] {
        if bytes.windows(marker.len()).any(|window| window.eq_ignore_ascii_case(marker.as_bytes())) {
            found.insert(label.into());
        }
    }
}

// Never walk game archives or read entire executables. Files installed by our mod
// are excluded, so its own upscaler libraries cannot establish game support.
pub fn detect(root: &Path, executable: Option<&Path>) -> Vec<String> {
    let mut found = BTreeSet::new();
    let mut installed = BTreeSet::new();
    let mut queue = vec![(root.to_path_buf(), 0)];
    let mut entries = 0;
    let mut files_read = 0;
    let mut byte_budget = 8 * 1024 * 1024;
    if let Some(exe) = executable {
        if let Ok(mut file) = fs::File::open(exe) {
            let length = file.metadata().map(|meta| meta.len()).unwrap_or(0);
            let mut bytes = Vec::new();
            let _ = (&mut file).take(2 * 1024 * 1024).read_to_end(&mut bytes);
            if length > 2 * 1024 * 1024 && file.seek(SeekFrom::Start(length.saturating_sub(2 * 1024 * 1024).max(2 * 1024 * 1024))).is_ok() {
                let _ = (&mut file).take(2 * 1024 * 1024).read_to_end(&mut bytes);
            }
            byte_budget -= bytes.len();
            files_read += 1;
            binary_signals(&bytes, &mut found);
        }
    }
    while let Some((dir, depth)) = queue.pop() {
        // Manifests are relative to the installation folder, including nested EXEs.
        let mut manifest = String::new();
        if let Ok(file) = fs::File::open(dir.join("dlssnr-amd-install.txt")) {
            let _ = file.take(65536).read_to_string(&mut manifest);
        }
        for line in manifest.lines() {
            installed.insert(dir.join(line.trim().replace('\\', "/")).to_string_lossy().to_ascii_lowercase());
        }
        let Ok(children) = fs::read_dir(&dir) else { continue };
        for child in children {
            entries += 1;
            if entries > 8000 { return found.into_iter().collect(); }
            let Ok(child) = child else { continue };
            let path = child.path();
            let Ok(kind) = child.file_type() else { continue };
            if kind.is_symlink() { continue; }
            let name = child.file_name().to_string_lossy().to_ascii_lowercase();
            let relative = path.to_string_lossy().to_ascii_lowercase();
            if installed.contains(&relative) || ["optiscaler.dll", "dlssnr_core.dll", "nvngx.dll_dlssnr.dll", "nvngx_dlssnr.dll", "dlssnr-launcher-original-dxgi.dll"].contains(&name.as_str()) { continue; }
            if kind.is_dir() {
                if depth < 4 && !["optiscaler", "dlssnr-amd", "reshade-shaders", ".git", "videos", "movies"].contains(&name.as_str()) {
                    queue.push((path, depth + 1));
                }
            } else if kind.is_file() {
                filename_signals(&name, &mut found);
                // Read only the main executable and small original DLLs, at most 8 MiB total.
                if files_read < 8 && byte_budget > 0 && name.ends_with(".dll") && child.metadata().map(|m| m.len() <= 512 * 1024).unwrap_or(false) {
                    if let Ok(file) = fs::File::open(&path) {
                        let limit = (512 * 1024).min(byte_budget);
                        let mut bytes = Vec::new();
                        let _ = file.take(limit as u64).read_to_end(&mut bytes);
                        byte_budget -= bytes.len();
                        files_read += 1;
                        binary_signals(&bytes, &mut found);
                    }
                }
            }
        }
    }
    found.into_iter().collect()
}
