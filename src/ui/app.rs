use crate::{
    core::{self, Node},
    plain::human,
    ui::{bar, lang::Language, store::Store},
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use std::{
    io,
    sync::mpsc::{self, Receiver},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use unicode_width::UnicodeWidthChar;

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
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
    execute!(io::stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut s = Store::new(nodes, lang);
    let mut volume_usage = volume_usages(nodes, demo);
    let mut scan = initial_roots.map(|roots| start_scan(roots, Arc::clone(&cancelled)));
    if scan.is_some() {
        s.status = s
            .lang
            .text(
                "Scanning drives in the background · folders appear when ready",
                "Диски сканируются · папки появятся по завершении",
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
                        volume_usage = volume_usages(nodes, demo);
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
        terminal.draw(|f| draw(f, nodes, &s, &volume_usage, usage, demo))?;
        if scan.is_some() && !event::poll(Duration::from_millis(100))? {
            continue;
        }
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
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
                            if s.current == 0 {
                                let count = nodes[0].children.len();
                                if count > 0 {
                                    s.selected = (s.selected + 1).min(count - 1);
                                }
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if s.current == 0 {
                                s.selected = s.selected.saturating_sub(1);
                            }
                        }
                        KeyCode::Tab => s.focus_right = !s.focus_right,
                        KeyCode::Char('l') => s.lang.toggle(),
                        KeyCode::Enter | KeyCode::Right => {
                            if s.current == 0 {
                                s.status = s
                                    .lang
                                    .text(
                                        "Folder browsing unlocks when the scan finishes",
                                        "Папки станут доступны после завершения сканирования",
                                    )
                                    .into();
                            }
                        }
                        _ => {}
                    }
                }
                continue;
            }
            let children = core::browse::sorted_children(nodes, s.current);
            let shown: Vec<_> = children
                .into_iter()
                .filter(|&i| !s.only_junk || cleanup_candidate(nodes, i))
                .collect();
            match k.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('c') if k.modifiers.contains(event::KeyModifiers::CONTROL) => break,
                KeyCode::Down | KeyCode::Char('j') => {
                    if !shown.is_empty() {
                        s.selected = (s.selected + 1).min(shown.len() - 1);
                    }
                    s.confirm_delete = false;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    s.selected = s.selected.saturating_sub(1);
                    s.confirm_delete = false;
                }
                KeyCode::Right | KeyCode::Enter => {
                    if let Some(&i) = shown.get(s.selected) {
                        if nodes[i].directory {
                            s.current = i;
                            s.selected = 0;
                            s.confirm_delete = false;
                        }
                    }
                }
                KeyCode::Left | KeyCode::Esc => {
                    if s.current != 0 {
                        s.current = nodes[s.current]
                            .path
                            .parent()
                            .and_then(|p| nodes.iter().position(|n| n.path == p))
                            .unwrap_or(0);
                        s.selected = 0;
                        s.confirm_delete = false;
                    }
                }
                KeyCode::Char('g') => {
                    if let Some(&i) = shown.get(s.selected) {
                        if let Some(c) = core::browse::culprit(nodes, i) {
                            s.current = c;
                            s.selected = 0;
                            s.confirm_delete = false;
                        }
                    }
                }
                KeyCode::Char('f') => {
                    s.only_junk = !s.only_junk;
                    s.selected = 0;
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
                    if let Some(&i) = shown.get(s.selected) {
                        if nodes[0].name == "All disks" && nodes[0].children.contains(&i) {
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
    Finished(std::io::Result<Vec<Node>>),
}

struct ScanJob {
    updates: Receiver<ScanUpdate>,
    cancelled: Arc<AtomicBool>,
}

fn start_scan(roots: Vec<std::path::PathBuf>, cancelled: Arc<AtomicBool>) -> ScanJob {
    let (updates_tx, updates) = mpsc::channel();
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

fn draw(
    f: &mut Frame,
    nodes: &[Node],
    s: &Store,
    volume_usage: &[bar::Usage],
    usage: bar::Usage,
    demo: bool,
) {
    let area = f.area();
    let overview = s.current == 0;
    let current_path = nodes[s.current].path.display().to_string();
    let title = if overview {
        s.lang.text("Disk overview", "Обзор дисков")
    } else {
        &current_path
    };
    let layout = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(7),
        Constraint::Length(2),
    ])
    .split(area);
    bar::render(f, layout[0], title, usage, s.lang);
    let compact = area.width < 110;
    let panes = Layout::horizontal(if compact {
        [Constraint::Percentage(55), Constraint::Percentage(45)]
    } else {
        [Constraint::Percentage(62), Constraint::Percentage(38)]
    })
    .split(layout[1]);
    let shown: Vec<_> = core::browse::sorted_children(nodes, s.current)
        .into_iter()
        .filter(|&i| !s.only_junk || cleanup_candidate(nodes, i))
        .collect();
    let folders = shown.iter().filter(|&&i| nodes[i].directory).count();
    let files = shown.len() - folders;
    let list_title = if overview {
        format!(
            " {}  /  {} {} ",
            s.lang.text("SCAN ROOTS", "ТОЧКИ СКАНИРОВАНИЯ"),
            folders,
            s.lang.text("volumes", "дисков")
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
            " {}  {folders}  /  {}  {files} ",
            s.lang.text("FOLDERS", "ПАПКИ"),
            s.lang.text("FILES", "ФАЙЛЫ")
        )
    };
    let items = shown
        .iter()
        .map(|&i| {
            let n = &nodes[i];
            let (tag, color) = match n.tag {
                core::classify::Tag::Junk => (s.lang.text("JUNK", "МУСОР"), Color::Red),
                core::classify::Tag::Unused => (s.lang.text("UNUSED", "СТАРОЕ"), Color::Yellow),
                core::classify::Tag::System => (s.lang.text("SYSTEM", "СИСТЕМА"), Color::Magenta),
                _ => ("", Color::Cyan),
            };
            let name_width = if compact { 12 } else { 20 };
            let name = if overview {
                volume_name(n, demo, s.lang)
            } else if n.directory {
                format!("▸ {}", n.name)
            } else {
                n.name.clone()
            };
            let mut row = vec![Span::raw(format!(
                "{:<name_width$} ",
                fit(&name, name_width)
            ))];
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
        .block(border(!s.focus_right))
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
    f.render_stateful_widget(list, panes[0], &mut state);
    let selected = shown.get(s.selected).copied();
    let selected_usage = selected
        .filter(|&i| overview && nodes[0].children.contains(&i))
        .and_then(|i| nodes[0].children.iter().position(|&root| root == i))
        .and_then(|i| volume_usage.get(i).copied());
    let detail = selected
        .map(|i| details(nodes, i, selected_usage, demo, s.lang))
        .unwrap_or_else(|| {
            vec![Line::from(s.lang.text(
                "No items in this folder",
                "В этой папке нет элементов",
            ))]
        });
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
        panes[1],
    );
    let warning = if s.confirm_delete {
        s.lang.text(
            "DELETE  ·  press d again to move this item to Trash",
            "УДАЛИТЬ  ·  нажмите d ещё раз для перемещения в корзину",
        )
    } else if compact {
        s.lang.text(
            "↑↓ move  Enter open  ← back  g largest  f filter  l lang  d Trash  q quit",
            "↑↓ выбор  Enter открыть  ← назад  g крупное  f фильтр  l язык  d корзина  q выход",
        )
    } else {
        s.lang.text(
            "↑↓ move   Enter open   ← up   g largest   f filter   l language   d Trash   r rescan   q quit",
            "↑↓ выбор   Enter открыть   ← назад   g крупное   f фильтр   l язык   d корзина   r скан   q выход",
        )
    };
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
                Span::raw("   "),
                Span::styled(&s.status, Style::default().fg(Color::Yellow)),
            ]),
        ]),
        layout[2],
    );
}

fn details(
    nodes: &[Node],
    i: usize,
    usage: Option<bar::Usage>,
    demo: bool,
    lang: Language,
) -> Vec<Line<'static>> {
    if let Some(usage) = usage {
        return volume_details(nodes, i, usage, demo, lang);
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
                .fg(Color::White)
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
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("   {} {}", n.files, lang.text("files", "файлов"))),
        ]),
        Line::from(vec![
            Span::raw(format!("{}  ", lang.text("Type", "Тип"))),
            Span::styled(
                tag,
                Style::default().fg(match n.tag {
                    core::classify::Tag::Junk => Color::Red,
                    core::classify::Tag::Unused => Color::Yellow,
                    core::classify::Tag::System => Color::Magenta,
                    _ => Color::Green,
                }),
            ),
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
        Line::from(""),
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
    ];
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
        aggregate_usage, cleanup_candidate, draw, ensure_scan_root, pending_tree, scan_roots,
        selected_volume, volume_usages,
    };
    use crate::{
        core,
        ui::{lang::Language, store::Store},
    };
    use ratatui::{backend::TestBackend, Terminal};

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
        let rendered = render_text(&nodes, Language::En);
        assert!(rendered.contains("Disk overview"), "{rendered}");
        assert!(rendered.contains("C:"), "{rendered}");
        assert!(rendered.contains("D:"), "{rendered}");
    }

    #[test]
    fn demo_opens_on_the_disk_overview() {
        let mut nodes = core::demo::tree();
        ensure_scan_root(&mut nodes);
        let rendered = render_text(&nodes, Language::En);
        assert!(rendered.contains("Disk overview"), "{rendered}");
        assert!(rendered.contains("DISK DETAILS"), "{rendered}");
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
        let rendered = render_text(&nodes, Language::Ru);
        assert!(rendered.contains("Обзор дисков"), "{rendered}");
        assert!(rendered.contains("ДИСК"), "{rendered}");
        assert!(rendered.contains("Ёмкость диска"), "{rendered}");
    }

    fn render_text(nodes: &[core::Node], lang: Language) -> String {
        let volume_usage = volume_usages(nodes, true);
        let mut terminal = Terminal::new(TestBackend::new(140, 38)).unwrap();
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    nodes,
                    &Store::new(nodes, lang),
                    &volume_usage,
                    volume_usage[0],
                    true,
                )
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
