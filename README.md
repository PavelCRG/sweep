<div align="center">

# sweep

### See what is filling your disks. Clean up with confidence.

Local-first, keyboard-driven disk space analysis for Windows and Linux.

[![CI](https://github.com/PavelCRG/sweep/actions/workflows/ci.yml/badge.svg)](https://github.com/PavelCRG/sweep/actions/workflows/ci.yml)
[![Release](https://img.shields.io/badge/release-v0.1.5-22d3ee)](https://github.com/PavelCRG/sweep/releases/tag/v0.1.5)
[![Windows · Linux](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-64748b)](#install)

</div>

Sweep opens a disk overview while scans run. Browse folders as entries arrive; sizes are marked as partial until that disk finishes. Find the largest folders or files, review cleanup candidates, and move selected items to the system Trash.

## Install

Download a standalone x86-64 binary and its checksum from the [v0.1.5 release](https://github.com/PavelCRG/sweep/releases/tag/v0.1.5):

| Platform | Download |
| --- | --- |
| Windows | [sweep-windows-x86_64.zip](https://github.com/PavelCRG/sweep/releases/download/v0.1.5/sweep-windows-x86_64.zip) |
| Linux | [sweep-linux-x86_64.tar.gz](https://github.com/PavelCRG/sweep/releases/download/v0.1.5/sweep-linux-x86_64.tar.gz) |
| Checksums | [SHA256SUMS.txt](https://github.com/PavelCRG/sweep/releases/download/v0.1.5/SHA256SUMS.txt) |

Extract the archive and run `sweep` (`sweep.exe` on Windows).

The current `main` branch includes the largest-files (`a`) and cleanup-candidates (`c`) views. These views are newer than the v0.1.5 release binaries; build from source to use them until the next release:

```sh
cargo build --release --locked
```

The binary is at `target/release/sweep` on Linux and `target/release/sweep.exe` on Windows.

## Quick start

| Command | What it does |
| --- | --- |
| `sweep` | Scan mounted disks and open the TUI |
| `sweep --demo` | Browse virtual disks without touching real files |
| `sweep --path ~/Projects` | Scan a chosen directory |
| `sweep --lang ru` | Start the interface in Russian; press `l` to switch languages |
| `sweep --dry-run` | Preview cleanup without changing files |
| `sweep --plain` | Print a readable text report |

Export or compare scans with `sweep --json > scan.json`, `sweep --csv > scan.csv`, and `sweep --compare scan.json`.

## Keyboard controls

| Key | Action |
| --- | --- |
| `↑` / `↓`, `j` / `k` | Move through the list |
| `Enter` / `→` | Open a disk or folder |
| `Esc` / `←` | Go up; cancel the initial scan with `Esc` |
| `g` | Jump to the largest folder below the selected item |
| `a` | Show the 100 largest files under the current folder or disk overview |
| `c` | Show Junk cleanup candidates |
| `f` | Show all items or only Junk and Unused |
| `d` twice | Move the selected item to Trash |
| `r` | Rescan |
| `Tab` | Switch panel focus |
| `l` | Switch interface language |
| `q` | Quit |

## Safe cleanup

Review the item, its size, and its label before cleanup. Press `d` twice to move it to the system Trash. Sweep blocks protected system paths and shows the total successfully moved during the current session. Empty the system Trash to reclaim disk space. Use `--dry-run` to preview actions; `--demo` never changes real files.

## Local data and reports

Scanning and reports run locally. Sweep contacts GitHub only when you explicitly use `--check-updates`; scan data is not sent. JSON snapshots can be compared later to show size changes and added or removed paths. CSV and JSON reports include paths, sizes, categories, and cleanup labels.

## Build and verify

```sh
cargo fmt --all -- --check
cargo test --locked
cargo build --release --locked
```

GitHub Actions checks Windows and Linux builds. Version tags produce standalone archives and SHA-256 checksums.

## Project

Created and maintained by [PavelCRG](https://github.com/PavelCRG). Bug reports and contributions are welcome in [Issues](https://github.com/PavelCRG/sweep/issues).
