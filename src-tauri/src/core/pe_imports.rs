//! The DLLs a Windows executable imports (regular and delay-loaded), read
//! from its PE headers without executing or loading the whole file.
//! Format reference: Microsoft PE/COFF, import and delay-load directories.
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// Bytes read per executable at most; import tables are a few kilobytes.
const BUDGET: usize = 1024 * 1024;
const MAX_DLLS: usize = 512;

struct Reader {
    file: fs::File,
    len: u64,
    budget: usize,
    sections: Vec<(u32, u32, u32)>, // virtual address, size, file offset
}
impl Reader {
    fn read(&mut self, offset: u64, size: usize) -> Option<Vec<u8>> {
        if size > self.budget || offset.checked_add(size as u64)? > self.len {
            return None;
        }
        self.budget -= size;
        self.file.seek(SeekFrom::Start(offset)).ok()?;
        let mut bytes = vec![0; size];
        self.file.read_exact(&mut bytes).ok()?;
        Some(bytes)
    }
    fn offset(&self, rva: u32) -> Option<u64> {
        self.sections.iter().find_map(|&(address, size, offset)| {
            let delta = rva.checked_sub(address)?;
            (delta < size).then(|| u64::from(offset) + u64::from(delta))
        })
    }
    fn at(&mut self, rva: u32, size: usize) -> Option<Vec<u8>> {
        let offset = self.offset(rva)?;
        self.read(offset, size)
    }
    /// A NUL-terminated ASCII name at `rva` (DLL names are short).
    fn name(&mut self, rva: u32) -> Option<String> {
        let offset = self.offset(rva)?;
        let size = 64.min(self.len.saturating_sub(offset) as usize);
        let bytes = self.read(offset, size)?;
        let end = bytes.iter().position(|&b| b == 0)?;
        let name = std::str::from_utf8(&bytes[..end]).ok()?;
        (!name.is_empty() && name.bytes().all(|b| b.is_ascii_graphic())).then(|| name.to_ascii_lowercase())
    }
}
fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Lowercase DLL names, or `None` when the file is not a readable PE.
pub fn imports(path: &Path) -> Option<Vec<String>> {
    let file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut pe = Reader { file, len, budget: BUDGET, sections: Vec::new() };
    let dos = pe.read(0, 64)?;
    if &dos[..2] != b"MZ" {
        return None;
    }
    let header_at = u64::from(u32_at(&dos, 60)?);
    let header = pe.read(header_at, 24)?;
    if &header[..4] != b"PE\0\0" {
        return None;
    }
    let count = usize::from(u16_at(&header, 6)?);
    let optional_size = usize::from(u16_at(&header, 20)?);
    if count == 0 || count > 96 || optional_size > 4096 {
        return None;
    }
    let optional = pe.read(header_at + 24, optional_size)?;
    let directories = match u16_at(&optional, 0)? {
        0x10b => 96,
        0x20b => 112,
        _ => return None,
    };
    let entries = u32_at(&optional, directories - 4)? as usize;
    let directory = |index: usize| (index < entries).then(|| u32_at(&optional, directories + index * 8)).flatten().filter(|&rva| rva != 0);
    let (import, delay) = (directory(1), directory(13));
    let table = pe.read(header_at + 24 + optional_size as u64, count * 40)?;
    pe.sections = table.chunks_exact(40)
        .filter_map(|s| Some((u32_at(s, 12)?, u32_at(s, 8)?.max(u32_at(s, 16)?), u32_at(s, 20)?)))
        .collect();
    let mut dlls = Vec::new();
    // Import descriptors: 20 bytes, name RVA at 12. Delay-load: 32 bytes, name RVA at 4.
    for (start, stride, name_at) in [(import, 20u32, 12usize), (delay, 32, 4)] {
        let Some(start) = start else { continue };
        for index in 0..MAX_DLLS as u32 {
            let Some(descriptor) = pe.at(start.checked_add(index * stride)?, stride as usize) else { break };
            if descriptor.iter().all(|&b| b == 0) {
                break;
            }
            if let Some(name) = u32_at(&descriptor, name_at).and_then(|rva| pe.name(rva)) {
                if !dlls.contains(&name) {
                    dlls.push(name);
                }
            }
            if dlls.len() >= MAX_DLLS {
                break;
            }
        }
    }
    Some(dlls)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal 64-bit PE importing `regular` and delay-loading `delayed`,
    /// padded to `size` bytes. Enough for the import reader, never runnable.
    pub(crate) fn fake_pe(regular: &[&str], delayed: &[&str], size: usize) -> Vec<u8> {
        const SECTION_RVA: u32 = 0x1000;
        const SECTION_FILE: u32 = 0x200;
        let mut data = Vec::new();
        let import_at = 0u32;
        let delay_at = (regular.len() as u32 + 1) * 20;
        data.resize((delay_at + (delayed.len() as u32 + 1) * 32) as usize, 0);
        let mut names = Vec::new();
        for (index, dll) in regular.iter().chain(delayed).enumerate() {
            let name_rva = SECTION_RVA + (data.len() + names.len()) as u32;
            names.extend_from_slice(dll.as_bytes());
            names.push(0);
            let at = if index < regular.len() { (import_at + index as u32 * 20 + 12) as usize } else { (delay_at + (index - regular.len()) as u32 * 32 + 4) as usize };
            data[at..at + 4].copy_from_slice(&name_rva.to_le_bytes());
        }
        data.extend(names);
        let mut pe = vec![0u8; SECTION_FILE as usize];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&0x40u32.to_le_bytes());
        pe[0x40..0x44].copy_from_slice(b"PE\0\0");
        pe[0x44..0x46].copy_from_slice(&0x8664u16.to_le_bytes());
        pe[0x46..0x48].copy_from_slice(&1u16.to_le_bytes()); // one section
        pe[0x54..0x56].copy_from_slice(&240u16.to_le_bytes()); // PE32+ optional header
        let optional = 0x58;
        pe[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        pe[optional + 108..optional + 112].copy_from_slice(&16u32.to_le_bytes());
        let directories = optional + 112;
        if !regular.is_empty() { pe[directories + 8..directories + 12].copy_from_slice(&(SECTION_RVA + import_at).to_le_bytes()); }
        if !delayed.is_empty() { pe[directories + 104..directories + 108].copy_from_slice(&(SECTION_RVA + delay_at).to_le_bytes()); }
        let section = optional + 240;
        pe[section..section + 6].copy_from_slice(b".idata");
        pe[section + 8..section + 12].copy_from_slice(&(data.len() as u32).to_le_bytes());
        pe[section + 12..section + 16].copy_from_slice(&SECTION_RVA.to_le_bytes());
        pe[section + 16..section + 20].copy_from_slice(&(data.len() as u32).to_le_bytes());
        pe[section + 20..section + 24].copy_from_slice(&SECTION_FILE.to_le_bytes());
        pe.extend(data);
        if pe.len() < size { pe.resize(size, 0); }
        pe
    }

    #[test]
    fn reads_regular_and_delay_loaded_imports() {
        let path = std::env::temp_dir().join(format!("peligames-pe-{}.exe", uuid::Uuid::new_v4()));
        fs::write(&path, fake_pe(&["KERNEL32.dll", "d3d12.dll"], &["dxgi.dll"], 4096)).unwrap();
        assert_eq!(imports(&path).unwrap(), ["kernel32.dll", "d3d12.dll", "dxgi.dll"]);
        fs::write(&path, b"MZ not a real executable").unwrap();
        assert!(imports(&path).is_none());
        fs::remove_file(path).unwrap();
    }
}
