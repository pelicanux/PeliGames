use std::path::Path;
#[cfg(feature = "tar-xz")]
pub fn unpack_tar_xz(bytes: &[u8], stage: &Path) -> Result<(), String> {
    let decoder = xz2::read::XzDecoder::new(bytes);
    unpack_tar(decoder, stage, 256 * 1024 * 1024)
}
pub(crate) fn unpack_tar(
    reader: impl std::io::Read,
    stage: &Path,
    limit: u64,
) -> Result<(), String> {
    let mut archive = tar::Archive::new(reader);
    let mut total = 0u64;
    for (index, item) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
        if index >= 10000 {
            return Err("Too many archive entries".into());
        }
        let mut item = item.map_err(|e| e.to_string())?;
        let rel = item.path().map_err(|e| e.to_string())?.into_owned();
        if rel.is_absolute()
            || rel.components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
        {
            return Err("Caminho inseguro no pacote.".into());
        }
        let kind = item.header().entry_type();
        if !(kind.is_dir() || kind.is_file()) {
            return Err("O pacote contém links ou arquivos especiais não permitidos.".into());
        }
        total = total
            .checked_add(item.size())
            .ok_or("Pacote excede o limite.")?;
        if total > limit {
            return Err("Pacote excede o limite.".into());
        }
        item.unpack_in(stage).map_err(|e| e.to_string())?;
    }
    Ok(())
}
