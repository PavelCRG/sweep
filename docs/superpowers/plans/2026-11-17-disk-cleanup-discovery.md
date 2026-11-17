# Disk Cleanup and Discovery Implementation Plan

**Goal:** Make cleanup candidates and largest files easy to find in the existing TUI and show the session's total size moved to Trash.

**Architecture:** Reuse the scanned `Node` tree and existing Trash cleanup flow. Add one recursive browse helper, two lightweight view flags, and a reclaimed byte counter in `Store`.

**Tech Stack:** Existing Rust, Ratatui, and `trash` dependency.

**Spec:** The user's approved request to implement the proposed Sweep improvements.

## Global Constraints

- Keep scanning local and use existing scan results.
- Cleanup continues to require confirmation and moves items to Trash.
- Preserve the current English and Russian interface.
- Add no dependencies.

## Review Focus

- Empty and file-only folders must render without selection errors.
- A largest-files view must exclude directories and preserve normal cleanup readiness checks.
- Session Trash totals must not include dry-run moves or failed cleanup.

### Task 1: Add largest-file browsing and cleanup view

**Files:** Modify `src/core/browse.rs`, `src/ui/store.rs`, and `src/ui/app.rs`.

- [x] Recursively collect files under the current node and sort descending by size.
- [x] Add `a` to toggle the largest-files view and `c` to toggle Junk-only cleanup candidates; retain `f` for Junk plus Unused.
- [x] Render the active view in the existing list and update the keyboard hints and view title.
- [x] Keep directory navigation, readiness checks, and double-press Trash confirmation intact.

### Task 2: Show session cleanup total

**Files:** Modify `src/ui/store.rs` and `src/ui/app.rs`.

- [x] Accumulate the selected node's size only after a successful non-dry-run Trash move.
- [x] Show the session total in the status area; do not count dry runs or failures.

## Execution Notes

- Existing disk overview, capacity display, largest folders, and Trash cleanup are retained as-is.
- Scan history is deferred; it requires persistent local storage and was described as a later addition.
