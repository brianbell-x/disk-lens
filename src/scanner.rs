use crossbeam_deque::{Injector, Steal, Worker};
use dashmap::DashSet;
use serde::Serialize;
use std::collections::HashSet;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
};

type Handle = *mut c_void;
type NtStatus = i32;

const STATUS_SUCCESS: NtStatus = 0;
const STATUS_NO_MORE_FILES: NtStatus = 0x80000006u32 as i32;

const FILE_LIST_DIRECTORY: u32 = 0x0000_0001;
const SYNCHRONIZE: u32 = 0x0010_0000;
const FILE_SHARE_ALL: u32 = 0x0000_0007;
const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0000_0020;
const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const OBJ_CASE_INSENSITIVE: u32 = 0x0000_0040;

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;

const FILE_ID_FULL_DIRECTORY_INFORMATION_CLASS: u32 = 38;

const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_RAMDISK: u32 = 6;

const BITSET_WORDS: usize = 262_144; // 16,777,216 MFT slots = 2 MB

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: Handle,
    object_name: *const UnicodeString,
    attributes: u32,
    security_descriptor: *const c_void,
    security_quality_of_service: *const c_void,
}

#[repr(C)]
struct IoStatusBlock {
    status_or_pointer: *mut c_void,
    information: usize,
}

#[repr(C)]
struct FileIdFullDirInformation {
    next_entry_offset: u32,
    file_index: u32,
    creation_time: i64,
    last_access_time: i64,
    last_write_time: i64,
    change_time: i64,
    end_of_file: i64,
    allocation_size: i64,
    file_attributes: u32,
    file_name_length: u32,
    ea_size: u32,
    file_id: i64,
    file_name: [u16; 1],
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtOpenFile(
        file_handle: *mut Handle,
        desired_access: u32,
        object_attributes: *const ObjectAttributes,
        io_status_block: *mut IoStatusBlock,
        share_access: u32,
        open_options: u32,
    ) -> NtStatus;

    fn NtQueryDirectoryFile(
        file_handle: Handle,
        event: Handle,
        apc_routine: *const c_void,
        apc_context: *const c_void,
        io_status_block: *mut IoStatusBlock,
        file_information: *mut c_void,
        length: u32,
        file_information_class: u32,
        return_single_entry: u8,
        file_name: *const UnicodeString,
        restart_scan: u8,
    ) -> NtStatus;

    fn NtClose(handle: Handle) -> NtStatus;
}

#[derive(Clone, Debug, Serialize)]
pub struct DriveInfo {
    pub root: String,
    pub label: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

pub fn list_drives() -> Vec<DriveInfo> {
    let mut drives = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for bit in 0..26u32 {
        if (mask & (1 << bit)) == 0 {
            continue;
        }
        let letter = (b'A' + bit as u8) as char;
        let root = format!("{letter}:\\");
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let dtype = unsafe { GetDriveTypeW(wide.as_ptr()) };
        if dtype != DRIVE_FIXED && dtype != DRIVE_REMOVABLE && dtype != DRIVE_RAMDISK {
            continue;
        }
        let mut free_avail = 0u64;
        let mut total = 0u64;
        let mut total_free = 0u64;
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_avail,
                &mut total,
                &mut total_free,
            )
        };
        if ok != 0 && total > 0 {
            drives.push(DriveInfo {
                label: format!("{letter}:"),
                root,
                total_bytes: total,
                used_bytes: total.saturating_sub(total_free),
                free_bytes: total_free,
            });
        }
    }
    drives
}

struct DirHandle(Handle);
unsafe impl Send for DirHandle {}
unsafe impl Sync for DirHandle {}

impl Drop for DirHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = NtClose(self.0);
            }
        }
    }
}

struct WorkItem {
    id: u32,
    parent_id: u32,
    parent_handle: Option<Arc<DirHandle>>,
    name_utf16: Box<[u16]>,
    is_cloud_reparse: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileEntry {
    pub name: Box<str>,
    pub size: u64,
}

#[derive(Clone, Debug, Default)]
pub struct RawDirRecord {
    pub id: u32,
    pub parent_id: u32,
    pub name: Box<str>,
    pub direct_alloc_size: u64,
    pub direct_logical_size: u64,
    pub direct_file_count: u32,
    pub direct_last_write_time: i64,
    pub direct_has_exe: bool,
    pub access_denied: bool,
    pub top_files: Vec<FileEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct DirNode {
    pub id: u32,
    pub parent_id: u32,
    pub name: Box<str>,
    pub total_size: u64,
    pub direct_size: u64,
    pub direct_file_count: u32,
    pub total_file_count: u64,
    pub total_dir_count: u32,
    pub direct_last_write_time: i64,
    pub last_write_time: i64,
    pub direct_has_exe: bool,
    pub has_exe: bool,
    pub access_denied: bool,
    pub children: Vec<u32>,
    pub top_files: Vec<FileEntry>,
}

#[derive(Clone, Debug)]
pub struct ScanStats {
    pub elapsed_ms: u128,
    pub total_dirs: usize,
    pub total_files: u64,
    pub deduped_hardlinks: u64,
    pub denied_dirs: u64,
    pub total_alloc_bytes: u64,
    pub total_logical_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ScanTree {
    pub root_path: String,
    pub nodes: Vec<DirNode>,
    pub stats: ScanStats,
}

impl ScanTree {
    pub fn node_path(&self, id: u32) -> Option<String> {
        let idx = id as usize;
        if idx >= self.nodes.len() || self.nodes[idx].name.is_empty() {
            return None;
        }
        if id == 0 {
            return Some(self.root_path.clone());
        }
        let mut parts: Vec<&str> = Vec::with_capacity(12);
        let mut cur = idx;
        let mut guard = 0usize;
        while cur != 0 && cur < self.nodes.len() && guard < 4096 {
            parts.push(&self.nodes[cur].name);
            let p = self.nodes[cur].parent_id as usize;
            if p >= cur && p != 0 && p >= self.nodes.len() {
                break;
            }
            cur = p;
            guard += 1;
        }
        let mut full = self.root_path.trim_end_matches('\\').to_string();
        for seg in parts.into_iter().rev() {
            full.push('\\');
            full.push_str(seg);
        }
        Some(full)
    }

    pub fn find_node_by_path(&self, full_path: &str) -> Option<u32> {
        if self.nodes.is_empty() {
            return None;
        }
        let norm = full_path.replace('/', "\\");
        let root_trim = self.root_path.trim_end_matches('\\');
        let trimmed = norm.trim_end_matches('\\');
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(root_trim) {
            return Some(0);
        }
        let rest = if norm.len() >= root_trim.len()
            && norm[..root_trim.len()].eq_ignore_ascii_case(root_trim)
        {
            &norm[root_trim.len()..]
        } else {
            return None;
        };

        let mut cur = 0u32;
        for seg in rest.split('\\').filter(|s| !s.is_empty()) {
            cur = self.find_child_by_name(cur, seg)?;
        }
        Some(cur)
    }

    pub fn find_child_by_name(&self, parent_id: u32, name: &str) -> Option<u32> {
        let p = self.nodes.get(parent_id as usize)?;
        for &cid in &p.children {
            if let Some(ch) = self.nodes.get(cid as usize) {
                if ch.name.eq_ignore_ascii_case(name)
                    || ch.name.to_lowercase() == name.to_lowercase()
                {
                    return Some(cid);
                }
            }
        }
        None
    }

    fn sort_children_of(&mut self, id: usize) {
        if self.nodes[id].children.len() > 1 {
            let mut ch = std::mem::take(&mut self.nodes[id].children);
            ch.sort_unstable_by(|&a, &b| {
                self.nodes[b as usize]
                    .total_size
                    .cmp(&self.nodes[a as usize].total_size)
                    .then_with(|| self.nodes[a as usize].name.cmp(&self.nodes[b as usize].name))
            });
            self.nodes[id].children = ch;
        }
    }

    fn recompute_ancestors_from(&mut self, start_id: u32) {
        let mut cur = start_id as usize;
        let mut guard = 0usize;
        while cur < self.nodes.len() && guard < 4096 {
            let mut total_size = self.nodes[cur].direct_size;
            let mut total_files = self.nodes[cur].direct_file_count as u64;
            let mut total_dirs = 0u32;
            let mut lwt = self.nodes[cur].direct_last_write_time;
            let mut has_exe = self.nodes[cur].direct_has_exe;
            for &cid in &self.nodes[cur].children {
                let c = &self.nodes[cid as usize];
                total_size = total_size.saturating_add(c.total_size);
                total_files = total_files.saturating_add(c.total_file_count);
                total_dirs = total_dirs.saturating_add(c.total_dir_count).saturating_add(1);
                if c.last_write_time > lwt {
                    lwt = c.last_write_time;
                }
                has_exe |= c.has_exe;
            }
            self.nodes[cur].total_size = total_size;
            self.nodes[cur].total_file_count = total_files;
            self.nodes[cur].total_dir_count = total_dirs;
            self.nodes[cur].last_write_time = lwt;
            self.nodes[cur].has_exe = has_exe;
            self.sort_children_of(cur);

            if cur == 0 {
                break;
            }
            let p = self.nodes[cur].parent_id;
            if p == u32::MAX {
                break;
            }
            cur = p as usize;
            guard += 1;
        }

        if let Some(root) = self.nodes.first() {
            self.stats.total_alloc_bytes = root.total_size;
            self.stats.total_files = root.total_file_count;
            self.stats.total_dirs = root.total_dir_count as usize + 1;
        }
    }

    pub fn refresh_changed_paths(&mut self, rel_paths: &[String]) -> bool {
        let mut targets: Vec<u32> = Vec::new();
        let mut seen_targets: HashSet<u32> = HashSet::new();

        for rel in rel_paths {
            let mut chain: Vec<u32> = vec![0];
            let mut cur = 0u32;
            for seg in rel.split('\\').filter(|s| !s.is_empty()) {
                if let Some(next_id) = self.find_child_by_name(cur, seg) {
                    chain.push(next_id);
                    cur = next_id;
                } else {
                    break;
                }
            }
            let mut surviving = 0u32;
            for &cand in chain.iter().rev() {
                if let Some(p) = self.node_path(cand) {
                    if Path::new(&p).is_dir() {
                        surviving = cand;
                        break;
                    }
                }
            }
            if seen_targets.insert(surviving) {
                targets.push(surviving);
            }
        }

        if targets.is_empty() {
            return false;
        }

        let mut meaningful_change = false;
        for dir_id in targets {
            let Some(dir_path) = self.node_path(dir_id) else {
                continue;
            };
            let Some(single) = scan_single_directory(&dir_path) else {
                continue;
            };

            let d_idx = dir_id as usize;
            let prev_total_size = self.nodes[d_idx].total_size;
            let prev_child_count = self.nodes[d_idx].children.len();

            self.nodes[d_idx].direct_size = single.direct_alloc_size;
            self.nodes[d_idx].direct_file_count = single.direct_file_count;
            self.nodes[d_idx].direct_last_write_time = single.direct_last_write_time;
            self.nodes[d_idx].direct_has_exe = single.direct_has_exe;
            self.nodes[d_idx].top_files = single.top_files;

            let disk_lower: HashSet<String> = single
                .subdirs
                .iter()
                .map(|(n, _)| n.to_lowercase())
                .collect();

            // Remove deleted child directories
            let old_children = std::mem::take(&mut self.nodes[d_idx].children);
            let mut kept_children: Vec<u32> = Vec::with_capacity(old_children.len());
            let mut existing_lower: HashSet<String> = HashSet::with_capacity(old_children.len());

            for cid in old_children {
                let cname_lower = self.nodes[cid as usize].name.to_lowercase();
                if disk_lower.contains(&cname_lower) {
                    existing_lower.insert(cname_lower);
                    kept_children.push(cid);
                } else {
                    self.nodes[cid as usize].total_size = 0;
                    self.nodes[cid as usize].direct_size = 0;
                }
            }

            // Scan any newly created child directories
            for (sub_name, _is_cloud) in single.subdirs {
                if !existing_lower.contains(&sub_name.to_lowercase()) {
                    let sub_full = format!("{}\\{}", dir_path.trim_end_matches('\\'), sub_name);
                    let sub_tree = scan_root(&sub_full, None, None, None, None);
                    if !sub_tree.nodes.is_empty() {
                        let base_id = self.nodes.len() as u32;
                        for (idx, mut snode) in sub_tree.nodes.into_iter().enumerate() {
                            if idx > 0 && snode.name.is_empty() {
                                self.nodes.push(DirNode::default());
                                continue;
                            }
                            snode.id = base_id + idx as u32;
                            if idx == 0 {
                                snode.parent_id = dir_id;
                                snode.name = sub_name.clone().into_boxed_str();
                            } else {
                                snode.parent_id += base_id;
                            }
                            for cid_ref in &mut snode.children {
                                *cid_ref += base_id;
                            }
                            self.nodes.push(snode);
                        }
                        kept_children.push(base_id);
                    }
                }
            }

            let new_child_count = kept_children.len();
            self.nodes[d_idx].children = kept_children;
            self.recompute_ancestors_from(dir_id);

            let new_total_size = self.nodes[d_idx].total_size;
            if new_child_count != prev_child_count
                || new_total_size.abs_diff(prev_total_size) >= 65_536
            {
                meaningful_change = true;
            }
        }

        meaningful_change
    }
}

struct SingleDirSummary {
    direct_alloc_size: u64,
    direct_file_count: u32,
    direct_last_write_time: i64,
    direct_has_exe: bool,
    top_files: Vec<FileEntry>,
    subdirs: Vec<(String, bool)>,
}

#[inline(always)]
fn is_app_exe_utf16(name: &[u16]) -> bool {
    let n = name.len();
    if n < 5 {
        return false;
    }
    if name[n - 4] != b'.' as u16
        || (name[n - 3] | 0x20) != b'e' as u16
        || (name[n - 2] | 0x20) != b'x' as u16
        || (name[n - 1] | 0x20) != b'e' as u16
    {
        return false;
    }
    // Exclude leftover uninstaller & Squirrel updater stub binaries (`unins*`, `update.exe`, `squirrel.exe`)
    if n >= 9
        && (name[0] | 0x20) == b'u' as u16
        && (name[1] | 0x20) == b'n' as u16
        && (name[2] | 0x20) == b'i' as u16
        && (name[3] | 0x20) == b'n' as u16
        && (name[4] | 0x20) == b's' as u16
    {
        return false;
    }
    if n == 10
        && (name[0] | 0x20) == b'u' as u16
        && (name[1] | 0x20) == b'p' as u16
        && (name[2] | 0x20) == b'd' as u16
        && (name[3] | 0x20) == b'a' as u16
        && (name[4] | 0x20) == b't' as u16
        && (name[5] | 0x20) == b'e' as u16
    {
        return false;
    }
    if n == 12
        && (name[0] | 0x20) == b's' as u16
        && (name[1] | 0x20) == b'q' as u16
        && (name[2] | 0x20) == b'u' as u16
        && (name[3] | 0x20) == b'i' as u16
        && (name[4] | 0x20) == b'r' as u16
        && (name[5] | 0x20) == b'r' as u16
        && (name[6] | 0x20) == b'e' as u16
        && (name[7] | 0x20) == b'l' as u16
    {
        return false;
    }
    true
}

fn scan_single_directory(full_win_path: &str) -> Option<SingleDirSummary> {
    let trimmed = full_win_path.trim_end_matches('\\');
    let nt_path = if trimmed.ends_with(':') {
        format!("\\??\\{}\\", trimmed)
    } else {
        format!("\\??\\{}", trimmed)
    };
    let utf16: Vec<u16> = nt_path.encode_utf16().collect();
    let handle = open_dir(None, &utf16, false)
        .or_else(|_| open_dir(None, &utf16, true))
        .ok()?;

    let mut buf = Box::new(AlignedBuf([0u8; 65536]));
    let mut direct_alloc_size = 0u64;
    let mut direct_file_count = 0u32;
    let mut direct_last_write_time = 0i64;
    let mut direct_has_exe = false;
    let mut top_files: Vec<FileEntry> = Vec::new();
    let mut subdirs: Vec<(String, bool)> = Vec::new();
    let mut first_query = 1u8;

    loop {
        let mut iosb = IoStatusBlock {
            status_or_pointer: null_mut(),
            information: 0,
        };
        let status = unsafe {
            NtQueryDirectoryFile(
                handle.0,
                null_mut(),
                null(),
                null(),
                &mut iosb,
                buf.0.as_mut_ptr() as *mut c_void,
                buf.0.len() as u32,
                FILE_ID_FULL_DIRECTORY_INFORMATION_CLASS,
                0,
                null(),
                first_query,
            )
        };
        first_query = 0;
        if status == STATUS_NO_MORE_FILES || status < STATUS_SUCCESS || iosb.information == 0 {
            break;
        }

        let mut offset = 0usize;
        loop {
            let entry =
                unsafe { &*(buf.0.as_ptr().add(offset) as *const FileIdFullDirInformation) };
            let name_u16_len = (entry.file_name_length as usize) / 2;
            let name_slice =
                unsafe { std::slice::from_raw_parts(entry.file_name.as_ptr(), name_u16_len) };

            let is_dot = (name_u16_len == 1 && name_slice[0] == b'.' as u16)
                || (name_u16_len == 2
                    && name_slice[0] == b'.' as u16
                    && name_slice[1] == b'.' as u16);

            if !is_dot && name_u16_len > 0 {
                let attrs = entry.file_attributes;
                let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                if is_dir {
                    if !(is_reparse && is_name_surrogate_reparse(entry.ea_size)) {
                        subdirs.push((String::from_utf16_lossy(name_slice), is_reparse));
                    }
                } else {
                    if entry.last_write_time > direct_last_write_time {
                        direct_last_write_time = entry.last_write_time;
                    }
                    if !direct_has_exe && is_app_exe_utf16(name_slice) {
                        direct_has_exe = true;
                    }
                    let effective_size = compute_effective_file_size(
                        attrs,
                        entry.end_of_file,
                        entry.allocation_size,
                    );
                    direct_alloc_size += effective_size;
                    direct_file_count += 1;
                    record_top_file(&mut top_files, name_slice, effective_size);
                }
            }

            if entry.next_entry_offset == 0 {
                break;
            }
            offset += entry.next_entry_offset as usize;
        }
    }

    if top_files.len() > 1 && top_files.len() < MAX_TOP_FILES {
        top_files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    }

    Some(SingleDirSummary {
        direct_alloc_size,
        direct_file_count,
        direct_last_write_time,
        direct_has_exe,
        top_files,
        subdirs,
    })
}

#[inline(always)]
fn compute_effective_file_size(attrs: u32, end_of_file: i64, allocation_size: i64) -> u64 {
    let eof = end_of_file.max(0) as u64;
    let alloc = allocation_size.max(0) as u64;
    let is_cloud_dehydrated = (attrs
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
            | FILE_ATTRIBUTE_RECALL_ON_OPEN))
        != 0;
    if is_cloud_dehydrated {
        alloc
    } else if alloc == 0 && eof <= 4096 {
        eof
    } else {
        alloc
    }
}

const MAX_TOP_FILES: usize = 12;

#[inline(always)]
fn record_top_file(top_files: &mut Vec<FileEntry>, name_slice: &[u16], effective_size: u64) {
    if top_files.len() < MAX_TOP_FILES {
        top_files.push(FileEntry {
            name: String::from_utf16_lossy(name_slice).into_boxed_str(),
            size: effective_size,
        });
        if top_files.len() == MAX_TOP_FILES {
            top_files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
        }
    } else if effective_size > top_files[MAX_TOP_FILES - 1].size {
        top_files[MAX_TOP_FILES - 1] = FileEntry {
            name: String::from_utf16_lossy(name_slice).into_boxed_str(),
            size: effective_size,
        };
        top_files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    }
}

struct FileIdTracker {
    bitset: Box<[AtomicU64]>,
    overflow: DashSet<u64>,
}

impl FileIdTracker {
    fn new() -> Self {
        let mut vec = Vec::with_capacity(BITSET_WORDS);
        for _ in 0..BITSET_WORDS {
            vec.push(AtomicU64::new(0));
        }
        Self {
            bitset: vec.into_boxed_slice(),
            overflow: DashSet::new(),
        }
    }

    #[inline(always)]
    fn mark_seen(&self, file_id: u64) -> bool {
        if file_id == 0 {
            return false;
        }
        let mft_idx = (file_id & 0x0000_FFFF_FFFF_FFFF) as usize;
        let max_bits = BITSET_WORDS * 64;
        if mft_idx < max_bits {
            let word = mft_idx >> 6;
            let mask = 1u64 << (mft_idx & 63);
            let prev = self.bitset[word].fetch_or(mask, Ordering::Relaxed);
            (prev & mask) != 0
        } else {
            !self.overflow.insert(file_id)
        }
    }
}

#[inline(always)]
fn is_name_surrogate_reparse(tag: u32) -> bool {
    (tag & 0x2000_0000) != 0
}

fn open_dir(
    parent: Option<&DirHandle>,
    name_utf16: &[u16],
    open_reparse_raw: bool,
) -> Result<DirHandle, NtStatus> {
    let byte_len = (name_utf16.len() * 2) as u16;
    let us = UnicodeString {
        length: byte_len,
        maximum_length: byte_len,
        buffer: name_utf16.as_ptr(),
    };
    let oa = ObjectAttributes {
        length: std::mem::size_of::<ObjectAttributes>() as u32,
        root_directory: parent.map_or(null_mut(), |h| h.0),
        object_name: &us,
        attributes: OBJ_CASE_INSENSITIVE,
        security_descriptor: null(),
        security_quality_of_service: null(),
    };
    let mut iosb = IoStatusBlock {
        status_or_pointer: null_mut(),
        information: 0,
    };
    let mut handle: Handle = null_mut();
    let mut opts = FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT;
    if open_reparse_raw {
        opts |= FILE_OPEN_REPARSE_POINT;
    }
    let status = unsafe {
        NtOpenFile(
            &mut handle,
            FILE_LIST_DIRECTORY | SYNCHRONIZE,
            &oa,
            &mut iosb,
            FILE_SHARE_ALL,
            opts,
        )
    };
    if status >= STATUS_SUCCESS && !handle.is_null() {
        Ok(DirHandle(handle))
    } else {
        Err(status)
    }
}

#[repr(align(8))]
struct AlignedBuf([u8; 65536]);

pub fn scan_root(
    drive_root: &str,
    live_scanned_bytes: Option<&AtomicU64>,
    live_scanned_files: Option<&AtomicU64>,
    live_scanned_dirs: Option<&AtomicU64>,
    cancel: Option<&AtomicBool>,
) -> ScanTree {
    let start = Instant::now();
    let clean_root = drive_root.trim_end_matches('\\');
    let nt_root = if clean_root.ends_with(':') {
        format!("\\??\\{}\\", clean_root)
    } else {
        format!("\\??\\{}", clean_root)
    };
    let nt_root_utf16: Vec<u16> = nt_root.encode_utf16().collect();

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get().clamp(8, 24))
        .unwrap_or(16);

    let injector: Arc<Injector<WorkItem>> = Arc::new(Injector::new());
    let mut workers: Vec<Worker<WorkItem>> = Vec::with_capacity(num_threads);
    let mut stealers = Vec::with_capacity(num_threads);
    for _ in 0..num_threads {
        let w = Worker::new_lifo();
        stealers.push(w.stealer());
        workers.push(w);
    }
    let stealers = Arc::new(stealers);

    let next_id = Arc::new(AtomicU32::new(1));
    let in_flight = Arc::new(AtomicUsize::new(1));
    let done = Arc::new(AtomicBool::new(false));
    let file_ids = Arc::new(FileIdTracker::new());

    let dedup_count = Arc::new(AtomicU64::new(0));
    let denied_count = Arc::new(AtomicU64::new(0));

    injector.push(WorkItem {
        id: 0,
        parent_id: u32::MAX,
        parent_handle: None,
        name_utf16: nt_root_utf16.into_boxed_slice(),
        is_cloud_reparse: false,
    });

    let root_display_name = if clean_root.ends_with(':') {
        format!("{}\\", clean_root)
    } else {
        clean_root.to_string()
    };

    let thread_records: Vec<Vec<RawDirRecord>> = std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(num_threads);
        for (worker_idx, local) in workers.into_iter().enumerate() {
            let injector = Arc::clone(&injector);
            let stealers = Arc::clone(&stealers);
            let next_id = Arc::clone(&next_id);
            let in_flight = Arc::clone(&in_flight);
            let done = Arc::clone(&done);
            let file_ids = Arc::clone(&file_ids);
            let dedup_count = Arc::clone(&dedup_count);
            let denied_count = Arc::clone(&denied_count);
            let root_name = root_display_name.clone();

            handles.push(s.spawn(move || {
                let mut buf = Box::new(AlignedBuf([0u8; 65536]));
                let mut out: Vec<RawDirRecord> = Vec::with_capacity(8192);
                let mut spin = 0u32;

                loop {
                    if let Some(c) = cancel {
                        if c.load(Ordering::Relaxed) {
                            break;
                        }
                    }

                    let item = local.pop().or_else(|| {
                        loop {
                            match injector.steal_batch_and_pop(&local) {
                                Steal::Success(t) => return Some(t),
                                Steal::Retry => continue,
                                Steal::Empty => break,
                            }
                        }
                        let n = stealers.len();
                        for offset in 1..n {
                            let target = (worker_idx + offset) % n;
                            loop {
                                match stealers[target].steal_batch_and_pop(&local) {
                                    Steal::Success(t) => return Some(t),
                                    Steal::Retry => continue,
                                    Steal::Empty => break,
                                }
                            }
                        }
                        None
                    });

                    let Some(work) = item else {
                        if done.load(Ordering::Acquire) {
                            break;
                        }
                        spin += 1;
                        if spin < 64 {
                            std::hint::spin_loop();
                        } else {
                            std::thread::yield_now();
                            if spin > 256 {
                                std::thread::sleep(std::time::Duration::from_micros(50));
                            }
                        }
                        continue;
                    };
                    spin = 0;

                    let display_name: Box<str> = if work.id == 0 {
                        root_name.clone().into_boxed_str()
                    } else {
                        String::from_utf16_lossy(&work.name_utf16).into_boxed_str()
                    };

                    let open_res = if work.id == 0 {
                        open_dir(None, &work.name_utf16, false)
                    } else if work.is_cloud_reparse {
                        open_dir(work.parent_handle.as_deref(), &work.name_utf16, true).or_else(
                            |_| open_dir(work.parent_handle.as_deref(), &work.name_utf16, false),
                        )
                    } else {
                        open_dir(work.parent_handle.as_deref(), &work.name_utf16, true)
                    };
                    drop(work.parent_handle);

                    let Ok(dir_handle) = open_res else {
                        denied_count.fetch_add(1, Ordering::Relaxed);
                        out.push(RawDirRecord {
                            id: work.id,
                            parent_id: work.parent_id,
                            name: display_name,
                            access_denied: true,
                            ..Default::default()
                        });
                        if let Some(d) = live_scanned_dirs {
                            d.fetch_add(1, Ordering::Relaxed);
                        }
                        if in_flight.fetch_sub(1, Ordering::AcqRel) == 1 {
                            done.store(true, Ordering::Release);
                            break;
                        }
                        continue;
                    };

                    let shared_handle = Arc::new(dir_handle);
                    let mut direct_alloc_size = 0u64;
                    let mut direct_logical_size = 0u64;
                    let mut direct_file_count = 0u32;
                    let mut direct_last_write_time = 0i64;
                    let mut direct_has_exe = false;
                    let mut local_dedup = 0u64;
                    let mut top_files: Vec<FileEntry> = Vec::new();
                    let mut first_query = 1u8;
                    let mut query_len: u32 = 8192;

                    loop {
                        let mut iosb = IoStatusBlock {
                            status_or_pointer: null_mut(),
                            information: 0,
                        };
                        let status = unsafe {
                            NtQueryDirectoryFile(
                                shared_handle.0,
                                null_mut(),
                                null(),
                                null(),
                                &mut iosb,
                                buf.0.as_mut_ptr() as *mut c_void,
                                query_len,
                                FILE_ID_FULL_DIRECTORY_INFORMATION_CLASS,
                                0,
                                null(),
                                first_query,
                            )
                        };
                        first_query = 0;

                        if status == STATUS_NO_MORE_FILES
                            || status < STATUS_SUCCESS
                            || iosb.information == 0
                        {
                            break;
                        }
                        if iosb.information >= 4096 {
                            query_len = buf.0.len() as u32;
                        } else {
                            query_len = 4096;
                        }

                        let mut offset = 0usize;
                        loop {
                            let entry = unsafe {
                                &*(buf.0.as_ptr().add(offset) as *const FileIdFullDirInformation)
                            };
                            let name_u16_len = (entry.file_name_length as usize) / 2;
                            let name_slice = unsafe {
                                std::slice::from_raw_parts(entry.file_name.as_ptr(), name_u16_len)
                            };

                            let is_dot = (name_u16_len == 1 && name_slice[0] == b'.' as u16)
                                || (name_u16_len == 2
                                    && name_slice[0] == b'.' as u16
                                    && name_slice[1] == b'.' as u16);

                            if !is_dot && name_u16_len > 0 {
                                let attrs = entry.file_attributes;
                                let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                                let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                                if is_dir {
                                    let skip =
                                        is_reparse && is_name_surrogate_reparse(entry.ea_size);
                                    if !skip {
                                        let child_id = next_id.fetch_add(1, Ordering::Relaxed);
                                        in_flight.fetch_add(1, Ordering::Relaxed);
                                        local.push(WorkItem {
                                            id: child_id,
                                            parent_id: work.id,
                                            parent_handle: Some(Arc::clone(&shared_handle)),
                                            name_utf16: name_slice.into(),
                                            is_cloud_reparse: is_reparse,
                                        });
                                    }
                                } else {
                                    if entry.last_write_time > direct_last_write_time {
                                        direct_last_write_time = entry.last_write_time;
                                    }
                                    if !direct_has_exe && is_app_exe_utf16(name_slice) {
                                        direct_has_exe = true;
                                    }
                                    let fid = entry.file_id as u64;
                                    if file_ids.mark_seen(fid) {
                                        local_dedup += 1;
                                    } else {
                                        let eof = entry.end_of_file.max(0) as u64;
                                        let effective_size = compute_effective_file_size(
                                            attrs,
                                            entry.end_of_file,
                                            entry.allocation_size,
                                        );
                                        direct_alloc_size += effective_size;
                                        direct_logical_size += eof;
                                        direct_file_count += 1;
                                        record_top_file(&mut top_files, name_slice, effective_size);
                                    }
                                }
                            }

                            if entry.next_entry_offset == 0 {
                                break;
                            }
                            offset += entry.next_entry_offset as usize;
                        }
                    }

                    drop(shared_handle);

                    if top_files.len() > 1 && top_files.len() < 24 {
                        top_files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
                    }

                    if local_dedup > 0 {
                        dedup_count.fetch_add(local_dedup, Ordering::Relaxed);
                    }
                    if let Some(b) = live_scanned_bytes {
                        b.fetch_add(direct_alloc_size, Ordering::Relaxed);
                    }
                    if let Some(f) = live_scanned_files {
                        f.fetch_add(direct_file_count as u64, Ordering::Relaxed);
                    }
                    if let Some(d) = live_scanned_dirs {
                        d.fetch_add(1, Ordering::Relaxed);
                    }

                    out.push(RawDirRecord {
                        id: work.id,
                        parent_id: work.parent_id,
                        name: display_name,
                        direct_alloc_size,
                        direct_logical_size,
                        direct_file_count,
                        direct_last_write_time,
                        direct_has_exe,
                        access_denied: false,
                        top_files,
                    });

                    if in_flight.fetch_sub(1, Ordering::AcqRel) == 1 {
                        done.store(true, Ordering::Release);
                        break;
                    }
                }
                out
            }));
        }

        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let total_nodes = next_id.load(Ordering::Relaxed) as usize;
    let mut nodes: Vec<DirNode> = vec![DirNode::default(); total_nodes];
    let mut total_logical_bytes = 0u64;

    for batch in thread_records {
        for rec in batch {
            let idx = rec.id as usize;
            total_logical_bytes += rec.direct_logical_size;
            nodes[idx] = DirNode {
                id: rec.id,
                parent_id: rec.parent_id,
                name: rec.name,
                total_size: rec.direct_alloc_size,
                direct_size: rec.direct_alloc_size,
                direct_file_count: rec.direct_file_count,
                total_file_count: rec.direct_file_count as u64,
                total_dir_count: 0,
                direct_last_write_time: rec.direct_last_write_time,
                last_write_time: rec.direct_last_write_time,
                direct_has_exe: rec.direct_has_exe,
                has_exe: rec.direct_has_exe,
                access_denied: rec.access_denied,
                children: Vec::new(),
                top_files: rec.top_files,
            };
        }
    }

    for id in 1..total_nodes {
        let p = nodes[id].parent_id as usize;
        if p < total_nodes && !nodes[id].name.is_empty() {
            nodes[p].children.push(id as u32);
        }
    }

    for id in (1..total_nodes).rev() {
        let p = nodes[id].parent_id as usize;
        if p < total_nodes && !nodes[id].name.is_empty() {
            let child_size = nodes[id].total_size;
            let child_files = nodes[id].total_file_count;
            let child_dirs = nodes[id].total_dir_count + 1;
            let child_lwt = nodes[id].last_write_time;
            let child_exe = nodes[id].has_exe;
            nodes[p].total_size += child_size;
            nodes[p].total_file_count += child_files;
            nodes[p].total_dir_count += child_dirs;
            if child_lwt > nodes[p].last_write_time {
                nodes[p].last_write_time = child_lwt;
            }
            nodes[p].has_exe |= child_exe;
        }
    }

    for id in 0..total_nodes {
        if nodes[id].children.len() > 1 {
            let mut ch = std::mem::take(&mut nodes[id].children);
            ch.sort_unstable_by(|&a, &b| {
                nodes[b as usize]
                    .total_size
                    .cmp(&nodes[a as usize].total_size)
                    .then_with(|| nodes[a as usize].name.cmp(&nodes[b as usize].name))
            });
            nodes[id].children = ch;
        }
    }

    let elapsed_ms = start.elapsed().as_millis();
    let total_alloc_bytes = nodes.first().map_or(0, |n| n.total_size);
    let total_files = nodes.first().map_or(0, |n| n.total_file_count);

    ScanTree {
        root_path: root_display_name,
        nodes,
        stats: ScanStats {
            elapsed_ms,
            total_dirs: total_nodes,
            total_files,
            deduped_hardlinks: dedup_count.load(Ordering::Relaxed),
            denied_dirs: denied_count.load(Ordering::Relaxed),
            total_alloc_bytes,
            total_logical_bytes,
        },
    }
}
