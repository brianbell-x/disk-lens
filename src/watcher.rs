use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_SIZE, FILE_NOTIFY_INFORMATION,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        lp_file_name: *const u16,
        dw_desired_access: u32,
        dw_share_mode: u32,
        lp_security_attributes: *const c_void,
        dw_creation_disposition: u32,
        dw_flags_and_attributes: u32,
        h_template_file: *mut c_void,
    ) -> *mut c_void;
}

#[repr(align(8))]
struct NotifyBuf([u8; 65536]);

pub fn spawn_drive_watcher<F>(drive_root: String, stop: Arc<AtomicBool>, mut on_batch: F)
where
    F: FnMut(Vec<String>) + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<String>();
    let stop_reader = Arc::clone(&stop);

    // Reader thread blocking on ReadDirectoryChangesW
    std::thread::spawn(move || {
        let wide: Vec<u16> = drive_root
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                FILE_LIST_DIRECTORY,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return;
        }

        let mut buf = Box::new(NotifyBuf([0u8; 65536]));
        while !stop_reader.load(Ordering::Relaxed) {
            let mut bytes_returned = 0u32;
            let ok = unsafe {
                ReadDirectoryChangesW(
                    handle,
                    buf.0.as_mut_ptr() as *mut c_void,
                    buf.0.len() as u32,
                    1, // watch entire subtree
                    FILE_NOTIFY_CHANGE_FILE_NAME
                        | FILE_NOTIFY_CHANGE_DIR_NAME
                        | FILE_NOTIFY_CHANGE_SIZE,
                    &mut bytes_returned,
                    null_mut(),
                    None,
                )
            };
            if ok == 0 || stop_reader.load(Ordering::Relaxed) {
                break;
            }
            if bytes_returned == 0 {
                continue;
            }

            let mut offset = 0usize;
            loop {
                let info = unsafe {
                    &*(buf.0.as_ptr().add(offset) as *const FILE_NOTIFY_INFORMATION)
                };
                let u16_len = (info.FileNameLength as usize) / 2;
                if u16_len > 0 {
                    let slice =
                        unsafe { std::slice::from_raw_parts(info.FileName.as_ptr(), u16_len) };
                    let rel = String::from_utf16_lossy(slice);
                    // Ignore DiskLens's own cache & WebView2 state directory
                    let lower = rel.to_ascii_lowercase();
                    if !lower.contains("\\appdata\\local\\disklens")
                        && !lower.contains("\\.gemini\\antigravity\\brain\\")
                    {
                        if tx.send(rel).is_err() {
                            unsafe {
                                CloseHandle(handle);
                            }
                            return;
                        }
                    }
                }
                if info.NextEntryOffset == 0 {
                    break;
                }
                offset += info.NextEntryOffset as usize;
            }
        }
        unsafe {
            CloseHandle(handle);
        }
    });

    // Debouncer thread coalescing bursts over 1.2s windows
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let Ok(first) = rx.recv_timeout(Duration::from_millis(800)) else {
                continue;
            };
            let mut batch = vec![first];
            let deadline = Instant::now() + Duration::from_millis(1200);
            while Instant::now() < deadline && batch.len() < 512 {
                match rx.recv_timeout(Duration::from_millis(150)) {
                    Ok(item) => batch.push(item),
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            if !stop.load(Ordering::Relaxed) {
                on_batch(batch);
            }
        }
    });
}
