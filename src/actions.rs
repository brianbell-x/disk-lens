use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::{null, null_mut};
use windows_sys::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
    SHFileOperationW,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn is_deletable_path(path: &str) -> bool {
    let cleaned = path.replace('/', "\\");
    let trimmed = if cleaned.len() > 3 {
        cleaned.trim_end_matches('\\')
    } else {
        cleaned.as_str()
    };
    if trimmed.len() <= 3 || trimmed.ends_with(':') {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_str) = exe.to_str() {
            let exe_clean = exe_str.trim_start_matches(r"\\?\").to_ascii_lowercase();
            if lower == exe_clean {
                return false;
            }
        }
    }

    let bytes = lower.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        let rel = lower[2..].trim_start_matches('\\');
        if matches!(
            rel,
            "windows"
                | "program files"
                | "program files (x86)"
                | "programdata"
                | "users"
                | "$recycle.bin"
                | "system volume information"
                | "recovery"
                | "boot"
                | "efi"
                | "perflogs"
                | "documents and settings"
                | "config.msi"
                | "pagefile.sys"
                | "hiberfil.sys"
                | "swapfile.sys"
                | "dumpstack.log"
                | "dumpstack.log.tmp"
                | "bootmgr"
        ) {
            return false;
        }

        if rel.starts_with("windows\\system32")
            || rel.starts_with("windows\\syswow64")
            || rel.starts_with("windows\\winsxs")
            || rel.starts_with("windows\\boot")
        {
            return false;
        }

        if let Some(after_users) = rel.strip_prefix("users\\") {
            let parts: Vec<&str> = after_users.split('\\').collect();
            match parts.as_slice() {
                [_user] => return false,
                [_user, "appdata"]
                | [_user, "desktop"]
                | [_user, "documents"]
                | [_user, "onedrive"] => return false,
                [_user, "appdata", "local"]
                | [_user, "appdata", "locallow"]
                | [_user, "appdata", "roaming"]
                | [_user, "onedrive", "desktop"] => return false,
                [_user, "appdata", "local", "packages"]
                | [_user, "appdata", "local", "programs"] => return false,
                _ => {}
            }
        }
    }

    true
}

pub fn move_to_recycle_bin(path: &str) -> Result<(), String> {
    let cleaned = path.replace('/', "\\");
    let target = if cleaned.len() > 3 {
        cleaned.trim_end_matches('\\').to_string()
    } else {
        cleaned
    };

    if !is_deletable_path(&target) {
        return Err("Protected system or profile folder cannot be deleted.".to_string());
    }

    let p = Path::new(&target);
    if !p.exists() {
        return Err(format!("Path no longer exists on disk: {target}"));
    }

    let mut from_wide: Vec<u16> = target.encode_utf16().collect();
    from_wide.push(0);
    from_wide.push(0);

    let mut op = SHFILEOPSTRUCTW {
        hwnd: null_mut(),
        wFunc: FO_DELETE,
        pFrom: from_wide.as_ptr(),
        pTo: null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: null_mut(),
        lpszProgressTitle: null(),
    };

    let rc = unsafe { SHFileOperationW(&mut op) };
    if rc != 0 {
        let detail = match rc {
            5 | 0x78 => "Permission denied while moving item to the Recycle Bin.".to_string(),
            32 => "Item is currently in use by a running application.".to_string(),
            _ => format!("Could not move item to the Recycle Bin (code {rc})."),
        };
        return Err(detail);
    }
    if op.fAnyOperationsAborted != 0 || p.exists() {
        return Err(
            "Move to Recycle Bin did not complete because a file may be locked.".to_string(),
        );
    }

    Ok(())
}

pub fn open_in_explorer(path: &str, select_file: bool) -> Result<(), String> {
    let cleaned = path.replace('/', "\\");
    let p = Path::new(&cleaned);

    let mut cmd = Command::new("explorer.exe");
    if select_file {
        if p.exists() {
            cmd.raw_arg(format!("/select,\"{cleaned}\""));
        } else if let Some(parent) = p.parent() {
            if parent.exists() {
                cmd.arg(parent);
            } else {
                return Err(format!("Path no longer exists on disk: {cleaned}"));
            }
        } else {
            return Err(format!("Path no longer exists on disk: {cleaned}"));
        }
    } else if p.exists() {
        cmd.arg(&cleaned);
    } else {
        return Err(format!("Path no longer exists on disk: {cleaned}"));
    }

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not launch File Explorer: {e}"))
}

fn find_vscode_executable() -> Option<PathBuf> {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let user_code = Path::new(&local_app_data)
            .join("Programs")
            .join("Microsoft VS Code")
            .join("Code.exe");
        if user_code.is_file() {
            return Some(user_code);
        }
        let insiders = Path::new(&local_app_data)
            .join("Programs")
            .join("Microsoft VS Code Insiders")
            .join("Code - Insiders.exe");
        if insiders.is_file() {
            return Some(insiders);
        }
    }

    for candidate in [
        r"C:\Program Files\Microsoft VS Code\Code.exe",
        r"C:\Program Files (x86)\Microsoft VS Code\Code.exe",
    ] {
        let p = Path::new(candidate);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    None
}

pub fn open_in_vscode(path: &str) -> Result<(), String> {
    let cleaned = path.replace('/', "\\");
    if !Path::new(&cleaned).exists() {
        return Err(format!("Path no longer exists on disk: {cleaned}"));
    }

    if let Some(code_exe) = find_vscode_executable() {
        Command::new(code_exe)
            .arg(&cleaned)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not launch VS Code: {e}"))
    } else {
        Command::new("cmd.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/C", "code", &cleaned])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not find VS Code on this computer: {e}"))
    }
}
