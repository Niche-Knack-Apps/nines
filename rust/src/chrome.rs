//! The header and status widgets — CONTRACT.md §1.
//!
//! These are plain render functions over a `Buffer` rather than stateful
//! widgets, which keeps them testable the way the contract insists screens are:
//! give one a rect and a buffer, then assert on the cells. No terminal, no event
//! loop.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::action::Hint;
use crate::theme::Theme;

/// One row of the info panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoField {
    pub key: String,
    pub value: String,
}

/// Ordered `key: value` rows.
///
/// Insertion order is the render order, and updating an existing key keeps its
/// position. Sorting would make the panel reshuffle whenever a slow value
/// arrives, and a header that moves while you are reading it is worse than one
/// in an arbitrary but stable order.
#[derive(Debug, Clone, Default)]
pub struct InfoPanel {
    fields: Vec<InfoField>,
}

impl InfoPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update `key`, keeping its original position.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        match self.fields.iter_mut().find(|f| f.key == key) {
            Some(existing) => existing.value = value,
            None => self.fields.push(InfoField { key, value }),
        }
    }

    pub fn fields(&self) -> &[InfoField] {
        &self.fields
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let width = self
            .fields
            .iter()
            .map(|f| f.key.chars().count())
            .max()
            .unwrap_or(0);

        let lines: Vec<Line> = self
            .fields
            .iter()
            .take(area.height as usize)
            .map(|f| {
                Line::from(vec![
                    Span::styled(format!("{:>width$}: ", f.key, width = width), theme.dim()),
                    Span::styled(f.value.clone(), Style::default()),
                ])
            })
            .collect();

        Paragraph::new(lines).render(area, buf);
    }
}

/// Render the drill-down trail, e.g. `balances > Assets:US Bank:Checking`.
///
/// Truncates from the left when it will not fit: the tail is where the user is,
/// and losing the root is less disorienting than losing the leaf.
pub fn render_breadcrumbs(crumbs: &[String], area: Rect, buf: &mut Buffer, theme: &Theme) {
    if crumbs.is_empty() || area.height == 0 {
        return;
    }

    let separator = " > ";
    let mut spans: Vec<Span> = Vec::new();
    for (i, crumb) in crumbs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(separator, theme.dim()));
        }
        let last = i + 1 == crumbs.len();
        spans.push(Span::styled(
            crumb.clone(),
            if last { theme.header() } else { theme.dim() },
        ));
    }

    let full: String = spans.iter().map(|s| s.content.as_ref()).collect();
    if full.chars().count() > area.width as usize {
        let keep = area.width.saturating_sub(1) as usize;
        let tail: String = full
            .chars()
            .rev()
            .take(keep)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Paragraph::new(Line::from(vec![
            Span::styled("…", theme.dim()),
            Span::styled(tail, theme.header()),
        ]))
        .render(area, buf);
        return;
    }

    Paragraph::new(Line::from(spans)).render(area, buf);
}

/// Render the persistent key grid.
///
/// Column-major, so reading down a column follows the order hints were declared
/// — the same order the tool thinks about them in. Row-major would scatter
/// related bindings across the grid as the width changes.
pub fn render_legend(hints: &[Hint], area: Rect, buf: &mut Buffer, theme: &Theme) {
    if hints.is_empty() || area.height == 0 || area.width == 0 {
        return;
    }

    let rows = area.height as usize;
    let columns = hints.len().div_ceil(rows);

    // Size each column to its own widest entry rather than dividing the width
    // evenly. Even division truncates a long description just because some other
    // column is short — the first version of this rendered "refres" and "comman"
    // while leaving blank space beside `<q> quit`.
    let mut cells: Vec<Vec<String>> = Vec::with_capacity(columns);
    for column in 0..columns {
        let mut cell_column = Vec::with_capacity(rows);
        // Keys align within a column, not across the whole grid, so a single
        // wide key does not indent every description on screen.
        let key_width = (0..rows)
            .filter_map(|row| hints.get(column * rows + row))
            .map(|h| h.key.chars().count())
            .max()
            .unwrap_or(0);
        for row in 0..rows {
            cell_column.push(match hints.get(column * rows + row) {
                Some(h) => format!("<{:width$}> {}", h.key, h.desc, width = key_width),
                None => String::new(),
            });
        }
        cell_column
            .iter_mut()
            .for_each(|c| *c = c.trim_end().to_string());
        cells.push(cell_column);
    }

    let gutter = 2usize;
    let widths: Vec<usize> = cells
        .iter()
        .map(|c| c.iter().map(|s| s.chars().count()).max().unwrap_or(0))
        .collect();

    let mut lines: Vec<Line> = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut spans: Vec<Span> = Vec::new();
        let mut used = 0usize;
        for (column, width) in widths.iter().enumerate() {
            let needed = width + gutter;
            // Drop a column that will not fit rather than shaving every entry.
            if used + needed > area.width as usize && used > 0 {
                break;
            }
            let cell = &cells[column][row];
            spans.push(Span::styled(
                format!("{cell:<w$}", w = width + gutter),
                theme.dim(),
            ));
            used += needed;
        }
        lines.push(Line::from(spans));
    }

    Paragraph::new(lines).render(area, buf);
}

/// What the status line is currently showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Nothing to say — the line shows the current hints.
    Idle,
    Info(String),
    Ok(String),
    Warn(String),
    Error(String),
}

impl Status {
    /// Render, falling back to `hints` when idle.
    ///
    /// A flash replaces the hints rather than sharing the row, which is exactly
    /// why the legend exists separately: the reference must survive the message.
    pub fn render(&self, hints: &[Hint], area: Rect, buf: &mut Buffer, theme: &Theme) {
        let line = match self {
            Status::Idle => Line::from(
                hints
                    .iter()
                    .flat_map(|h| {
                        vec![
                            Span::styled(format!("<{}> ", h.key), theme.header()),
                            Span::styled(format!("{}  ", h.desc), theme.dim()),
                        ]
                    })
                    .collect::<Vec<_>>(),
            ),
            Status::Info(m) => Line::from(Span::styled(m.clone(), theme.dim())),
            Status::Ok(m) => Line::from(Span::styled(m.clone(), theme.ok())),
            Status::Warn(m) => Line::from(Span::styled(m.clone(), theme.warn())),
            Status::Error(m) => Line::from(Span::styled(m.clone(), theme.error())),
        };
        Paragraph::new(line).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(w: u16, h: u16) -> (Rect, Buffer) {
        let area = Rect::new(0, 0, w, h);
        (area, Buffer::empty(area))
    }

    fn text(buf: &Buffer) -> String {
        let area = *buf.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn info_fields_keep_insertion_order_when_updated() {
        let mut panel = InfoPanel::new();
        panel.set("Project", "batteas-ledger");
        panel.set("Accounts", "67");
        panel.set("Source", "index");

        // A late-arriving value must not reorder the panel.
        panel.set("Project", "something-else");

        let keys: Vec<&str> = panel.fields().iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["Project", "Accounts", "Source"]);
        assert_eq!(panel.fields()[0].value, "something-else");
    }

    #[test]
    fn info_panel_renders_keys_and_values() {
        let mut panel = InfoPanel::new();
        panel.set("Project", "batteas-ledger");
        panel.set("Source", "ledger files");

        let (area, mut buf) = buffer(40, 2);
        panel.render(area, &mut buf, &Theme::default());
        let out = text(&buf);
        assert!(out.contains("Project: batteas-ledger"), "got:\n{out}");
        assert!(out.contains("Source: ledger files"), "got:\n{out}");
    }

    #[test]
    fn breadcrumbs_join_with_separators() {
        let (area, mut buf) = buffer(40, 1);
        render_breadcrumbs(
            &["balances".into(), "Assets:Cash".into()],
            area,
            &mut buf,
            &Theme::default(),
        );
        assert!(text(&buf).starts_with("balances > Assets:Cash"));
    }

    #[test]
    fn breadcrumbs_truncate_from_the_left_keeping_where_you_are() {
        let (area, mut buf) = buffer(20, 1);
        render_breadcrumbs(
            &[
                "dashboard".into(),
                "balances".into(),
                "Assets:US Bank:Checking".into(),
            ],
            area,
            &mut buf,
            &Theme::default(),
        );
        let out = text(&buf);
        assert!(out.starts_with('…'), "should mark the elision: {out:?}");
        assert!(
            out.contains("Checking"),
            "the leaf is where the user is and must survive: {out:?}"
        );
    }

    #[test]
    fn legend_fills_columns_downward() {
        let hints: Vec<Hint> = ["a", "b", "c", "d"]
            .iter()
            .map(|k| Hint::new(*k, format!("do-{k}")))
            .collect();

        let (area, mut buf) = buffer(40, 2);
        render_legend(&hints, area, &mut buf, &Theme::default());
        let out = text(&buf);
        let first_row = out.lines().next().unwrap_or_default();

        // Column-major over 2 rows: a and b fill column one, so row one holds a
        // and c. Row-major would put a and b together.
        assert!(first_row.contains("do-a"), "got:\n{out}");
        assert!(
            first_row.contains("do-c"),
            "second column should start the third hint, not the second:\n{out}"
        );
    }

    #[test]
    fn an_empty_legend_renders_nothing_rather_than_panicking() {
        let (area, mut buf) = buffer(40, 3);
        render_legend(&[], area, &mut buf, &Theme::default());
        assert!(text(&buf).trim().is_empty());
    }

    #[test]
    fn status_shows_hints_when_idle_and_the_message_otherwise() {
        let hints = vec![Hint::new("r", "refresh")];
        let theme = Theme::default();

        let (area, mut buf) = buffer(60, 1);
        Status::Idle.render(&hints, area, &mut buf, &theme);
        assert!(text(&buf).contains("refresh"));

        let (area, mut buf) = buffer(60, 1);
        Status::Error("could not reach the remote".into()).render(&hints, area, &mut buf, &theme);
        let out = text(&buf);
        assert!(out.contains("could not reach the remote"));
        assert!(
            !out.contains("refresh"),
            "a flash replaces the hints — which is why the legend exists separately"
        );
    }

    #[test]
    fn a_zero_height_area_is_not_a_panic() {
        let (area, mut buf) = buffer(40, 0);
        render_breadcrumbs(&["x".into()], area, &mut buf, &Theme::default());
        render_legend(&[Hint::new("a", "b")], area, &mut buf, &Theme::default());
    }
}
