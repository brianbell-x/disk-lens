# Disk Lens

Minimalist Windows utility where folders and files are ranked by disk space across any depth of nesting.

<p align="center">
  <img src="docs/demo.gif" width="420" alt="Disk Lens demo">
</p>

<p align="center">
  <a href="docs/demo.mp4">Full-resolution video (MP4 · 1080×1350)</a>
</p>

## Usage

- **Nested folders** — Any folder row can be clicked to expand nested subfolders and files to any depth.
- **App leftovers** — Uninstalled app remnants are grouped by app under **Leftovers** with confidence tiers (`Confirmed`, `Likely`, `Review`), visible evidence, ignore/keep persistence, and batch Recycle Bin cleanup.
- **Row actions** — Any item can be opened in File Explorer, opened in VS Code, kept/ignored, or moved to the Recycle Bin from the icons on the right.

## Build

A standalone Windows executable is built and launched with Cargo:

```powershell
cargo run --release
```
