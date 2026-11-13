use peligames_addon_kit::{catalog::Catalog, manifest::Manifest, package, Result};
use std::{fs, io::Write, path::Path};

fn run(args: &[String]) -> Result<()> {
    match args {
        [command, file] if command == "validate-manifest" => {
            let manifest: Manifest = serde_json::from_slice(&fs::read(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            manifest.validate()?;
            println!("{} {}: valid", manifest.id, manifest.version);
        }
        [command, file] if command == "validate-catalog" => {
            let catalog: Catalog = serde_json::from_slice(&fs::read(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            catalog.validate()?;
            println!("{} catalog entries: valid", catalog.addons.len());
        }
        [command, manifest, payload, output] if command == "pack" => {
            let manifest: Manifest = serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let bytes = package::pack(manifest, &Path::new(payload).canonicalize().map_err(|e| e.to_string())?)?;
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(output).map_err(|e| e.to_string())?;
            if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
                drop(file);
                fs::remove_file(output).map_err(|e| format!("{error}; output cleanup failed: {e}"))?;
                return Err(error.to_string());
            }
            println!("{}", serde_json::json!({"file":output, "bytes":bytes.len(), "sha256":package::digest(&bytes)}));
        }
        _ => return Err("Usage: peligames-addon validate-manifest FILE | validate-catalog FILE | pack MANIFEST PAYLOAD OUTPUT.zip".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
