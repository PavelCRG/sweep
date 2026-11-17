use ratatui::{prelude::*, widgets::*};

pub fn render(
    f: &mut Frame,
    area: Rect,
    path: &str,
    total: u64,
    used: u64,
    free: u64,
    junk: u64,
    unused: u64,
) {
    let total = total.max(used).max(1);
    let free = free.min(total);
    let junk = junk.min(used);
    let unused = unused.min(used.saturating_sub(junk));
    let personal = used.saturating_sub(junk + unused);
    let width = area.width.saturating_sub(2) as u64;
    let ratio = |n: u64| (width * n / total).min(width) as usize;
    let mut parts = vec![
        ("Personal", personal, Color::Cyan),
        ("Junk", junk, Color::Red),
        ("Unused", unused, Color::Yellow),
        ("Free", free, Color::DarkGray),
    ];
    let mut dots = Vec::new();
    let mut legend = Vec::new();
    for (name, amount, color) in parts.drain(..) {
        dots.push(Span::styled(
            "\u{25cf}".repeat(ratio(amount)),
            Style::default().fg(color),
        ));
        legend.push(Span::styled(
            format!("\u{25a0} {name} {}   ", crate::plain::human(amount)),
            Style::default().fg(color),
        ));
    }
    let title = Line::from(vec![
        Span::styled(
            "SWEEP",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  ·  {}", path)),
    ]);
    let content = vec![Line::from(dots), Line::from(legend)];
    f.render_widget(
        Paragraph::new(content).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        ),
        area,
    );
}
