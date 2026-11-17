use crate::{
    core::{self, Node},
    plain::human,
    ui::{bar, store::Store},
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use std::io;
use unicode_width::UnicodeWidthChar;

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub fn run(nodes: &mut Vec<Node>, dry: bool, demo: bool) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut s = Store::new(nodes);
    let mut junk_unused = classified_sizes(nodes, 0);
    loop {
        let (total, free) = if demo {
            (300_000_000_000, 44_000_000_000)
        } else {
            disk_totals(nodes)
        };
        let used = total.saturating_sub(free);
        let (junk, unused) = junk_unused;
        terminal.draw(|f| draw(f, nodes, &s, total, used, free, junk, unused))?;
        if let Event::Key(k) = event::read()? {
            let children = core::browse::sorted_children(nodes, s.current);
            let shown: Vec<_> = children
                .into_iter()
                .filter(|&i| {
                    !s.only_junk
                        || matches!(
                            nodes[i].tag,
                            core::classify::Tag::Junk | core::classify::Tag::Unused
                        )
                })
                .collect();
            match k.code {
                KeyCode::Char('q') => break,
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
                KeyCode::Char('r') => {
                    if demo {
                        s.status = "Demo data is already up to date".into();
                    } else {
                        s.confirm_delete = false;
                        let roots = scan_roots(nodes);
                        match core::scan::scan_many(&roots) {
                            Ok(updated) => {
                                *nodes = updated;
                                s.current = 0;
                                s.selected = 0;
                                s.status = "Scan refreshed".into();
                                junk_unused = classified_sizes(nodes, 0);
                            }
                            Err(e) => s.status = format!("Rescan failed: {e}"),
                        }
                    }
                }
                KeyCode::Tab => s.focus_right = !s.focus_right,
                KeyCode::Char('d') => {
                    if let Some(&i) = shown.get(s.selected) {
                        if nodes[0].name == "All disks" && nodes[0].children.contains(&i) {
                            s.status = "A disk root cannot be moved to Trash".into();
                            s.confirm_delete = false;
                        } else if !s.confirm_delete {
                            s.confirm_delete = true;
                            s.status = "Press d again to move this item to Trash".into();
                        } else {
                            match core::clean::clean(&nodes[i], dry) {
                                Ok(m) => s.status = m,
                                Err(e) => s.status = e.to_string(),
                            }
                            s.confirm_delete = false;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
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

fn disk_totals(nodes: &[Node]) -> (u64, u64) {
    scan_roots(nodes)
        .iter()
        .fold((0, 0), |(total, free), path| {
            (
                total.saturating_add(fs2::total_space(path).unwrap_or(0)),
                free.saturating_add(fs2::available_space(path).unwrap_or(0)),
            )
        })
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
    total: u64,
    used: u64,
    free: u64,
    junk: u64,
    unused: u64,
) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(7),
        Constraint::Length(2),
    ])
    .split(area);
    bar::render(
        f,
        layout[0],
        &nodes[s.current].path.display().to_string(),
        total,
        used,
        free,
        junk,
        unused,
    );
    let compact = area.width < 110;
    let panes = Layout::horizontal(if compact {
        [Constraint::Percentage(55), Constraint::Percentage(45)]
    } else {
        [Constraint::Percentage(62), Constraint::Percentage(38)]
    })
    .split(layout[1]);
    let shown: Vec<_> = core::browse::sorted_children(nodes, s.current)
        .into_iter()
        .filter(|&i| {
            !s.only_junk
                || matches!(
                    nodes[i].tag,
                    core::classify::Tag::Junk | core::classify::Tag::Unused
                )
        })
        .collect();
    let folders = shown.iter().filter(|&&i| nodes[i].directory).count();
    let files = shown.len() - folders;
    let list_title = if s.only_junk {
        format!(" JUNK + UNUSED  /  {folders} folders, {files} files ")
    } else if nodes[s.current].name == "All disks" {
        format!(" MOUNTED DISKS  /  {folders} volumes ")
    } else {
        format!(" FOLDERS  {folders}  /  FILES  {files} ")
    };
    let items = shown
        .iter()
        .map(|&i| {
            let n = &nodes[i];
            let (tag, color) = match n.tag {
                core::classify::Tag::Junk => ("JUNK", Color::Red),
                core::classify::Tag::Unused => ("UNUSED", Color::Yellow),
                core::classify::Tag::System => ("SYSTEM", Color::Magenta),
                _ => ("", Color::Cyan),
            };
            let name_width = if compact { 12 } else { 20 };
            let width = if compact { 5 } else { 8 };
            let filled = if nodes[s.current].size == 0 {
                0
            } else {
                (n.size.saturating_mul(width as u64) / nodes[s.current].size).min(width as u64)
                    as usize
            };
            let size_bar = format!(
                "{}{}",
                "\u{2588}".repeat(filled),
                "\u{2591}".repeat(width - filled)
            );
            let reason = (!compact)
                .then(|| core::browse::culprit(nodes, i))
                .flatten()
                .filter(|&c| c != i)
                .map(|c| format!("  mostly in {}", nodes[c].name))
                .unwrap_or_default();
            let name = if n.directory {
                format!("\u{25b8} {}", n.name)
            } else {
                n.name.clone()
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<name_width$} ", fit(&name, name_width))),
                Span::styled(size_bar, Style::default().fg(color)),
                Span::raw(format!(" {:>8} ", human(n.size))),
                Span::styled(tag, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(reason, Style::default().fg(Color::DarkGray)),
            ]))
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
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{276f} ");
    let mut state = ListState::default();
    if !shown.is_empty() {
        state.select(Some(s.selected.min(shown.len() - 1)));
    }
    f.render_stateful_widget(list, panes[0], &mut state);
    let detail = shown
        .get(s.selected)
        .map(|&i| details(nodes, i))
        .unwrap_or_else(|| vec![Line::from("No items in this folder")]);
    f.render_widget(
        Paragraph::new(detail)
            .block(
                Block::default()
                    .title(" ITEM DETAILS ")
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
        "DELETE  ·  press d again to move this item to Trash"
    } else {
        "↑↓ move   Enter open   ← up   g culprit   f filter   d Trash   r scan   q quit"
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
                    format!("Tab panels   {} free", human(free)),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("   "),
                Span::styled(&s.status, Style::default().fg(Color::Yellow)),
            ]),
        ]),
        layout[2],
    );
}

fn details(nodes: &[Node], i: usize) -> Vec<Line<'static>> {
    let n = &nodes[i];
    let cause = core::browse::culprit(nodes, i).filter(|&c| c != i);
    let category = match n.category {
        core::categories::Category::ProjectsAndCode => "Projects & code",
        core::categories::Category::Downloads => "Downloads",
        core::categories::Category::CachesAndTemp => "Caches & temporary files",
        core::categories::Category::MediaAndArchives => "Media & archives",
        core::categories::Category::Other => "Other",
    };
    let tag = match n.tag {
        core::classify::Tag::Junk => "JUNK",
        core::classify::Tag::Unused => "UNUSED",
        core::classify::Tag::System => "SYSTEM",
        _ => "PERSONAL",
    };
    let date = |v: Option<chrono::DateTime<chrono::Utc>>| {
        v.map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".into())
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
            Span::raw(format!("   {} files", n.files)),
        ]),
        Line::from(vec![
            Span::raw("Type      "),
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
        Line::from(format!("Category  {category}")),
        Line::from(format!("Last open {}", date(n.atime))),
        Line::from(format!("Modified  {}", date(n.mtime))),
        Line::from(format!(
            "Mostly in  {}",
            cause
                .map(|c| nodes[c].name.as_str())
                .unwrap_or("Several locations")
        )),
        Line::from(Span::styled(
            n.reason.clone().unwrap_or_else(|| "Your files".into()),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    if cause.is_none() && !n.children.is_empty() {
        lines.push(Line::from("Largest items"));
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
