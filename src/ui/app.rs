use crate::{
    core::{self, Node},
    plain::human,
    ui::{bar, lang::Language, store::Store},
};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use std::{
    io,
    sync::mpsc::{self, Receiver},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use unicode_width::UnicodeWidthChar;

type Category = core::categories::Category;

const CATEGORIES: [Category; 5] = [
    Category::ProjectsAndCode,
    Category::Downloads,
    Category::CachesAndTemp,
    Category::MediaAndArchives,
    Category::Other,
];

#[derive(Clone, Default)]
struct CategoryData {
    sizes: [u64; 5],
    items: [Vec<usize>; 5],
}

#[derive(Clone, Copy, Default)]
struct ScreenAreas {
    map: Rect,
    categories: Rect,
    items: Rect,
    item_offset: usize,
    map_segments: [usize; 6],
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

pub fn run(
    nodes: &mut Vec<Node>,
    dry: bool,
    demo: bool,
    lang: Language,
    cancelled: Arc<AtomicBool>,
    initial_roots: Option<Vec<std::path::PathBuf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    ensure_scan_root(nodes);
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut s = Store::new(nodes, lang);
    let mut volume_usage = volume_usages(nodes, demo);
    let progressive_scan = initial_roots
        .as_ref()
        .is_some_and(|roots| independent_roots(roots));
    let initial_scan = initial_roots.is_some();
    let mut scan = initial_roots.map(|roots| {
        if progressive_scan {
            start_initial_scan(roots, Arc::clone(&cancelled))
        } else {
            start_scan(roots, Arc::clone(&cancelled))
        }
    });
    let mut ready_roots: std::collections::HashSet<usize> = if initial_scan {
        std::collections::HashSet::new()
    } else {
        nodes[0].children.iter().copied().collect()
    };
    let root_indices: std::collections::HashMap<_, _> = nodes[0]
        .children
        .iter()
        .map(|&index| (nodes[index].path.clone(), index))
        .collect();
    let mut scan_indices: std::collections::HashMap<
        std::path::PathBuf,
        std::collections::HashMap<std::path::PathBuf, usize>,
    > = std::collections::HashMap::new();
    let mut scan_pending: std::collections::HashMap<
        std::path::PathBuf,
        Vec<core::scan::ScanEntry>,
    > = std::collections::HashMap::new();
    let mut category_data = category_data_by_volume(nodes);
    let mut available_nodes = std::collections::HashSet::new();
    if scan.is_some() {
        s.status = s
            .lang
            .text(
                "Scanning drives · browse each disk as soon as it is ready",
                "Идёт сканирование · открывайте диск, как только он будет готов",
            )
            .into();
    }
    loop {
        let mut scan_finished = false;
        if let Some(job) = &scan {
            let updates: Vec<_> = job.updates.try_iter().collect();
            let mut finished = None;
            for update in updates {
                match update {
                    ScanUpdate::Progress(count) => {
                        s.status = format!(
                            "{} {} · {} {} · Esc {}",
                            s.lang.text("Scanning", "Сканирование"),
                            count.visited,
                            s.lang.text("skipped", "пропущено"),
                            count.skipped,
                            s.lang.text("cancels", "отмена")
                        );
                    }
                    ScanUpdate::RootProgress(path, count) => {
                        s.status = format!(
                            "{} {} · {} {} · {}",
                            s.lang.text("Scanning", "Сканируется"),
                            path.display(),
                            count.visited,
                            s.lang.text("items", "элементов"),
                            s.lang.text("Esc cancels", "Esc — отмена")
                        );
                    }
                    ScanUpdate::RootEntries(path, entries) => {
                        if let Some(&root) = root_indices.get(&path) {
                            let index = scan_indices.entry(path.clone()).or_default();
                            let pending = scan_pending.entry(path).or_default();
                            let added =
                                install_scanned_entries(nodes, root, index, pending, entries);
                            if let Some(volume) = nodes[0].children.iter().position(|&i| i == root)
                            {
                                update_category_data(
                                    &mut category_data[volume],
                                    nodes,
                                    root,
                                    index,
                                    &added,
                                );
                            }
                            available_nodes.extend(added);
                        }
                    }
                    ScanUpdate::RootFinished(path, result) => match result {
                        Ok(_) => {
                            if let Some(&root) = root_indices.get(&path) {
                                ready_roots.insert(root);
                                volume_usage = volume_usages(nodes, demo);
                            }
                        }
                        Err(error) => {
                            s.status = format!(
                                "{}: {error}",
                                s.lang.text("Scan failed", "Ошибка сканирования")
                            );
                        }
                    },
                    ScanUpdate::InitialFinished => {
                        let was_cancelled = cancelled.swap(false, Ordering::Relaxed);
                        s.status = format!(
                            "{} · {}/{} {}",
                            if was_cancelled {
                                s.lang.text("Scan cancelled", "Сканирование отменено")
                            } else {
                                s.lang.text("Scan complete", "Сканирование завершено")
                            },
                            ready_roots.len(),
                            nodes[0].children.len(),
                            s.lang.text("disks ready", "дисков готово")
                        );
                        scan = None;
                        scan_finished = true;
                    }
                    ScanUpdate::Finished(result) => finished = Some(result),
                }
            }
            if let Some(result) = finished {
                scan = None;
                scan_finished = true;
                match result {
                    Ok(updated) => {
                        *nodes = updated;
                        s.current = 0;
                        s.selected = 0;
                        s.status = s
                            .lang
                            .text("Scan refreshed", "Сканирование завершено")
                            .into();
                        ensure_scan_root(nodes);
                        ready_roots = nodes[0].children.iter().copied().collect();
                        volume_usage = volume_usages(nodes, demo);
                        category_data = category_data_by_volume(nodes);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                        cancelled.store(false, Ordering::Relaxed);
                        s.status = s
                            .lang
                            .text("Scan cancelled", "Сканирование отменено")
                            .into();
                    }
                    Err(e) => {
                        s.status = format!(
                            "{}: {e}",
                            s.lang.text("Rescan failed", "Ошибка сканирования")
                        )
                    }
                }
            }
        }
        if scan_finished {
            continue;
        }
        let selected = selected_volume(nodes, s.current);
        let usage = selected
            .and_then(|i| nodes[0].children.iter().position(|&root| root == i))
            .and_then(|i| volume_usage.get(i).copied())
            .unwrap_or_else(|| aggregate_usage(&volume_usage));
        let mut areas = ScreenAreas::default();
        terminal.draw(|f| {
            areas = draw(
                f,
                nodes,
                &s,
                &volume_usage,
                &category_data,
                usage,
                demo,
                scan.is_some(),
            )
        })?;
        if scan.is_some() && !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let input = event::read()?;
        if let Event::Mouse(mouse) = &input {
            if !s.menu_open {
                handle_mouse(nodes, &mut s, &category_data, areas, *mouse);
            }
            continue;
        }
        if let Event::Key(mut k) = input {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            if s.menu_open {
                let actions = menu_actions(s.lang, scan.is_some(), demo);
                s.menu_selected = s.menu_selected.min(actions.len().saturating_sub(1));
                match k.code {
                    KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('m') => {
                        s.menu_open = false;
                        continue;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        s.menu_selected = s.menu_selected.saturating_sub(1);
                        continue;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        s.menu_selected = (s.menu_selected + 1).min(actions.len() - 1);
                        continue;
                    }
                    KeyCode::Enter => {
                        k.code = actions[s.menu_selected].0;
                        s.menu_open = false;
                    }
                    code if actions.iter().any(|(action, _, _)| *action == code) => {
                        s.menu_open = false;
                    }
                    _ => continue,
                }
            } else if matches!(k.code, KeyCode::Char('?') | KeyCode::Char('m')) {
                s.menu_open = true;
                s.menu_selected = 0;
                continue;
            }
            if let Some(job) = &scan {
                if k.code == KeyCode::Esc
                    || (k.code == KeyCode::Char('c')
                        && k.modifiers.contains(event::KeyModifiers::CONTROL))
                {
                    job.cancelled.store(true, Ordering::Relaxed);
                    s.status = "Cancelling scan…".into();
                } else if k.code == KeyCode::Char('q') {
                    job.cancelled.store(true, Ordering::Relaxed);
                    break;
                } else {
                    match k.code {
                        KeyCode::Down | KeyCode::Char('j') => {
                            if s.focus_right {
                                let shown = visible_children(nodes, &s, &category_data);
                                if !shown.is_empty() {
                                    s.selected = (s.selected + 1).min(shown.len() - 1);
                                }
                            } else {
                                s.selected_category =
                                    (s.selected_category + 1).min(CATEGORIES.len() - 1);
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if s.focus_right {
                                s.selected = s.selected.saturating_sub(1);
                            } else {
                                s.selected_category = s.selected_category.saturating_sub(1);
                            }
                        }
                        KeyCode::Tab => s.focus_right = !s.focus_right,
                        KeyCode::Char('l') => s.lang.toggle(),
                        KeyCode::Char('a') => {
                            s.largest_files = !s.largest_files;
                            s.junk_only = false;
                            s.only_junk = false;
                            s.focus_right = true;
                            s.selected = 0;
                        }
                        KeyCode::Char('c') => {
                            s.junk_only = !s.junk_only;
                            s.largest_files = false;
                            s.only_junk = false;
                            s.focus_right = true;
                            s.selected = 0;
                        }
                        KeyCode::Char('f') => {
                            s.only_junk = !s.only_junk;
                            s.junk_only = false;
                            s.largest_files = false;
                            s.focus_right = true;
                            s.selected = 0;
                        }
                        KeyCode::Enter | KeyCode::Right => {
                            if !s.focus_right {
                                s.focus_right = true;
                                s.selected = 0;
                            } else {
                                let shown = visible_children(nodes, &s, &category_data);
                                if let Some(&item) = shown.get(s.selected) {
                                    let root = selected_volume(nodes, item);
                                    if nodes[item].directory
                                        && (available_nodes.contains(&item)
                                            || root.is_some_and(|root| ready_roots.contains(&root)))
                                    {
                                        s.current = item;
                                        s.selected = 0;
                                        s.selected_category = category_index(nodes[item].category);
                                        continue;
                                    }
                                    s.status = s
                                        .lang
                                        .text(
                                            "This disk is still being scanned",
                                            "Этот диск ещё сканируется",
                                        )
                                        .into();
                                }
                            }
                        }
                        KeyCode::Left => {
                            if s.focus_right && s.current != 0 {
                                go_up(nodes, &mut s);
                            } else {
                                s.focus_right = false;
                            }
                        }
                        KeyCode::Char('g') => {
                            if s.focus_right {
                                let shown = visible_children(nodes, &s, &category_data);
                                if let Some(&item) = shown.get(s.selected) {
                                    if let Some(culprit) = core::browse::culprit(nodes, item) {
                                        s.current = culprit;
                                        s.selected = 0;
                                        s.selected_category =
                                            category_index(nodes[culprit].category);
                                    }
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            s.status = s
                                .lang
                                .text(
                                    "Cleanup unlocks when this disk finishes scanning",
                                    "Очистка станет доступна после сканирования этого диска",
                                )
                                .into();
                        }
                        _ => {}
                    }
                }
                continue;
            }
            let shown = visible_children(nodes, &s, &category_data);
            match k.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('c') if k.modifiers.contains(event::KeyModifiers::CONTROL) => break,
                KeyCode::Down | KeyCode::Char('j') => {
                    if s.focus_right {
                        if !shown.is_empty() {
                            s.selected = (s.selected + 1).min(shown.len() - 1);
                        }
                    } else {
                        s.selected_category = (s.selected_category + 1).min(CATEGORIES.len() - 1);
                    }
                    s.confirm_delete = false;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if s.focus_right {
                        s.selected = s.selected.saturating_sub(1);
                    } else {
                        s.selected_category = s.selected_category.saturating_sub(1);
                    }
                    s.confirm_delete = false;
                }
                KeyCode::Right | KeyCode::Enter => {
                    if !s.focus_right {
                        s.focus_right = true;
                        s.selected = 0;
                    } else if let Some(&i) = shown.get(s.selected) {
                        if nodes[i].directory {
                            s.current = i;
                            s.selected = 0;
                            s.selected_category = category_index(nodes[i].category);
                            s.confirm_delete = false;
                        }
                    }
                }
                KeyCode::Left | KeyCode::Esc => {
                    if s.focus_right && s.current != 0 {
                        go_up(nodes, &mut s);
                    } else {
                        s.focus_right = false;
                    }
                    s.confirm_delete = false;
                }
                KeyCode::Char('g') => {
                    if s.focus_right {
                        if let Some(&i) = shown.get(s.selected) {
                            if let Some(c) = core::browse::culprit(nodes, i) {
                                s.current = c;
                                s.selected = 0;
                                s.selected_category = category_index(nodes[c].category);
                                s.confirm_delete = false;
                            }
                        }
                    }
                }
                KeyCode::Char('f') => {
                    s.only_junk = !s.only_junk;
                    s.junk_only = false;
                    s.selected = 0;
                    s.focus_right = true;
                    s.confirm_delete = false;
                }
                KeyCode::Char('c') => {
                    s.junk_only = !s.junk_only;
                    s.largest_files = false;
                    s.only_junk = false;
                    s.selected = 0;
                    s.focus_right = true;
                    s.confirm_delete = false;
                }
                KeyCode::Char('a') => {
                    s.largest_files = !s.largest_files;
                    s.junk_only = false;
                    s.only_junk = false;
                    s.selected = 0;
                    s.focus_right = true;
                    s.confirm_delete = false;
                }
                KeyCode::Char('l') => {
                    s.lang.toggle();
                    s.status = s.lang.text("Language: English", "Язык: русский").into();
                    s.confirm_delete = false;
                }
                KeyCode::Char('r') => {
                    if demo {
                        s.status = s
                            .lang
                            .text("Demo data is up to date", "Демо-данные актуальны")
                            .into();
                    } else {
                        s.confirm_delete = false;
                        let roots = scan_roots(nodes);
                        cancelled.store(false, Ordering::Relaxed);
                        scan = Some(start_scan(roots, Arc::clone(&cancelled)));
                        s.status = s
                            .lang
                            .text("Scanning…  Esc cancels", "Сканирование…  Esc — отмена")
                            .into();
                    }
                }
                KeyCode::Tab => s.focus_right = !s.focus_right,
                KeyCode::Char('d') => {
                    if !s.focus_right {
                        s.status = s
                            .lang
                            .text(
                                "Select an item in the right pane first",
                                "Сначала выберите элемент справа",
                            )
                            .into();
                    } else if let Some(&i) = shown.get(s.selected) {
                        if !ready_to_clean(nodes, i, &ready_roots) {
                            s.status = s
                                .lang
                                .text(
                                    "Wait for this disk to finish scanning before cleanup",
                                    "Дождитесь завершения сканирования диска перед очисткой",
                                )
                                .into();
                            s.confirm_delete = false;
                        } else if nodes[0].name == "All disks" && nodes[0].children.contains(&i) {
                            s.status = s
                                .lang
                                .text(
                                    "A scan root cannot be moved to Trash",
                                    "Корень диска нельзя переместить в корзину",
                                )
                                .into();
                            s.confirm_delete = false;
                        } else if !s.confirm_delete {
                            s.confirm_delete = true;
                            s.status = s
                                .lang
                                .text(
                                    "Press d again to move this item to Trash",
                                    "Нажмите d ещё раз, чтобы переместить в корзину",
                                )
                                .into();
                        } else {
                            match core::clean::clean(&nodes[i], dry) {
                                Ok(_) => {
                                    s.status = if dry {
                                        format!(
                                            "{} {}",
                                            s.lang.text(
                                                "Would move to Trash:",
                                                "Будет перемещено в корзину:"
                                            ),
                                            nodes[i].path.display()
                                        )
                                    } else {
                                        format!(
                                            "{} {}",
                                            s.lang.text("Moved to Trash:", "Перемещено в корзину:"),
                                            nodes[i].name
                                        )
                                    };
                                    if !dry {
                                        s.trashed = s.trashed.saturating_add(nodes[i].size);
                                        cancelled.store(false, Ordering::Relaxed);
                                        scan = Some(start_scan(
                                            scan_roots(nodes),
                                            Arc::clone(&cancelled),
                                        ));
                                    }
                                }
                                Err(e) => {
                                    let message = e.to_string();
                                    s.status = if message.starts_with("System paths") {
                                        s.lang
                                            .text(
                                                "System paths cannot be deleted",
                                                "Системные пути нельзя удалять",
                                            )
                                            .into()
                                    } else {
                                        message
                                    };
                                }
                            }
                            s.confirm_delete = false;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(job) = scan {
        job.cancelled.store(true, Ordering::Relaxed);
    }
    Ok(())
}

enum ScanUpdate {
    Progress(core::scan::ScanProgress),
    RootProgress(std::path::PathBuf, core::scan::ScanProgress),
    RootEntries(std::path::PathBuf, Vec<core::scan::ScanEntry>),
    RootFinished(
        std::path::PathBuf,
        std::io::Result<core::scan::ScanProgress>,
    ),
    InitialFinished,
    Finished(std::io::Result<Vec<Node>>),
}

struct ScanJob {
    updates: Receiver<ScanUpdate>,
    cancelled: Arc<AtomicBool>,
}

fn start_scan(roots: Vec<std::path::PathBuf>, cancelled: Arc<AtomicBool>) -> ScanJob {
    let (updates_tx, updates) = mpsc::sync_channel(16);
    let progress_tx = updates_tx.clone();
    let worker_cancelled = Arc::clone(&cancelled);
    thread::spawn(move || {
        let result = core::scan::scan_many_with_progress(&roots, &worker_cancelled, |count| {
            if count.visited % 250 == 0 {
                let _ = progress_tx.send(ScanUpdate::Progress(count));
            }
        })
        .map(|mut nodes| {
            ensure_scan_root(&mut nodes);
            nodes
        });
        let _ = updates_tx.send(ScanUpdate::Finished(result));
    });
    ScanJob { updates, cancelled }
}

fn start_initial_scan(roots: Vec<std::path::PathBuf>, cancelled: Arc<AtomicBool>) -> ScanJob {
    let (updates_tx, updates) = mpsc::sync_channel(16);
    let worker_cancelled = Arc::clone(&cancelled);
    thread::spawn(move || {
        let roots = Arc::new(roots);
        let next_root = Arc::new(AtomicUsize::new(0));
        let workers: Vec<_> = (0..roots.len().min(4))
            .map(|_| {
                let roots = Arc::clone(&roots);
                let next_root = Arc::clone(&next_root);
                let cancelled = Arc::clone(&worker_cancelled);
                let updates = updates_tx.clone();
                thread::spawn(move || loop {
                    let index = next_root.fetch_add(1, Ordering::Relaxed);
                    let Some(root) = roots.get(index).cloned() else {
                        break;
                    };
                    let progress = updates.clone();
                    let entries = updates.clone();
                    let result = core::scan::scan_one_with_updates(
                        &root,
                        &cancelled,
                        |count| {
                            if count.visited % 250 == 0 {
                                let _ =
                                    progress.send(ScanUpdate::RootProgress(root.clone(), count));
                            }
                        },
                        |batch| {
                            let _ = entries.send(ScanUpdate::RootEntries(root.clone(), batch));
                        },
                    );
                    let _ = updates.send(ScanUpdate::RootFinished(root, result));
                })
            })
            .collect();
        for worker in workers {
            let _ = worker.join();
        }
        let _ = updates_tx.send(ScanUpdate::InitialFinished);
    });
    ScanJob { updates, cancelled }
}

fn independent_roots(roots: &[std::path::PathBuf]) -> bool {
    roots.iter().enumerate().all(|(index, root)| {
        roots
            .iter()
            .enumerate()
            .all(|(other_index, other)| index == other_index || !other.starts_with(root))
    })
}

fn install_scanned_entries(
    nodes: &mut Vec<Node>,
    root: usize,
    paths: &mut std::collections::HashMap<std::path::PathBuf, usize>,
    pending: &mut Vec<core::scan::ScanEntry>,
    entries: Vec<core::scan::ScanEntry>,
) -> Vec<usize> {
    pending.extend(entries);
    let mut available = Vec::with_capacity(pending.len());
    loop {
        let mut waiting = Vec::new();
        let mut installed = 0;
        for mut entry in std::mem::take(pending) {
            let Some(parent) = entry.parent.as_ref() else {
                let path = entry.node.path.clone();
                nodes[root] = entry.node;
                paths.insert(path, root);
                available.push(root);
                installed += 1;
                continue;
            };
            let Some(&parent_index) = paths.get(parent) else {
                waiting.push(entry);
                continue;
            };
            if matches!(
                nodes[parent_index].tag,
                core::classify::Tag::System | core::classify::Tag::Junk
            ) {
                entry.node.tag = nodes[parent_index].tag;
                entry.node.reason = nodes[parent_index].reason.clone();
            }
            let size = entry.node.size;
            let files = entry.node.files;
            let path = entry.node.path.clone();
            let index = nodes.len();
            nodes[parent_index].children.push(index);
            nodes.push(entry.node);
            paths.insert(path, index);
            available.push(index);

            let mut ancestor = Some(parent_index);
            while let Some(index) = ancestor {
                nodes[index].size = nodes[index].size.saturating_add(size);
                nodes[index].files = nodes[index].files.saturating_add(files);
                if index == root {
                    break;
                }
                ancestor = nodes[index]
                    .path
                    .parent()
                    .and_then(|parent| paths.get(parent))
                    .copied();
            }
            nodes[0].size = nodes[0].size.saturating_add(size);
            nodes[0].files = nodes[0].files.saturating_add(files);
            installed += 1;
        }
        *pending = waiting;
        if installed == 0 {
            break;
        }
    }
    available
}

pub fn pending_tree(roots: &[std::path::PathBuf]) -> Vec<Node> {
    let mut nodes = vec![Node {
        path: std::path::PathBuf::from("<all disks>"),
        name: "All disks".into(),
        size: 0,
        files: 0,
        directory: true,
        atime: None,
        mtime: None,
        children: (1..=roots.len()).collect(),
        tag: core::classify::Tag::Personal,
        reason: None,
        category: core::categories::Category::Other,
    }];
    nodes.extend(roots.iter().map(|path| {
        Node {
            path: path.clone(),
            name: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .trim_end_matches(['\\', '/'])
                .to_owned(),
            size: 0,
            files: 0,
            directory: true,
            atime: None,
            mtime: None,
            children: vec![],
            tag: core::classify::Tag::Personal,
            reason: None,
            category: core::categories::Category::Other,
        }
    }));
    nodes
}

fn scan_roots(nodes: &[Node]) -> Vec<std::path::PathBuf> {
    if nodes[0].name == "All disks" {
        nodes[0]
            .children
            .iter()
            .map(|&i| nodes[i].path.clone())
            .collect()
    } else {
        vec![nodes[0].path.clone()]
    }
}

fn ensure_scan_root(nodes: &mut Vec<Node>) {
    if nodes.is_empty() || nodes[0].name == "All disks" {
        return;
    }
    let root = nodes[0].clone();
    for node in nodes.iter_mut() {
        for child in &mut node.children {
            *child += 1;
        }
    }
    nodes.insert(
        0,
        Node {
            path: std::path::PathBuf::from("<all disks>"),
            name: "All disks".into(),
            size: root.size,
            files: root.files,
            directory: true,
            atime: None,
            mtime: None,
            children: vec![1],
            tag: core::classify::Tag::Personal,
            reason: None,
            category: core::categories::Category::Other,
        },
    );
}

fn volume_usages(nodes: &[Node], demo: bool) -> Vec<bar::Usage> {
    nodes[0]
        .children
        .iter()
        .map(|&root| usage_for_volume(nodes, root, demo))
        .collect()
}

fn usage_for_volume(nodes: &[Node], root: usize, demo: bool) -> bar::Usage {
    let (total, free) = if demo && nodes[root].name == "System volume" {
        (160_000_000_000, 24_000_000_000)
    } else if demo && nodes[root].name == "Media volume" {
        (140_000_000_000, 20_000_000_000)
    } else if demo {
        (300_000_000_000, 44_000_000_000)
    } else {
        (
            fs2::total_space(&nodes[root].path).unwrap_or(0),
            fs2::available_space(&nodes[root].path).unwrap_or(0),
        )
    };
    let used = total.saturating_sub(free.min(total));
    let scanned = nodes[root].size.min(used);
    let (junk, unused) = classified_sizes(nodes, root);
    let junk = junk.min(scanned);
    let unused = unused.min(scanned.saturating_sub(junk));
    bar::Usage {
        personal: scanned.saturating_sub(junk + unused),
        junk,
        unused,
        other: used.saturating_sub(scanned),
        free: free.min(total),
    }
}

fn aggregate_usage(usages: &[bar::Usage]) -> bar::Usage {
    usages
        .iter()
        .fold(bar::Usage::default(), |mut total, usage| {
            total.personal = total.personal.saturating_add(usage.personal);
            total.junk = total.junk.saturating_add(usage.junk);
            total.unused = total.unused.saturating_add(usage.unused);
            total.other = total.other.saturating_add(usage.other);
            total.free = total.free.saturating_add(usage.free);
            total
        })
}

fn selected_volume(nodes: &[Node], current: usize) -> Option<usize> {
    if current == 0 {
        return None;
    }
    nodes[0]
        .children
        .iter()
        .copied()
        .find(|&root| nodes[current].path.starts_with(&nodes[root].path))
}

fn category_index(category: Category) -> usize {
    CATEGORIES
        .iter()
        .position(|&item| item == category)
        .unwrap_or(4)
}

fn category_name(category: Category, lang: Language) -> &'static str {
    match category {
        Category::ProjectsAndCode => lang.text("Projects & code", "Проекты и код"),
        Category::Downloads => lang.text("Downloads", "Загрузки"),
        Category::CachesAndTemp => lang.text("Caches & temp", "Кэши и временные"),
        Category::MediaAndArchives => lang.text("Media & archives", "Медиа и архивы"),
        Category::Other => lang.text("Other", "Другое"),
    }
}

fn short_category_name(category: Category, lang: Language) -> &'static str {
    match category {
        Category::ProjectsAndCode => lang.text("Code", "РљРѕРґ"),
        Category::Downloads => lang.text("Down", "Р—Р°РіСЂ."),
        Category::CachesAndTemp => lang.text("Caches", "РљСЌС€Рё"),
        Category::MediaAndArchives => lang.text("Media", "РњРµРґРёР°"),
        Category::Other => lang.text("Other", "Р”СЂСѓРіРѕРµ"),
    }
}

fn category_color(category: Category) -> Color {
    match category {
        Category::ProjectsAndCode => Color::Rgb(73, 157, 255),
        Category::Downloads => Color::Rgb(50, 220, 135),
        Category::CachesAndTemp => Color::Rgb(255, 130, 48),
        Category::MediaAndArchives => Color::Rgb(245, 65, 170),
        Category::Other => Color::Rgb(44, 220, 218),
    }
}

fn volume_color(index: usize) -> Color {
    const COLORS: [Color; 6] = [
        Color::Rgb(72, 166, 255),
        Color::Rgb(70, 210, 160),
        Color::Rgb(196, 120, 255),
        Color::Rgb(255, 164, 78),
        Color::Rgb(255, 105, 145),
        Color::Rgb(245, 210, 90),
    ];
    COLORS[index % COLORS.len()]
}

fn item_color(tag: core::classify::Tag, category: Category) -> Color {
    match tag {
        core::classify::Tag::Junk => Color::Rgb(255, 102, 102),
        core::classify::Tag::Unused => Color::Rgb(255, 196, 87),
        core::classify::Tag::System => Color::Rgb(185, 135, 255),
        core::classify::Tag::Personal => category_color(category),
    }
}

fn category_data_by_volume(nodes: &[Node]) -> Vec<CategoryData> {
    nodes[0]
        .children
        .iter()
        .map(|&root| {
            let mut data = CategoryData::default();
            let mut stack: Vec<(usize, Option<usize>)> = vec![(root, None)];
            while let Some((index, parent)) = stack.pop() {
                let node = &nodes[index];
                let category = category_index(node.category);
                if let Some(parent) = parent {
                    if nodes[parent].category != node.category {
                        data.items[category].push(index);
                    }
                } else {
                    data.items[4].push(index);
                }
                if node.directory {
                    stack.extend(node.children.iter().map(|&child| (child, Some(index))));
                } else {
                    data.sizes[category] = data.sizes[category].saturating_add(node.size);
                }
            }
            data
        })
        .collect()
}

fn update_category_data(
    data: &mut CategoryData,
    nodes: &[Node],
    root: usize,
    paths: &std::collections::HashMap<std::path::PathBuf, usize>,
    added: &[usize],
) {
    for &index in added {
        let node = &nodes[index];
        let category = category_index(node.category);
        if index == root {
            if !data.items[4].contains(&root) {
                data.items[4].push(root);
            }
        } else if node
            .path
            .parent()
            .and_then(|parent| paths.get(parent))
            .is_some_and(|&parent| nodes[parent].category != node.category)
        {
            data.items[category].push(index);
        }
        if !node.directory {
            data.sizes[category] = data.sizes[category].saturating_add(node.size);
        }
    }
}

fn aggregate_category_data(data: &[CategoryData]) -> CategoryData {
    let mut all = CategoryData::default();
    for volume in data {
        for index in 0..CATEGORIES.len() {
            all.sizes[index] = all.sizes[index].saturating_add(volume.sizes[index]);
            all.items[index].extend(volume.items[index].iter().copied());
        }
    }
    all
}

fn category_items(nodes: &[Node], store: &Store, data: &[CategoryData]) -> Vec<usize> {
    let category = CATEGORIES[store.selected_category.min(CATEGORIES.len() - 1)];
    let mut items = if store.junk_only {
        core::browse::cleanup_candidates(nodes, store.current)
    } else if store.largest_files {
        core::browse::largest_files(nodes, store.current)
    } else if store.current == 0 {
        nodes[0].children.clone()
    } else if selected_volume(nodes, store.current) == Some(store.current)
        && category != Category::Other
    {
        let root = nodes[store.current].path.clone();
        let volume = nodes[0]
            .children
            .iter()
            .position(|&i| nodes[i].path == root);
        volume
            .and_then(|i| data.get(i))
            .map(|volume| volume.items[category_index(category)].clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|&i| nodes[i].path.starts_with(&root))
            .collect()
    } else {
        nodes[store.current]
            .children
            .iter()
            .copied()
            .filter(|&i| nodes[i].category == category)
            .collect()
    };
    items.retain(|&i| !store.only_junk || cleanup_candidate(nodes, i));
    items.sort_by_key(|&i| std::cmp::Reverse(nodes[i].size));
    items
}

fn current_category_data(nodes: &[Node], store: &Store, data: &[CategoryData]) -> CategoryData {
    if store.current == 0 {
        return aggregate_category_data(data);
    }
    let Some(volume) = selected_volume(nodes, store.current) else {
        return CategoryData::default();
    };
    nodes[0]
        .children
        .iter()
        .position(|&root| root == volume)
        .and_then(|index| data.get(index))
        .cloned()
        .unwrap_or_default()
}

fn render_space_map(
    f: &mut Frame,
    area: Rect,
    title: &str,
    usage: bar::Usage,
    data: &CategoryData,
    selected: usize,
    scanning: bool,
    lang: Language,
) -> [usize; 6] {
    let inner_width = area.width.saturating_sub(2) as usize;
    let segments = map_widths(data, usage, inner_width);
    let percent_full = percent(usage.used(), usage.total());
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!(" {} ", human(usage.used())),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            lang.text("used of", "занято из"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!(" {}  ·  ", human(usage.total())),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            format!(
                "{} {}  ·  {}%",
                human(usage.free),
                lang.text("free", "свободно"),
                percent_full
            ),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            if scanning {
                "  ·  LIVE"
            } else {
                "  ·  READY"
            },
            Style::default().fg(if scanning {
                Color::Yellow
            } else {
                Color::Green
            }),
        ),
    ])];
    let glyphs = ["●", "●", "●", "●", "●", "·"];
    for _ in 0..3 {
        let mut row = Vec::new();
        for (index, width) in segments.iter().enumerate() {
            let color = if index == selected.min(4) {
                if index == 5 {
                    Color::DarkGray
                } else {
                    category_color(CATEGORIES[index])
                }
            } else if index == 5 || data.sizes.get(index).copied().unwrap_or(0) == 0 {
                Color::Rgb(40, 45, 52)
            } else {
                category_color(CATEGORIES[index])
            };
            row.push(Span::styled(
                glyphs[index].repeat(*width),
                Style::default().fg(color),
            ));
        }
        lines.push(Line::from(row));
    }
    let mut legend = Vec::new();
    for (index, category) in CATEGORIES.iter().enumerate() {
        legend.push(Span::styled(
            "■ ",
            Style::default().fg(category_color(*category)),
        ));
        let label = if inner_width < 100 {
            short_category_name(*category, lang)
        } else {
            category_name(*category, lang)
        };
        legend.push(Span::styled(
            format!("{}  ", label),
            Style::default()
                .fg(category_color(*category))
                .add_modifier(if index == selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
    }
    lines.push(Line::from(legend));
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(format!(" {title}  ·  SPACE MAP "))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
    segments
}

fn handle_mouse(
    nodes: &[Node],
    store: &mut Store,
    data: &[CategoryData],
    areas: ScreenAreas,
    mouse: event::MouseEvent,
) {
    if !matches!(
        mouse.kind,
        MouseEventKind::Moved | MouseEventKind::Down(MouseButton::Left)
    ) {
        return;
    }
    let x = mouse.column;
    let y = mouse.row;
    if areas.categories.contains(Position { x, y }) {
        let row = y.saturating_sub(areas.categories.y + 1) as usize;
        if row < CATEGORIES.len() {
            store.selected_category = row;
            store.selected = 0;
            store.confirm_delete = false;
        }
    } else if areas.map.contains(Position { x, y }) {
        let offset = x.saturating_sub(areas.map.x + 1) as usize;
        let mut edge = 0;
        for index in 0..5 {
            edge += areas.map_segments[index];
            if offset < edge {
                store.selected_category = index;
                store.selected = 0;
                break;
            }
        }
    } else if areas.items.contains(Position { x, y }) {
        let shown = category_items(nodes, store, data);
        let index = y.saturating_sub(areas.items.y + 1) as usize + areas.item_offset;
        if index < shown.len() {
            store.selected = index;
            store.focus_right = true;
            store.confirm_delete = false;
        }
    }
}

fn go_up(nodes: &[Node], store: &mut Store) {
    store.current = nodes[store.current]
        .path
        .parent()
        .and_then(|parent| nodes.iter().position(|node| node.path == parent))
        .unwrap_or(0);
    store.selected = 0;
    store.confirm_delete = false;
}

fn map_widths(data: &CategoryData, usage: bar::Usage, width: usize) -> [usize; 6] {
    let mut amounts = data.sizes.to_vec();
    let classified = data
        .sizes
        .iter()
        .fold(0u64, |sum, size| sum.saturating_add(*size));
    amounts.push(usage.used().saturating_sub(classified));
    let total = amounts.iter().map(|&v| u128::from(v)).sum::<u128>();
    let mut widths = [0usize; 6];
    if total == 0 {
        return widths;
    }
    let mut remainders = [(0usize, 0u128); 6];
    let mut allocated = 0;
    for (index, amount) in amounts.iter().enumerate() {
        let numerator = width as u128 * u128::from(*amount);
        widths[index] = (numerator / total) as usize;
        allocated += widths[index];
        remainders[index] = (index, numerator % total);
    }
    remainders
        .sort_by_key(|&(index, remainder)| (amounts[index] == 0, std::cmp::Reverse(remainder)));
    for &(index, _) in remainders.iter().take(width.saturating_sub(allocated)) {
        widths[index] += 1;
    }
    widths
}

fn classified_sizes(nodes: &[Node], i: usize) -> (u64, u64) {
    match nodes[i].tag {
        core::classify::Tag::Junk => (nodes[i].size, 0),
        core::classify::Tag::Unused => (0, nodes[i].size),
        _ => nodes[i].children.iter().fold((0, 0), |(j, u), &c| {
            let (cj, cu) = classified_sizes(nodes, c);
            (j.saturating_add(cj), u.saturating_add(cu))
        }),
    }
}

fn menu_actions(
    lang: Language,
    scanning: bool,
    demo: bool,
) -> Vec<(KeyCode, &'static str, &'static str)> {
    let mut actions = vec![
        (
            KeyCode::Enter,
            "↵",
            lang.text("Open selected item", "Открыть выбранное"),
        ),
        (
            KeyCode::Char('g'),
            "g",
            lang.text("Go to largest folder", "Перейти в большую папку"),
        ),
        (
            KeyCode::Char('a'),
            "a",
            lang.text("Show largest files", "Крупнейшие файлы"),
        ),
        (
            KeyCode::Char('c'),
            "c",
            lang.text("Review cleanup candidates", "Кандидаты на очистку"),
        ),
        (
            KeyCode::Char('f'),
            "f",
            lang.text("Toggle Junk + Unused filter", "Фильтр мусора и старого"),
        ),
        (
            KeyCode::Char('d'),
            "d",
            lang.text("Move selected item to Trash", "Переместить в корзину"),
        ),
        (
            KeyCode::Char('l'),
            "l",
            lang.text("Switch interface language", "Сменить язык интерфейса"),
        ),
    ];
    if !scanning && !demo {
        actions.push((
            KeyCode::Char('r'),
            "r",
            lang.text("Rescan disks", "Пересканировать диски"),
        ));
    }
    actions.push((
        KeyCode::Char('q'),
        "q",
        lang.text("Quit sweep", "Выйти из sweep"),
    ));
    actions
}

fn draw_action_menu(
    f: &mut Frame,
    area: Rect,
    actions: &[(KeyCode, &'static str, &'static str)],
    selected: usize,
    lang: Language,
) {
    let width = area.width.min(58);
    let height = (actions.len() as u16 + 3).min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(lang.text(" ACTIONS  ·  ? to close ", " ДЕЙСТВИЯ  ·  ? закрыть "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(80, 190, 255)))
        .style(Style::default().bg(Color::Rgb(12, 16, 22)));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let rows = actions
        .iter()
        .map(|(_, key, label)| {
            let color = match *key {
                "d" => Color::Rgb(255, 102, 102),
                "c" => Color::Rgb(255, 196, 87),
                "r" => Color::Rgb(70, 210, 160),
                _ => Color::Rgb(80, 190, 255),
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {:>2} ", key),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {label}"), Style::default().fg(Color::White)),
            ]))
        })
        .collect::<Vec<_>>();
    let content = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let mut state = ListState::default();
    if !actions.is_empty() {
        state.select(Some(selected.min(actions.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(rows).highlight_symbol("› ").highlight_style(
            Style::default()
                .bg(Color::Rgb(26, 42, 57))
                .add_modifier(Modifier::BOLD),
        ),
        content[0],
        &mut state,
    );
    f.render_widget(
        Paragraph::new(lang.text(
            "↑/↓ choose  ·  Enter run  ·  Esc close",
            "↑/↓ выбор  ·  Enter выполнить  ·  Esc закрыть",
        ))
        .style(Style::default().fg(Color::DarkGray)),
        content[1],
    );
}

fn draw(
    f: &mut Frame,
    nodes: &[Node],
    s: &Store,
    volume_usage: &[bar::Usage],
    category_data: &[CategoryData],
    usage: bar::Usage,
    demo: bool,
    scanning: bool,
) -> ScreenAreas {
    let area = f.area();
    if area.width < 64 || area.height < 18 {
        f.render_widget(
            Paragraph::new(s.lang.text(
                "Resize the terminal to at least 64 × 18 to browse disks.",
                "РЈРІРµР»РёС‡СЊС‚Рµ С‚РµСЂРјРёРЅР°Р» РґРѕ 64 × 18 РґР»СЏ РїСЂРѕСЃРјРѕС‚СЂР° РґРёСЃРєРѕРІ.",
            ))
            .block(
                Block::default()
                    .title(s.lang.text(" Resize required ", " РќСѓР¶РЅРѕ СѓРІРµР»РёС‡РёС‚СЊ "))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Center),
            area,
        );
        return ScreenAreas::default();
    }
    let overview = s.current == 0;
    let current_path = nodes[s.current].path.display().to_string();
    let title = if overview {
        s.lang.text("Disk overview", "Обзор дисков")
    } else {
        &current_path
    };
    let map_title = if scanning {
        format!("{title} · Live scan")
    } else {
        title.to_owned()
    };
    let layout = Layout::vertical([
        Constraint::Length(7),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .split(area);
    let map_data = current_category_data(nodes, s, category_data);
    let map_segments = render_space_map(
        f,
        layout[0],
        &map_title,
        usage,
        &map_data,
        s.selected_category,
        scanning,
        s.lang,
    );
    let compact = area.width < 110;
    let panes = Layout::horizontal(if compact {
        [Constraint::Length(23), Constraint::Min(32)]
    } else {
        [Constraint::Length(31), Constraint::Min(60)]
    })
    .split(layout[1]);
    let show_details = layout[1].height >= 13;
    let content = if show_details {
        Layout::vertical([Constraint::Min(5), Constraint::Length(7)]).split(panes[1])
    } else {
        Layout::vertical([Constraint::Min(1), Constraint::Length(0)]).split(panes[1])
    };
    let shown = category_items(nodes, s, category_data);
    let category = CATEGORIES[s.selected_category.min(CATEGORIES.len() - 1)];
    let folders = shown.iter().filter(|&&i| nodes[i].directory).count();
    let files = shown.len() - folders;
    let list_title = if s.largest_files {
        format!(" {} ", s.lang.text("LARGEST FILES", "КРУПНЕЙШИЕ ФАЙЛЫ"))
    } else if s.junk_only {
        format!(
            " {}  /  {}",
            s.lang.text("CLEANUP CANDIDATES", "К ОЧИСТКЕ"),
            shown.len()
        )
    } else if s.only_junk {
        format!(
            " {}  /  {} {}  ·  {} {} ",
            s.lang.text("JUNK + UNUSED", "МУСОР + СТАРОЕ"),
            folders,
            s.lang.text("folders", "папок"),
            files,
            s.lang.text("files", "файлов")
        )
    } else {
        format!(
            " {}  ·  {folders} {}  /  {files} {} ",
            category_name(category, s.lang),
            s.lang.text("folders", "папок"),
            s.lang.text("files", "файлов")
        )
    };
    let items = shown
        .iter()
        .map(|&i| {
            let n = &nodes[i];
            let color = item_color(n.tag, n.category);
            let (tag, color) = match n.tag {
                core::classify::Tag::Junk => (s.lang.text("JUNK", "МУСОР"), color),
                core::classify::Tag::Unused => (s.lang.text("UNUSED", "СТАРОЕ"), color),
                core::classify::Tag::System => (s.lang.text("SYSTEM", "СИСТЕМА"), color),
                _ => ("", color),
            };
            let name_width = if compact { 10 } else { 20 };
            let name = if overview {
                volume_name(n, demo, s.lang)
            } else if n.directory {
                format!("▸ {}", n.name)
            } else {
                format!("· {}", n.name)
            };
            let name_color = if overview {
                nodes[0]
                    .children
                    .iter()
                    .position(|&root| root == i)
                    .map(volume_color)
                    .unwrap_or(color)
            } else {
                color
            };
            let name_style =
                Style::default()
                    .fg(name_color)
                    .add_modifier(if overview || n.directory {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
            let mut row = vec![Span::styled(
                format!("{:<name_width$} ", fit(&name, name_width)),
                name_style,
            )];
            if overview {
                let item_usage = nodes[0]
                    .children
                    .iter()
                    .position(|&root| root == i)
                    .and_then(|index| volume_usage.get(index).copied())
                    .unwrap_or_default();
                row.extend(usage_spans(item_usage, if compact { 5 } else { 8 }));
                row.push(Span::raw(format!(
                    " {:>8} / {:>8} {:>3}%",
                    human(item_usage.used()),
                    human(item_usage.total()),
                    percent(item_usage.used(), item_usage.total())
                )));
                if !compact {
                    row.push(Span::styled(
                        format!(
                            "  {} {}",
                            human(n.size),
                            s.lang.text("scanned", "просмотрено")
                        ),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            } else {
                let width = if compact { 5 } else { 8 };
                let filled = n
                    .size
                    .saturating_mul(width as u64)
                    .checked_div(nodes[s.current].size.max(1))
                    .unwrap_or(0)
                    .min(width as u64) as usize;
                row.push(Span::styled("█".repeat(filled), Style::default().fg(color)));
                row.push(Span::styled(
                    "·".repeat(width - filled),
                    Style::default().fg(Color::DarkGray),
                ));
                row.push(Span::raw(format!(" {:>8} ", human(n.size))));
                row.push(Span::styled(
                    tag,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
                if !compact {
                    if let Some(c) = core::browse::culprit(nodes, i).filter(|&c| c != i) {
                        row.push(Span::styled(
                            format!(
                                "  {} {}",
                                s.lang.text("mostly in", "в основном в"),
                                nodes[c].name
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }
            }
            ListItem::new(Line::from(row))
        })
        .collect::<Vec<_>>();
    let border = |focused| {
        Block::default()
            .title(list_title.as_str())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            })
    };
    let list = List::new(items)
        .block(border(s.focus_right))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(24, 45, 57))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{276f} ");
    let mut state = ListState::default();
    if !shown.is_empty() {
        state.select(Some(s.selected.min(shown.len() - 1)));
    }
    let category_data = current_category_data(nodes, s, category_data);
    let category_rows = CATEGORIES
        .iter()
        .enumerate()
        .map(|(index, category)| {
            let size = category_data.sizes[index];
            let max = category_data
                .sizes
                .iter()
                .copied()
                .max()
                .unwrap_or(1)
                .max(1);
            let bar_width = if compact { 3 } else { 8 };
            let filled = ((size as u128 * bar_width) / max as u128) as usize;
            let label = if compact {
                short_category_name(*category, s.lang)
            } else {
                category_name(*category, s.lang)
            };
            let mut row = vec![
                Span::styled(
                    if index == s.selected_category {
                        "› "
                    } else {
                        "  "
                    },
                    Style::default().fg(category_color(*category)),
                ),
                Span::styled(
                    label,
                    Style::default().fg(category_color(*category)).add_modifier(
                        if index == s.selected_category {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        },
                    ),
                ),
            ];
            if compact {
                row.extend([
                    Span::styled(
                        format!(" {}", "■".repeat(filled)),
                        Style::default().fg(category_color(*category)),
                    ),
                    Span::styled(
                        format!(" {:>7}", human(size)),
                        Style::default().fg(Color::Gray),
                    ),
                ]);
            } else {
                row.extend([
                    Span::raw("\n  "),
                    Span::styled(
                        "■".repeat(filled),
                        Style::default().fg(category_color(*category)),
                    ),
                    Span::styled("·".repeat(8 - filled), Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!(
                            "  {:>7}  · {}",
                            human(size),
                            category_data.items[index].len()
                        ),
                        Style::default().fg(Color::Gray),
                    ),
                ]);
            }
            ListItem::new(Line::from(row))
        })
        .collect::<Vec<_>>();
    f.render_widget(
        List::new(category_rows).block(
            Block::default()
                .title(s.lang.text(" CATEGORIES ", " КАТЕГОРИИ "))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if s.focus_right {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Cyan)
                }),
        ),
        panes[0],
    );
    f.render_stateful_widget(list, content[0], &mut state);
    let item_offset = state.offset();
    let selected = shown.get(s.selected).copied();
    let selected_usage = selected
        .filter(|&i| overview && nodes[0].children.contains(&i))
        .and_then(|i| nodes[0].children.iter().position(|&root| root == i))
        .and_then(|i| volume_usage.get(i).copied());
    let detail = selected
        .map(|i| details(nodes, i, selected_usage, demo, s.lang, scanning))
        .unwrap_or_else(|| {
            vec![Line::from(s.lang.text(
                "No items in this folder",
                "В этой папке нет элементов",
            ))]
        });
    if show_details {
        f.render_widget(
            Paragraph::new(detail)
                .block(
                    Block::default()
                        .title(if selected_usage.is_some() {
                            s.lang.text(" DISK DETAILS ", " ДИСК ")
                        } else {
                            s.lang.text(" ITEM DETAILS ", " ЭЛЕМЕНТ ")
                        })
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(if s.focus_right {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        }),
                )
                .wrap(Wrap { trim: true }),
            content[1],
        );
    }
    let warning = if s.confirm_delete {
        s.lang.text(
            "DELETE  ·  press d again to move this item to Trash",
            "УДАЛИТЬ  ·  нажмите d ещё раз для перемещения в корзину",
        )
    } else if compact {
        s.lang.text(
            "↑↓ move Enter open ← up g folder a files c clean f filter d Trash ? menu q quit",
            "↑↓ выбор Enter открыть ← назад g папка a файлы c очистка f фильтр l язык d корзина q выход",
        )
    } else {
        s.lang.text(
            "↑↓ move   Enter open   ← up   g largest folder   a files   c cleanup   f filter   l language   d Trash   r rescan   ? menu   q quit",
            "↑↓ выбор   Enter открыть   ← назад   g большая папка   a файлы   c очистка   f фильтр   l язык   d корзина   r скан   q выход",
        )
    };
    let status_width = layout[2].width.saturating_sub(42) as usize;
    let status = fit(&s.status, status_width);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                warning,
                if s.confirm_delete {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            )),
            Line::from(vec![
                Span::styled(
                    format!(
                        "{}   {} {}",
                        s.lang.text("Tab: panels", "Tab: панели"),
                        human(usage.free),
                        s.lang.text("free", "свободно")
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(format!(
                    "   {} {}   ",
                    human(s.trashed),
                    s.lang.text("sent to Trash", "в корзину")
                )),
                Span::styled(status, Style::default().fg(Color::Yellow)),
            ]),
        ]),
        layout[2],
    );
    let areas = ScreenAreas {
        map: layout[0],
        categories: panes[0],
        items: content[0],
        item_offset,
        map_segments,
    };
    if s.menu_open {
        let actions = menu_actions(s.lang, scanning, demo);
        draw_action_menu(f, area, &actions, s.menu_selected, s.lang);
    }
    areas
}

fn details(
    nodes: &[Node],
    i: usize,
    usage: Option<bar::Usage>,
    demo: bool,
    lang: Language,
    scanning: bool,
) -> Vec<Line<'static>> {
    if let Some(usage) = usage {
        return volume_details(nodes, i, usage, demo, lang, scanning);
    }
    let n = &nodes[i];
    let cause = core::browse::culprit(nodes, i).filter(|&c| c != i);
    let category = match n.category {
        core::categories::Category::ProjectsAndCode => {
            lang.text("Projects & code", "Проекты и код")
        }
        core::categories::Category::Downloads => lang.text("Downloads", "Загрузки"),
        core::categories::Category::CachesAndTemp => {
            lang.text("Caches & temporary files", "Кэши и временные файлы")
        }
        core::categories::Category::MediaAndArchives => {
            lang.text("Media & archives", "Медиа и архивы")
        }
        core::categories::Category::Other => lang.text("Other", "Другое"),
    };
    let tag = match n.tag {
        core::classify::Tag::Junk => lang.text("JUNK", "МУСОР"),
        core::classify::Tag::Unused => lang.text("UNUSED", "СТАРОЕ"),
        core::classify::Tag::System => lang.text("SYSTEM", "СИСТЕМА"),
        _ => lang.text("PERSONAL", "ЛИЧНОЕ"),
    };
    let date = |v: Option<chrono::DateTime<chrono::Utc>>| {
        v.map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| lang.text("unknown", "неизвестно").into())
    };
    let mut lines = vec![
        Line::from(Span::styled(
            n.name.clone(),
            Style::default()
                .fg(item_color(n.tag, n.category))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            n.path.display().to_string(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                human(n.size),
                Style::default()
                    .fg(item_color(n.tag, n.category))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("   {} {}", n.files, lang.text("files", "файлов"))),
        ]),
        Line::from(vec![
            Span::raw(format!("{}  ", lang.text("Type", "Тип"))),
            Span::styled(tag, Style::default().fg(item_color(n.tag, n.category))),
        ]),
        Line::from(format!(
            "{}  {category}",
            lang.text("Category", "Категория")
        )),
        Line::from(format!(
            "{} {}",
            lang.text("Last open", "Открыт"),
            date(n.atime)
        )),
        Line::from(format!(
            "{}  {}",
            lang.text("Modified", "Изменён"),
            date(n.mtime)
        )),
        Line::from(format!(
            "{}  {}",
            lang.text("Mostly in", "В основном в"),
            cause
                .map(|c| nodes[c].name.as_str())
                .unwrap_or(lang.text("Several locations", "В нескольких местах"))
        )),
        Line::from(Span::styled(
            localized_reason(n.reason.as_deref(), lang),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    if cause.is_none() && !n.children.is_empty() {
        lines.push(Line::from(
            lang.text("Largest items", "Крупнейшие элементы"),
        ));
        lines.extend(
            core::browse::largest_children(nodes, i)
                .into_iter()
                .take(3)
                .map(|child| {
                    Line::from(format!(
                        "  {:<14} {:>8}",
                        fit(&nodes[child].name, 14),
                        human(nodes[child].size)
                    ))
                }),
        );
    }
    lines
}

fn volume_details(
    nodes: &[Node],
    i: usize,
    usage: bar::Usage,
    demo: bool,
    lang: Language,
    scanning: bool,
) -> Vec<Line<'static>> {
    let total = usage.total();
    let mut lines = vec![
        Line::from(Span::styled(
            volume_name(&nodes[i], demo, lang),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            nodes[i].path.display().to_string(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(format!(
            "{}  {}",
            lang.text("Disk capacity", "Ёмкость диска"),
            human(total)
        )),
        Line::from(vec![
            Span::styled(
                format!("{}  {}", lang.text("Used", "Занято"), human(usage.used())),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("   {}%", percent(usage.used(), total))),
        ]),
        Line::from(format!(
            "{}  {}",
            lang.text("Available", "Свободно"),
            human(usage.free)
        )),
        Line::from(format!(
            "{}  {}",
            lang.text("Files scanned", "Найдено файлов"),
            nodes[i].files
        )),
    ];
    if scanning {
        lines.push(Line::from(Span::styled(
            lang.text(
                "Live scan · folder sizes are partial",
                "Идёт сканирование · размеры папок пока неполные",
            ),
            Style::default().fg(Color::Yellow),
        )));
    } else {
        lines.push(Line::from(""));
        lines.extend([
            Line::from(vec![
                Span::styled("■ ", Style::default().fg(Color::Red)),
                Span::raw(format!(
                    "{}  {}",
                    lang.text("Junk", "Мусор"),
                    human(usage.junk)
                )),
            ]),
            Line::from(vec![
                Span::styled("■ ", Style::default().fg(Color::Yellow)),
                Span::raw(format!(
                    "{}  {}",
                    lang.text("Unused", "Давно не открывали"),
                    human(usage.unused)
                )),
            ]),
            Line::from(vec![
                Span::styled("■ ", Style::default().fg(Color::Magenta)),
                Span::raw(format!(
                    "{}  {}",
                    lang.text("Other on disk", "Остальные данные диска"),
                    human(usage.other)
                )),
            ]),
        ]);
    }
    if demo {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            lang.text(
                "Demo volume · nothing will be changed",
                "Демо-диск · файлы не изменяются",
            ),
            Style::default().fg(Color::Green),
        )));
    }
    let largest: Vec<_> = core::browse::largest_children(nodes, i)
        .into_iter()
        .filter(|&child| nodes[child].directory)
        .take(3)
        .collect();
    if !largest.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(
            lang.text("Largest folders", "Самые крупные папки"),
        ));
        lines.extend(largest.into_iter().map(|child| {
            Line::from(format!(
                "  {:<16} {:>8}",
                fit(&nodes[child].name, 16),
                human(nodes[child].size)
            ))
        }));
    }
    lines
}

fn volume_name(node: &Node, demo: bool, lang: Language) -> String {
    if demo {
        match node.name.as_str() {
            "System volume" => lang.text("System volume", "Системный диск").into(),
            "Media volume" => lang.text("Media volume", "Медиа-диск").into(),
            _ => node.name.clone(),
        }
    } else {
        node.name.clone()
    }
}

fn localized_reason(reason: Option<&str>, lang: Language) -> String {
    match reason {
        Some("Rebuilds automatically") => {
            lang.text("Rebuilds automatically", "Пересоздаётся автоматически")
        }
        Some("Installer no longer needed") => {
            lang.text("Installer no longer needed", "Установщик уже не нужен")
        }
        Some("Not touched in 6+ months") => {
            lang.text("Not touched in 6+ months", "Не открывали больше полугода")
        }
        Some("Managed by system, deletion blocked") => lang.text(
            "Managed by system, deletion blocked",
            "Системная папка: удаление запрещено",
        ),
        Some(other) => other,
        None => lang.text("Your files", "Ваши файлы"),
    }
    .to_owned()
}

fn cleanup_candidate(nodes: &[Node], i: usize) -> bool {
    matches!(
        nodes[i].tag,
        core::classify::Tag::Junk | core::classify::Tag::Unused
    ) || (nodes[0].name == "All disks"
        && nodes[0].children.contains(&i)
        && classified_sizes(nodes, i) != (0, 0))
}

fn visible_children(nodes: &[Node], store: &Store, data: &[CategoryData]) -> Vec<usize> {
    category_items(nodes, store, data)
}

fn ready_to_clean(
    nodes: &[Node],
    item: usize,
    ready_roots: &std::collections::HashSet<usize>,
) -> bool {
    let root = if nodes[0].children.contains(&item) {
        Some(item)
    } else {
        selected_volume(nodes, item)
    };
    root.is_some_and(|root| ready_roots.contains(&root))
}

fn usage_spans(usage: bar::Usage, width: usize) -> Vec<Span<'static>> {
    let amounts = [
        usage.personal,
        usage.junk,
        usage.unused,
        usage.other,
        usage.free,
    ];
    let widths = bar::segment_widths(&amounts, width);
    [
        Color::Cyan,
        Color::Red,
        Color::Yellow,
        Color::Magenta,
        Color::DarkGray,
    ]
    .into_iter()
    .zip(widths)
    .map(|(color, width)| Span::styled("█".repeat(width), Style::default().fg(color)))
    .collect()
}

fn percent(amount: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        (u128::from(amount) * 100 / u128::from(total)).min(100) as u64
    }
}

fn fit(value: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    let mut truncated = false;
    for ch in value.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > width {
            truncated = true;
            break;
        }
        out.push(ch);
        used += w;
    }
    if truncated && width > 0 {
        while used >= width {
            if let Some(ch) = out.pop() {
                used -= ch.width().unwrap_or(0);
            }
        }
        out.push('\u{2026}');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_usage, category_data_by_volume, cleanup_candidate, draw, ensure_scan_root,
        independent_roots, install_scanned_entries, map_widths, pending_tree, ready_to_clean,
        scan_roots, selected_volume, volume_usages, CategoryData,
    };
    use crate::{
        core,
        ui::{bar, lang::Language, store::Store},
    };
    use crossterm::event::KeyCode;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn disk_accents_are_distinct_and_item_colors_follow_meaning() {
        let accents = (0..6).map(super::volume_color).collect::<Vec<_>>();
        assert!(accents
            .iter()
            .enumerate()
            .all(|(i, color)| accents.iter().skip(i + 1).all(|other| color != other)));
        assert_eq!(
            super::item_color(
                core::classify::Tag::Personal,
                core::categories::Category::MediaAndArchives
            ),
            super::category_color(core::categories::Category::MediaAndArchives)
        );
        assert_eq!(
            super::item_color(
                core::classify::Tag::Junk,
                core::categories::Category::ProjectsAndCode
            ),
            ratatui::style::Color::Rgb(255, 102, 102)
        );
        assert_eq!(
            super::item_color(
                core::classify::Tag::System,
                core::categories::Category::ProjectsAndCode
            ),
            ratatui::style::Color::Rgb(185, 135, 255)
        );
    }

    #[test]
    fn action_menu_exposes_cleanup_and_hides_rescan_while_busy() {
        let ready = super::menu_actions(Language::En, false, false);
        assert!(ready
            .iter()
            .any(|(key, _, label)| { *key == KeyCode::Char('d') && label.contains("Trash") }));
        assert!(ready
            .iter()
            .any(|(key, _, label)| { *key == KeyCode::Char('r') && label.contains("Rescan") }));
        let busy = super::menu_actions(Language::En, true, false);
        assert!(!busy.iter().any(|(key, _, _)| *key == KeyCode::Char('r')));
    }

    #[test]
    fn action_menu_renders_over_the_disk_view() {
        let mut nodes = core::demo::tree();
        ensure_scan_root(&mut nodes);
        let volume_usage = volume_usages(&nodes, true);
        let mut store = Store::new(&nodes, Language::En);
        store.menu_open = true;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                let _ = draw(
                    frame,
                    &nodes,
                    &store,
                    &volume_usage,
                    &category_data_by_volume(&nodes),
                    volume_usage[0],
                    true,
                    false,
                );
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("ACTIONS"), "{rendered}");
        assert!(
            rendered.contains("Move selected item to Trash"),
            "{rendered}"
        );
        assert!(rendered.contains("Esc close"), "{rendered}");
    }

    #[test]
    fn pending_tree_shows_every_drive_before_scanning_finishes() {
        let roots = vec![
            std::path::PathBuf::from("C:\\"),
            std::path::PathBuf::from("D:\\"),
        ];
        let nodes = pending_tree(&roots);
        assert_eq!(nodes[0].name, "All disks");
        assert_eq!(nodes[0].children, [1, 2]);
        assert_eq!(nodes[1].path, roots[0]);
        assert_eq!(nodes[2].path, roots[1]);
        assert!(nodes[1].directory && nodes[2].directory);
        let rendered = render_text(&nodes, Language::En, false);
        assert!(rendered.contains("Disk overview"), "{rendered}");
        assert!(rendered.contains("C:"), "{rendered}");
        assert!(rendered.contains("D:"), "{rendered}");
    }

    #[test]
    fn completed_drive_becomes_browsable_while_other_drives_scan() {
        let root = std::path::PathBuf::from("volume-c");
        let folder = root.join("folder");
        let file = folder.join("sample.bin");
        let mut nodes = pending_tree(&[root.clone(), std::path::PathBuf::from("volume-d")]);
        let mut paths = std::collections::HashMap::new();
        let mut pending = Vec::new();
        let root_index = install_scanned_entries(
            &mut nodes,
            1,
            &mut paths,
            &mut pending,
            vec![core::scan::ScanEntry {
                parent: None,
                node: core::Node {
                    path: root.clone(),
                    name: "C:".into(),
                    size: 0,
                    files: 0,
                    directory: true,
                    atime: None,
                    mtime: None,
                    children: vec![],
                    tag: core::classify::Tag::Personal,
                    reason: None,
                    category: core::categories::Category::Other,
                },
            }],
        );
        assert_eq!(root_index, [1]);
        let file_index = install_scanned_entries(
            &mut nodes,
            1,
            &mut paths,
            &mut pending,
            vec![core::scan::ScanEntry {
                parent: Some(folder.clone()),
                node: core::Node {
                    path: file.clone(),
                    name: "sample.bin".into(),
                    size: 12,
                    files: 1,
                    directory: false,
                    atime: None,
                    mtime: None,
                    children: vec![],
                    tag: core::classify::Tag::Personal,
                    reason: None,
                    category: core::categories::Category::Other,
                },
            }],
        );
        assert!(file_index.is_empty());
        assert_eq!(pending.len(), 1);
        let available = install_scanned_entries(
            &mut nodes,
            1,
            &mut paths,
            &mut pending,
            vec![core::scan::ScanEntry {
                parent: Some(root.clone()),
                node: core::Node {
                    path: folder.clone(),
                    name: "folder".into(),
                    size: 0,
                    files: 0,
                    directory: true,
                    atime: None,
                    mtime: None,
                    children: vec![],
                    tag: core::classify::Tag::Junk,
                    reason: Some("Rebuilds automatically".into()),
                    category: core::categories::Category::Other,
                },
            }],
        );
        assert_eq!(available, [3, 4]);
        assert_eq!(nodes[0].size, 12);
        assert_eq!(nodes[1].size, 12);
        assert_eq!(nodes[1].children, [3]);
        assert_eq!(nodes[3].path, folder);
        assert_eq!(nodes[3].children, [4]);
        assert_eq!(nodes[4].path, file);
        assert_eq!(nodes[4].tag, core::classify::Tag::Junk);
        assert_eq!(nodes[0].children, [1, 2]);
        let mut ready = std::collections::HashSet::new();
        assert!(!ready_to_clean(&nodes, 4, &ready));
        ready.insert(1);
        assert!(ready_to_clean(&nodes, 4, &ready));
    }

    #[test]
    fn overlapping_scan_roots_keep_the_deduplicating_scan_path() {
        assert!(independent_roots(&[
            std::path::PathBuf::from("volume-a"),
            std::path::PathBuf::from("volume-b"),
        ]));
        assert!(!independent_roots(&[
            std::path::PathBuf::from("folder"),
            std::path::PathBuf::from("folder/subfolder"),
        ]));
        assert!(!independent_roots(&[
            std::path::PathBuf::from("duplicate"),
            std::path::PathBuf::from("duplicate"),
        ]));
    }

    #[test]
    fn demo_opens_on_the_disk_overview() {
        let mut nodes = core::demo::tree();
        ensure_scan_root(&mut nodes);
        let rendered = render_text(&nodes, Language::En, false);
        assert!(rendered.contains("Disk overview"), "{rendered}");
        assert!(rendered.contains("CATEGORIES"), "{rendered}");
        assert!(rendered.contains("SPACE MAP"), "{rendered}");
    }

    #[test]
    fn space_map_segments_fill_width_and_preserve_unknown_space() {
        let mut data = CategoryData::default();
        data.sizes[0] = 60;
        data.sizes[1] = 20;
        let widths = map_widths(
            &data,
            bar::Usage {
                personal: 100,
                ..bar::Usage::default()
            },
            40,
        );
        assert_eq!(widths.iter().sum::<usize>(), 40);
        assert_eq!(widths[5], 8);
        assert_eq!(widths[0], 24);
        assert_eq!(widths[1], 8);
    }

    #[test]
    fn single_scan_root_is_wrapped_once_and_keeps_its_tree() {
        let path = std::path::PathBuf::from("/scan");
        let mut nodes = vec![core::Node {
            path: path.clone(),
            name: "scan".into(),
            size: 12,
            files: 1,
            directory: true,
            atime: None,
            mtime: None,
            children: vec![],
            tag: core::classify::Tag::Personal,
            reason: None,
            category: core::categories::Category::Other,
        }];
        let size = nodes[0].size;
        let files = nodes[0].files;
        ensure_scan_root(&mut nodes);
        ensure_scan_root(&mut nodes);
        assert_eq!(nodes[0].name, "All disks");
        assert_eq!(nodes[0].children, vec![1]);
        assert_eq!((nodes[0].size, nodes[0].files), (size, files));
        assert_eq!(nodes[1].path, path);
        assert_eq!(scan_roots(&nodes), vec![path]);
    }

    #[test]
    fn each_volume_keeps_its_own_capacity_and_filter_result() {
        let nodes = core::demo::tree();
        let roots = nodes[0].children.clone();
        let usage = volume_usages(&nodes, true);
        assert_eq!(usage.len(), 2);
        assert_eq!(
            (usage[0].total(), usage[0].free),
            (160_000_000_000, 24_000_000_000)
        );
        assert_eq!(
            (usage[1].total(), usage[1].free),
            (140_000_000_000, 20_000_000_000)
        );
        assert_eq!(aggregate_usage(&usage).total(), 300_000_000_000);
        assert_eq!(
            selected_volume(&nodes, nodes[roots[0]].children[0]),
            Some(roots[0])
        );
        assert!(cleanup_candidate(&nodes, roots[0]));
        assert!(!cleanup_candidate(&nodes, roots[1]));
    }

    #[test]
    fn russian_language_renders_localized_overview_and_volume_details() {
        let mut nodes = core::demo::tree();
        ensure_scan_root(&mut nodes);
        let rendered = render_text(&nodes, Language::Ru, false);
        assert!(rendered.contains("Обзор дисков"), "{rendered}");
        assert!(rendered.contains("ДИСК"), "{rendered}");
        assert!(rendered.contains("Ёмкость диска"), "{rendered}");
    }

    #[test]
    fn live_scan_marks_folder_sizes_partial_and_hides_final_categories() {
        let mut nodes = core::demo::tree();
        ensure_scan_root(&mut nodes);
        let rendered = render_text(&nodes, Language::En, true);
        assert!(rendered.contains("Live scan"), "{rendered}");
        assert!(!rendered.contains("Other on disk"), "{rendered}");
    }

    #[test]
    fn tiny_terminal_shows_resize_message_without_drawing_panels() {
        let mut nodes = core::demo::tree();
        ensure_scan_root(&mut nodes);
        let volume_usage = volume_usages(&nodes, true);
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal
            .draw(|frame| {
                let areas = draw(
                    frame,
                    &nodes,
                    &Store::new(&nodes, Language::En),
                    &volume_usage,
                    &category_data_by_volume(&nodes),
                    volume_usage[0],
                    true,
                    false,
                );
                assert_eq!(areas.map.width, 0);
                assert_eq!(areas.categories.height, 0);
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Resize the terminal"));
        assert!(!rendered.contains("CATEGORIES"));
    }

    fn render_text(nodes: &[core::Node], lang: Language, scanning: bool) -> String {
        let volume_usage = volume_usages(nodes, true);
        let mut terminal = Terminal::new(TestBackend::new(140, 38)).unwrap();
        terminal
            .draw(|frame| {
                let _ = draw(
                    frame,
                    nodes,
                    &Store::new(nodes, lang),
                    &volume_usage,
                    &category_data_by_volume(nodes),
                    volume_usage[0],
                    true,
                    scanning,
                );
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }
}
