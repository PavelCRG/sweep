# sweep

### Find what's filling your disks. Review it. Clean it safely.

[![CI](https://github.com/PavelCRG/sweep/actions/workflows/ci.yml/badge.svg)](https://github.com/PavelCRG/sweep/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-Windows%20%7C%20Linux-orange?logo=rust)](https://github.com/PavelCRG/sweep)

**sweep** is a fast, keyboard-first disk analyzer for Windows and Linux. It scans mounted disks or selected folders, shows where space is going, and moves reviewed items to the system Trash.

## Install

Download the Windows or Linux archive from [GitHub Releases](https://github.com/PavelCRG/sweep/releases/latest), extract it, and run `sweep` (`sweep.exe` on Windows). Releases include SHA-256 checksums.

To build from source, install Rust and run:

```sh
cargo build --release --locked
```

The standalone executable is `target/release/sweep` on Linux and `target/release/sweep.exe` on Windows.

## Start here

```sh
sweep                              # scan mounted disks and open the TUI
sweep --path ~/Projects            # scan one folder
sweep --path C:\Users\Pavel D:\   # scan selected Windows paths
sweep --demo                       # explore a virtual disk safely
sweep --plain                      # print a readable report
sweep --json > before.json         # save a JSON snapshot
sweep --compare before.json        # compare this scan with the snapshot
sweep --csv > report.csv           # export a spreadsheet-friendly report
sweep --check-updates              # check GitHub Releases on request
```

Use `sweep --help` for the full command list. Scanning works locally. Only `--check-updates` connects to GitHub; it checks the latest stable release and sends no scan results.

## TUI controls

| Key | Action |
| --- | --- |
| `↑` / `↓`, `j` / `k` | Move through folders and files |
| `Enter` / `→` | Open a folder or disk |
| `Esc` / `←` | Go up; cancel a running rescan with `Esc` |
| `g` | Jump into the largest space-using child |
| `f` | Filter to Junk and Unused items |
| `d` twice | Move the selected item to Trash |
| `r` | Rescan the selected disks; progress and skipped entries are shown |
| `Tab` | Switch panel focus |
| `q` | Quit |

Press `Ctrl+C` to cancel the initial scan. Unreadable or locked entries are skipped and counted; they are never treated as zero-byte files in cleanup recommendations.

## Safe cleanup

Sweep explains each `JUNK`, `UNUSED`, and `SYSTEM` label. Review the item and its size before pressing `d` twice. Selected items go to the operating system's Trash, and protected system paths are blocked. Use `--dry-run` to preview the action without moving anything.

## Reports

JSON snapshots can be saved with normal shell redirection and compared later with `--compare`. Comparison matches paths and reports size changes, including files or folders added or removed. CSV includes paths, sizes in bytes, tags, categories, and the reason for each label.

## Development

```sh
cargo fmt --all -- --check
cargo test --locked
cargo build --release --locked
```

GitHub Actions runs formatting, tests, and release builds on Windows and Linux for changes to `main`. Pushing a version tag such as `v0.2.0` builds both standalone archives, adds SHA-256 checksums, and publishes a GitHub Release.

## Project

Maintained by [PavelCRG](https://github.com/PavelCRG). Issues and contributions are welcome.
