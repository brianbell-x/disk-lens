use crate::cache;
use crate::scanner::ScanTree;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Component, Path, PathBuf};
use std::ptr::{null, null_mut};
use std::time::{SystemTime, UNIX_EPOCH};

type HKey = *mut c_void;

const HKEY_CURRENT_USER_USIZE: usize = 0x8000_0001;
const HKEY_LOCAL_MACHINE_USIZE: usize = 0x8000_0002;
const KEY_READ: u32 = 0x20019;
const KEY_WOW64_64KEY: u32 = 0x0100;
const KEY_WOW64_32KEY: u32 = 0x0200;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;

const TINY_THRESHOLD_BYTES: u64 = 65_536;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegOpenKeyExW(
        hkey: HKey,
        subkey: *const u16,
        options: u32,
        sam_desired: u32,
        phk_result: *mut HKey,
    ) -> i32;

    fn RegEnumKeyExW(
        hkey: HKey,
        index: u32,
        lp_name: *mut u16,
        lpcch_name: *mut u32,
        lp_reserved: *const u32,
        lp_class: *mut u16,
        lpcch_class: *mut u32,
        lpft_last_write_time: *mut u64,
    ) -> i32;

    fn RegQueryValueExW(
        hkey: HKey,
        lp_value_name: *const u16,
        lp_reserved: *const u32,
        lp_type: *mut u32,
        lp_data: *mut u8,
        lpcb_data: *mut u32,
    ) -> i32;

    fn RegCloseKey(hkey: HKey) -> i32;
}

struct RegKey(HKey);
impl Drop for RegKey {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }
}

fn open_reg_key(root: usize, subkey: &str, extra_sam: u32) -> Option<RegKey> {
    let wide: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut out: HKey = null_mut();
    let rc = unsafe {
        RegOpenKeyExW(
            root as HKey,
            wide.as_ptr(),
            0,
            KEY_READ | extra_sam,
            &mut out,
        )
    };
    if rc == 0 && !out.is_null() {
        Some(RegKey(out))
    } else {
        None
    }
}

fn enum_subkeys(key: &RegKey) -> Vec<String> {
    let mut names = Vec::new();
    let mut buf = [0u16; 512];
    let mut idx = 0u32;
    loop {
        let mut len = buf.len() as u32;
        let rc = unsafe {
            RegEnumKeyExW(
                key.0,
                idx,
                buf.as_mut_ptr(),
                &mut len,
                null(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if rc != 0 {
            break;
        }
        names.push(String::from_utf16_lossy(&buf[..len as usize]));
        idx += 1;
    }
    names
}

fn query_reg_string(key: &RegKey, value_name: &str) -> Option<String> {
    let wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut ty = 0u32;
    let mut byte_len = 0u32;
    let rc1 = unsafe {
        RegQueryValueExW(
            key.0,
            wide.as_ptr(),
            null(),
            &mut ty,
            null_mut(),
            &mut byte_len,
        )
    };
    if rc1 != 0 || (ty != REG_SZ && ty != REG_EXPAND_SZ) || byte_len < 2 || byte_len > 32768 {
        return None;
    }
    let mut data = vec![0u8; byte_len as usize];
    let rc2 = unsafe {
        RegQueryValueExW(
            key.0,
            wide.as_ptr(),
            null(),
            &mut ty,
            data.as_mut_ptr(),
            &mut byte_len,
        )
    };
    if rc2 != 0 {
        return None;
    }
    let u16_len = (byte_len as usize) / 2;
    let u16_slice = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u16, u16_len) };
    let trimmed_len = u16_slice
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(u16_slice.len());
    let s = String::from_utf16_lossy(&u16_slice[..trimmed_len])
        .trim()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(expand_env_vars(&s))
    }
}

fn is_env_var_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'(' | b')' | b' ')
}

fn try_expand_env_at(input: &str, start: usize) -> Option<(String, usize)> {
    let rest = input.get(start + 1..)?;
    let rel_end = rest.find('%')?;
    let var_name = &rest[..rel_end];
    if var_name.is_empty() || !var_name.bytes().all(is_env_var_byte) {
        return None;
    }
    let val = std::env::var(var_name).ok()?;
    Some((val, start + rel_end + 2))
}

/// Expands `%VAR%` environment placeholders in Windows paths and command strings.
fn expand_env_vars(input: &str) -> String {
    if !input.contains('%') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some((val, next_i)) = try_expand_env_at(input, i) {
                out.push_str(&val);
                i = next_i;
                continue;
            }
        }
        let Some(ch) = input[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Normalizes a name to lowercase alphanumeric characters only (e.g. "LM-Studio" -> "lmstudio").
pub fn norm_compact(s: &str) -> String {
    let stripped = s.trim_start_matches('.');
    stripped
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

const STOP_WORDS: &[&str] = &[
    "inc",
    "llc",
    "ltd",
    "corp",
    "corporation",
    "company",
    "co",
    "gmbh",
    "bv",
    "pte",
    "limited",
    "software",
    "technologies",
    "technology",
    "systems",
    "studio",
    "desktop",
    "client",
    "app",
    "apps",
    "application",
    "launcher",
    "installer",
    "setup",
    "updater",
    "update",
    "service",
    "helper",
    "agent",
    "cli",
    "sdk",
    "runtime",
    "tools",
    "tool",
    "x64",
    "x86",
    "arm64",
    "win64",
    "win32",
    "windows",
    "current",
    "user",
    "edition",
    "community",
    "pro",
    "free",
    "local",
    "roaming",
    "data",
    "com",
    "org",
    "net",
    "io",
    "dev",
    "the",
    "for",
    "and",
    "with",
    "from",
    "version",
    "release",
    "stable",
    "beta",
    "nightly",
    "preview",
    "insiders",
];

/// Generic English words that must NEVER exonerate a folder by single-token overlap alone.
const GENERIC_TOKENS: &[&str] = &[
    "browser",
    "code",
    "proxy",
    "cloud",
    "file",
    "files",
    "clean",
    "cleaner",
    "print",
    "screen",
    "open",
    "visual",
    "shared",
    "common",
    "core",
    "base",
    "lock",
    "hub",
    "sync",
    "drive",
    "mail",
    "chat",
    "media",
    "player",
    "viewer",
    "editor",
    "kit",
    "kits",
    "api",
    "web",
    "test",
    "testing",
    "debug",
    "cache",
    "temp",
    "logs",
    "log",
    "config",
    "state",
    "daemon",
    "server",
    "plugin",
    "extension",
    "engine",
    "driver",
    "bridge",
    "manager",
    "monitor",
    "guard",
    "shield",
    "secure",
    "safe",
    "auto",
    "smart",
    "plus",
    "lite",
    "mini",
    "one",
    "new",
    "win",
    "system",
    "users",
    "public",
    "default",
    "backup",
    "history",
    "store",
    "game",
    "games",
    "video",
    "audio",
    "music",
    "photo",
    "photos",
    "image",
    "images",
    "pdf",
    "doc",
    "docs",
    "text",
    "note",
    "notes",
    "font",
    "fonts",
    "icon",
    "icons",
    "theme",
    "themes",
    "time",
    "task",
    "tasks",
    "work",
    "office",
    "home",
    "start",
    "menu",
    "panel",
    "shell",
    "terminal",
    "console",
    "script",
    "scripts",
    "build",
    "make",
    "run",
    "bin",
    "lib",
    "pkg",
    "pack",
    "package",
    "packages",
    "module",
    "node",
    "env",
    "venv",
    "virtual",
    "box",
    "remote",
    "connect",
    "link",
    "port",
    "host",
    "tunnel",
    "vpn",
    "http",
    "ssh",
    "lab",
    "labs",
    "works",
    "soft",
    "tech",
    "group",
    "team",
    "project",
    "foundation",
    "network",
    "security",
    "mobile",
    "global",
    "snapshots",
    "snapshot",
    "stream",
    "metadata",
    "corrupt",
    "detector",
    "hook",
    "secrets",
    "token",
    "tokens",
    "crash",
    "crashreporter",
    "assistant",
    "download",
    "settings",
    "setting",
    "easy",
    "meetings",
    "meeting",
    "addin",
];

fn is_generic_token(tok: &str) -> bool {
    GENERIC_TOKENS.contains(&tok) || STOP_WORDS.contains(&tok)
}

/// Splits a name into meaningful lowercase alphanumeric words, correctly preserving ALL-CAPS acronyms
/// (e.g. "OBS Studio" -> ["obs"], "LGHUB" -> ["lghub"], "BraveSoftware" -> ["brave"], "PDFViewer" -> ["pdf", "viewer"]).
pub fn norm_tokens(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_orig: Option<char> = None;

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_alphanumeric() {
            let next_orig = chars.get(i + 1).copied();
            let is_camel_split = prev_orig.is_some_and(|p| p.is_ascii_lowercase())
                && ch.is_ascii_uppercase();
            let is_acronym_end = prev_orig.is_some_and(|p| p.is_ascii_uppercase())
                && ch.is_ascii_uppercase()
                && next_orig.is_some_and(|n| n.is_ascii_lowercase());

            if (is_camel_split || is_acronym_end) && !cur.is_empty() {
                push_token(&mut out, &cur);
                cur.clear();
            }
            cur.push(ch.to_ascii_lowercase());
            prev_orig = Some(ch);
        } else {
            if !cur.is_empty() {
                push_token(&mut out, &cur);
                cur.clear();
            }
            prev_orig = None;
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, &cur);
    }
    out
}

fn push_token(out: &mut Vec<String>, tok: &str) {
    if tok.len() >= 3 && !tok.chars().all(|c| c.is_ascii_digit()) && !STOP_WORDS.contains(&tok) {
        out.push(tok.to_string());
    }
}

fn extract_exe_path(cmd: &str) -> Option<String> {
    let expanded = expand_env_vars(cmd);
    let trimmed = expanded.trim();
    if let Some(rest) = trimmed.strip_prefix('"') {
        let end = rest.find('"')?;
        let candidate = &rest[..end];
        if candidate.contains(":\\") {
            return Some(candidate.to_string());
        }
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(pos) = lower.find(".exe") {
        let candidate = trimmed[..pos + 4].trim();
        if candidate.contains(":\\") {
            return Some(candidate.to_string());
        }
    }
    None
}

fn is_windows_os_dir(p: &Path) -> bool {
    let lower = p.to_string_lossy().to_ascii_lowercase();
    lower.contains("\\windows\\") || lower.ends_with("\\windows")
}

/// Checks if a directory looks dead/empty (does not exist, has zero files, or only `deleteonreboot`).
fn dir_is_empty_or_delete_on_reboot(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return !dir.exists();
    };
    let mut found_real = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name == "deleteonreboot" {
            continue;
        }
        found_real = true;
        break;
    }
    !found_real
}

fn dir_has_delete_on_reboot(dir: &Path) -> bool {
    dir.join("deleteonreboot").exists()
}

/// Checks whether a directory contains a real application executable (excluding uninstallers,
/// updaters, crash reporters, and embedded runtime shims like python.exe unless the folder is Python).
fn dir_has_main_app_exe(dir: &Path, folder_name: &str, max_depth: usize) -> bool {
    let folder_compact = norm_compact(folder_name);
    let folder_toks = norm_tokens(folder_name);
    let is_python_folder = folder_compact.contains("python");
    dir_has_main_app_exe_inner(dir, &folder_compact, &folder_toks, is_python_folder, max_depth)
}

fn exe_stem_lower(p: &Path) -> Option<String> {
    let ext = p.extension()?.to_str()?;
    if !ext.eq_ignore_ascii_case("exe") {
        return None;
    }
    let stem = p.file_stem()?.to_str()?;
    Some(stem.to_ascii_lowercase())
}

fn stem_matches_folder(stem: &str, folder_compact: &str, folder_toks: &[String]) -> bool {
    let stem_c = norm_compact(stem);
    let direct_match = !folder_compact.is_empty()
        && (stem_c == folder_compact
            || stem_c.contains(folder_compact)
            || (folder_compact.contains(&stem_c) && stem_c.len() >= 4));
    if direct_match {
        return true;
    }
    folder_toks
        .iter()
        .any(|ft| !is_generic_token(ft) && stem_c.contains(ft.as_str()))
}

fn is_ignored_runtime_subdir(name_lower: &str) -> bool {
    name_lower.starts_with("python-core")
        || matches!(name_lower, "uninstall" | "update" | "deleteonreboot")
}

fn dir_has_main_app_exe_inner(
    dir: &Path,
    folder_compact: &str,
    folder_toks: &[String],
    is_python_folder: bool,
    max_depth: usize,
) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            let Some(stem) = exe_stem_lower(&p) else {
                continue;
            };
            if is_stub_or_helper_exe(&stem, is_python_folder) {
                continue;
            }
            if stem_matches_folder(&stem, folder_compact, folder_toks) {
                return true;
            }
            continue;
        }
        if max_depth <= 1 || !p.is_dir() {
            continue;
        }
        let sub_name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if is_ignored_runtime_subdir(&sub_name) {
            continue;
        }
        if dir_has_main_app_exe_inner(&p, folder_compact, folder_toks, is_python_folder, max_depth - 1) {
            return true;
        }
    }
    false
}

fn is_stub_or_helper_exe(stem_lower: &str, is_python_folder: bool) -> bool {
    if stem_lower.starts_with("unins")
        || stem_lower.contains("uninstall")
        || stem_lower == "update"
        || stem_lower == "squirrel"
        || stem_lower.ends_with("updater")
        || stem_lower.ends_with("_updater")
        || stem_lower.ends_with(".updater")
        || stem_lower.contains("crashpad")
        || stem_lower.contains("crashreporter")
        || stem_lower.contains("bug_report")
        || stem_lower.contains("elevation_service")
        || stem_lower == "setup"
    {
        return true;
    }
    !is_python_folder && matches!(stem_lower, "python" | "pythonw")
}

fn dir_contains_any_non_stub_exe_disk(dir: &Path, max_depth: usize, is_python_folder: bool) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            let Some(stem) = exe_stem_lower(&p) else {
                continue;
            };
            if !is_stub_or_helper_exe(&stem, is_python_folder) {
                return true;
            }
            continue;
        }
        if max_depth > 1
            && p.is_dir()
            && dir_contains_any_non_stub_exe_disk(&p, max_depth - 1, is_python_folder)
        {
            return true;
        }
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Confirmed,
    Likely,
    Review,
}

impl Confidence {
    pub fn rank(self) -> u8 {
        match self {
            Confidence::Confirmed => 0,
            Confidence::Likely => 1,
            Confidence::Review => 2,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct OrphanMatch {
    pub node_id: u32,
    pub app_key: String,
    pub app_label: String,
    pub name: String,
    pub path: String,
    pub location_group: &'static str,
    pub size: u64,
    pub is_tiny: bool,
    pub days_idle: Option<u64>,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrphanGroup {
    pub app_key: String,
    pub app_label: String,
    pub confidence: Confidence,
    pub total_size: u64,
    pub non_tiny_size: u64,
    pub has_non_tiny: bool,
    pub max_days_idle: Option<u64>,
    pub min_days_idle: Option<u64>,
    pub summary_reason: String,
    pub evidence: Vec<String>,
    pub items: Vec<OrphanMatch>,
}

#[derive(Clone, Debug)]
struct BrokenSignal {
    label: String,
    detail: String,
}

pub struct InstalledIndex {
    /// Exact compact names of verified installed apps, services, shortcuts, and user CLIs.
    compact_names: HashSet<String>,
    /// Unambiguous primary brand tokens (len >= 3, NOT generic words) from verified installed apps.
    primary_brands: HashSet<String>,
    /// Multi-token sets for each verified installed app (for strict subset matching).
    app_token_sets: Vec<HashSet<String>>,
    /// Verified publisher compact names (only matched on exact vendor folder names / reverse-DNS).
    publisher_compacts: HashSet<String>,
    /// Active MSIX package family names whose PackageRootFolder exists.
    appx_families_lower: HashSet<String>,
    /// Normalized lowercase root paths of active installed dirs (PATH bin parents, InstallLocations).
    active_install_paths_lower: HashSet<String>,
    /// Positive evidence of uninstalled/broken apps keyed by normalized compact slug.
    broken_signals: HashMap<String, BrokenSignal>,
}

impl InstalledIndex {
    pub fn gather() -> Self {
        let mut idx = Self {
            compact_names: HashSet::with_capacity(2048),
            primary_brands: HashSet::with_capacity(1024),
            app_token_sets: Vec::with_capacity(512),
            publisher_compacts: HashSet::with_capacity(256),
            appx_families_lower: HashSet::with_capacity(512),
            active_install_paths_lower: HashSet::with_capacity(256),
            broken_signals: HashMap::with_capacity(64),
        };

        // 1. Registry Uninstall entries (verifying InstallLocation / UninstallString exist & are non-empty)
        let uninstall_paths = [
            (
                HKEY_LOCAL_MACHINE_USIZE,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                KEY_WOW64_64KEY,
            ),
            (
                HKEY_LOCAL_MACHINE_USIZE,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                KEY_WOW64_32KEY,
            ),
            (
                HKEY_CURRENT_USER_USIZE,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                0,
            ),
        ];

        for (hive, base_subkey, flag) in uninstall_paths {
            let Some(parent) = open_reg_key(hive, base_subkey, flag) else {
                continue;
            };
            for sub in enum_subkeys(&parent) {
                let full_sub = format!("{base_subkey}\\{sub}");
                let Some(entry_key) = open_reg_key(hive, &full_sub, flag) else {
                    continue;
                };
                let display_name = query_reg_string(&entry_key, "DisplayName");
                let publisher = query_reg_string(&entry_key, "Publisher");
                let install_loc = query_reg_string(&entry_key, "InstallLocation");
                let uninstall_str = query_reg_string(&entry_key, "UninstallString");

                let mut verified_alive = true;
                let mut dead_detail: Option<String> = None;

                if let Some(ref loc) = install_loc {
                    let clean_loc = loc.trim_matches('"').trim().trim_end_matches('\\');
                    if clean_loc.len() > 3 && clean_loc.contains(":\\") {
                        let p = Path::new(clean_loc);
                        if !p.exists() || dir_is_empty_or_delete_on_reboot(p) {
                            verified_alive = false;
                            dead_detail = Some("Ghost Windows uninstall entry".to_string());
                        }
                    }
                }
                if verified_alive {
                    if let Some(ref uninst) = uninstall_str {
                        let lower_u = uninst.to_ascii_lowercase();
                        if !lower_u.contains("msiexec") && !lower_u.starts_with("winget") {
                            if let Some(exe_p) = extract_exe_path(uninst) {
                                let sys_bin = exe_p
                                    .to_ascii_lowercase()
                                    .contains("\\windows\\system32\\");
                                if !sys_bin && !Path::new(&exe_p).exists() {
                                    verified_alive = false;
                                    dead_detail = Some("Uninstaller executable missing".to_string());
                                }
                            }
                        }
                    }
                }

                if !verified_alive {
                    let label = display_name.clone().unwrap_or_else(|| sub.clone());
                    let detail = dead_detail
                        .unwrap_or_else(|| "Ghost Windows uninstall entry".to_string());
                    idx.record_broken_phrase(&label, &label, &detail);
                    if let Some(ref loc) = install_loc {
                        idx.record_broken_path(loc, &label, &detail);
                    }
                    continue;
                }

                if let Some(dn) = display_name {
                    idx.insert_app_phrase(&dn);
                }
                if let Some(pub_name) = publisher {
                    idx.insert_publisher(&pub_name);
                }
                if !sub.starts_with('{') {
                    idx.insert_app_phrase(&sub);
                }
                if let Some(loc) = install_loc {
                    let clean_loc = loc.trim_matches('"').trim().trim_end_matches('\\');
                    if clean_loc.len() > 3 && clean_loc.contains(":\\") {
                        idx.active_install_paths_lower
                            .insert(clean_loc.to_ascii_lowercase());
                        if let Some(leaf) = Path::new(clean_loc).file_name().and_then(|s| s.to_str())
                        {
                            idx.insert_app_phrase(leaf);
                        }
                        if let Some(parent_dir) = Path::new(clean_loc)
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                        {
                            if !is_container_dir_name(parent_dir) {
                                idx.insert_publisher(parent_dir);
                            }
                        }
                    }
                }
            }
        }

        idx.gather_appx_packages();
        idx.gather_services();
        idx.gather_start_menu();
        let cli_stems = idx.gather_path_clis();
        idx.register_toolchain_caches(&cli_stems);

        // Cross-check Program Files\Google\Drive File Stream for deleteonreboot remnant
        for pf in [r"C:\Program Files\Google\Drive File Stream", r"C:\Program Files (x86)\Google\Drive File Stream"] {
            let p = Path::new(pf);
            if p.exists() && dir_is_empty_or_delete_on_reboot(p) {
                let detail = if dir_has_delete_on_reboot(p) {
                    "Uninstall remnant (deleteonreboot)"
                } else {
                    "Empty Program Files shell"
                };
                idx.record_broken_key("drivefs", "Google Drive", detail);
                idx.record_broken_key("drivefilestream", "Google Drive", detail);
            }
        }

        // Never let a broken sub-product shortcut poison an active publisher or active installed app
        let active_compacts = idx.compact_names.clone();
        let active_pubs = idx.publisher_compacts.clone();
        idx.broken_signals
            .retain(|k, _| !active_compacts.contains(k) && !active_pubs.contains(k));

        idx
    }

    fn gather_appx_packages(&mut self) {
        let base = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";
        let Some(appx_key) = open_reg_key(HKEY_CURRENT_USER_USIZE, base, 0) else {
            return;
        };

        for pkg_full in enum_subkeys(&appx_key) {
            let full_pkg = format!("{base}\\{pkg_full}");
            let Some(pk) = open_reg_key(HKEY_CURRENT_USER_USIZE, &full_pkg, 0) else {
                continue;
            };
            if let Some(root_dir) = query_reg_string(&pk, "PackageRootFolder") {
                if !Path::new(&root_dir).exists() {
                    continue;
                }
            }

            let parts: Vec<&str> = pkg_full.split('_').collect();
            let (Some(&first), Some(&last)) = (parts.first(), parts.last()) else {
                continue;
            };
            self.appx_families_lower
                .insert(format!("{first}_{last}").to_ascii_lowercase());
            self.insert_app_phrase(first);

            let Some(dn) = query_reg_string(&pk, "DisplayName") else {
                continue;
            };
            if !dn.starts_with('@') && !dn.starts_with("ms-resource:") {
                self.insert_app_phrase(&dn);
            }
        }
    }

    fn gather_services(&mut self) {
        let base = r"SYSTEM\CurrentControlSet\Services";
        let Some(svc_root) = open_reg_key(HKEY_LOCAL_MACHINE_USIZE, base, 0) else {
            return;
        };

        for svc_name in enum_subkeys(&svc_root) {
            let full_svc = format!("{base}\\{svc_name}");
            let Some(sk) = open_reg_key(HKEY_LOCAL_MACHINE_USIZE, &full_svc, 0) else {
                continue;
            };
            let Some(img) = query_reg_string(&sk, "ImagePath") else {
                continue;
            };
            let Some(exe_p) = extract_exe_path(&img) else {
                continue;
            };

            let p = Path::new(&exe_p);
            if is_windows_os_dir(p) || !p.exists() {
                continue;
            }
            self.insert_app_phrase(&svc_name);
            self.register_exe_parent_dir(p);
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                self.insert_app_phrase(stem);
            }
        }
    }

    fn register_exe_parent_dir(&mut self, exe_path: &Path) {
        let Some(parent_p) = exe_path.parent() else {
            return;
        };
        let parent_lower = parent_p
            .to_string_lossy()
            .trim_end_matches('\\')
            .to_ascii_lowercase();
        self.active_install_paths_lower.insert(parent_lower);

        let Some(dir_name) = parent_p.file_name().and_then(|s| s.to_str()) else {
            return;
        };
        if !is_container_dir_name(dir_name)
            && !matches!(
                dir_name.to_ascii_lowercase().as_str(),
                "bin" | "apps" | "application"
            )
        {
            self.insert_app_phrase(dir_name);
        }
    }

    fn gather_start_menu(&mut self) {
        let mut start_menus = vec![PathBuf::from(
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs",
        )];
        if let Ok(appdata) = std::env::var("APPDATA") {
            start_menus
                .push(Path::new(&appdata).join(r"Microsoft\Windows\Start Menu\Programs"));
        }
        for sm in start_menus {
            collect_start_menu_names(&sm, 0, self);
        }
    }

    fn gather_path_clis(&mut self) -> HashSet<String> {
        let mut bin_dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        if let Ok(home) = std::env::var("USERPROFILE") {
            let hp = Path::new(&home);
            bin_dirs.push(hp.join(r".cargo\bin"));
            bin_dirs.push(hp.join(r".local\bin"));
            bin_dirs.push(hp.join(r"go\bin"));
            bin_dirs.push(hp.join(r".bun\bin"));
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            bin_dirs.push(Path::new(&appdata).join("npm"));
        }
        if let Ok(lad) = std::env::var("LOCALAPPDATA") {
            bin_dirs.push(Path::new(&lad).join(r"Microsoft\WinGet\Links"));
        }

        let mut seen_bin_dirs: HashSet<String> = HashSet::new();
        let mut cli_stems: HashSet<String> = HashSet::new();
        for bdir in bin_dirs {
            if is_windows_os_dir(&bdir) || !bdir.is_dir() {
                continue;
            }
            let bdir_lower = bdir
                .to_string_lossy()
                .trim_end_matches('\\')
                .to_ascii_lowercase();
            if !seen_bin_dirs.insert(bdir_lower.clone()) {
                continue;
            }
            if self.scan_cli_bin_dir(&bdir, &mut cli_stems) {
                self.register_active_bin_dir(&bdir, bdir_lower);
            }
        }
        cli_stems
    }

    fn scan_cli_bin_dir(&mut self, bdir: &Path, cli_stems: &mut HashSet<String>) -> bool {
        let Ok(entries) = std::fs::read_dir(bdir) else {
            return false;
        };
        let mut found = false;
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let Some(s) = fname.to_str() else {
                continue;
            };
            let lower = s.to_ascii_lowercase();
            let Some(stem) = strip_cli_ext(&lower) else {
                continue;
            };
            if stem.len() < 2 {
                continue;
            }
            found = true;
            cli_stems.insert(stem.to_string());
            self.insert_cli_stem(stem);
            if let Some(ctl_base) = stem.strip_suffix("ctl") {
                if ctl_base.len() >= 3 {
                    cli_stems.insert(ctl_base.to_string());
                    self.insert_cli_stem(ctl_base);
                }
            }
        }
        found
    }

    fn register_active_bin_dir(&mut self, bdir: &Path, bdir_lower: String) {
        self.active_install_paths_lower.insert(bdir_lower);
        let Some(leaf) = bdir.file_name().and_then(|s| s.to_str()) else {
            return;
        };
        let is_sub_bin = matches!(
            leaf.to_ascii_lowercase().as_str(),
            "bin" | "scripts" | "cmd" | "current"
        );
        if !is_sub_bin {
            if !is_container_dir_name(leaf) {
                self.insert_app_phrase(leaf);
            }
            return;
        }
        let Some(owner) = bdir.parent() else {
            return;
        };
        let owner_lower = owner
            .to_string_lossy()
            .trim_end_matches('\\')
            .to_ascii_lowercase();
        self.active_install_paths_lower.insert(owner_lower);
        if let Some(owner_name) = owner.file_name().and_then(|s| s.to_str()) {
            if !is_container_dir_name(owner_name) {
                self.insert_app_phrase(owner_name);
            }
        }
    }

    fn register_toolchain_caches(&mut self, cli_stems: &HashSet<String>) {
        if cli_stems.contains("node") || cli_stems.contains("npm") {
            self.insert_compacts(&[
                "npm", "npmcache", "nodegyp", "nodejs", "viteplus", "turborepo", "biomejs", "vercel",
            ]);
        }
        if cli_stems.contains("pnpm") {
            self.insert_compacts(&["pnpm", "pnpmcache", "pnpmstate"]);
        }
        if cli_stems.contains("bun") {
            self.insert_compacts(&["bun"]);
        }
        if cli_stems.contains("python") || cli_stems.contains("uv") || cli_stems.contains("pip") {
            self.insert_compacts(&[
                "python", "pip", "pypa", "uv", "ipython", "matplotlib", "jupyter", "nltkdata",
                "jedi", "black", "imageio", "pypoetry",
            ]);
        }
        if cli_stems.contains("cargo") || cli_stems.contains("rustc") {
            self.insert_compacts(&["cargo", "rustup"]);
        }
        if cli_stems.contains("dotnet") {
            self.insert_compacts(&["dotnet", "nuget", "templateengine"]);
        }
        if cli_stems.contains("go") {
            self.insert_compacts(&["go"]);
        }
        if cli_stems.contains("code") {
            self.insert_compacts(&["code", "vscode", "vscodeshared"]);
        }
        if cli_stems.contains("claude") {
            self.insert_compacts(&["claude", "claude3p", "claudedata", "claudecode"]);
        }
        if cli_stems.contains("codex") || cli_stems.contains("ocx") {
            self.insert_compacts(&["codex", "openai", "opencodex"]);
        }
        if cli_stems.contains("agent-reach") {
            self.insert_compacts(&["agentreach", "agentreachvenv"]);
        }
    }

    fn insert_compacts(&mut self, names: &[&str]) {
        for &k in names {
            self.compact_names.insert(k.to_string());
        }
    }

    fn insert_app_phrase(&mut self, phrase: &str) {
        let c = norm_compact(phrase);
        if c.len() >= 3 && !is_generic_token(&c) {
            self.compact_names.insert(c);
        }
        let toks = norm_tokens(phrase);
        let non_generic: Vec<String> = toks
            .iter()
            .filter(|t| !is_generic_token(t))
            .cloned()
            .collect();

        // Only add to primary_brands if the app has a single clear primary brand token
        // (e.g. "OBS Studio" -> ["obs"], "Discord Inc" -> ["discord"], "Blender 5.2" -> ["blender"])
        if non_generic.len() == 1 && non_generic[0].len() >= 3 {
            self.primary_brands.insert(non_generic[0].clone());
        }
        // Also record multi-token set (excluding version numbers) for strict multi-token matching
        if !toks.is_empty() {
            let set: HashSet<String> = toks.into_iter().collect();
            self.app_token_sets.push(set);
        }
    }

    fn insert_cli_stem(&mut self, stem: &str) {
        let c = norm_compact(stem);
        if c.len() >= 2 && !is_generic_token(&c) {
            self.compact_names.insert(c.clone());
            // Standalone CLI binary like `rclone`, `qpdf`, `cloudflared`, `ngrok`, `gh`, `uv`, `pi`
            if c.len() >= 3 && !stem.contains('-') && !stem.contains('_') && !stem.contains(' ') {
                self.primary_brands.insert(c);
            }
        }
        let toks = norm_tokens(stem);
        if toks.len() >= 2 {
            self.app_token_sets.push(toks.into_iter().collect());
        }
    }

    fn insert_publisher(&mut self, pub_name: &str) {
        let c = norm_compact(pub_name);
        if c.len() >= 3 && !is_generic_token(&c) {
            self.publisher_compacts.insert(c);
        }
        if let Some(tok) = single_non_generic_token(pub_name, 4) {
            self.publisher_compacts.insert(tok);
        }
    }

    fn record_broken_key(&mut self, compact_key: &str, label: &str, detail: &str) {
        if compact_key.len() >= 3 && !is_generic_token(compact_key) {
            self.broken_signals
                .entry(compact_key.to_string())
                .or_insert_with(|| BrokenSignal {
                    label: label.to_string(),
                    detail: detail.to_string(),
                });
        }
    }

    fn record_broken_phrase(&mut self, phrase: &str, label: &str, detail: &str) {
        let c = norm_compact(phrase);
        self.record_broken_key(&c, label, detail);
        if let Some(tok) = single_non_generic_token(phrase, 3) {
            self.record_broken_key(&tok, label, detail);
        }
        if c.contains("ghub") || c.contains("lghub") {
            self.record_broken_key("ghub", "Logitech G HUB", detail);
            self.record_broken_key("lghub", "Logitech G HUB", detail);
        }
    }

    fn record_broken_path(&mut self, target_path: &str, label: &str, detail: &str) {
        let cleaned = target_path.trim_matches('"').trim();
        for comp in Path::new(cleaned).components() {
            let Component::Normal(os) = comp else {
                continue;
            };
            let Some(s) = os.to_str() else {
                continue;
            };
            let seg = s
                .strip_suffix(".exe")
                .or_else(|| s.strip_suffix(".EXE"))
                .unwrap_or(s);
            if is_generic_path_segment(seg) {
                continue;
            }
            let c = norm_compact(seg);
            if c.len() >= 3 && !is_generic_token(&c) {
                self.record_broken_key(&c, label, detail);
            }
        }
    }

    fn broken_signal_for(&self, folder_name: &str) -> Option<&BrokenSignal> {
        let compact = norm_compact(folder_name);
        if let Some(sig) = self.broken_signals.get(&compact) {
            return Some(sig);
        }
        for suffix in ["updater", "setup", "installer", "cache", "data", "log", "logs", "cli"] {
            if let Some(stem) = compact.strip_suffix(suffix) {
                if let Some(sig) = self.broken_signals.get(stem) {
                    return Some(sig);
                }
            }
        }
        for tok in norm_tokens(folder_name) {
            if !is_generic_token(&tok) {
                if let Some(sig) = self.broken_signals.get(&tok) {
                    return Some(sig);
                }
            }
        }
        None
    }

    fn is_under_active_install_path(&self, full_path: &str) -> bool {
        let lower = full_path.trim_end_matches('\\').to_ascii_lowercase();
        if self.active_install_paths_lower.contains(&lower) {
            return true;
        }
        let prefix = format!("{lower}\\");
        self.active_install_paths_lower
            .iter()
            .any(|p| p.starts_with(&prefix))
    }

    pub fn matches_installed(&self, folder_name: &str, full_path: &str) -> bool {
        if self.is_under_active_install_path(full_path) {
            return true;
        }

        let compact = norm_compact(folder_name);
        if compact.is_empty() {
            return true;
        }

        // If we have explicit positive broken-install evidence AND the exact folder is NOT in active installed compacts,
        // do NOT let a loose publisher or fuzzy token match exonerate it!
        if self.compact_names.contains(&compact) {
            return true;
        }

        // Exact publisher match (e.g. "Adobe", "Blender Foundation", "Logitech", "Bytedance")
        if self.publisher_compacts.contains(&compact) {
            return true;
        }

        // Reverse-DNS convention: `com.<vendor>.<app>` (e.g. `com.adobe.dunamis`, `com.logitech`, `com.vercel.cli`)
        if self.matches_reverse_dns(folder_name) {
            return true;
        }

        // Companion suffixes (`obs-studio-hook`, `openscreen-updater`, `antigravity-updater`, `claude-3p`, etc.)
        for suffix in [
            "updater", "setup", "cache", "data", "state", "log", "logs", "cli", "hook", "hooks",
            "3p", "shared", "venv", "nodejs",
        ] {
            if let Some(stem) = compact.strip_suffix(suffix) {
                if stem.len() >= 3
                    && !is_generic_token(stem)
                    && (self.compact_names.contains(stem) || self.primary_brands.contains(stem))
                {
                    return true;
                }
            }
        }

        // Token-based matching (STRICT: never match on generic words, and never treat empty tokens as installed!)
        let toks = norm_tokens(folder_name);
        if toks.is_empty() {
            return false;
        }
        let non_generic: Vec<&String> = toks.iter().filter(|t| !is_generic_token(t)).collect();
        if non_generic.is_empty() {
            return false;
        }

        // If folder has a single non-generic brand token (e.g. "obs-studio-hook" -> ["obs"], "BlenderMCP" -> ["blender"]),
        // match ONLY against primary_brands or exact compact_names of installed apps!
        if non_generic.len() == 1 {
            let brand = non_generic[0];
            if self.compact_names.contains(brand) || self.primary_brands.contains(brand) {
                return true;
            }
            return false;
        }

        // Multi-token folder: require ALL non-generic tokens (at least 2) to match a single installed app's token set
        for app_set in &self.app_token_sets {
            if non_generic.iter().all(|t| app_set.contains(*t)) {
                return true;
            }
        }

        false
    }

    fn is_known_brand(&self, compact: &str, include_publishers: bool) -> bool {
        self.compact_names.contains(compact)
            || self.primary_brands.contains(compact)
            || (include_publishers && self.publisher_compacts.contains(compact))
    }

    fn matches_reverse_dns(&self, folder_name: &str) -> bool {
        let lower_raw = folder_name.trim_start_matches('.').to_ascii_lowercase();
        let Some(rest) = lower_raw
            .strip_prefix("com.")
            .or_else(|| lower_raw.strip_prefix("org."))
            .or_else(|| lower_raw.strip_prefix("io."))
        else {
            return false;
        };
        if self.broken_signal_for(folder_name).is_some() {
            return false;
        }

        let parts: Vec<&str> = rest.split('.').filter(|p| !p.is_empty()).collect();
        let vendor_hit = parts
            .first()
            .is_some_and(|v| self.is_known_brand(&norm_compact(v), true));
        if vendor_hit {
            return true;
        }
        parts.get(1).is_some_and(|a| {
            let ac = norm_compact(a);
            !is_generic_token(&ac) && self.is_known_brand(&ac, false)
        })
    }
}

fn strip_cli_ext(lower: &str) -> Option<&str> {
    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(stem) = lower.strip_suffix(ext) {
            return Some(stem);
        }
    }
    None
}

fn single_non_generic_token(phrase: &str, min_len: usize) -> Option<String> {
    let toks = norm_tokens(phrase);
    let mut iter = toks.into_iter().filter(|t| !is_generic_token(t));
    let first = iter.next()?;
    if iter.next().is_some() || first.len() < min_len {
        return None;
    }
    Some(first)
}

fn is_generic_path_segment(seg: &str) -> bool {
    is_container_dir_name(seg)
        || matches!(
            seg.to_ascii_lowercase().as_str(),
            "bin" | "apps" | "release" | "win-unpacked" | "system_tray" | "uninstall" | "desktop"
        )
}

fn is_container_dir_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "program files"
            | "program files (x86)"
            | "programdata"
            | "programs"
            | "local"
            | "locallow"
            | "roaming"
            | "appdata"
            | "users"
            | "common files"
            | "packages"
            | "microsoft"
            | "windowsapps"
            | "winget"
            | "links"
            | "start menu"
            | "startup"
            | "administrative tools"
            | "system tools"
            | "accessories"
            | "accessibility"
            | "maintenance"
    )
}

#[derive(Debug, PartialEq, Eq)]
enum LnkStatus {
    Alive(String),
    Broken(String),
    Unknown,
}

fn normalize_path_buf(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Inspects a Windows Shell Link (.lnk, MS-SHLLINK format) across LinkInfo, RelativePath, WorkingDir,
/// and EnvironmentVariableDataBlock (0xA0000001) to resolve its local target path and check existence.
fn inspect_lnk_target(lnk_path: &Path) -> LnkStatus {
    let Ok(bytes) = std::fs::read(lnk_path) else {
        return LnkStatus::Unknown;
    };
    if bytes.len() < 76 || bytes[0..4] != [0x4C, 0x00, 0x00, 0x00] {
        return LnkStatus::Unknown;
    }
    let link_flags = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let is_unicode = (link_flags & 0x80) != 0;
    let mut offset = 76usize;

    // 1. Optional LinkTargetIDList
    if (link_flags & 0x01) != 0 {
        if offset + 2 > bytes.len() {
            return LnkStatus::Unknown;
        }
        let id_list_size = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset = offset.saturating_add(2 + id_list_size);
    }

    // 2. Optional LinkInfo
    let mut extracted_target: Option<String> = None;
    if (link_flags & 0x02) != 0 && offset + 28 <= bytes.len() {
        let link_info_size = read_u32_le(&bytes, offset) as usize;
        extracted_target = parse_link_info_target(&bytes, offset);
        offset = offset.saturating_add(link_info_size);
    }

    // 3. StringData (Name=0x04, RelativePath=0x08, WorkingDir=0x10, Arguments=0x20, IconLocation=0x40)
    let mut rel_path: Option<String> = None;
    let mut work_dir: Option<String> = None;
    for bit in [0x04u32, 0x08, 0x10, 0x20, 0x40] {
        if (link_flags & bit) != 0 {
            if offset + 2 > bytes.len() {
                break;
            }
            let cc = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
            offset += 2;
            let byte_len = if is_unicode { cc * 2 } else { cc };
            if offset + byte_len > bytes.len() {
                break;
            }
            let s = if is_unicode {
                let u16s: Vec<u16> = bytes[offset..offset + byte_len]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&u16s)
            } else {
                String::from_utf8_lossy(&bytes[offset..offset + byte_len]).into_owned()
            };
            offset += byte_len;
            if bit == 0x08 && !s.is_empty() {
                rel_path = Some(s);
            } else if bit == 0x10 && !s.is_empty() {
                work_dir = Some(s);
            }
        }
    }

    // 4. ExtraData blocks (EnvironmentVariableDataBlock sig = 0xA0000001)
    let mut env_target: Option<String> = None;
    while offset + 8 <= bytes.len() {
        let block_size = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        if block_size < 4 || offset + block_size > bytes.len() {
            break;
        }
        let sig = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        if sig == 0xA000_0001 && block_size >= 788 {
            let uni = read_null_utf16(&bytes[offset + 268..offset + 788]);
            let chosen = if !uni.is_empty() {
                uni
            } else {
                read_null_ansi(&bytes[offset + 8..offset + 268])
            };
            if !chosen.is_empty() {
                env_target = Some(expand_env_vars(&chosen));
            }
        }
        offset += block_size;
    }

    // Pick best resolved local path candidate
    let candidate = extracted_target
        .or(env_target)
        .or_else(|| {
            rel_path.and_then(|rp| {
                let parent = lnk_path.parent()?;
                let joined = normalize_path_buf(&parent.join(rp));
                Some(joined.to_string_lossy().into_owned())
            })
        })
        .or(work_dir);

    if let Some(raw_target) = candidate {
        let target = expand_env_vars(raw_target.trim());
        if target.len() > 3 && target.contains(":\\") {
            return if Path::new(&target).exists() {
                LnkStatus::Alive(target)
            } else {
                LnkStatus::Broken(target)
            };
        }
    }
    LnkStatus::Unknown
}

fn read_u32_le(bytes: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

fn parse_link_info_target(bytes: &[u8], offset: usize) -> Option<String> {
    let info_header_size = read_u32_le(bytes, offset + 4) as usize;
    let info_flags = read_u32_le(bytes, offset + 8);
    if (info_flags & 0x01) == 0 {
        return None;
    }

    if info_header_size >= 0x24 && offset + 32 <= bytes.len() {
        let uni_off = read_u32_le(bytes, offset + 28) as usize;
        if uni_off > 0 && offset + uni_off + 2 <= bytes.len() {
            let s = read_null_utf16(&bytes[offset + uni_off..]);
            if !s.is_empty() {
                return Some(s);
            }
        }
    }

    let ansi_off = read_u32_le(bytes, offset + 16) as usize;
    if ansi_off > 0 && offset + ansi_off < bytes.len() {
        let s = read_null_ansi(&bytes[offset + ansi_off..]);
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn read_null_utf16(slice: &[u8]) -> String {
    let mut u16s = Vec::new();
    let mut p = 0;
    while p + 1 < slice.len() {
        let ch = u16::from_le_bytes([slice[p], slice[p + 1]]);
        if ch == 0 {
            break;
        }
        u16s.push(ch);
        p += 2;
    }
    String::from_utf16_lossy(&u16s)
}

fn read_null_ansi(slice: &[u8]) -> String {
    let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..len]).into_owned()
}

fn process_start_menu_lnk(p: &Path, idx: &mut InstalledIndex) -> bool {
    let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let lower = stem.to_ascii_lowercase();
    if lower.starts_with("uninstall ") || lower == "uninstall" {
        return false;
    }
    match inspect_lnk_target(p) {
        LnkStatus::Broken(target_path) => {
            let detail = "Broken Start Menu shortcut";
            idx.record_broken_phrase(stem, stem, detail);
            idx.record_broken_path(&target_path, stem, detail);
            false
        }
        LnkStatus::Alive(target_path) => {
            let tp = Path::new(&target_path);
            if !is_windows_os_dir(tp) {
                idx.insert_app_phrase(stem);
                idx.register_exe_parent_dir(tp);
            }
            true
        }
        LnkStatus::Unknown => false,
    }
}

fn collect_start_menu_names(dir: &Path, depth: usize, idx: &mut InstalledIndex) -> bool {
    if depth > 3 {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut has_valid_shortcut = false;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if !collect_start_menu_names(&p, depth + 1, idx) {
                continue;
            }
            has_valid_shortcut = true;
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                if !is_container_dir_name(name) {
                    idx.insert_app_phrase(name);
                }
            }
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            has_valid_shortcut |= process_start_menu_lnk(&p, idx);
        }
    }
    has_valid_shortcut
}

/// Core Windows OS, hardware driver, and shared runtime buckets (never app-specific personal whitelists).
fn is_system_or_shared_bucket(name: &str) -> bool {
    let lower = name.trim_start_matches('.').to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "microsoft"
            | "microsoftwindows"
            | "microsoft office"
            | "microsoft office 15"
            | "microsoft onedrive"
            | "microsoft devdiv"
            | "microsoft sdks"
            | "microsoft visual studio"
            | "microsoft.net"
            | "microsoft update health tools"
            | "windows"
            | "windowspowershell"
            | "windows defender"
            | "windows defender advanced threat protection"
            | "windows nt"
            | "windows mail"
            | "windows media player"
            | "windows photo viewer"
            | "windows sidebar"
            | "windows kits"
            | "windowsapps"
            | "windows app certification kit"
            | "windowsholographicdevices"
            | "windowsperformancerecorder"
            | "packages"
            | "programs"
            | "package cache"
            | "packagemanagement"
            | "publishers"
            | "connecteddevicesplatform"
            | "comms"
            | "crashdumps"
            | "d3dscache"
            | "temp"
            | "temporary internet files"
            | "application data"
            | "history"
            | "backup"
            | "cache"
            | "config"
            | "local"
            | "ssh"
            | "thumbnails"
            | "xdg.config"
            | "xdg.data"
            | "fontconfig"
            | "squirreltemp"
            | "virtualstore"
            | "peerdistrepub"
            | "placeholdertilelogofolder"
            | "toastnotificationmanagercompat"
            | "cef"
            | "electron"
            | "sentry"
            | "disklens"
            | "intel"
            | "neo"
            | "nvidia"
            | "nvidia corporation"
            | "amd"
            | "realtek"
            | "dolby"
            | "asus"
            | "asusproart"
            | "screenpadquickkeys"
            | "common files"
            | "modifiablewindowsapps"
            | "reference assemblies"
            | "msbuild"
            | "internet explorer"
            | "difx"
            | "hyper-v"
            | "wsl"
            | "onedrive"
            | "usbhub"
            | "regid.1991-06.com.microsoft"
            | "softwaredistribution"
            | "software distribution"
            | "usoprivate"
            | "usoshared"
            | "whesvc"
            | "uninstall information"
            | "start menu"
            | "desktop"
            | "documents"
            | "templates"
            | "speech"
            | "common"
            | "agents"
            | "gemini"
    )
}

fn filetime_to_days_ago(ft: i64) -> Option<u64> {
    if ft <= 0 {
        return None;
    }
    let secs_since_1601 = (ft as u64) / 10_000_000;
    let unix_secs = secs_since_1601.checked_sub(11_644_473_600)?;
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(now_secs.saturating_sub(unix_secs) / 86_400)
}

pub fn format_idle_age(days: u64) -> String {
    if days >= 365 {
        let years = (days as f64) / 365.25;
        if years >= 2.0 {
            format!("Untouched {:.1}y", years)
        } else {
            format!("Untouched {:.1}y", years)
        }
    } else if days >= 45 {
        let months = (days + 15) / 30;
        format!("Untouched {months}mo")
    } else if days == 0 {
        "Modified today".to_string()
    } else if days == 1 {
        "Untouched 1d".to_string()
    } else {
        format!("Untouched {days}d")
    }
}

/// Derives a canonical `(app_key, app_label)` for grouping scattered fragments of the same app
/// across `Program Files`, `ProgramData`, `AppData\Local`, `AppData\Roaming`, and `Users\<user>\.*`.
fn derive_app_group(
    folder_name: &str,
    scope_vendor: Option<&str>,
    broken_sig: Option<&BrokenSignal>,
) -> (String, String) {
    let compact = norm_compact(folder_name);

    // Explicit well-known multi-name aliases
    if compact == "lghub" || compact == "ghub" || compact == "logitechghub" {
        return ("lghub".to_string(), "Logitech G HUB".to_string());
    }
    if compact == "drivefs" || compact == "drivefilestream" {
        return ("googledrive".to_string(), "Google Drive".to_string());
    }
    if compact.contains("hermes") {
        return ("hermes".to_string(), "Hermes".to_string());
    }
    if compact == "dockerdesktop" || compact == "dockersecretsengine" || compact == "docker" {
        return ("docker".to_string(), "Docker".to_string());
    }
    if compact == "apple" || compact == "applecomputer" {
        return ("apple".to_string(), "Apple".to_string());
    }
    if compact == "avastsoftware" || compact == "avast" {
        return ("avast".to_string(), "Avast".to_string());
    }
    if compact == "teamsmeetingaddin" || compact == "teamsmeetingaddinmsis" || compact == "teams" {
        return ("teams".to_string(), "Microsoft Teams".to_string());
    }

    if let Some(sig) = broken_sig {
        let key = norm_compact(&sig.label);
        if key.len() >= 3 {
            return (key, sig.label.clone());
        }
    }

    // Strip reverse-DNS prefixes (`com.foo.bar.setup` -> `bar`)
    let lower_raw = folder_name.trim_start_matches('.').to_ascii_lowercase();
    if let Some(rest) = lower_raw
        .strip_prefix("com.")
        .or_else(|| lower_raw.strip_prefix("org."))
        .or_else(|| lower_raw.strip_prefix("io."))
    {
        let parts: Vec<&str> = rest
            .split('.')
            .filter(|p| !p.is_empty() && !STOP_WORDS.contains(p))
            .collect();
        if let Some(&last) = parts.last() {
            let key = norm_compact(last);
            if key.len() >= 3 {
                return (key, prettify_label(last));
            }
        }
    }

    // Strip companion suffixes for grouping (`foo-updater` + `foo` -> `foo`)
    for suffix in ["updater", "setup", "installer", "cache", "data", "state", "logs", "log"] {
        if let Some(stem) = compact.strip_suffix(suffix) {
            if stem.len() >= 3 && !is_generic_token(stem) {
                return (stem.to_string(), prettify_label(stem));
            }
        }
    }

    let label = if let Some(v) = scope_vendor {
        format!("{v} {folder_name}")
    } else {
        folder_name.trim_start_matches('.').to_string()
    };
    let key = if let Some(v) = scope_vendor {
        norm_compact(&format!("{v}{folder_name}"))
    } else {
        compact
    };
    (key, label)
}

fn prettify_label(raw: &str) -> String {
    let mut chars = raw.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
    }
}

// --- Persistent Ignored / Dismissed Leftovers ---

#[derive(Default, Serialize, Deserialize)]
struct IgnoredFile {
    paths: Vec<String>,
}

fn ignored_file_path() -> PathBuf {
    cache::data_dir().join("ignored_orphans.json")
}

pub fn load_ignored_paths() -> HashSet<String> {
    let path = ignored_file_path();
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    let Ok(parsed) = serde_json::from_str::<IgnoredFile>(&text) else {
        return HashSet::new();
    };
    parsed
        .paths
        .into_iter()
        .map(|p| p.trim_end_matches('\\').to_ascii_lowercase())
        .collect()
}

pub fn add_ignored_paths(paths: &[String]) -> Result<usize, String> {
    let mut set = load_ignored_paths();
    for p in paths {
        let norm = p.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase();
        if !norm.is_empty() {
            set.insert(norm);
        }
    }
    save_ignored_set(&set)?;
    Ok(set.len())
}

pub fn clear_ignored_paths() -> Result<(), String> {
    save_ignored_set(&HashSet::new())
}

fn save_ignored_set(set: &HashSet<String>) -> Result<(), String> {
    let mut list: Vec<String> = set.iter().cloned().collect();
    list.sort();
    let payload = IgnoredFile { paths: list };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed serializing ignored list: {e}"))?;
    std::fs::write(ignored_file_path(), json)
        .map_err(|e| format!("Failed writing ignored list: {e}"))
}

// --- Orphan Report & Detection ---

pub struct OrphanReport {
    pub by_path_lower: HashMap<String, OrphanMatch>,
    pub sorted_list: Vec<OrphanMatch>,
    pub groups: Vec<OrphanGroup>,
    pub non_tiny_count: usize,
    pub non_tiny_bytes: u64,
    pub tiny_count: usize,
    pub ignored_count: usize,
}

pub fn detect_orphans(tree: &ScanTree) -> OrphanReport {
    let idx = InstalledIndex::gather();
    let ignored = load_ignored_paths();
    let mut matches: Vec<OrphanMatch> = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();

    let root = tree.root_path.trim_end_matches('\\');
    let mut scopes: Vec<(String, &'static str, bool, Option<&'static str>)> = Vec::new();
    scopes.push((format!(r"{root}\ProgramData"), "ProgramData", false, None));
    scopes.push((format!(r"{root}\Program Files"), "Program Files", false, None));
    scopes.push((
        format!(r"{root}\Program Files (x86)"),
        "Program Files (x86)",
        false,
        None,
    ));
    // Depth-2 vendor install shells in Program Files
    scopes.push((
        format!(r"{root}\Program Files\Google"),
        "Program Files\\Google",
        false,
        Some("Google"),
    ));
    scopes.push((
        format!(r"{root}\Program Files (x86)\Google"),
        "Program Files (x86)\\Google",
        false,
        Some("Google"),
    ));

    if let Some(users_id) = tree.find_node_by_path(&format!(r"{root}\Users")) {
        for &uid in &tree.nodes[users_id as usize].children {
            let uname = &tree.nodes[uid as usize].name;
            if uname.eq_ignore_ascii_case("Public")
                || uname.eq_ignore_ascii_case("Default")
                || uname.eq_ignore_ascii_case("Default User")
                || uname.eq_ignore_ascii_case("All Users")
            {
                continue;
            }
            let ubase = format!(r"{root}\Users\{uname}");
            scopes.push((format!(r"{ubase}\AppData\Local"), "AppData\\Local", false, None));
            scopes.push((
                format!(r"{ubase}\AppData\Local\Programs"),
                "AppData\\Local\\Programs",
                false,
                None,
            ));
            scopes.push((
                format!(r"{ubase}\AppData\LocalLow"),
                "AppData\\LocalLow",
                false,
                None,
            ));
            scopes.push((
                format!(r"{ubase}\AppData\Roaming"),
                "AppData\\Roaming",
                false,
                None,
            ));
            scopes.push((
                format!(r"{ubase}\AppData\Local\Packages"),
                "Store Packages",
                true,
                None,
            ));
            // Depth-2 vendor scopes for multi-product vendors
            scopes.push((
                format!(r"{ubase}\AppData\Local\Google"),
                "AppData\\Local\\Google",
                false,
                Some("Google"),
            ));

            collect_user_special_candidates(
                tree,
                &idx,
                &ignored,
                &ubase,
                &mut seen_paths,
                &mut matches,
            );
        }
    }

    for (scope_path, group_label, is_appx_scope, vendor_ctx) in scopes {
        let Some(scope_id) = tree.find_node_by_path(&scope_path) else {
            continue;
        };
        for &cid in &tree.nodes[scope_id as usize].children {
            evaluate_candidate(
                tree,
                &idx,
                &ignored,
                cid,
                group_label,
                is_appx_scope,
                vendor_ctx,
                &mut seen_paths,
                &mut matches,
            );
        }
    }

    matches.sort_unstable_by(|a, b| {
        a.confidence
            .rank()
            .cmp(&b.confidence.rank())
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.path.cmp(&b.path))
    });

    // Build app-level groups
    let mut group_map: HashMap<String, Vec<OrphanMatch>> = HashMap::new();
    for m in &matches {
        group_map
            .entry(m.app_key.clone())
            .or_default()
            .push(m.clone());
    }

    let mut groups: Vec<OrphanGroup> = Vec::with_capacity(group_map.len());
    for (app_key, mut items) in group_map {
        items.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
        let app_label = items[0].app_label.clone();
        let best_conf = items
            .iter()
            .map(|i| i.confidence)
            .min_by_key(|c| c.rank())
            .unwrap_or(Confidence::Review);
        let total_size: u64 = items.iter().map(|i| i.size).sum();
        let non_tiny_size: u64 = items
            .iter()
            .filter(|i| !i.is_tiny)
            .map(|i| i.size)
            .sum();
        let has_non_tiny = items.iter().any(|i| !i.is_tiny);
        let max_days_idle = items.iter().filter_map(|i| i.days_idle).max();
        let min_days_idle = items.iter().filter_map(|i| i.days_idle).min();

        // Merge distinct evidence badges across fragments (keeping top 3 most informative)
        let mut merged_ev: Vec<String> = Vec::new();
        for it in &items {
            for ev in &it.evidence {
                if !ev.starts_with("Untouched ")
                    && !ev.starts_with("Modified ")
                    && !merged_ev.contains(ev)
                {
                    merged_ev.push(ev.clone());
                }
            }
        }
        if let Some(d) = min_days_idle {
            merged_ev.push(format_idle_age(d));
        }
        merged_ev.truncate(3);
        let summary_reason = merged_ev.join(" · ");

        groups.push(OrphanGroup {
            app_key,
            app_label,
            confidence: best_conf,
            total_size,
            non_tiny_size,
            has_non_tiny,
            max_days_idle,
            min_days_idle,
            summary_reason,
            evidence: merged_ev,
            items,
        });
    }

    groups.sort_unstable_by(|a, b| {
        b.non_tiny_size
            .cmp(&a.non_tiny_size)
            .then_with(|| a.confidence.rank().cmp(&b.confidence.rank()))
            .then_with(|| b.total_size.cmp(&a.total_size))
            .then_with(|| a.app_label.cmp(&b.app_label))
    });

    let mut non_tiny_bytes = 0u64;
    let mut non_tiny_count = 0usize;
    let mut tiny_count = 0usize;
    let mut by_path_lower = HashMap::with_capacity(matches.len());

    for m in &matches {
        if m.is_tiny {
            tiny_count += 1;
        } else {
            non_tiny_count += 1;
            non_tiny_bytes = non_tiny_bytes.saturating_add(m.size);
        }
        by_path_lower.insert(m.path.to_ascii_lowercase(), m.clone());
    }

    OrphanReport {
        by_path_lower,
        sorted_list: matches,
        groups,
        non_tiny_count,
        non_tiny_bytes,
        tiny_count,
        ignored_count: ignored.len(),
    }
}

fn collect_user_special_candidates(
    tree: &ScanTree,
    idx: &InstalledIndex,
    ignored: &HashSet<String>,
    ubase: &str,
    seen_paths: &mut HashSet<String>,
    matches: &mut Vec<OrphanMatch>,
) {
    let ms_dirs = [
        (format!(r"{ubase}\AppData\Local\Microsoft"), "AppData\\Local\\Microsoft"),
        (format!(r"{ubase}\AppData\Roaming\Microsoft"), "AppData\\Roaming\\Microsoft"),
    ];
    for (ms_sub, ms_label) in ms_dirs {
        let Some(ms_id) = tree.find_node_by_path(&ms_sub) else {
            continue;
        };
        for &cid in &tree.nodes[ms_id as usize].children {
            let cname = tree.nodes[cid as usize].name.to_ascii_lowercase();
            if !is_removable_ms_subapp(&cname) {
                continue;
            }
            evaluate_candidate(
                tree,
                idx,
                ignored,
                cid,
                ms_label,
                false,
                Some("Microsoft"),
                seen_paths,
                matches,
            );
        }
    }

    let Some(u_node_id) = tree.find_node_by_path(ubase) else {
        return;
    };
    for &cid in &tree.nodes[u_node_id as usize].children {
        if !tree.nodes[cid as usize].name.starts_with('.') {
            continue;
        }
        evaluate_candidate(
            tree,
            idx,
            ignored,
            cid,
            "User Profile",
            false,
            None,
            seen_paths,
            matches,
        );
    }
}

fn is_removable_ms_subapp(lower_name: &str) -> bool {
    matches!(
        lower_name,
        "teams"
            | "microsoft teams"
            | "teamsmeetingadd-in"
            | "teamsmeetingaddinmsis"
            | "skype"
            | "skype for desktop"
            | "powertoys"
            | "bingwallpaper"
            | "pcmanager"
            | "devhome"
    )
}

fn is_google_core_subapp(lower_name: &str) -> bool {
    matches!(lower_name, "googleupdater" | "update" | "crashreports" | "software reporter tool")
}

#[allow(clippy::too_many_arguments)]
fn evaluate_candidate(
    tree: &ScanTree,
    idx: &InstalledIndex,
    ignored: &HashSet<String>,
    cid: u32,
    group_label: &'static str,
    is_appx_scope: bool,
    vendor_ctx: Option<&'static str>,
    seen_paths: &mut HashSet<String>,
    out: &mut Vec<OrphanMatch>,
) {
    let cnode = &tree.nodes[cid as usize];
    if cnode.access_denied {
        return;
    }
    let name = cnode.name.as_ref();
    if vendor_ctx.is_none() && is_system_or_shared_bucket(name) {
        return;
    }
    if vendor_ctx == Some("Google") && is_google_core_subapp(&name.to_ascii_lowercase()) {
        return;
    }

    let Some(full_path) = tree.node_path(cid) else {
        return;
    };
    let lower_path = full_path.trim_end_matches('\\').to_ascii_lowercase();
    if ignored.contains(&lower_path) || !seen_paths.insert(lower_path.clone()) {
        return;
    }

    let days_idle = filetime_to_days_ago(cnode.last_write_time);
    let is_tiny = cnode.total_size < TINY_THRESHOLD_BYTES;
    let lower_name = name.to_ascii_lowercase();

    // Handle Store Packages (`AppData\Local\Packages`)
    if is_appx_scope {
        if lower_name.starts_with("microsoft.")
            || lower_name.starts_with("microsoftwindows.")
            || lower_name.starts_with("windows.")
            || lower_name.starts_with("cr.sb.")
        {
            return;
        }
        if idx.appx_families_lower.contains(&lower_name) {
            return;
        }
        let mut evidence = vec!["Store package no longer installed".to_string()];
        if let Some(d) = days_idle {
            evidence.push(format_idle_age(d));
        }
        let reason = evidence.join(" · ");
        let app_base = name.split('_').next().unwrap_or(name);
        let (app_key, app_label) = derive_app_group(app_base, None, None);
        out.push(OrphanMatch {
            node_id: cid,
            app_key,
            app_label,
            name: name.to_string(),
            path: full_path,
            location_group: group_label,
            size: cnode.total_size,
            is_tiny,
            days_idle,
            confidence: Confidence::Confirmed,
            evidence,
            reason,
        });
        return;
    }

    let broken_sig = idx.broken_signal_for(name);
    let path_obj = Path::new(&full_path);
    let is_program_scope = group_label.starts_with("Program Files")
        || group_label == "AppData\\Local\\Programs";
    let is_user_dotfolder = group_label == "User Profile";

    // Positive disk-level remnant signals inside Program Files / Programs
    let has_delete_on_reboot = is_program_scope && dir_has_delete_on_reboot(path_obj);
    let is_empty_program_shell =
        is_program_scope && (cnode.total_file_count == 0 || dir_is_empty_or_delete_on_reboot(path_obj));

    // Check if folder is matched by a verified installed app
    // (For vendor sub-apps like Google\DriveFS, match the sub-app name itself, NOT the parent vendor "Google"!)
    let matched_installed = if let Some(vendor) = vendor_ctx {
        idx.matches_installed(name, &full_path)
            || idx.matches_installed(&format!("{vendor} {name}"), &full_path)
    } else {
        idx.matches_installed(name, &full_path)
    };

    if matched_installed && broken_sig.is_none() && !has_delete_on_reboot {
        return;
    }

    // Node.js package cache convention (`*-nodejs`) when Node.js is installed:
    // Skip small or recent (< 90d) caches; surface large stale ones as Review.
    let is_nodejs_cache = lower_name.ends_with("-nodejs") && idx.compact_names.contains("nodejs");
    if is_nodejs_cache && (is_tiny || days_idle.is_some_and(|d| d < 90)) {
        return;
    }

    // Special handling for Program Files / AppData\Local\Programs:
    // If it's NOT matched by Registry/Services/StartMenu, check whether it's a live portable app
    // (has a main exe matching the folder name and no broken-app signal) vs a dead remnant!
    let mut exe_note: Option<&'static str> = None;
    if is_program_scope {
        if broken_sig.is_none() && !has_delete_on_reboot && !is_empty_program_shell {
            if dir_has_main_app_exe(path_obj, name, 3) {
                // Looks like a live portable/unregistered install (e.g. "PDF Metadata Cleaner.exe")
                return;
            }
            if cnode.has_exe {
                if dir_contains_any_non_stub_exe_disk(path_obj, 3, lower_name.contains("python")) {
                    // Contains some non-stub exe that doesn't match the folder name
                    if days_idle.is_some_and(|d| d < 30) {
                        return;
                    }
                    exe_note = Some("Unregistered install dir (only helper/bundled binaries)");
                } else {
                    // Only leftover stub/bundled runtime exes (e.g. LibreOffice's bundled python.exe)
                    exe_note = Some("No main app executable left in install folder");
                }
            }
        }
    } else if !is_user_dotfolder && cnode.has_exe {
        // Non-program scopes (AppData, ProgramData) that contain an `.exe`:
        // Skip live Squirrel/Electron apps unless we have broken-app proof or only stub exes or it's ancient embedded runtime!
        let has_real_exe =
            dir_contains_any_non_stub_exe_disk(path_obj, 4, lower_name.contains("python"));
        if has_real_exe && broken_sig.is_none() {
            if lower_name.ends_with("-updater")
                || lower_name.contains("setup")
                || lower_name.contains("installer")
            {
                exe_note = Some("Leftover updater/installer");
            } else if days_idle.is_some_and(|d| d >= 180) {
                // Embedded runtime untouched for 6+ months with no installed parent app (e.g. JxBrowser 368 MB!)
                exe_note = Some("Contains embedded executable");
            } else {
                // Recent AppData folder containing real executables -> likely an unregistered portable/Electron app
                return;
            }
        }
    }

    // Protect actively written folders (< 2 days) UNLESS there is positive uninstall proof
    let has_positive_proof =
        broken_sig.is_some() || has_delete_on_reboot || is_empty_program_shell;
    if !has_positive_proof && days_idle.is_some_and(|d| d < 2) {
        return;
    }

    // Determine confidence & human-readable evidence chips
    let mut evidence: Vec<String> = Vec::with_capacity(3);
    let confidence = if let Some(sig) = broken_sig {
        evidence.push(sig.detail.clone());
        Confidence::Confirmed
    } else if has_delete_on_reboot {
        evidence.push("Uninstall remnant (deleteonreboot)".to_string());
        Confidence::Confirmed
    } else if is_empty_program_shell {
        evidence.push("Empty install folder".to_string());
        Confidence::Confirmed
    } else if is_program_scope && !cnode.has_exe {
        evidence.push("No executables left in install folder".to_string());
        Confidence::Confirmed
    } else if is_nodejs_cache {
        evidence.push("Stale Node.js package cache".to_string());
        Confidence::Review
    } else if let Some(note) = exe_note {
        evidence.push(note.to_string());
        if note.starts_with("No main app executable") {
            Confidence::Confirmed
        } else if days_idle.is_some_and(|d| d >= 180) {
            Confidence::Likely
        } else {
            Confidence::Review
        }
    } else if days_idle.is_some_and(|d| d < 14) {
        evidence.push("No matching installed app".to_string());
        evidence.push("Recently modified".to_string());
        Confidence::Review
    } else {
        evidence.push("No matching installed app".to_string());
        Confidence::Likely
    };

    if let Some(d) = days_idle {
        evidence.push(format_idle_age(d));
    } else if cnode.total_file_count == 0 {
        evidence.push("Empty folder".to_string());
    }

    let reason = evidence.join(" · ");
    let (app_key, app_label) = derive_app_group(name, vendor_ctx, broken_sig);

    out.push(OrphanMatch {
        node_id: cid,
        app_key,
        app_label,
        name: name.to_string(),
        path: full_path,
        location_group: group_label,
        size: cnode.total_size,
        is_tiny,
        days_idle,
        confidence,
        evidence,
        reason,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_tokens_preserves_acronyms_and_camelcase() {
        assert_eq!(norm_tokens("OBS Studio"), vec!["obs"]);
        assert_eq!(norm_tokens("LGHUB"), vec!["lghub"]);
        assert_eq!(norm_tokens("G HUB"), vec!["hub"]);
        assert_eq!(norm_tokens("TUG"), vec!["tug"]);
        assert_eq!(norm_tokens("NEO"), vec!["neo"]);
        assert_eq!(norm_tokens("BraveSoftware"), vec!["brave"]);
        assert_eq!(norm_tokens("PDFViewer"), vec!["pdf", "viewer"]);
        assert_eq!(norm_tokens("obs-studio-hook"), vec!["obs", "hook"]);
        assert_eq!(norm_tokens("BrowserLock"), vec!["browser", "lock"]);
        assert_eq!(
            norm_tokens(".chromium-browser-snapshots"),
            vec!["chromium", "browser", "snapshots"]
        );
    }

    #[test]
    fn matching_blocks_generic_word_exonerations_and_keeps_real_companions() {
        let mut idx = InstalledIndex {
            compact_names: HashSet::new(),
            primary_brands: HashSet::new(),
            app_token_sets: Vec::new(),
            publisher_compacts: HashSet::new(),
            appx_families_lower: HashSet::new(),
            active_install_paths_lower: HashSet::new(),
            broken_signals: HashMap::new(),
        };
        idx.insert_app_phrase("OBS Studio");
        idx.insert_app_phrase("Microsoft Visual Studio Code");
        idx.compact_names.insert("code".to_string());
        idx.insert_app_phrase("Openscreen 1.4.0");
        idx.insert_publisher("Adobe");

        // Real companion folders SHOULD match
        assert!(idx.matches_installed("obs-studio-hook", r"C:\ProgramData\obs-studio-hook"));
        assert!(idx.matches_installed("openscreen-updater", r"C:\Users\u\AppData\Local\openscreen-updater"));
        assert!(idx.matches_installed("com.adobe.dunamis", r"C:\Users\u\AppData\Roaming\com.adobe.dunamis"));

        // Generic word collisions MUST NOT match
        assert!(!idx.matches_installed(".kimi-code", r"C:\Users\u\.kimi-code"));
        assert!(!idx.matches_installed("BrowserLock", r"C:\Users\u\AppData\Local\BrowserLock"));
        assert!(!idx.matches_installed(
            ".chromium-browser-snapshots",
            r"C:\Users\u\.chromium-browser-snapshots"
        ));
        assert!(!idx.matches_installed("TUG", r"C:\Users\u\AppData\Roaming\TUG"));
        assert!(!idx.matches_installed("G HUB", r"C:\Users\u\AppData\Roaming\G HUB"));
    }
}
