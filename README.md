# sweep

### Know what is filling your disks. Clean up with confidence.

**sweep** is an offline-first disk analyzer for Windows and Linux, built by [PavelCRG](https://github.com/PavelCRG). It maps large folders, points to the real space hog, explains what looks safe to remove, and sends selected items to the system Trash.

One command opens an interactive, keyboard-first view. No account, telemetry, or network connection is needed at runtime.

```text
  SWEEP | DISK MAP
  [#########...] Personal   [##.........] Junk   [###........] Unused   Free
  > Projects       [#######.]  42.1 GB   mostly in app/target
    Downloads      [###.....]   8.4 GB   INSTALLER
```

## What it does

- **Finds the big stuff.** A parallel scan follows each folder's largest child to show where the space actually went.
- **Works across volumes.** The default scan discovers mounted disks; use `--path` to scan specific folders or several paths together.
- **Explains its labels.** `JUNK`, `UNUSED`, and `SYSTEM` come with a reason in the details panel.
- **Cleans safely.** Selected items go to the operating system's Trash. Press `d` twice to confirm; `--dry-run` shows what would happen without changing files.
- **Stays local.** Scans and reports run on your computer. The app does not need internet access.
- **Includes a safe demo.** Explore a realistic virtual 256 GB disk without touching real files.

## Get started

Build a standalone release binary with Rust installed:

```sh
cargo build --release
```

The executable is `target/release/sweep` on Linux and `target/release/sweep.exe` on Windows.

```sh
sweep                         # discover and scan mounted disks
sweep --path ~/Projects       # scan one directory
sweep --path C:\ --path D:\  # scan two Windows volumes
sweep --demo                  # browse the virtual disk
sweep --dry-run               # run the TUI without moving files
sweep --plain                 # print a text report
sweep --json                  # print structured JSON
```

Use `sweep --help` to see all options. `--plain` and `--json` scan the discovered volumes too; pass `--path` to limit the report.

## In the TUI

| Key | Action |
| --- | --- |
| `Up` / `Down`, `j` / `k` | Move through the list |
| `Enter` / `Right` | Open a folder or volume |
| `Esc` / `Left` | Go up one level |
| `g` | Jump to the largest culprit inside the selected folder |
| `f` | Show only Junk and Unused items |
| `d` twice | Move the selected item to Trash |
| `r` | Rescan all selected volumes |
| `Tab` | Highlight the other pane |
| `q` | Quit |

## Labels

| Label | Meaning |
| --- | --- |
| `JUNK` | Rebuildable caches and build output, plus old installers in Downloads |
| `UNUSED` | Items with both access and modification dates older than six months |
| `SYSTEM` | OS-managed paths that sweep blocks from deletion |
| `PERSONAL` | Everything else; review it before removing |

These labels are suggestions to guide review. Sweep never permanently deletes a selected item; the operating system's Trash gives you a chance to restore it.

## Project

The project is maintained by [PavelCRG](https://github.com/PavelCRG). Issues and contributions are welcome.
