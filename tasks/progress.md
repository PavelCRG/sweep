# Final terminal UI execution ledger

## Baseline

- Plan: `tasks/plan.md`, 20 implementation points.
- Starting commit: `c15bbfb` on `main`; user explicitly authorized execution, commit, and push as Pavel.
- Scope: `src/ui/app.rs`, `src/ui/store.rs`, targeted UI tests, and this checklist. No new dependencies; plain/JSON/CSV output and scan/cleanup core stay unchanged.

## Checkpoint 1: color diagnosis and screen foundation

- Confirmed the shell launching the TUI sets `NO_COLOR=1` and `TERM=dumb`.
- Crossterm 0.28 suppresses every foreground/background color when `NO_COLOR` is non-empty. This is the root cause of the all-white screen.
- Ruling: force crossterm colors only when entering the full-screen TUI. `--plain`, `--json`, and `--csv` remain untouched because the user specifically wants a colored interactive interface.
- Added a failing test, observed the missing color-policy function, implemented `enable_tui_colors()`, and verified the test passes.
- Existing scan roots are sorted/deduplicated by `core::volumes::mount_points`; the current ordered palette therefore stays stable across rescans with the same mounts. It can change after adding/removing a mount.
- Fixed a filter-view bug found while verifying safety tags: the root screen was showing volume rows even when the list had switched to cleanup candidates. Disk rows now render only on the unfiltered overview.
- The action card now explains that cleanup unlocks after a live scan, previews Trash actions in demo mode, and lists Back/Tab navigation. Actions still route through the existing key handlers.
- The modal and disk overview are rendered at 64×18, 80×24, and 120×30 in tests. Full test run found one stale expectation for demo Trash wording; corrected to match its read-only preview behavior.

## Verification so far

- `cargo test --locked tui_color_policy_overrides_no_color_environment`: passed after the implementation.
- `cargo test --locked fullness_colors_warn_as_a_disk_fills`: passed after the implementation.
- `cargo fmt --all` and `git diff --check`: passed.
- `cargo test --locked`: 34 passed, 0 failed.
- `cargo build --locked`: passed. It reports one pre-existing dead-code warning for `src/ui/bar.rs::render`.
- Manual PTY verification: demo screen emitted truecolor RGB sequences; live scan immediately showed real `C:\` and `D:\` volumes. Both runs exited cleanly with `q`.
- Final review: changes are limited to the TUI color/render behavior, its tests, and this execution checklist; no dependency or disk-cleanup behavior changed.
- Commit and push as Pavel are the final authorized operations.

## Follow-up visual refinement

- User asked for visible per-folder color variation and the complete command list inside the popup instead of the footer.
- Personal directories now use a stable name-derived accent while Junk/Unused/System safety colors stay higher priority. The footer now only points to the actions menu and quit; the popup retains the full labeled action list.
- `cargo test --locked`: 35 passed. `cargo build --locked`: passed; the old running window had to close before Windows would replace the executable. Relaunched the updated Russian UI in a separate window.

## Final modal and language refinement

- The action menu now dims the already-rendered screen with the terminal DIM attribute, then draws the popup at full brightness.
- English is explicitly the CLI default; the menu retains the language switch.
- `cargo test --locked`: 37 passed. `cargo build --locked`: passed. Relaunched a fresh default-English window and confirmed its process responds.

## Scan performance refinement

- Baseline on the same warm local `target` tree (~27k entries), debug CLI runs: 3.48 s, 2.76 s, 3.17 s (median 3.17 s).
- Replaced whole-path lowercase allocations in category detection with case-insensitive comparisons over path components and the filename. Category precedence and uppercase matching are covered by the existing test.
- After the change, debug runs: 2.47 s, 3.06 s, 2.47 s (median 2.47 s, about 22% faster than baseline). Release runs: 1.97 s, 1.93 s, 1.92 s (median 1.93 s).
- `cargo test --locked`: 37 passed. `cargo build --locked` and `cargo build --locked --release`: passed. The separate window now runs the release binary.

## Navigation usability refinement

- Startup keyboard focus now lands on the disk/item list, so the first Enter opens the selected disk. Tab still switches focus to categories; clicking the category pane updates focus to match.
- Parent navigation now uses a compact index vector maintained as progressive scan batches arrive, replacing a full-tree path search on each Back action.
- `cargo test --locked`: 38 passed. Debug and release builds passed; the fresh release window is running and responsive.
