//! Confirmation — the pause before something that cannot be taken back easily.
//!
//! CONTRACT.md §3.1. A dangerous action is not a keystroke; it is a keystroke,
//! a statement of exactly what will happen, and an answer.
//!
//! The reason this lives in the shell rather than in each application is that
//! the *phrasing* is the safety feature. A dialog that says "Merge? (y/n)" has
//! told the user nothing they did not already know. One that says "rewrites 47
//! transactions across 6 files" is the difference between an informed yes and a
//! reflex.
//!
//! # Key ownership
//!
//! While a confirmation is open it owns **every** key, exactly as a form does
//! (§4.1). Nothing behind it may interpret a keystroke — least of all a list
//! view whose `d` still deletes.
//!
//! # Why `y` is not the default
//!
//! There is no "press enter to accept". Enter is the key people press to get
//! past things, and a dialog cleared by reflex is not a confirmation. The answer
//! must be the affirmative key itself.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Widget, Wrap};

use crate::action::{Danger, Hint};
use crate::theme::Theme;

/// What the user said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Still asking.
    Pending,
    Yes,
    No,
}

/// A modal question about an action that is about to happen.
#[derive(Debug, Clone)]
pub struct Confirm {
    title: String,
    /// What will happen, in the user's terms. One fact per line.
    consequences: Vec<String>,
    /// The question itself, e.g. "Merge them?".
    question: String,
    danger: Danger,
    theme: Theme,
}

impl Confirm {
    pub fn new(theme: Theme, title: impl Into<String>, question: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            consequences: Vec::new(),
            question: question.into(),
            danger: Danger::Mutation,
            theme,
        }
    }

    /// State a consequence. Call once per fact; they are listed, not run
    /// together into a paragraph the user will skim.
    pub fn saying(mut self, line: impl Into<String>) -> Self {
        self.consequences.push(line.into());
        self
    }

    pub fn with_danger(mut self, danger: Danger) -> Self {
        self.danger = danger;
        self
    }

    pub fn consequences(&self) -> &[String] {
        &self.consequences
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Resolve a keystroke.
    ///
    /// `y` accepts. `n`, `q` and `esc` decline. **Everything else is ignored**
    /// rather than treated as a decline: a dialog that vanishes on an unrelated
    /// keypress teaches people that keystrokes go nowhere, and the next one they
    /// press will be aimed at whatever was behind it.
    pub fn on_key(&mut self, key: KeyEvent) -> Answer {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => Answer::Yes,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') | KeyCode::Esc => {
                Answer::No
            }
            _ => Answer::Pending,
        }
    }

    pub fn hints(&self) -> Vec<Hint> {
        vec![Hint::new("y", "yes"), Hint::new("n/esc", "no")]
    }

    /// Draw over whatever is behind, centred.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let width = self
            .preferred_width()
            .min(area.width.saturating_sub(2))
            .max(20);
        let height = self.preferred_height().min(area.height).max(5);
        let dialog = centre(area, width, height);

        // The screen behind stays visible around the edges, which keeps the
        // context of the decision on screen; the dialog's own footprint is
        // cleared so it does not read as text over text.
        Clear.render(dialog, buf);

        let accent = match self.danger {
            Danger::Destructive => self.theme.error(),
            _ => self.theme.warn(),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(accent)
            // A column either side. Text against a border reads as cramped, and
            // this dialog is asking someone to make a decision.
            .padding(Padding::horizontal(1))
            .title(Span::styled(format!(" {} ", self.title), accent));
        let inner = block.inner(dialog);
        block.render(dialog, buf);

        let mut lines: Vec<Line> = self
            .consequences
            .iter()
            .map(|c| Line::from(Span::styled(c.clone(), Style::default())))
            .collect();
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(vec![
            Span::styled(self.question.clone(), accent),
            Span::raw("  "),
            Span::styled("y", self.theme.ok()),
            Span::styled(" / ", self.theme.dim()),
            Span::styled("n", self.theme.dim()),
        ]));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }

    fn preferred_width(&self) -> u16 {
        let longest = self
            .consequences
            .iter()
            .map(|c| c.chars().count())
            .chain(std::iter::once(self.question.chars().count() + 10))
            .chain(std::iter::once(self.title.chars().count() + 4))
            .max()
            .unwrap_or(30);
        // Two border columns plus the padding on each side.
        (longest as u16).saturating_add(4)
    }

    fn preferred_height(&self) -> u16 {
        // consequences + blank + question + two border rows
        let body = self.consequences.len() + if self.consequences.is_empty() { 1 } else { 2 };
        (body as u16).saturating_add(2)
    }
}

/// Centre a box in an area, clamped so it always fits.
fn centre(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn confirm() -> Confirm {
        Confirm::new(Theme::default(), "Merge payees", "Merge them?")
            .saying("rewrites 47 transactions")
            .saying("across 6 files")
    }

    #[test]
    fn y_accepts_and_n_declines() {
        assert_eq!(confirm().on_key(key(KeyCode::Char('y'))), Answer::Yes);
        assert_eq!(confirm().on_key(key(KeyCode::Char('n'))), Answer::No);
    }

    #[test]
    fn case_does_not_matter() {
        assert_eq!(confirm().on_key(key(KeyCode::Char('Y'))), Answer::Yes);
        assert_eq!(confirm().on_key(key(KeyCode::Char('N'))), Answer::No);
    }

    #[test]
    fn esc_declines() {
        assert_eq!(confirm().on_key(key(KeyCode::Esc)), Answer::No);
    }

    /// The whole point of a confirmation is that it is not cleared by reflex.
    #[test]
    fn enter_does_not_accept() {
        assert_eq!(
            confirm().on_key(key(KeyCode::Enter)),
            Answer::Pending,
            "enter is the key people press to get past dialogs"
        );
        assert_eq!(confirm().on_key(key(KeyCode::Char(' '))), Answer::Pending);
    }

    /// A stray keystroke must not dismiss the dialog: if it did, the key after
    /// it would land on whatever was behind.
    #[test]
    fn an_unrelated_key_is_ignored_rather_than_declining() {
        assert_eq!(confirm().on_key(key(KeyCode::Char('j'))), Answer::Pending);
        assert_eq!(confirm().on_key(key(KeyCode::Down)), Answer::Pending);
    }

    fn render_to_text(confirm: &Confirm, w: u16, h: u16) -> String {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        confirm.render(area, &mut buf);
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_dialog_states_the_consequences_not_just_the_question() {
        let out = render_to_text(&confirm(), 60, 14);
        assert!(out.contains("47 transactions"), "got:\n{out}");
        assert!(out.contains("6 files"), "got:\n{out}");
        assert!(out.contains("Merge them?"), "got:\n{out}");
    }

    #[test]
    fn it_fits_a_small_terminal_without_panicking() {
        // Narrower and shorter than the dialog wants to be.
        let out = render_to_text(&confirm(), 24, 6);
        assert!(!out.is_empty());
    }

    #[test]
    fn the_screen_behind_stays_visible_around_it() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        // Paint the whole background, then draw the dialog over part of it.
        for y in 0..24 {
            for x in 0..80 {
                buf[(x, y)].set_symbol("#");
            }
        }
        confirm().render(area, &mut buf);
        let top_left = buf[(0u16, 0u16)].symbol().to_string();
        assert_eq!(top_left, "#", "the dialog must not clear the whole screen");
    }
}
