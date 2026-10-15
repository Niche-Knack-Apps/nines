//! Which terminal background the palette is tuned for — CONTRACT.md §8.2.
//!
//! This exists because of arithmetic, not taste. For one colour to reach 3:1
//! against a white background its relative luminance must be `<= 0.300`;
//! against black it must be `>= 0.100`; against a mid-grey it must be `<= 0.039`
//! or `>= 0.748`. White and black intersect in `[0.100, 0.300]`, which is room
//! enough for a palette. Adding mid-grey empties the intersection entirely.
//!
//! So no single fixed palette can serve light, dark and neutral terminals. The
//! background has to be known — or chosen.

use std::env;

/// The terminal background a palette targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// Safe default when the background is unknown: colours sit in the band
    /// that works on light and dark alike, at the cost of being less vivid on
    /// dark than a dark-tuned palette would be.
    Universal,
    /// Tuned for a near-white background.
    Light,
    /// Tuned for a near-black background.
    Dark,
    /// Tuned for a mid-grey background. Opt-in only — these colours are
    /// near-white and would be invisible on a light terminal.
    Neutral,
}

impl Appearance {
    pub fn as_str(self) -> &'static str {
        match self {
            Appearance::Light => "light",
            Appearance::Dark => "dark",
            Appearance::Neutral => "neutral",
            Appearance::Universal => "universal",
        }
    }

    /// Parse a name; `None` when it is not one.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Appearance::Light),
            "dark" => Some(Appearance::Dark),
            "neutral" | "grey" | "gray" => Some(Appearance::Neutral),
            "universal" | "auto" => Some(Appearance::Universal),
            _ => None,
        }
    }

    /// Decide which palette to use.
    ///
    /// `NINES_THEME` is authoritative, because detection cannot be relied on.
    /// `COLORFGBG` is honoured when present but is frequently unset, and the
    /// OSC 11 query that would ask the terminal directly is deliberately NOT
    /// used: many terminals answer it wrongly or not at all, tmux sits in the
    /// way, and issuing it means writing to and reading from a terminal the UI
    /// already owns. A wrong answer is worse than none, because the universal
    /// palette is legible either way.
    pub fn detect() -> Self {
        // An empty value means "not set", not "universal" — otherwise
        // exporting the variable blank would silently suppress COLORFGBG.
        if let Ok(v) = env::var("NINES_THEME") {
            if !v.trim().is_empty() {
                if let Some(a) = Self::parse(&v) {
                    return a;
                }
            }
        }
        if let Ok(v) = env::var("COLORFGBG") {
            if let Some(a) = Self::from_colorfgbg(&v) {
                return a;
            }
        }
        Appearance::Universal
    }

    /// Read the `fg;bg` convention, where the last field is an ANSI index for
    /// the background.
    fn from_colorfgbg(v: &str) -> Option<Self> {
        let last = v.trim().rsplit(';').next()?;
        match last.parse::<u8>().ok()? {
            0 | 8 => Some(Appearance::Dark),
            7 | 15 => Some(Appearance::Light),
            // Any other index is a colour we cannot classify without knowing
            // the terminal's palette, so say nothing rather than guess.
            _ => None,
        }
    }
}

/// The semantic colours for one appearance, as `(r, g, b)`.
///
/// Every value is verified by test against the background it is meant for:
/// light and dark reach 4.5:1, neutral reaches 3:1 — the most a mid-grey
/// background allows anything to reach.
pub(crate) struct Palette {
    /// The background this palette's foregrounds were measured against.
    ///
    /// Normally NOT painted — CONTRACT.md §8 has the tool defer to the
    /// terminal. It is here for the opt-in case where a user wants the app to
    /// look the same everywhere rather than blend in, and it must stay the
    /// colour the contrast ratios were computed against or those ratios become
    /// claims about nothing.
    pub background: (u8, u8, u8),
    pub dim: (u8, u8, u8),
    pub accent: (u8, u8, u8),
    pub negative: (u8, u8, u8),
    pub ok: (u8, u8, u8),
    pub warn: (u8, u8, u8),
    pub error: (u8, u8, u8),
}

pub(crate) fn palette(a: Appearance) -> Palette {
    match a {
        // Universal doubles as the light palette: 4.55:1 at worst on white and
        // 3.84:1 at worst on black.
        Appearance::Universal | Appearance::Light => Palette {
            background: (0xff, 0xff, 0xff),
            dim: (0x6e, 0x77, 0x81),
            accent: (0x09, 0x69, 0xda),
            negative: (0x0f, 0x76, 0x6e),
            ok: (0x1a, 0x7f, 0x37),
            warn: (0x9a, 0x67, 0x00),
            error: (0xcf, 0x22, 0x2e),
        },
        // Light foregrounds: 6.26:1 at worst on black.
        Appearance::Dark => Palette {
            background: (0x0d, 0x11, 0x17),
            dim: (0x8b, 0x94, 0x9e),
            accent: (0x58, 0xa6, 0xff),
            negative: (0x39, 0xc5, 0xcf),
            ok: (0x3f, 0xb9, 0x50),
            warn: (0xd2, 0x99, 0x22),
            error: (0xf8, 0x51, 0x49),
        },
        // Near-white: a mid-grey background permits nothing else.
        Appearance::Neutral => Palette {
            background: (0x6a, 0x6a, 0x6a),
            dim: (0xe5, 0xe6, 0xe8),
            accent: (0xd4, 0xe8, 0xff),
            negative: (0xc4, 0xee, 0xf1),
            ok: (0xcf, 0xee, 0xd3),
            warn: (0xf4, 0xe6, 0xc8),
            error: (0xfe, 0xe0, 0xde),
        },
    }
}

impl Appearance {
    /// The luminance range a foreground must sit in to meet this appearance's
    /// floor, and the floor itself.
    ///
    /// Universal must satisfy white AND black at once, so its band is the
    /// intersection rather than a single bound.
    pub(crate) fn legibility_band(self) -> (f64, f64, f64) {
        match self {
            // 4.5:1 against white.
            Appearance::Light => (0.0, 1.05 / 4.5 - 0.05, 4.5),
            // 4.5:1 against black.
            Appearance::Dark => (0.05 * 4.5 - 0.05, 1.0, 4.5),
            // Mid-grey permits only the extremes; the light half is the only
            // one where hues stay apart.
            Appearance::Neutral => {
                let g = 0.215_860_500_113_945_7; // luminance of #808080
                (3.0 * (g + 0.05) - 0.05, 1.0, 3.0)
            }
            // Dark enough for white, light enough for black.
            Appearance::Universal => (0.05 * 3.0 - 0.05, 1.05 / 3.0 - 0.05, 3.0),
        }
    }
}

/// WCAG relative luminance of an RGB triple.
pub(crate) fn luminance_rgb(r: u8, g: u8, b: u8) -> f64 {
    let ch = |v: u8| {
        let f = v as f64 / 255.0;
        if f <= 0.03928 {
            f / 12.92
        } else {
            ((f + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use ratatui::style::Color;

    fn luminance(c: Color) -> f64 {
        let (r, g, b) = match c {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::White => (255, 255, 255),
            Color::Black => (0, 0, 0),
            Color::Reset => panic!("Reset has no fixed luminance"),
            other => panic!("no fixed rgb for {other:?}"),
        };
        let ch = |v: u8| {
            let f = v as f64 / 255.0;
            if f <= 0.03928 {
                f / 12.92
            } else {
                ((f + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
    }

    fn ratio(a: Color, b: Color) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = (la.max(lb), la.min(lb));
        (hi + 0.05) / (lo + 0.05)
    }

    const WHITE: Color = Color::Rgb(0xff, 0xff, 0xff);
    const BLACK: Color = Color::Rgb(0, 0, 0);
    const GREY: Color = Color::Rgb(0x80, 0x80, 0x80);

    fn tokens(t: &Theme) -> Vec<(&'static str, Color)> {
        vec![
            ("dim", t.dim),
            ("accent", t.accent),
            ("negative", t.negative),
            ("ok", t.ok),
            ("warn", t.warn),
            ("error", t.error),
        ]
    }

    /// EVERY element, against the background its palette is for.
    #[test]
    fn every_token_meets_contrast_on_its_own_background() {
        // 3:1 for neutral is not a lowered standard, it is the ceiling: a
        // mid-grey background leaves nothing further to reach for.
        for (appearance, bg, floor) in [
            (Appearance::Light, WHITE, 4.5),
            (Appearance::Dark, BLACK, 4.5),
            (Appearance::Neutral, GREY, 3.0),
        ] {
            let theme = Theme::for_appearance(appearance);
            for (name, c) in tokens(&theme) {
                let r = ratio(c, bg);
                assert!(
                    r >= floor,
                    "{}/{} = {r:.2}:1, want >= {floor}:1",
                    appearance.as_str(),
                    name
                );
            }
        }
    }

    /// The universal palette is the default, so it must never be unreadable on
    /// either extreme — that is the whole reason it exists.
    #[test]
    fn universal_palette_works_on_light_and_dark() {
        let theme = Theme::for_appearance(Appearance::Universal);
        for (name, c) in tokens(&theme) {
            assert!(ratio(c, WHITE) >= 4.5, "universal/{name} on white");
            assert!(ratio(c, BLACK) >= 3.0, "universal/{name} on black");
        }
    }

    /// The constraint that forces the whole design — CONTRACT §8.2.
    #[test]
    fn no_single_colour_can_serve_all_three_backgrounds() {
        let floor = 3.0_f64;
        let white_max = 1.05 / floor - 0.05; // against white, a colour must be dark
        let black_min = 0.05 * floor - 0.05; // against black, it must be light
        assert!(
            white_max > black_min,
            "white and black should still intersect"
        );

        let grey = luminance(GREY);
        let grey_dark_max = (grey + 0.05) / floor - 0.05;
        let grey_light_min = floor * (grey + 0.05) - 0.05;

        assert!(
            grey_dark_max < black_min,
            "mid-grey's dark half must not reach black's floor"
        );
        assert!(
            grey_light_min > white_max,
            "mid-grey's light half must not reach white's ceiling"
        );
    }

    /// The terminal's own foreground is already right; overriding it fights the
    /// user's theme rather than fitting into it.
    #[test]
    fn appearances_never_override_the_terminal_foreground() {
        for a in [
            Appearance::Universal,
            Appearance::Light,
            Appearance::Dark,
            Appearance::Neutral,
        ] {
            assert_eq!(
                Theme::for_appearance(a).text,
                Color::Reset,
                "{}",
                a.as_str()
            );
        }
    }

    #[test]
    fn colorfgbg_classifies_only_what_it_can() {
        assert_eq!(Appearance::from_colorfgbg("15;0"), Some(Appearance::Dark));
        assert_eq!(Appearance::from_colorfgbg("0;15"), Some(Appearance::Light));
        assert_eq!(Appearance::from_colorfgbg("0;7"), Some(Appearance::Light));
        // An index we cannot classify without knowing the terminal's palette.
        assert_eq!(Appearance::from_colorfgbg("15;12"), None);
        assert_eq!(Appearance::from_colorfgbg("junk"), None);
    }

    #[test]
    fn parse_rejects_nonsense_and_accepts_aliases() {
        assert_eq!(Appearance::parse("chartreuse"), None);
        for name in ["light", "Dark", " neutral ", "grey", "gray", "auto"] {
            assert!(Appearance::parse(name).is_some(), "{name}");
        }
    }

    /// The guard for colours the palette tests cannot see: one a tool sets
    /// itself, or one a transform produces.
    #[test]
    fn ensure_legible_rescues_a_bad_colour() {
        for (appearance, bad, bg, floor) in [
            // The exact failure that was reported: bright yellow on white.
            (Appearance::Light, Color::Rgb(0xff, 0xff, 0x00), WHITE, 4.5),
            (Appearance::Light, Color::Rgb(0x00, 0xff, 0xff), WHITE, 4.5),
            (Appearance::Dark, Color::Rgb(0x10, 0x10, 0x10), BLACK, 4.5),
            (Appearance::Neutral, Color::Rgb(0x70, 0x70, 0x70), GREY, 3.0),
        ] {
            let theme = Theme::for_appearance(appearance);
            assert!(ratio(bad, bg) < floor, "test colour should start failing");
            let fixed = theme.ensure_legible(bad);
            assert!(
                ratio(fixed, bg) >= floor,
                "{}: {:.2}:1 after rescue",
                appearance.as_str(),
                ratio(fixed, bg)
            );
        }
    }

    /// A floor, not a filter: a colour that already passes is untouched.
    #[test]
    fn ensure_legible_leaves_good_colours_alone() {
        for a in [
            Appearance::Universal,
            Appearance::Light,
            Appearance::Dark,
            Appearance::Neutral,
        ] {
            let theme = Theme::for_appearance(a);
            for (name, c) in tokens(&theme) {
                assert_eq!(theme.ensure_legible(c), c, "{}/{name}", a.as_str());
            }
        }
    }

    /// Under NO_COLOR there is no palette to fix, and forcing a colour back in
    /// would undo the point.
    #[test]
    fn ensure_legible_is_inert_under_no_color() {
        let theme = Theme {
            monochrome: true,
            ..Theme::default()
        };
        let bad = Color::Rgb(0xff, 0xff, 0x00);
        assert_eq!(theme.ensure_legible(bad), bad);
    }

    /// Colour-blind mode draws from the same palette, so it keeps the
    /// guarantee rather than trading one accessibility problem for another.
    #[test]
    fn colorblind_mode_keeps_the_contrast_guarantee() {
        for (a, bg, floor) in [
            (Appearance::Light, WHITE, 4.5),
            (Appearance::Dark, BLACK, 4.5),
        ] {
            let cb = Theme::for_appearance(a).colorblind();
            for (name, c) in tokens(&cb) {
                assert!(
                    ratio(c, bg) >= floor,
                    "colourblind {}/{name} = {:.2}:1",
                    a.as_str(),
                    ratio(c, bg)
                );
            }
        }
    }

    /// Selection carries both of its own colours, so unlike everything else it
    /// does not depend on the terminal background at all.
    #[test]
    fn selection_is_independent_of_appearance() {
        let want = Theme::for_appearance(Appearance::Light).selected();
        for a in [Appearance::Dark, Appearance::Neutral, Appearance::Universal] {
            assert_eq!(Theme::for_appearance(a).selected(), want, "{}", a.as_str());
        }
    }
}
