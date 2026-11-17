<div align="center">

# sweep

### See what is filling your disks. Clean up with confidence.

Fast, keyboard-first disk space analysis for Windows and Linux.

[![CI](https://github.com/PavelCRG/sweep/actions/workflows/ci.yml/badge.svg)](https://github.com/PavelCRG/sweep/actions/workflows/ci.yml)
[![Release](https://img.shields.io/badge/release-v0.1.5-22d3ee)](https://github.com/PavelCRG/sweep/releases/tag/v0.1.5)
[![Windows · Linux](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-64748b)](#install)

</div>

<p align="center">
  <img src="assets/sweep-demo.png" alt="Sweep demo: two disks, colorful space bars, capacity details, and the largest folders" width="92%">
  <br><sub>Two demo volumes · per-disk capacity · largest folders · safe demo data</sub>
</p>

Sweep opens its disk overview immediately, then shows folders and sizes as they are found. You can start drilling into a drive before its full scan finishes. Cleanup unlocks for a drive only when that drive is fully scanned. The screenshot uses synthetic demo disks only; it contains no personal paths or file names.

## Install

Download the archive for your system from the [v0.1.5 release](https://github.com/PavelCRG/sweep/releases/tag/v0.1.5), extract it, and run `sweep` (or `sweep.exe` on Windows). Each release includes a SHA-256 checksum file.

Or build the standalone binary from source with Rust installed:

```sh
cargo build --release --locked
```

The binary is at `target/release/sweep` on Linux and `target/release/sweep.exe` on Windows.

## Quick start

| Command | What it does |
| --- | --- |
| `sweep` | Scan mounted disks and open the TUI |
| `sweep --demo` | Browse two virtual disks without touching real files |
| `sweep --path ~/Projects` | Scan a chosen folder instead of all disks |
| `sweep --lang ru` | Start in Russian; press `l` to switch languages |
| `sweep --dry-run` | Preview cleanup actions without changing files |
| `sweep --plain` | Print a readable report in the terminal |

For machine-readable reports, use `sweep --json > before.json` or `sweep --csv > report.csv`. Compare a saved JSON scan with `sweep --compare before.json`.

Scans and reports run locally. Sweep connects to GitHub only when you explicitly run `--check-updates`; no scan data is sent.

## Explore the TUI

| Key | Action |
| --- | --- |
| `↑` / `↓` or `j` / `k` | Move through the list |
| `Enter` / `→` | Open a folder or disk |
| `Esc` / `←` | Go up; cancel a rescan |
| `g` | Jump into the largest space-using child |
| `a` | Show the 100 largest files under the current folder or disk overview |
| `c` | Show safe cleanup candidates such as caches and temporary folders |
| `f` | Show everything or only Junk and Unused |
| `l` | Switch between English and Russian |
| `d` twice | Move the selected item to Trash |
| `r` | Rescan the selected disks |
| `Tab` | Switch panel focus |
| `q` | Quit |

`Ctrl+C` cancels the initial scan. Unreadable or locked entries are skipped and counted.

## Safe cleanup

Sweep explains its `JUNK`, `UNUSED`, and `SYSTEM` labels. Press `c` to list cache and temporary cleanup candidates, review each item, then press `d` twice to move it to the operating system's Trash. The TUI totals the size moved during the session; empty the operating system's Trash to reclaim disk space. Protected system paths are blocked. Use `--dry-run` to preview cleanup without changing files; `--demo` is always non-destructive.

## Reports

JSON snapshots can be compared later with `--compare`. Comparisons match paths and show size changes, including added and removed files. CSV exports include paths, sizes, tags, categories, and label reasons.

## Build and verify

```sh
cargo fmt --all -- --check
cargo test --locked
cargo build --release --locked
```

GitHub Actions checks Windows and Linux builds. Version tags such as `v0.1.5` produce standalone archives and SHA-256 checksums.

The TUI starts in English; use `--lang ru` to start in Russian or press `l` to switch while browsing.

## Project

Created and maintained by [PavelCRG](https://github.com/PavelCRG). Bug reports and contributions are welcome in [Issues](https://github.com/PavelCRG/sweep/issues).
