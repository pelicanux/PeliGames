#[cfg(feature = "normalize")]
mod conversion;
#[cfg(feature = "normalize")]
pub use conversion::*;
#[cfg(any(feature = "tar-xz", feature = "tar-gz"))]
mod tar_xz;
#[cfg(feature = "tar-xz")]
pub use tar_xz::unpack_tar_xz;

#[cfg(feature = "tar-gz")]
pub fn unpack_tar_gz(bytes: &[u8], stage: &std::path::Path) -> Result<(), String> {
    tar_xz::unpack_tar(
        flate2::read::GzDecoder::new(bytes),
        stage,
        2 * 1024 * 1024 * 1024,
    )
}
