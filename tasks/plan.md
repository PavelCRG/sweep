# Sweep terminal UI: final visual and command plan

**Goal:** Make the Rust/Ratatui screen read like the supplied disk overview: a long segmented usage band, unmistakable category and drive colors, readable folder/file rows, and a keyboard-operable actions menu.

**Architecture:** Keep the current terminal-only app and its existing scan, browse, and cleanup handlers. Refine the palette and `render_space_map`, then finish the modal menu in `ui::app`/`ui::store`; do not add React, a GUI, or dependencies.

**Design contract:** Black terminal canvas, bright semantic accents, neutral supporting text, no logo, no overlap. Color must reinforce text and symbols, never be the only way to identify an item. Keep `?`/`m` for the menu, arrows plus Enter to choose, Esc to close, and double `d` to confirm Trash.

## 20 implementation points

1. **Lock the screen hierarchy.** Keep the full-width disk overview at top, categories at left, selected category and sorted items at right, selected-item details below, and a two-line command/status footer. Acceptance: each area has one job and none covers another at supported sizes.
2. **Set category color tokens.** Use distinct, bright blue for Projects, green for Downloads, orange for Caches, pink for Media, and cyan for Other. Acceptance: every category has the same color in its label, marker, legend, and bars.
3. **Assign drive accents by mount identity.** Give each discovered mount a deterministic accent that remains stable across rescans; keep drive accents separate from category colors. Acceptance: C: and D: are immediately distinguishable, and rescanning does not swap their colors.
4. **Color every category label.** Remove the gray-only treatment from unselected categories; show each label in its accent and mark selection with bold text plus the existing pointer. Acceptance: category remains identifiable without reading its name closely.
5. **Make sidebar bars legible.** Use a longer, consistent mini-meter per category and keep its number aligned. Acceptance: nonzero small categories remain visible and all values fit the sidebar.
6. **Extend the disk usage band.** Fill the available terminal width with the dotted band from the reference, split proportionally among category colors and free space. Acceptance: segment widths sum exactly to the available width, with no overflow or gaps.
7. **Keep disk totals easy to scan.** Align used, total, free, and percent on one header row; color the percentage by fullness and keep LIVE/READY state explicit. Acceptance: resize does not collide with the title or totals.
8. **Make drive rows distinct.** Color the drive name and its small marker with that drive’s accent, while preserving the multicolor usage meter for what occupies the drive. Acceptance: disk identity and disk contents remain visually distinct concepts.
9. **Differentiate folders and files.** Use a clear folder arrow and file marker, category color for personal items, and bold only for folders. Acceptance: type is visible even without color support.
10. **Prioritize safety tags.** Override category color for Junk (coral), Unused (amber), and System (violet); retain explicit `[JUNK]`, `[UNUSED]`, and `[SYSTEM]` labels. Acceptance: users can understand cleanup state in monochrome and color.
11. **Style the selected row.** Keep a restrained dark highlight, bright readable name, and visible size; avoid turning the whole screen white. Acceptance: the current item is obvious while neighboring categories remain visible.
12. **Color the detail panel consistently.** Use the item accent for name, size, category, and safety tag; keep path and dates subdued and the reason text readable. Acceptance: details visually match the selected row and retain the System deletion block.
13. **Keep the layout responsive.** Verify compact, standard, and wide layouts; shorten labels before allowing overlap and retain the minimum-size message. Acceptance: 64×18, 80×24, and 120×30 terminals have no clipped panels or commands.
14. **Use one centered action card.** Keep the menu over the current view with a dark fill, rounded border, clear selection, and no underlying mouse actions while open. Acceptance: the screen remains recognizable around the modal and clicks cannot change hidden selection.
15. **List the everyday actions.** Include Open, largest-folder jump, largest files, cleanup candidates, Junk/Unused filter, move to Trash, language, rescan, and quit with their key hints. Acceptance: every item names the effect, not just the key.
16. **Make the menu keyboard-complete.** Open with `?` or `m`; move with arrows or `j/k`; run with Enter; close with Esc; allow a listed shortcut directly. Acceptance: focus and selection return to the previous screen after an action.
17. **Route actions through existing handlers.** Do not duplicate scan, browse, filter, language, or cleanup behavior in menu code. Acceptance: menu and direct hotkeys produce the same state changes.
18. **Respect scan and cleanup state.** Hide Rescan while a scan is active or in demo; show unavailable cleanup clearly until the volume is ready; keep Trash behind the existing double-`d` confirmation and System-path block. Acceptance: menu cannot bypass any safety check.
19. **Test behavior and geometry.** Add tests for distinct drive/category colors, proportional band widths, menu item availability during scans, selected action routing, and overlay rendering at 80×24 and 64×18. Acceptance: `cargo test --locked` passes.
20. **Review and ship the final screen.** Run `cargo fmt --all`, tests, `cargo build --locked`, and manual PTY checks at compact/standard/wide sizes in both live scan and completed scan. Then commit and push as `Pavel <pavelslepcov864@gmail.com>`. Acceptance: no overflow, all menu actions work, real drives appear, working tree is clean, and the pushed commit is authored by Pavel.

## Checkpoints

- After points 1–7: color and band tests pass; the 80-column overview is readable.
- After points 8–13: drive, item, detail, and responsive layouts agree.
- After points 14–18: modal menu works and does not weaken cleanup safety.
- After points 19–20: verification passes and the reviewed result is pushed.

## Risks to verify

- True-color output depends on the terminal; keep labels and icons meaningful in 16-color or monochrome modes.
- Disk discovery can return multiple mount points for one physical device; label volumes as mounted roots rather than claiming physical-disk grouping.
- Menu actions during a live scan must reflect what is currently available instead of silently doing nothing.
