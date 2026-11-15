use crate::scanner::{DirNode, FileEntry, ScanStats, ScanTree};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 8] = b"DLNS\x03\x00\x00\x00";

pub fn data_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join("DiskLens");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn cache_path_for_drive(drive_root: &str) -> PathBuf {
    let letter: String = drive_root
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let tag = if letter.is_empty() {
        "root".to_string()
    } else {
        letter.to_uppercase()
    };
    data_dir().join(format!("tree_{tag}.bin"))
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn save_snapshot(tree: &ScanTree) -> Result<(), String> {
    let target = cache_path_for_drive(&tree.root_path);
    let tmp = target.with_extension(format!("tmp.{}", std::process::id()));

    let mut out: Vec<u8> = Vec::with_capacity(tree.nodes.len() * 48 + 256);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&now_unix_secs().to_le_bytes());
    out.extend_from_slice(&(tree.stats.elapsed_ms as u64).to_le_bytes());
    out.extend_from_slice(&tree.stats.total_files.to_le_bytes());
    out.extend_from_slice(&tree.stats.deduped_hardlinks.to_le_bytes());
    out.extend_from_slice(&tree.stats.denied_dirs.to_le_bytes());
    out.extend_from_slice(&tree.stats.total_alloc_bytes.to_le_bytes());
    out.extend_from_slice(&tree.stats.total_logical_bytes.to_le_bytes());

    let root_bytes = tree.root_path.as_bytes();
    out.extend_from_slice(&(root_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(root_bytes);

    out.extend_from_slice(&(tree.nodes.len() as u32).to_le_bytes());

    for node in &tree.nodes {
        out.extend_from_slice(&node.parent_id.to_le_bytes());
        out.extend_from_slice(&node.total_size.to_le_bytes());
        out.extend_from_slice(&node.direct_size.to_le_bytes());
        out.extend_from_slice(&node.direct_file_count.to_le_bytes());
        out.extend_from_slice(&node.total_file_count.to_le_bytes());
        out.extend_from_slice(&node.total_dir_count.to_le_bytes());
        out.extend_from_slice(&node.direct_last_write_time.to_le_bytes());
        out.extend_from_slice(&node.last_write_time.to_le_bytes());
        let mut flags = 0u8;
        if node.access_denied {
            flags |= 1;
        }
        if node.direct_has_exe {
            flags |= 2;
        }
        if node.has_exe {
            flags |= 4;
        }
        out.push(flags);

        let name_b = node.name.as_bytes();
        let name_len = name_b.len().min(u16::MAX as usize);
        out.extend_from_slice(&(name_len as u16).to_le_bytes());
        out.extend_from_slice(&name_b[..name_len]);

        out.extend_from_slice(&(node.children.len() as u32).to_le_bytes());
        for &cid in &node.children {
            out.extend_from_slice(&cid.to_le_bytes());
        }

        let tf_len = node.top_files.len().min(255);
        out.push(tf_len as u8);
        for f in &node.top_files[..tf_len] {
            out.extend_from_slice(&f.size.to_le_bytes());
            let fb = f.name.as_bytes();
            let flen = fb.len().min(u16::MAX as usize);
            out.extend_from_slice(&(flen as u16).to_le_bytes());
            out.extend_from_slice(&fb[..flen]);
        }
    }

    fs::write(&tmp, &out).map_err(|e| format!("Failed writing cache temp file: {e}"))?;
    fs::rename(&tmp, &target).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed committing cache file: {e}")
    })
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos.checked_add(n)? > self.buf.len() {
            return None;
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn str(&mut self, len: usize) -> Option<Box<str>> {
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes).ok().map(Box::from)
    }
}

pub fn load_snapshot(drive_root: &str) -> Option<(ScanTree, u64)> {
    let path = cache_path_for_drive(drive_root);
    let data = fs::read(&path).ok()?;
    let mut r = Reader { buf: &data, pos: 0 };

    if r.take(8)? != MAGIC {
        return None;
    }
    let saved_at = r.u64()?;
    let elapsed_ms = r.u64()? as u128;
    let total_files = r.u64()?;
    let deduped_hardlinks = r.u64()?;
    let denied_dirs = r.u64()?;
    let total_alloc_bytes = r.u64()?;
    let total_logical_bytes = r.u64()?;

    let root_len = r.u32()? as usize;
    let root_path = r.str(root_len)?.into_string();

    let node_count = r.u32()? as usize;
    if node_count == 0 || node_count > 25_000_000 {
        return None;
    }

    let mut nodes = Vec::with_capacity(node_count);
    for idx in 0..node_count {
        let parent_id = r.u32()?;
        let total_size = r.u64()?;
        let direct_size = r.u64()?;
        let direct_file_count = r.u32()?;
        let total_file_count = r.u64()?;
        let total_dir_count = r.u32()?;
        let direct_last_write_time = r.i64()?;
        let last_write_time = r.i64()?;
        let flags = r.u8()?;
        let name_len = r.u16()? as usize;
        let name = r.str(name_len)?;

        let ch_len = r.u32()? as usize;
        if ch_len > node_count {
            return None;
        }
        let mut children = Vec::with_capacity(ch_len);
        for _ in 0..ch_len {
            children.push(r.u32()?);
        }

        let tf_len = r.u8()? as usize;
        let mut top_files = Vec::with_capacity(tf_len);
        for _ in 0..tf_len {
            let size = r.u64()?;
            let flen = r.u16()? as usize;
            let fname = r.str(flen)?;
            top_files.push(FileEntry { name: fname, size });
        }

        nodes.push(DirNode {
            id: idx as u32,
            parent_id,
            name,
            total_size,
            direct_size,
            direct_file_count,
            total_file_count,
            total_dir_count,
            direct_last_write_time,
            last_write_time,
            direct_has_exe: (flags & 2) != 0,
            has_exe: (flags & 4) != 0,
            access_denied: (flags & 1) != 0,
            children,
            top_files,
        });
    }

    Some((
        ScanTree {
            root_path,
            nodes,
            stats: ScanStats {
                elapsed_ms,
                total_dirs: node_count,
                total_files,
                deduped_hardlinks,
                denied_dirs,
                total_alloc_bytes,
                total_logical_bytes,
            },
        },
        saved_at,
    ))
}
