//! Read PE icon resources without executing Windows code or loading entire EXEs.
//! Format reference: Microsoft PE/COFF resource directory specification.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
const MAX_READ: usize = 32 * 1024 * 1024;
const MAX_ICON: usize = 4 * 1024 * 1024;
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid PE icon resource")
}
fn u16_at(b: &[u8], o: usize) -> io::Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(o..o.checked_add(2).ok_or_else(invalid)?)
            .ok_or_else(invalid)?
            .try_into()
            .unwrap(),
    ))
}
fn u32_at(b: &[u8], o: usize) -> io::Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(o..o.checked_add(4).ok_or_else(invalid)?)
            .ok_or_else(invalid)?
            .try_into()
            .unwrap(),
    ))
}
struct Section {
    rva: u32,
    size: u32,
    offset: u32,
}
struct Pe {
    file: fs::File,
    len: u64,
    budget: usize,
    sections: Vec<Section>,
    resource_rva: u32,
    resource_size: u32,
}
impl Pe {
    fn read(&mut self, offset: u64, size: usize) -> io::Result<Vec<u8>> {
        if size > self.budget || offset.checked_add(size as u64).ok_or_else(invalid)? > self.len {
            return Err(invalid());
        }
        self.budget -= size;
        self.file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; size];
        self.file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
    fn rva(&mut self, rva: u32, size: usize) -> io::Result<Vec<u8>> {
        let offset = self
            .sections
            .iter()
            .find_map(|section| {
                let delta = rva.checked_sub(section.rva)?;
                if u64::from(delta) + size as u64 > u64::from(section.size) {
                    return None;
                }
                Some(u64::from(section.offset) + u64::from(delta))
            })
            .ok_or_else(invalid)?;
        self.read(offset, size)
    }
    fn resource(&mut self, offset: u32, size: usize) -> io::Result<Vec<u8>> {
        if u64::from(offset) + size as u64 > u64::from(self.resource_size) {
            return Err(invalid());
        }
        self.rva(
            self.resource_rva.checked_add(offset).ok_or_else(invalid)?,
            size,
        )
    }
    fn directory(&mut self, offset: u32) -> io::Result<Vec<(u32, u32)>> {
        let header = self.resource(offset, 16)?;
        let count = usize::from(u16_at(&header, 12)?) + usize::from(u16_at(&header, 14)?);
        if count > 4096 {
            return Err(invalid());
        }
        let bytes = self.resource(offset.checked_add(16).ok_or_else(invalid)?, count * 8)?;
        (0..count)
            .map(|n| Ok((u32_at(&bytes, n * 8)?, u32_at(&bytes, n * 8 + 4)?)))
            .collect()
    }
    fn data(&mut self, offset: u32) -> io::Result<Vec<u8>> {
        if offset & 0x80000000 != 0 {
            return Err(invalid());
        }
        let bytes = self.resource(offset, 16)?;
        let size = u32_at(&bytes, 4)? as usize;
        if size == 0 || size > MAX_ICON {
            return Err(invalid());
        }
        self.rva(u32_at(&bytes, 0)?, size)
    }
    fn open(path: &Path) -> io::Result<Self> {
        let file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(invalid());
        }
        let mut pe = Self {
            file,
            len: metadata.len(),
            budget: MAX_READ,
            sections: vec![],
            resource_rva: 0,
            resource_size: 0,
        };
        let dos = pe.read(0, 64)?;
        if &dos[..2] != b"MZ" {
            return Err(invalid());
        }
        let position = u64::from(u32_at(&dos, 60)?);
        let header = pe.read(position, 24)?;
        if &header[..4] != b"PE\0\0" {
            return Err(invalid());
        }
        let count = usize::from(u16_at(&header, 6)?);
        let optional_size = usize::from(u16_at(&header, 20)?);
        if count == 0 || count > 96 || optional_size > 4096 {
            return Err(invalid());
        }
        let optional = pe.read(position + 24, optional_size)?;
        let directories = match u16_at(&optional, 0)? {
            0x10b => 96,
            0x20b => 112,
            _ => return Err(invalid()),
        };
        if u32_at(&optional, directories - 4)? < 3 {
            return Err(invalid());
        }
        pe.resource_rva = u32_at(&optional, directories + 16)?;
        pe.resource_size = u32_at(&optional, directories + 20)?;
        if pe.resource_rva == 0 || pe.resource_size < 16 {
            return Err(invalid());
        }
        let sections = pe.read(position + 24 + optional_size as u64, count * 40)?;
        for section in sections.chunks_exact(40) {
            pe.sections.push(Section {
                rva: u32_at(section, 12)?,
                size: u32_at(section, 16)?,
                offset: u32_at(section, 20)?,
            });
        }
        Ok(pe)
    }
}
fn decode_icon(group: &[u8], data: &[u8]) -> io::Result<ico::IconImage> {
    // Reject oversized compressed-image/DIB headers before decoder allocation.
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        let dimensions = data.get(16..24).ok_or_else(invalid)?;
        let width = u32::from_be_bytes(dimensions[..4].try_into().unwrap());
        let height = u32::from_be_bytes(dimensions[4..].try_into().unwrap());
        if width == 0 || height == 0 || width > 256 || height > 256 {
            return Err(invalid());
        }
    } else {
        let width = u32_at(data, 4)?;
        let height = u32_at(data, 8)?;
        if width == 0 || height == 0 || width > 256 || height > 512 {
            return Err(invalid());
        }
    }
    // Convert one GRPICONDIRENTRY + RT_ICON to a standard single-image ICO.
    let mut bytes = vec![0, 0, 1, 0, 1, 0];
    bytes.extend_from_slice(&group[..8]);
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&22u32.to_le_bytes());
    bytes.extend_from_slice(data);
    let directory = ico::IconDir::read(Cursor::new(bytes))?;
    directory.entries().first().ok_or_else(invalid)?.decode()
}
pub fn extract_png(executable: &Path) -> io::Result<Vec<u8>> {
    let mut pe = Pe::open(executable)?;
    let types = pe.directory(0)?;
    let type_dir = |kind| {
        types
            .iter()
            .find(|(id, offset)| *id == kind && offset & 0x80000000 != 0)
            .map(|(_, offset)| offset & 0x7fffffff)
            .ok_or_else(invalid)
    };
    let groups = pe.directory(type_dir(14)?)?;
    let icons = pe.directory(type_dir(3)?)?;
    let mut attempts = 0;
    // Use the first usable icon group, the application's main icon. Keep its
    // largest decodable variant rather than unrelated toolbar icons.
    for (_, group_dir) in groups.into_iter().take(32) {
        if group_dir & 0x80000000 == 0 {
            continue;
        }
        let languages = match pe.directory(group_dir & 0x7fffffff) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for (language, group_data) in languages.into_iter().take(16) {
            let group = match pe.data(group_data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if group.len() < 6 {
                continue;
            }
            if u16_at(&group, 0)? != 0 || u16_at(&group, 2)? != 1 {
                continue;
            }
            let count = usize::from(u16_at(&group, 4)?);
            if count == 0 || count > 256 || group.len() < 6 + count * 14 {
                continue;
            }
            let mut items: Vec<_> = group[6..6 + count * 14].chunks_exact(14).collect();
            items.sort_by_key(|item| {
                std::cmp::Reverse((
                    (if item[0] == 0 {
                        256
                    } else {
                        u32::from(item[0])
                    }) * (if item[1] == 0 {
                        256
                    } else {
                        u32::from(item[1])
                    }),
                    u16::from_le_bytes([item[6], item[7]]),
                ))
            });
            for item in items {
                let id = u32::from(u16_at(item, 12)?);
                let Some((_, icon_dir)) = icons
                    .iter()
                    .find(|(key, offset)| *key == id && offset & 0x80000000 != 0)
                else {
                    continue;
                };
                let mut variants = match pe.directory(icon_dir & 0x7fffffff) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                variants.sort_by_key(|(lang, _)| *lang != language);
                for (_, offset) in variants.into_iter().take(16) {
                    attempts += 1;
                    if attempts > 128 {
                        return Err(invalid());
                    }
                    let data = match pe.data(offset) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let image = match decode_icon(item, &data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if image.width() > 256 || image.height() > 256 {
                        continue;
                    }
                    let mut png = Vec::new();
                    {
                        let mut encoder =
                            png::Encoder::new(&mut png, image.width(), image.height());
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder
                            .write_header()?
                            .write_image_data(image.rgba_data())?;
                    }
                    return Ok(png);
                }
            }
        }
    }
    Err(invalid())
}
fn cache_png(directory: &Path, png: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let target = directory.join(format!("{:x}.png", Sha256::digest(png)));
    let temp = directory.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(png)?;
        fs::rename(&temp, &target)?;
        Ok(target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub fn shortcut_icon_at(executable: &Path, directory: &Path) -> io::Result<PathBuf> {
    let png = extract_png(executable)
        .unwrap_or_else(|_| include_bytes!("../../../public/peligames.png").to_vec());
    cache_png(directory, &png)
}
pub fn shortcut_icon(executable: &Path) -> Option<PathBuf> {
    let directory = dirs::config_dir()?.join("peligames/icons/executables");
    shortcut_icon_at(executable, &directory).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put16(b: &mut [u8], offset: usize, n: u16) {
        b[offset..offset + 2].copy_from_slice(&n.to_le_bytes());
    }
    fn put32(b: &mut [u8], offset: usize, n: u32) {
        b[offset..offset + 4].copy_from_slice(&n.to_le_bytes());
    }
    fn fixture(path: &Path, pe64: bool, bitmap: bool) {
        let entries: Vec<_> = [16u32, 256]
            .into_iter()
            .map(|size| {
                let mut rgba = vec![0; size as usize * size as usize * 4];
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel.copy_from_slice(&[20, 100, 200, 128]);
                }
                let image = ico::IconImage::from_rgba_data(size, size, rgba);
                if bitmap {
                    ico::IconDirEntry::encode_as_bmp(&image).unwrap()
                } else {
                    ico::IconDirEntry::encode_as_png(&image).unwrap()
                }
            })
            .collect();
        let mut resource = vec![0u8; 0x120];
        for (offset, children) in [
            (0, 2),
            (0x20, 2),
            (0x48, 1),
            (0x70, 1),
            (0x88, 1),
            (0xa0, 1),
        ] {
            put16(&mut resource, offset + 14, children);
        }
        for (offset, id, target) in [
            (16, 3, 0x80000020),
            (24, 14, 0x80000048),
            (0x30, 1, 0x80000070),
            (0x38, 2, 0x80000088),
            (0x58, 1, 0x800000a0),
            (0x80, 1033, 0xb8),
            (0x98, 1033, 0xc8),
            (0xb0, 1033, 0xd8),
        ] {
            put32(&mut resource, offset, id);
            put32(&mut resource, offset + 4, target);
        }
        put16(&mut resource, 0xea, 1);
        put16(&mut resource, 0xec, 2);
        for (n, entry) in entries.iter().enumerate() {
            let offset = resource.len();
            let payload = entry.data();
            put32(&mut resource, 0xb8 + n * 16, 0x2000 + offset as u32);
            put32(&mut resource, 0xbc + n * 16, payload.len() as u32);
            let group = 0xee + n * 14;
            resource[group] = entry.width() as u8;
            resource[group + 1] = entry.height() as u8;
            put16(&mut resource, group + 4, 1);
            put16(&mut resource, group + 6, 32);
            put32(&mut resource, group + 8, payload.len() as u32);
            put16(&mut resource, group + 12, n as u16 + 1);
            resource.extend_from_slice(payload);
        }
        put32(&mut resource, 0xd8, 0x20e8);
        put32(&mut resource, 0xdc, 34);
        let mut bytes = vec![0; 0x400];
        bytes[..2].copy_from_slice(b"MZ");
        put32(&mut bytes, 60, 0x80);
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        put16(&mut bytes, 0x86, 1);
        let optional = if pe64 { 240 } else { 224 };
        put16(&mut bytes, 0x94, optional);
        let start = 0x98;
        put16(&mut bytes, start, if pe64 { 0x20b } else { 0x10b });
        let directories = start + if pe64 { 112 } else { 96 };
        put32(&mut bytes, directories - 4, 16);
        put32(&mut bytes, directories + 16, 0x2000);
        put32(&mut bytes, directories + 20, resource.len() as u32);
        let section = start + optional as usize;
        put32(&mut bytes, section + 12, 0x2000);
        put32(&mut bytes, section + 16, resource.len() as u32);
        put32(&mut bytes, section + 20, 0x400);
        bytes.extend_from_slice(&resource);
        fs::write(path, bytes).unwrap();
    }
    #[test]
    fn pe32_and_pe64_extract_largest_png_and_bitmap_icons_with_alpha() {
        let root = std::env::temp_dir().join(format!("peli-icons-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        for pe64 in [false, true] {
            for bitmap in [false, true] {
                let exe = root.join("Program with spaces.exe");
                fixture(&exe, pe64, bitmap);
                let before = fs::read(&exe).unwrap();
                let png = extract_png(&exe).unwrap();
                let image = ico::IconImage::read_png(Cursor::new(&png)).unwrap();
                assert_eq!((image.width(), image.height()), (256, 256));
                assert_eq!(&image.rgba_data()[..4], &[20, 100, 200, 128]);
                let first = shortcut_icon_at(&exe, &root.join("cache")).unwrap();
                let second = shortcut_icon_at(&exe, &root.join("cache")).unwrap();
                assert_eq!(first, second);
                assert_eq!(fs::read(first).unwrap(), png);
                assert_eq!(fs::read(&exe).unwrap(), before);
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_icons_and_malformed_files_use_embedded_launcher_icon() {
        let root = std::env::temp_dir().join(format!("peli-icons-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let exe = root.join("broken.exe");
        fs::write(&exe, b"MZ invalid").unwrap();
        assert!(extract_png(&exe).is_err());
        let fallback = shortcut_icon_at(&exe, &root.join("cache")).unwrap();
        assert_eq!(
            fs::read(&fallback).unwrap(),
            include_bytes!("../../../public/peligames.png")
        );
        fixture(&exe, true, false);
        let mut bytes = fs::read(&exe).unwrap();
        put32(&mut bytes, 0x400 + 0xb8, 0xfffffff0);
        put32(&mut bytes, 0x400 + 0xc8, 0xfffffff0);
        fs::write(&exe, bytes).unwrap();
        assert!(extract_png(&exe).is_err());
        assert_eq!(
            shortcut_icon_at(&exe, &root.join("cache")).unwrap(),
            fallback
        );
        // No icon resource directory, and truncated section data.
        let mut bytes = fs::read(&exe).unwrap();
        put32(&mut bytes, 0x98 + 112 + 16, 0);
        fs::write(&exe, &bytes).unwrap();
        assert!(extract_png(&exe).is_err());
        for len in [0, 2, 63, 128, 200, 1024] {
            fs::write(&exe, &bytes[..len]).unwrap();
            assert!(extract_png(&exe).is_err());
        }
        assert!(fs::read_dir(root.join("cache")).unwrap().all(|entry| entry
            .unwrap()
            .path()
            .extension()
            .unwrap()
            == "png"));
        fs::remove_dir_all(root).unwrap();
    }
}
