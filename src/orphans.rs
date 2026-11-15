use crate::scanner::ScanTree;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::time::{SystemTime, UNIX_EPOCH};

type HKey = *mut c_void;

const HKEY_CLASSES_ROOT_USIZE: usize = 0x8000_0000;
const HKEY_CURRENT_USER_USIZE: usize = 0x8000_0001;
const HKEY_LOCAL_MACHINE_USIZE: usize = 0x8000_0002;
const KEY_READ: u32 = 0x20019;
const KEY_WOW64_64KEY: u32 = 0x0100;
const KEY_WOW64_32KEY: u32 = 0x0200;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;

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
    if s.is_empty() { None } else { Some(s) }
}

/// Normalizes a name to lowercase alphanumeric characters only (e.g. "LM-Studio" -> "lmstudio").
fn norm_compact(s: &str) -> String {
    let stripped = s.trim_start_matches('.');
    stripped
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Splits a name into meaningful lowercase alphanumeric words, dropping generic noise words.
fn norm_tokens(s: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "inc", "llc", "ltd", "corp", "corporation", "company", "co", "gmbh", "bv", "pte",
        "software", "technologies", "technology", "systems", "studio", "desktop", "client",
        "app", "apps", "application", "launcher", "installer", "setup", "updater", "update",
        "service", "helper", "agent", "cli", "sdk", "runtime", "tools", "tool", "x64", "x86",
        "arm64", "win64", "win32", "windows", "current", "user", "edition", "community", "pro",
        "free", "local", "roaming", "data", "com", "org", "net", "io", "dev", "the", "for",
    ];
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            // Split camelCase boundaries like "BraveSoftware" -> "brave", "software"
            if ch.is_ascii_uppercase()
                && !cur.is_empty()
                && cur.chars().last().is_some_and(|p| p.is_ascii_lowercase())
            {
                push_token(&mut out, &cur, STOP);
                cur.clear();
            }
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            push_token(&mut out, &cur, STOP);
            cur.clear();
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, &cur, STOP);
    }
    out
}

fn push_token(out: &mut Vec<String>, tok: &str, stop: &[&str]) {
    if tok.len() >= 3 && !tok.chars().all(|c| c.is_ascii_digit()) && !stop.contains(&tok) {
        out.push(tok.to_string());
    }
}

fn extract_exe_path(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim();
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

#[derive(Clone, Debug, Serialize)]
pub struct OrphanMatch {
    pub node_id: u32,
    pub name: String,
    pub path: String,
    pub location_group: &'static str,
    pub size: u64,
    pub days_idle: Option<u64>,
    pub reason: String,
}

pub struct InstalledIndex {
    compact_names: HashSet<String>,
    brand_tokens: HashSet<String>,
    appx_families_lower: HashSet<String>,
    broken_shortcuts: HashSet<String>,
}

impl InstalledIndex {
    pub fn gather() -> Self {
        let _ = HKEY_CLASSES_ROOT_USIZE;
        let mut idx = Self {
            compact_names: HashSet::with_capacity(2048),
            brand_tokens: HashSet::with_capacity(2048),
            appx_families_lower: HashSet::with_capacity(512),
            broken_shortcuts: HashSet::with_capacity(64),
        };

        // 1. Registry Uninstall entries (verifying InstallLocation / UninstallString still exist when local paths are given)
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

                // Verify entry is not a ghost entry pointing to a deleted directory/uninstaller
                let mut verified_alive = true;
                if let Some(ref loc) = install_loc {
                    let clean_loc = loc.trim_matches('"').trim();
                    if clean_loc.len() > 3 && clean_loc.contains(":\\") {
                        verified_alive = Path::new(clean_loc).exists();
                    }
                } else if let Some(ref uninst) = uninstall_str {
                    let lower_u = uninst.to_ascii_lowercase();
                    if !lower_u.contains("msiexec") && !lower_u.starts_with("winget") {
                        if let Some(exe_p) = extract_exe_path(uninst) {
                            let sys_bin = exe_p.to_ascii_lowercase().contains("\\windows\\system32\\");
                            if !sys_bin {
                                verified_alive = Path::new(&exe_p).exists();
                            }
                        }
                    }
                }

                if !verified_alive {
                    continue;
                }

                if let Some(dn) = display_name {
                    idx.insert_phrase(&dn);
                }
                if let Some(pub_name) = publisher {
                    idx.insert_phrase(&pub_name);
                }
                if !sub.starts_with('{') {
                    idx.insert_phrase(&sub);
                }
                if let Some(loc) = install_loc {
                    let clean_loc = loc.trim_matches('"').trim_end_matches('\\');
                    if let Some(leaf) = Path::new(clean_loc).file_name().and_then(|s| s.to_str()) {
                        idx.insert_phrase(leaf);
                    }
                    if let Some(parent_dir) = Path::new(clean_loc)
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                    {
                        if !parent_dir.eq_ignore_ascii_case("Program Files")
                            && !parent_dir.eq_ignore_ascii_case("Program Files (x86)")
                            && !parent_dir.eq_ignore_ascii_case("Programs")
                            && !parent_dir.eq_ignore_ascii_case("Local")
                            && !parent_dir.eq_ignore_ascii_case("Roaming")
                        {
                            idx.insert_phrase(parent_dir);
                        }
                    }
                }
            }
        }

        // 2. MSIX / Store Appx Repository Packages
        if let Some(appx_key) = open_reg_key(
            HKEY_CURRENT_USER_USIZE,
            r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages",
            0,
        ) {
            for pkg_full in enum_subkeys(&appx_key) {
                // Format: Name_Version_Arch_ResourceId_PublisherId -> Family = Name_PublisherId
                let parts: Vec<&str> = pkg_full.split('_').collect();
                if let (Some(&first), Some(&last)) = (parts.first(), parts.last()) {
                    idx.appx_families_lower
                        .insert(format!("{first}_{last}").to_ascii_lowercase());
                    idx.insert_phrase(first);
                }
            }
        }

        // 3. Installed Services (catches security suites like Avast, VPN daemons, drivers)
        if let Some(svc_root) = open_reg_key(
            HKEY_LOCAL_MACHINE_USIZE,
            r"SYSTEM\CurrentControlSet\Services",
            0,
        ) {
            for svc_name in enum_subkeys(&svc_root) {
                let full_svc = format!("SYSTEM\\CurrentControlSet\\Services\\{svc_name}");
                let Some(sk) = open_reg_key(HKEY_LOCAL_MACHINE_USIZE, &full_svc, 0) else {
                    continue;
                };
                if let Some(img) = query_reg_string(&sk, "ImagePath") {
                    if let Some(exe_p) = extract_exe_path(&img) {
                        if !exe_p.to_ascii_lowercase().contains("\\windows\\")
                            && Path::new(&exe_p).exists()
                        {
                            idx.insert_phrase(&svc_name);
                            for comp in Path::new(&exe_p).components() {
                                if let std::path::Component::Normal(os) = comp {
                                    if let Some(s) = os.to_str() {
                                        if !s.eq_ignore_ascii_case("Program Files")
                                            && !s.eq_ignore_ascii_case("Program Files (x86)")
                                        {
                                            idx.insert_phrase(s.trim_end_matches(".exe"));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Program Directories on disk (only counting folders that actually contain files/binaries)
        let mut prog_roots = vec![
            PathBuf::from(r"C:\Program Files"),
            PathBuf::from(r"C:\Program Files (x86)"),
        ];
        if let Ok(lad) = std::env::var("LOCALAPPDATA") {
            prog_roots.push(Path::new(&lad).join("Programs"));
        }
        for root in prog_roots {
            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && dir_contains_any_exe(&p, 3) {
                        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                            idx.insert_phrase(name);
                        }
                    }
                }
            }
        }

        // 5. Start Menu shortcuts (only when target or folder exists)
        let mut start_menus = vec![PathBuf::from(
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs",
        )];
        if let Ok(appdata) = std::env::var("APPDATA") {
            start_menus.push(
                Path::new(&appdata).join(r"Microsoft\Windows\Start Menu\Programs"),
            );
        }
        for sm in start_menus {
            collect_start_menu_names(&sm, 0, &mut idx);
        }

        // 6. Active CLI executables in PATH & standard user bin directories
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
            bin_dirs.push(Path::new(&lad).join(r"hermes\hermes-agent\venv\Scripts"));
        }

        let mut cli_stems: HashSet<String> = HashSet::new();
        for bdir in bin_dirs {
            if let Ok(entries) = std::fs::read_dir(&bdir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name();
                    let Some(s) = fname.to_str() else { continue };
                    let lower = s.to_ascii_lowercase();
                    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
                        if let Some(stem) = lower.strip_suffix(ext) {
                            if stem.len() >= 2 {
                                cli_stems.insert(stem.to_string());
                                idx.insert_phrase(stem);
                                if let Some(ctl_base) = stem.strip_suffix("ctl") {
                                    if ctl_base.len() >= 3 {
                                        cli_stems.insert(ctl_base.to_string());
                                        idx.insert_phrase(ctl_base);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Map active developer toolchains to their known cache/config folders
        if cli_stems.contains("node") || cli_stems.contains("npm") {
            for k in [
                "npm", "npmcache", "nodegyp", "nodejs", "viteplus", "turborepo", "biomejs",
                "vercel",
            ] {
                idx.compact_names.insert(k.to_string());
                idx.brand_tokens.insert(k.to_string());
            }
        }
        if cli_stems.contains("pnpm") {
            for k in ["pnpm", "pnpmcache", "pnpmstate"] {
                idx.compact_names.insert(k.to_string());
            }
        }
        if cli_stems.contains("bun") {
            idx.compact_names.insert("bun".to_string());
        }
        if cli_stems.contains("python") || cli_stems.contains("uv") || cli_stems.contains("pip") {
            for k in [
                "python", "pip", "pypa", "uv", "ipython", "matplotlib", "jupyter", "nltkdata",
                "jedi", "black", "imageio", "llamaindex", "pypoetry", "agentreach",
                "agentreachvenv", "hermes",
            ] {
                idx.compact_names.insert(k.to_string());
            }
        }
        if cli_stems.contains("cargo") || cli_stems.contains("rustc") {
            idx.compact_names.insert("cargo".to_string());
            idx.compact_names.insert("rustup".to_string());
        }
        if cli_stems.contains("dotnet") {
            for k in ["dotnet", "nuget", "templateengine"] {
                idx.compact_names.insert(k.to_string());
            }
        }
        if cli_stems.contains("go") {
            idx.compact_names.insert("go".to_string());
        }
        if cli_stems.contains("code") {
            for k in ["code", "vscode", "vscodeshared"] {
                idx.compact_names.insert(k.to_string());
            }
        }
        if cli_stems.contains("claude") {
            for k in ["claude", "claude3p", "claudedata", "claudecode"] {
                idx.compact_names.insert(k.to_string());
            }
        }
        if cli_stems.contains("codex") || cli_stems.contains("ocx") {
            for k in ["codex", "openai", "opencodex"] {
                idx.compact_names.insert(k.to_string());
            }
        }

        idx
    }

    fn insert_phrase(&mut self, phrase: &str) {
        let c = norm_compact(phrase);
        if c.len() >= 3 {
            self.compact_names.insert(c);
        }
        for tok in norm_tokens(phrase) {
            self.brand_tokens.insert(tok);
        }
    }

    fn matches_folder(&self, folder_name: &str) -> bool {
        let lower = folder_name.trim_start_matches('.').to_ascii_lowercase();
        // NodeJS CLI cache convention: `<tool>-nodejs`
        if lower.ends_with("-nodejs") && self.compact_names.contains("nodejs") {
            return true;
        }
        let compact = norm_compact(folder_name);
        if compact.is_empty() {
            return true;
        }
        if self.compact_names.contains(&compact) {
            return true;
        }
        // Check stripped updater/setup suffix only if base app is installed
        for suffix in ["updater", "setup", "cache", "data", "log", "logs", "cli"] {
            if let Some(stem) = compact.strip_suffix(suffix) {
                if stem.len() >= 3 && self.compact_names.contains(stem) {
                    return true;
                }
            }
        }
        let toks = norm_tokens(folder_name);
        if toks.is_empty() {
            return true;
        }
        // Reverse-domain or hyphenated folders like `com.vercel.cli`, `obs-studio-hook`, `com.logitech`
        for tok in &toks {
            if (tok.len() >= 4 && (self.compact_names.contains(tok) || self.brand_tokens.contains(tok)))
                || (tok.len() == 3 && self.compact_names.contains(tok))
            {
                return true;
            }
        }
        false
    }
}

fn dir_contains_any_exe(dir: &Path, max_depth: usize) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("exe") {
                    let stem = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !stem.starts_with("unins") && stem != "update" && stem != "squirrel" {
                        return true;
                    }
                }
            }
        } else if max_depth > 1 && p.is_dir() && dir_contains_any_exe(&p, max_depth - 1) {
            return true;
        }
    }
    false
}

#[derive(PartialEq, Eq)]
enum LnkStatus {
    Alive,
    Broken,
    Unknown,
}

/// Inspects a Windows Shell Link (.lnk, MS-SHLLINK format) and verifies whether its local target path still exists.
fn inspect_lnk_target(lnk_path: &Path) -> LnkStatus {
    let Ok(bytes) = std::fs::read(lnk_path) else {
        return LnkStatus::Unknown;
    };
    if bytes.len() < 76 || bytes[0..4] != [0x4C, 0x00, 0x00, 0x00] {
        return LnkStatus::Unknown;
    }
    let link_flags = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let mut offset = 76usize;
    if (link_flags & 0x01) != 0 {
        if offset + 2 > bytes.len() {
            return LnkStatus::Unknown;
        }
        let id_list_size = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2 + id_list_size;
    }
    if (link_flags & 0x02) == 0 || offset + 28 > bytes.len() {
        return LnkStatus::Unknown;
    }
    let info_header_size = u32::from_le_bytes([
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]) as usize;
    let info_flags = u32::from_le_bytes([
        bytes[offset + 8],
        bytes[offset + 9],
        bytes[offset + 10],
        bytes[offset + 11],
    ]);
    // VolumeIDAndLocalBasePath = bit 0
    if (info_flags & 0x01) == 0 {
        return LnkStatus::Unknown;
    }

    let mut extracted: Option<String> = None;
    if info_header_size >= 0x24 && offset + 32 <= bytes.len() {
        let uni_off = u32::from_le_bytes([
            bytes[offset + 28],
            bytes[offset + 29],
            bytes[offset + 30],
            bytes[offset + 31],
        ]) as usize;
        if uni_off > 0 && offset + uni_off + 2 <= bytes.len() {
            let mut u16s = Vec::new();
            let mut p = offset + uni_off;
            while p + 1 < bytes.len() {
                let ch = u16::from_le_bytes([bytes[p], bytes[p + 1]]);
                if ch == 0 {
                    break;
                }
                u16s.push(ch);
                p += 2;
            }
            if !u16s.is_empty() {
                extracted = Some(String::from_utf16_lossy(&u16s));
            }
        }
    }

    if extracted.is_none() {
        let ansi_off = u32::from_le_bytes([
            bytes[offset + 16],
            bytes[offset + 17],
            bytes[offset + 18],
            bytes[offset + 19],
        ]) as usize;
        if ansi_off > 0 && offset + ansi_off < bytes.len() {
            let slice = &bytes[offset + ansi_off..];
            let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            if len > 0 {
                extracted = Some(String::from_utf8_lossy(&slice[..len]).into_owned());
            }
        }
    }

    if let Some(target) = extracted {
        if target.len() > 3 && target.contains(":\\") {
            return if Path::new(&target).exists() {
                LnkStatus::Alive
            } else {
                LnkStatus::Broken
            };
        }
    }
    LnkStatus::Unknown
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
            let child_valid = collect_start_menu_names(&p, depth + 1, idx);
            if child_valid {
                has_valid_shortcut = true;
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if !name.eq_ignore_ascii_case("Startup")
                        && !name.eq_ignore_ascii_case("Administrative Tools")
                        && !name.eq_ignore_ascii_case("System Tools")
                        && !name.eq_ignore_ascii_case("Accessories")
                        && !name.eq_ignore_ascii_case("Accessibility")
                        && !name.eq_ignore_ascii_case("Maintenance")
                    {
                        idx.insert_phrase(name);
                    }
                }
            }
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                let lower = stem.to_ascii_lowercase();
                if lower.starts_with("uninstall ") {
                    continue;
                }
                match inspect_lnk_target(&p) {
                    LnkStatus::Broken => {
                        let c = norm_compact(stem);
                        if c.len() >= 3 {
                            idx.broken_shortcuts.insert(c);
                        }
                    }
                    LnkStatus::Alive | LnkStatus::Unknown => {
                        has_valid_shortcut = true;
                        idx.insert_phrase(stem);
                    }
                }
            }
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("url")) {
            has_valid_shortcut = true;
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                idx.insert_phrase(stem);
            }
        }
    }
    has_valid_shortcut
}

fn is_system_or_shared_bucket(name: &str) -> bool {
    let lower = name.trim_start_matches('.').to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "microsoft"
            | "microsoftwindows"
            | "windows"
            | "windowspowershell"
            | "windows defender"
            | "windows defender advanced threat protection"
            | "windows nt"
            | "windows mail"
            | "windows media player"
            | "windows photo viewer"
            | "windows kits"
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
            | "software distribution"
            | "sshfs-win"
            | "agents"
            | "gemini"
            | "antigravity"
            | "pi"
            | "pi-node"
            | "typesafe"
            | "speech"
    )
}

fn filetime_to_days_ago(ft: i64) -> Option<u64> {
    if ft <= 0 {
        return None;
    }
    // Windows FILETIME epoch (1601-01-01) to Unix epoch (1970-01-01) = 11_644_473_600 seconds
    let secs_since_1601 = (ft as u64) / 10_000_000;
    let unix_secs = secs_since_1601.checked_sub(11_644_473_600)?;
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(now_secs.saturating_sub(unix_secs) / 86_400)
}

pub struct OrphanReport {
    pub by_path_lower: HashMap<String, OrphanMatch>,
    pub sorted_list: Vec<OrphanMatch>,
    pub total_bytes: u64,
}

pub fn detect_orphans(tree: &ScanTree) -> OrphanReport {
    let idx = InstalledIndex::gather();
    let mut matches: Vec<OrphanMatch> = Vec::new();

    // Candidate parent scopes to inspect inside the scanned drive
    let root = tree.root_path.trim_end_matches('\\');
    let mut scopes: Vec<(String, &'static str, bool)> = Vec::new();
    scopes.push((format!(r"{root}\ProgramData"), "ProgramData", false));
    scopes.push((format!(r"{root}\Program Files"), "Program Files", false));
    scopes.push((
        format!(r"{root}\Program Files (x86)"),
        "Program Files (x86)",
        false,
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
            scopes.push((format!(r"{ubase}\AppData\Local"), "AppData\\Local", false));
            scopes.push((
                format!(r"{ubase}\AppData\LocalLow"),
                "AppData\\LocalLow",
                false,
            ));
            scopes.push((
                format!(r"{ubase}\AppData\Roaming"),
                "AppData\\Roaming",
                false,
            ));
            scopes.push((
                format!(r"{ubase}\AppData\Local\Packages"),
                "Store Packages",
                true,
            ));
            // Dot-folders directly under <drive>:\Users\<user>
            if let Some(u_node_id) = tree.find_node_by_path(&ubase) {
                for &cid in &tree.nodes[u_node_id as usize].children {
                    let cnode = &tree.nodes[cid as usize];
                    if cnode.name.starts_with('.') {
                        evaluate_candidate(
                            tree,
                            &idx,
                            cid,
                            "User Profile",
                            false,
                            &mut matches,
                        );
                    }
                }
            }
        }
    }

    for (scope_path, group_label, is_appx_scope) in scopes {
        let Some(scope_id) = tree.find_node_by_path(&scope_path) else {
            continue;
        };
        for &cid in &tree.nodes[scope_id as usize].children {
            evaluate_candidate(tree, &idx, cid, group_label, is_appx_scope, &mut matches);
        }
    }

    matches.sort_unstable_by(|a, b| b.size.cmp(&a.size));

    let mut total_bytes = 0u64;
    let mut by_path_lower = HashMap::with_capacity(matches.len());
    for m in &matches {
        total_bytes = total_bytes.saturating_add(m.size);
        by_path_lower.insert(m.path.to_ascii_lowercase(), m.clone());
    }

    OrphanReport {
        by_path_lower,
        sorted_list: matches,
        total_bytes,
    }
}

/// Checks whether a folder's `has_exe` flag is solely caused by leftover Squirrel updater stubs (`Update.exe` / `squirrel.exe`).
fn has_only_squirrel_stub_exes(tree: &ScanTree, cid: u32) -> bool {
    let node = &tree.nodes[cid as usize];
    let has_update_root = node
        .top_files
        .iter()
        .any(|f| f.name.eq_ignore_ascii_case("Update.exe"));
    if !has_update_root {
        return false;
    }
    for &sub_id in &node.children {
        let sub = &tree.nodes[sub_id as usize];
        if sub.has_exe {
            for f in &sub.top_files {
                let lower = f.name.to_ascii_lowercase();
                if lower.ends_with(".exe") && lower != "squirrel.exe" && lower != "update.exe" {
                    return false;
                }
            }
        }
    }
    true
}

fn evaluate_candidate(
    tree: &ScanTree,
    idx: &InstalledIndex,
    cid: u32,
    group_label: &'static str,
    is_appx_scope: bool,
    out: &mut Vec<OrphanMatch>,
) {
    let cnode = &tree.nodes[cid as usize];
    // Ignore tiny folders (< 64 KB) or restricted empty shells
    if cnode.total_size < 65_536 || cnode.access_denied {
        return;
    }
    let name = cnode.name.as_ref();
    if is_system_or_shared_bucket(name) {
        return;
    }

    let days_idle = filetime_to_days_ago(cnode.last_write_time);

    // Protect folders actively written to within the last 2 days
    if days_idle.is_some_and(|d| d < 2) {
        return;
    }

    let lower_name = name.to_ascii_lowercase();
    let is_user_dotfolder = group_label == "User Profile";

    // Skip folders containing live application executables (unless User Profile dot-folder with extension helpers or Squirrel updater stub)
    if !is_user_dotfolder
        && cnode.has_exe
        && !lower_name.ends_with("-updater")
        && !lower_name.contains("setup")
        && !lower_name.contains("installer")
        && !has_only_squirrel_stub_exes(tree, cid)
    {
        return;
    }

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
        let Some(full_path) = tree.node_path(cid) else {
            return;
        };
        let idle_note = days_idle
            .map(|d| format!(" · untouched {d}d"))
            .unwrap_or_default();
        out.push(OrphanMatch {
            node_id: cid,
            name: name.to_string(),
            path: full_path,
            location_group: group_label,
            size: cnode.total_size,
            days_idle,
            reason: format!("Store package is no longer installed{idle_note}"),
        });
        return;
    }

    if idx.matches_folder(name) {
        return;
    }

    let Some(full_path) = tree.node_path(cid) else {
        return;
    };
    let idle_note = days_idle
        .map(|d| format!(" · untouched {d}d"))
        .unwrap_or_default();
    let compact = norm_compact(name);
    let base_reason = if idx.broken_shortcuts.contains(&compact) {
        "App executable was removed"
    } else {
        "No matching installed app found in Windows"
    };
    out.push(OrphanMatch {
        node_id: cid,
        name: name.to_string(),
        path: full_path,
        location_group: group_label,
        size: cnode.total_size,
        days_idle,
        reason: format!("{base_reason}{idle_note}"),
    });
}
