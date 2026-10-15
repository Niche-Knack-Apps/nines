//! Chrome geometry — CONTRACT.md §1.
//!
//! Every fixed width here is measured from the art it holds, never written down
//! as a number. Changing a logo must not require touching a layout constant,
//! because the day it does is the day someone changes the logo and doesn't.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use unicode_width::UnicodeWidthStr;

/// Rows of the root layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chrome {
    /// Header. Zero-height when hidden.
    pub header: Rect,
    /// Command bar. Zero-height unless open.
    pub command: Rect,
    /// The current screen. The only region that flexes.
    pub body: Rect,
    /// Status / flash line. Always one row.
    pub status: Rect,
}

/// Columns of the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Ordered key/value rows.
    pub info: Rect,
    /// Breadcrumb trail — one row.
    pub crumbs: Rect,
    /// Persistent key grid, below the crumbs.
    pub legend: Rect,
    /// Block wordmark.
    pub logotype: Rect,
    /// Mascot art. Its height sets the header height.
    pub mascot: Rect,
}

/// Measure a block of art in terminal cells, ignoring colour markup.
///
/// Markup is stripped so `[#ff0000]X[-]` measures as one cell. Width is the
/// widest line, not the longest string — CJK and emoji occupy two columns, and a
/// logo measured in `chars()` would silently overflow its column.
pub fn art_size(art: &str) -> (u16, u16) {
    let mut width = 0usize;
    let mut height = 0usize;
    for line in art.lines() {
        height += 1;
        width = width.max(UnicodeWidthStr::width(strip_markup(line).as_str()));
    }
    (width as u16, height as u16)
}

/// Remove `[...]` colour tags.
///
/// Follows the tview convention the shared art is written in, so the same logo
/// measures identically in both implementations: a tag runs to its closing
/// bracket, and a literal `[` is escaped as `[[]` — the escape swallows the
/// closing bracket too, which is the part that is easy to get wrong.
fn strip_markup(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '[' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next(); // the doubled bracket
            out.push('[');
            // `[[]` is one escape: consume through its closing bracket so the
            // `]` is not counted as a visible cell.
            if chars.peek() == Some(&']') {
                chars.next();
            }
            continue;
        }
        // Skip to the closing bracket. An unterminated tag consumes the rest of
        // the line, which is what a renderer would do with it anyway.
        for c in chars.by_ref() {
            if c == ']' {
                break;
            }
        }
    }
    out
}

/// Split the terminal into chrome rows.
///
/// `header_height` is normally the mascot's height; pass 0 to hide the header
/// (`ctrl-e`). The command bar occupies a row only while open, pushing the body
/// down rather than covering it.
pub fn chrome(area: Rect, header_height: u16, command_open: bool) -> Chrome {
    let command_height = u16::from(command_open);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(command_height),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    Chrome {
        header: rows[0],
        command: rows[1],
        body: rows[2],
        status: rows[3],
    }
}

/// Split the header into its four columns.
///
/// Info and middle flex 2:3; the two art blocks take exactly the width they
/// need, plus the one-cell gutters that keep them from touching.
pub fn header(area: Rect, logotype: &str, mascot: &str) -> Header {
    let (logotype_w, _) = art_size(logotype);
    let (mascot_w, _) = art_size(mascot);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(2),
            Constraint::Fill(3),
            Constraint::Length(logotype_w.saturating_add(2)),
            Constraint::Length(mascot_w.saturating_add(1)),
        ])
        .split(area);

    // Breadcrumbs sit on one row with the legend filling what is left.
    let middle = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(cols[1]);

    Header {
        info: cols[0],
        crumbs: middle[0],
        legend: middle[1],
        logotype: cols[2],
        mascot: cols[3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASCOT: &str = "  ___\n /o o\\\n \\___/";
    const LOGOTYPE: &str = "N I N E S";

    fn area(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    /// Art measurement is the one piece of geometry both implementations must
    /// agree on exactly: shared artwork that lays out differently in Go and Rust
    /// is the drift this repository exists to prevent.
    ///
    /// The same table lives in go/conformance_test.go. Changing a case here
    /// without changing it there is a contract break.
    #[test]
    fn art_size_matches_the_contract() {
        let cases: &[(&str, &str, u16, u16)] = &[
            ("empty reserves nothing", "", 0, 0),
            ("single line", "abc", 3, 1),
            ("width is the widest line", "ab\nabcd\nabc", 4, 3),
            ("colour markup is invisible", "[#ff0000]X[-]", 1, 1),
            ("any tag is markup, not just hex", "[red]ab[-]c", 3, 1),
            ("escaped bracket is one cell", "[[]", 1, 1),
            ("wide glyphs take two columns", "\u{6f22}\u{5b57}", 4, 1),
            ("unterminated tag runs to end of line", "ab[unclosed", 2, 1),
            ("trailing newline terminates, not adds", "a\n", 1, 1),
            ("a blank line between art IS a line", "a\n\n", 1, 2),
        ];
        for (name, art, w, h) in cases {
            assert_eq!(
                art_size(art),
                (*w, *h),
                "{name}: art_size({art:?}) disagrees with the contract"
            );
        }
    }

    #[test]
    fn art_is_measured_by_widest_line() {
        assert_eq!(art_size(MASCOT), (6, 3));
        assert_eq!(art_size(LOGOTYPE), (9, 1));
    }

    #[test]
    fn markup_does_not_count_toward_width() {
        assert_eq!(art_size("[#ff0000]X[-]"), (1, 1));
        assert_eq!(art_size("[red]ab[-]c"), (3, 1));
    }

    #[test]
    fn an_escaped_bracket_is_one_cell() {
        assert_eq!(art_size("[[]"), (1, 1));
    }

    #[test]
    fn wide_glyphs_count_as_two_cells() {
        // A logo measured with chars() would under-report this by half and
        // overflow its column.
        assert_eq!(art_size("漢字"), (4, 1));
    }

    #[test]
    fn status_is_always_present_and_exactly_one_row() {
        for (w, h) in [(80, 24), (40, 10), (200, 60)] {
            let c = chrome(area(w, h), 3, false);
            assert_eq!(c.status.height, 1, "status must always be one row");
            assert_eq!(c.status.y + c.status.height, h, "status must be last");
        }
    }

    #[test]
    fn the_command_bar_pushes_the_body_down_rather_than_covering_it() {
        let closed = chrome(area(80, 24), 3, false);
        let open = chrome(area(80, 24), 3, true);

        assert_eq!(closed.command.height, 0);
        assert_eq!(open.command.height, 1);
        assert_eq!(
            open.body.height,
            closed.body.height - 1,
            "opening the bar must cost the body a row, not overlay it"
        );
        assert!(open.command.y < open.body.y, "the bar sits above the body");
    }

    #[test]
    fn hiding_the_header_gives_its_rows_to_the_body() {
        let shown = chrome(area(80, 24), 5, false);
        let hidden = chrome(area(80, 24), 0, false);
        assert_eq!(shown.header.height, 5);
        assert_eq!(hidden.header.height, 0);
        assert_eq!(
            hidden.body.height,
            shown.body.height + 5,
            "ctrl-e must trade chrome for rows"
        );
    }

    #[test]
    fn chrome_rows_tile_the_area_without_gaps_or_overlap() {
        let a = area(80, 24);
        let c = chrome(a, 4, true);
        assert_eq!(c.header.y, 0);
        assert_eq!(c.command.y, c.header.y + c.header.height);
        assert_eq!(c.body.y, c.command.y + c.command.height);
        assert_eq!(c.status.y, c.body.y + c.body.height);
        assert_eq!(c.status.y + c.status.height, a.height);
    }

    #[test]
    fn art_columns_take_exactly_their_measured_width() {
        let h = header(area(120, 3), LOGOTYPE, MASCOT);
        assert_eq!(h.logotype.width, art_size(LOGOTYPE).0 + 2);
        assert_eq!(h.mascot.width, art_size(MASCOT).0 + 1);
    }

    /// The rule the constant would break: a bigger logo must move the columns,
    /// not overflow them.
    #[test]
    fn a_wider_logo_widens_its_column_and_narrows_the_flex() {
        let narrow = header(area(120, 3), "abc", MASCOT);
        let wide = header(area(120, 3), "abcdefghijkl", MASCOT);
        assert!(wide.logotype.width > narrow.logotype.width);
        assert!(
            wide.info.width + wide.crumbs.width < narrow.info.width + narrow.crumbs.width,
            "the flexible columns must yield the space"
        );
    }

    #[test]
    fn header_columns_tile_the_row() {
        let a = area(120, 4);
        let h = header(a, LOGOTYPE, MASCOT);
        assert_eq!(h.info.x, 0);
        assert_eq!(h.crumbs.x, h.info.x + h.info.width);
        assert_eq!(h.logotype.x, h.crumbs.x + h.crumbs.width);
        assert_eq!(h.mascot.x, h.logotype.x + h.logotype.width);
        assert_eq!(h.mascot.x + h.mascot.width, a.width);
    }

    #[test]
    fn crumbs_are_one_row_and_the_legend_takes_the_rest() {
        let h = header(area(120, 6), LOGOTYPE, MASCOT);
        assert_eq!(h.crumbs.height, 1);
        assert_eq!(h.legend.height, 5);
        assert_eq!(h.legend.y, h.crumbs.y + 1);
    }

    /// A terminal too small for the chrome must not panic and must not produce
    /// geometry outside the area.
    ///
    /// Note what is *not* asserted: that the status line survives. At two rows
    /// there is genuinely not enough space for a 5-row header, a command bar and
    /// a status line, and something has to give. The guarantee is that whatever
    /// comes back is in bounds, not that every region keeps its size.
    #[test]
    fn a_tiny_terminal_degrades_instead_of_panicking() {
        for (w, h, header_h, cmd) in [(20, 2, 5, true), (1, 1, 3, false), (10, 0, 0, true)] {
            let a = area(w, h);
            let c = chrome(a, header_h, cmd);
            for r in [c.header, c.command, c.body, c.status] {
                assert!(
                    r.x + r.width <= a.width && r.y + r.height <= a.height,
                    "{r:?} escapes {a:?}"
                );
            }
        }
    }
}
