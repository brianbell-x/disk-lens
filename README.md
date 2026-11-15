# Disk Lens

Minimalist native Windows desktop utility for identifying high-consumption folders across any depth of nesting, surfacing orphaned folders from uninstalled applications, and safely moving unwanted items to the Recycle Bin.

## Highlights

- **Instant Startup**: Pre-computed flat binary snapshots (`DLNS` v3) in `%LOCALAPPDATA%\DiskLens` reload over 1.05 million directories in ~399 ms.
- **NT Kernel Parallel Scanner**: Multi-threaded `NtOpenFile` + `NtQueryDirectoryFile` (`FileIdFullDirectoryInformation`) traversal with a 2 MB lock-free MFT bitset for NTFS hardlink deduplication, Name Surrogate reparse-point filtering (skipping junctions and symlinks without skipping OneDrive directories), and physical `AllocationSize` accounting for cloud-dehydrated and sparse files.
- **Uninstalled App Leftover Detection**: Cross-references `AppData\Local`, `AppData\LocalLow`, `AppData\Roaming`, `AppData\Local\Packages`, `ProgramData`, `Program Files`, and user profile dot-folders against Registry `Uninstall` keys, MSIX/Store package repositories, Windows Services, verified `[MS-SHLLINK]` Start Menu shortcut targets, `Program Files` binaries, and active developer CLI toolchains. Suspected folders are badged inline (`Leftover?`) and aggregated in a one-click `Leftovers` view.
- **Safe Recycle Bin Deletion & OS Actions**: Every row provides off-to-the-side actions to open in File Explorer (highlighting individual files via `/select,`), open in Visual Studio Code (`Code.exe`), or move to the Windows Recycle Bin (`SHFileOperationW` with `FOF_ALLOWUNDO`) behind an inline confirmation bar. Critical OS and user profile roots are guarded against deletion.
- **Live Incremental Updates**: Background `ReadDirectoryChangesW` monitoring and post-delete hooks refresh modified or deleted folders in place in milliseconds without requiring a full-drive rescan.

## Desktop Shortcut

A shortcut at `%USERPROFILE%\OneDrive\Desktop\Disk Lens.lnk` points to `C:\dev\disk-lens\DiskLens.exe`.

## Building from Source

```powershell
cargo build --release
Copy-Item -Force target\release\disk-lens.exe DiskLens.exe
```

Optional CLI flags supported by `DiskLens.exe`:
- `--warm-cache`: Performs a full scan of the primary drive and updates the binary snapshot cache.
- `--check-orphans`: Loads the cached snapshot and prints detected leftover application folders.
- `--serve <port>`: Starts the embedded HTTP + SSE backend on `127.0.0.1:<port>` without opening a native window.
