use crate::{plain::human, ui::lang::Language};
use ratatui::{prelude::*, widgets::*};

#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    pub personal: u64,
    pub junk: u64,
    pub unused: u64,
    pub other: u64,
    pub free: u64,
}

impl Usage {
    pub fn total(self) -> u64 {
        self.personal
            .saturating_add(self.junk)
            .saturating_add(self.unused)
            .saturating_add(self.other)
            .saturating_add(self.free)
    }

    pub fn used(self) -> u64 {
        self.total().saturating_sub(self.free)
    }
}

pub fn render(f: &mut Frame, area: Rect, title: &str, usage: Usage, lang: Language) {
    let parts = [
        (lang.text("Personal", "Личное"), usage.personal, Color::Cyan),
        (lang.text("Junk", "Мусор"), usage.junk, Color::Red),
        (
            lang.text("Unused", "Давно не открывали"),
            usage.unused,
            Color::Yellow,
        ),
        (lang.text("Other", "Другое"), usage.other, Color::Magenta),
        (lang.text("Free", "Свободно"), usage.free, Color::DarkGray),
    ];
    let widths = segment_widths(
        &parts.map(|part| part.1),
        area.width.saturating_sub(2) as usize,
    );
    let mut meter = Vec::new();
    let mut legend = Vec::new();
    for ((name, amount, color), width) in parts.into_iter().zip(widths) {
        meter.push(Span::styled("█".repeat(width), Style::default().fg(color)));
        if amount > 0 {
            legend.push(Span::styled(
                format!("■ {name} {}   ", human(amount)),
                Style::default().fg(color),
            ));
        }
    }
    let title = Line::from(vec![
        Span::styled(
            "SWEEP",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  ·  {title}")),
    ]);
    f.render_widget(
        Paragraph::new(vec![Line::from(meter), Line::from(legend)]).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        ),
        area,
    );
}

pub(super) fn segment_widths(amounts: &[u64; 5], width: usize) -> [usize; 5] {
    let total: u128 = amounts.iter().map(|&amount| u128::from(amount)).sum();
    if total == 0 {
        return [0; 5];
    }
    let mut out = [0; 5];
    let mut remainders = [(0, 0u128); 5];
    let mut allocated = 0;
    for (i, amount) in amounts.iter().enumerate() {
        let numerator = width as u128 * u128::from(*amount);
        out[i] = (numerator / total) as usize;
        allocated += out[i];
        remainders[i] = (i, numerator % total);
    }
    remainders
        .sort_by_key(|&(index, remainder)| (amounts[index] == 0, std::cmp::Reverse(remainder)));
    for &(index, _) in remainders.iter().take(width.saturating_sub(allocated)) {
        out[index] += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{render, segment_widths, Usage};
    use crate::ui::lang::Language;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn meter_segments_fill_the_width_without_overflow() {
        assert_eq!(
            segment_widths(&[1, 1, 1, 1, 1], 17).iter().sum::<usize>(),
            17
        );
        assert_eq!(
            segment_widths(&[u64::MAX, u64::MAX, 0, 0, 1], 100)
                .iter()
                .sum::<usize>(),
            100
        );
        assert_eq!(segment_widths(&[u64::MAX, u64::MAX, 0, 0, 0], 99)[4], 0);
        assert_eq!(segment_widths(&[0; 5], 20), [0; 5]);
    }

    #[test]
    fn legend_keeps_small_nonzero_categories_visible() {
        let mut terminal = Terminal::new(TestBackend::new(120, 5)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    "Disk",
                    Usage {
                        personal: 1_000_000_000_000,
                        junk: 1,
                        free: 1_000_000_000_000,
                        ..Usage::default()
                    },
                    Language::En,
                )
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Junk"), "{rendered}");
        assert!(rendered.contains("1.0 B"), "{rendered}");
    }
}
