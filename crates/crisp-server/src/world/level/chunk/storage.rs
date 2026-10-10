pub mod entity_storage;
pub mod region_file_version;

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crisp_core::ChunkPos;
use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use region_file_version::RegionFileVersion;

#[cfg(windows)]
fn dsync(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
    options.custom_flags(FILE_FLAG_WRITE_THROUGH);
}

#[cfg(unix)]
fn dsync(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_DSYNC);
}

const SECTOR_BYTES: u64 = 4096;
const EXTERNAL_STREAM_FLAG: u8 = 128;

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

pub fn decode_nbt(bytes: &[u8]) -> io::Result<Nbt> {
    let (_, nbt) = Nbt::read_named(&mut Reader::new(bytes)).map_err(|e| invalid(&e.to_string()))?;
    Ok(nbt)
}

pub fn encode_nbt(nbt: &Nbt) -> Vec<u8> {
    let mut w = Writer::new();
    nbt.write_named("", &mut w);
    w.into_inner()
}

#[derive(Default)]
struct RegionBitmap {
    used: Vec<bool>,
}

impl RegionBitmap {
    fn force(&mut self, position: usize, size: usize) {
        if self.used.len() < position + size {
            self.used.resize(position + size, false);
        }
        self.used[position..position + size].fill(true);
    }

    fn free(&mut self, position: usize, size: usize) {
        let end = (position + size).min(self.used.len());
        if position < end {
            self.used[position..end].fill(false);
        }
    }

    fn allocate(&mut self, size: usize) -> usize {
        let mut current = 0;
        loop {
            let free_start = (current..).find(|&i| !self.used.get(i).copied().unwrap_or(false)).unwrap();
            let free_end = (free_start..self.used.len()).find(|&i| self.used[i]);
            match free_end {
                Some(end) if end - free_start < size => current = end,
                _ => {
                    self.force(free_start, size);
                    return free_start;
                }
            }
        }
    }
}

pub struct RegionFile {
    path: PathBuf,
    file: File,
    offsets: [u32; 1024],
    timestamps: [u32; 1024],
    used: RegionBitmap,
    version: RegionFileVersion,
}

impl RegionFile {
    pub fn open(path: &Path, sync: bool) -> io::Result<RegionFile> {
        fs::create_dir_all(path.parent().unwrap())?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        if sync {
            dsync(&mut options);
        }
        let mut file = options.open(path)?;
        let size = file.metadata()?.len();
        let mut header = vec![0u8; 2 * SECTOR_BYTES as usize];
        let read = file.read(&mut header)?;
        if read != 0 && read != header.len() {
            warn!("Region file {} has truncated header: {read}", path.display());
        }
        let mut region = RegionFile {
            path: path.to_owned(),
            file,
            offsets: [0; 1024],
            timestamps: [0; 1024],
            used: RegionBitmap::default(),
            version: RegionFileVersion::selected(),
        };
        region.used.force(0, 2);
        for i in 0..1024 {
            region.offsets[i] = u32::from_be_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
            region.timestamps[i] = u32::from_be_bytes(header[4096 + i * 4..4096 + i * 4 + 4].try_into().unwrap());
            let offset = region.offsets[i];
            if offset == 0 {
                continue;
            }
            let (sector, count) = ((offset >> 8) as usize, (offset & 0xFF) as usize);
            if sector < 2 || count == 0 || sector as u64 * SECTOR_BYTES > size {
                warn!("Region file {} has an invalid sector at index: {i}", path.display());
                region.offsets[i] = 0;
            } else {
                region.used.force(sector, count);
            }
        }
        Ok(region)
    }

    fn offset_index(pos: ChunkPos) -> usize {
        ((pos.x & 31) + (pos.z & 31) * 32) as usize
    }

    fn external_path(&self, pos: ChunkPos) -> PathBuf {
        self.path.with_file_name(format!("c.{}.{}.mcc", pos.x, pos.z))
    }

    pub fn read(&mut self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        let offset = self.offsets[Self::offset_index(pos)];
        if offset == 0 {
            return Ok(None);
        }
        let (sector, count) = ((offset >> 8) as u64, (offset & 0xFF) as u64);
        self.file.seek(SeekFrom::Start(sector * SECTOR_BYTES))?;
        let mut sectors = vec![0u8; (count * SECTOR_BYTES) as usize];
        let read = self.file.read(&mut sectors)?;
        if read < 5 {
            return Ok(None);
        }
        let length = u32::from_be_bytes(sectors[..4].try_into().unwrap()) as usize;
        let version = sectors[4];
        if length == 0 {
            return Ok(None);
        }
        let data = if version & EXTERNAL_STREAM_FLAG != 0 {
            fs::read(self.external_path(pos))?
        } else {
            sectors.get(5..4 + length).ok_or_else(|| invalid("chunk data is truncated"))?.to_vec()
        };
        let compression = version & !EXTERNAL_STREAM_FLAG;
        let Some(compression) = RegionFileVersion::from_id(compression) else {
            return Err(invalid(&format!("unsupported chunk compression {compression}")));
        };
        decode_nbt(&compression.decompress(&data)?).map(Some)
    }

    pub fn write(&mut self, pos: ChunkPos, nbt: &Nbt) -> io::Result<()> {
        let compressed = self.version.compress(&encode_nbt(nbt))?;
        let mut record = Vec::with_capacity(compressed.len() + 5);
        record.extend(((compressed.len() + 1) as u32).to_be_bytes());
        record.push(self.version.id());
        record.extend(&compressed);

        let index = Self::offset_index(pos);
        let old = self.offsets[index];
        let mut sectors_needed = (record.len() as u64).div_ceil(SECTOR_BYTES) as usize;
        let external = sectors_needed >= 256;
        let new_sector;
        if external {
            warn!("Saving oversized chunk {pos:?} ({} bytes) to external file", record.len());
            sectors_needed = 1;
            new_sector = self.used.allocate(sectors_needed);
            let tmp = self.external_path(pos).with_extension("tmp");
            fs::write(&tmp, &compressed)?;
            let mut stub = Vec::from(1u32.to_be_bytes());
            stub.push(self.version.id() | EXTERNAL_STREAM_FLAG);
            self.file.seek(SeekFrom::Start(new_sector as u64 * SECTOR_BYTES))?;
            self.file.write_all(&stub)?;
            fs::rename(tmp, self.external_path(pos))?;
        } else {
            new_sector = self.used.allocate(sectors_needed);
            self.file.seek(SeekFrom::Start(new_sector as u64 * SECTOR_BYTES))?;
            self.file.write_all(&record)?;
            let _ = fs::remove_file(self.external_path(pos));
        }
        self.offsets[index] = (new_sector as u32) << 8 | sectors_needed as u32;
        self.timestamps[index] = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
        self.write_header()?;
        if old != 0 {
            self.used.free((old >> 8) as usize, (old & 0xFF) as usize);
        }
        Ok(())
    }

    pub fn clear(&mut self, pos: ChunkPos) -> io::Result<()> {
        let index = Self::offset_index(pos);
        let offset = self.offsets[index];
        if offset != 0 {
            self.offsets[index] = 0;
            self.timestamps[index] = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
            self.write_header()?;
            let _ = fs::remove_file(self.external_path(pos));
            self.used.free((offset >> 8) as usize, (offset & 0xFF) as usize);
        }
        Ok(())
    }

    fn write_header(&mut self) -> io::Result<()> {
        let mut header = Vec::with_capacity(8192);
        self.offsets.iter().for_each(|o| header.extend(o.to_be_bytes()));
        self.timestamps.iter().for_each(|t| header.extend(t.to_be_bytes()));
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header)
    }

    fn pad_to_full_sector(&mut self) -> io::Result<()> {
        let size = self.file.metadata()?.len();
        let padded = size.div_ceil(SECTOR_BYTES) * SECTOR_BYTES;
        if padded != size {
            self.file.seek(SeekFrom::Start(padded - 1))?;
            self.file.write_all(&[0])?;
        }
        Ok(())
    }
}

pub struct RegionStorage {
    dir: PathBuf,
    sync: bool,
    open: HashMap<(i32, i32), RegionFile>,
}

impl RegionStorage {
    pub fn new(dir: PathBuf, sync: bool) -> RegionStorage {
        RegionStorage { dir, sync, open: HashMap::new() }
    }

    fn region_path(&self, pos: ChunkPos) -> PathBuf {
        self.dir.join(format!("r.{}.{}.mca", pos.x >> 5, pos.z >> 5))
    }

    fn region(&mut self, pos: ChunkPos) -> io::Result<&mut RegionFile> {
        let key = (pos.x >> 5, pos.z >> 5);
        if !self.open.contains_key(&key) {
            let region = RegionFile::open(&self.region_path(pos), self.sync)?;
            self.open.insert(key, region);
        }
        Ok(self.open.get_mut(&key).unwrap())
    }

    pub fn read(&mut self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        if !self.open.contains_key(&(pos.x >> 5, pos.z >> 5)) && !self.region_path(pos).exists() {
            return Ok(None);
        }
        self.region(pos)?.read(pos)
    }

    pub fn write(&mut self, pos: ChunkPos, nbt: &Nbt) -> io::Result<()> {
        self.region(pos)?.write(pos, nbt)
    }

    pub fn clear(&mut self, pos: ChunkPos) -> io::Result<()> {
        self.region(pos)?.clear(pos)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        for region in self.open.values_mut() {
            region.pad_to_full_sector()?;
            region.file.sync_data()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_round_trip() {
        let dir = std::env::temp_dir().join(format!("crisp-region-{}", std::process::id()));
        let mut store = RegionStorage::new(dir.clone(), false);
        let small = Nbt::Compound(vec![("a".into(), Nbt::Int(1))]);
        let large = Nbt::Compound(vec![("big".into(), Nbt::LongArray((0..3000).map(|i| i * 7919).collect()))]);
        store.write(ChunkPos::new(0, 0), &small).unwrap();
        store.write(ChunkPos::new(-1, 33), &large).unwrap();
        store.write(ChunkPos::new(0, 0), &large).unwrap();
        store.flush().unwrap();
        let mut reopened = RegionStorage::new(dir.clone(), false);
        assert_eq!(reopened.read(ChunkPos::new(0, 0)).unwrap(), Some(large.clone()));
        assert_eq!(reopened.read(ChunkPos::new(-1, 33)).unwrap(), Some(large));
        assert_eq!(reopened.read(ChunkPos::new(5, 5)).unwrap(), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn first_fit_reuses_freed_sectors() {
        let mut bitmap = RegionBitmap::default();
        bitmap.force(0, 2);
        assert_eq!(bitmap.allocate(3), 2);
        assert_eq!(bitmap.allocate(1), 5);
        bitmap.free(2, 3);
        assert_eq!(bitmap.allocate(2), 2);
        assert_eq!(bitmap.allocate(2), 6);
    }
}
