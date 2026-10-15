//! The state grammar — CONTRACT.md §8.1.
//!
//! Every 9s tool colours things that are *in a state*: a container, a package,
//! a task, a deployment. They share one vocabulary, so a reader who learned it
//! in k9s or p9s can read z9s without learning anything new.
//!
//! The Idle/Failed split is the rule that carries the weight. Painting every
//! stopped thing red is the easy mistake and it destroys the signal: on a
//! working machine most stopped things stopped on purpose, so red stops meaning
//! "look at this" and becomes wallpaper. A container that exited 0, or a
//! package on disk that was never deployed, is Idle — inert, not broken.
//!
//! The corollary costs something and is the point: green is spent only on the
//! live thing.

use ratatui::style::Style;

use crate::theme::Theme;

/// What state a thing is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// Inert: nothing is happening, and nothing is wrong.
    Idle,
    /// Working as intended — running, ready, deployed.
    Ok,
    /// On its way somewhere, and expected to settle.
    Transitional,
    /// Broken, and wants attention.
    Failed,
}

impl Health {
    /// The mark that carries the state when colour cannot.
    ///
    /// Per §8 colour never carries meaning alone: roughly 8% of men cannot
    /// separate red from green — exactly the pair a status column reaches for —
    /// and under `NO_COLOR` there is no palette at all.
    pub fn glyph(self) -> &'static str {
        match self {
            Health::Ok => "●",
            Health::Transitional => "◐",
            Health::Failed => "✖",
            Health::Idle => "·",
        }
    }

    /// The state's name.
    pub fn as_str(self) -> &'static str {
        match self {
            Health::Ok => "ok",
            Health::Transitional => "transitional",
            Health::Failed => "failed",
            Health::Idle => "idle",
        }
    }
}

impl Theme {
    /// Style for a state.
    ///
    /// Under `NO_COLOR` every branch resolves to an unstyled result, because
    /// `paint` drops the colour — the glyph is what survives.
    pub fn health(&self, h: Health) -> Style {
        match h {
            Health::Ok => self.ok(),
            Health::Transitional => self.warn(),
            Health::Failed => self.error(),
            Health::Idle => self.dim(),
        }
    }

    /// Render a state as text plus its style, falling back to the glyph when
    /// the caller has no word of its own.
    ///
    /// Mirrors `signed`: the marking lives in the TEXT and the colour is the
    /// redundant part, not the other way round.
    pub fn health_label(&self, h: Health, word: &str) -> (String, Style) {
        let text = if word.is_empty() {
            h.glyph().to_string()
        } else {
            word.to_string()
        };
        (text, self.health(h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Health; 4] = [
        Health::Idle,
        Health::Ok,
        Health::Transitional,
        Health::Failed,
    ];

    /// CONTRACT §8: colour never carries meaning alone, so every state must be
    /// distinguishable with no palette at all.
    #[test]
    fn every_state_has_a_distinct_glyph() {
        let mut seen = Vec::new();
        for h in ALL {
            let g = h.glyph();
            assert!(!g.is_empty(), "{h:?} has no glyph");
            assert!(!seen.contains(&g), "{h:?} reuses the glyph {g}");
            seen.push(g);
        }
    }

    #[test]
    fn states_are_distinct_in_colour_when_a_palette_exists() {
        let theme = Theme::default();
        assert_ne!(theme.health(Health::Ok), theme.health(Health::Failed));
        assert_ne!(theme.health(Health::Ok), theme.health(Health::Idle));
    }

    /// With colour switched off the styles collapse together, which is exactly
    /// why the glyph is required rather than decorative.
    #[test]
    fn monochrome_collapses_colour_but_not_meaning() {
        let theme = Theme {
            monochrome: true,
            ..Theme::default()
        };
        assert_eq!(theme.health(Health::Ok), theme.health(Health::Failed));
        assert_ne!(Health::Ok.glyph(), Health::Failed.glyph());

        for h in ALL {
            assert!(
                theme.health(h).fg.is_none(),
                "NO_COLOR must mean no colour, {h:?} still had one"
            );
        }
    }

    #[test]
    fn label_prefers_the_callers_word() {
        let theme = Theme::default();
        let (text, _) = theme.health_label(Health::Ok, "running");
        assert_eq!(text, "running");

        let (text, _) = theme.health_label(Health::Failed, "");
        assert_eq!(text, Health::Failed.glyph());
    }
}
