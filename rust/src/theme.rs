//! Colour — CONTRACT.md §8.
//!
//! Two rules here are not stylistic:
//!
//! 1. **Selection sets both colours explicitly.** The obvious implementation is
//!    reverse video, and it is a trap: against a default foreground on a default
//!    background, reversing resolves to default-on-default in some terminals and
//!    the selected row's text disappears entirely. z9s shipped that bug.
//! 2. **Colour never carries meaning alone.** Around 8% of men cannot separate
//!    red from green, which is precisely the pair tools reach for to mean
//!    bad/good. Every semantic colour here has a glyph beside it.
//! 3. **A negative number is not an error.** The first version of this palette
//!    had no `negative` role, so screens painted every negative amount with
//!    `error()`. A ledger is mostly negative amounts, so the result was a screen
//!    of alarm-red that rendered salmon-pink on a good many terminal palettes —
//!    and told the reader that ordinary spending was a fault. Outflows and
//!    faults are different things and now have different colours.
//!
//! # The palette
//!
//! Cool-forward, after k9s: its table borders and rows are cyan, its chrome
//! amber. Cyan carries structure here — headers, borders, focus — green means
//! money in, blue means money out. Red is reserved for things that are actually
//! wrong, which on a healthy ledger is nothing.

use ratatui::style::{Color, Modifier, Style};

use crate::appearance::{luminance_rgb, palette, Appearance};

/// Palette and the styles derived from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub accent: Color,
    /// Selection background. Deliberately a fixed dark value rather than an
    /// ANSI index: see `selected`.
    /// The background this palette's foregrounds were measured against.
    pub background: Color,
    /// Paint [`background`] instead of letting the terminal's own show through.
    ///
    /// Off by default: CONTRACT.md §8.3. On, the tool looks the same in every
    /// terminal rather than blending into the user's scheme — which is the only
    /// way "dark" can mean a dark app inside a light terminal.
    ///
    /// [`background`]: Self::background
    pub paint_background: bool,
    pub select: Color,
    pub ok: Color,
    /// Money going out. Deliberately NOT `error` — see the module docs.
    pub negative: Color,
    pub warn: Color,
    pub error: Color,
    pub dim: Color,
    pub text: Color,
    /// Drop to glyph-led, colour-blind-safe hues.
    pub colorblind: bool,
    /// Emit no colour at all (`NO_COLOR`).
    pub monochrome: bool,
    /// Which terminal background this palette was tuned for.
    pub appearance: Appearance,
}

impl Theme {
    /// Build the palette for a background — CONTRACT §8.2.
    ///
    /// `text` stays `Reset` in every appearance: the terminal's own foreground
    /// is already right, and overriding it fights the user's theme rather than
    /// fitting into it.
    pub fn for_appearance(a: Appearance) -> Self {
        let p = palette(a);
        let rgb = |c: (u8, u8, u8)| Color::Rgb(c.0, c.1, c.2);
        Self {
            background: rgb(p.background),
            paint_background: paint_background_requested(),
            accent: rgb(p.accent),
            select: Color::Rgb(0x1f, 0x33, 0x50),
            ok: rgb(p.ok),
            negative: rgb(p.negative),
            warn: rgb(p.warn),
            error: rgb(p.error),
            dim: rgb(p.dim),
            text: Color::Reset,
            colorblind: false,
            monochrome: false,
            appearance: a,
        }
        .enforce()
    }
}

/// Has the user asked for the background to be painted?
///
/// An environment variable rather than a config field so it reaches every 9s
/// tool at once, including those with no config file of their own.
fn paint_background_requested() -> bool {
    matches!(
        std::env::var("NINES_PAINT_BACKGROUND")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// The foreground of a selected row. Fixed RGB, never a palette index — see
/// [`Theme::selected`].
pub const SELECTION_TEXT: Color = Color::Rgb(0xff, 0xff, 0xff);

impl Default for Theme {
    /// The universal palette: legible on a light or a dark terminal without
    /// knowing which. `from_env` honours the user's own setting instead.
    fn default() -> Self {
        Self::for_appearance(Appearance::Universal)
    }
}

impl Theme {
    /// Build for this terminal: the appearance the user asked for (or that
    /// could be detected), honouring `NO_COLOR`.
    ///
    /// Per the `NO_COLOR` convention, any value — including an empty one —
    /// disables colour; only absence enables it.
    /// Build a theme from a configured setting.
    ///
    /// Precedence, highest first — the ordinary one, which this did not have:
    ///
    /// 1. `NINES_THEME` in the environment
    /// 2. the setting passed in, from a config file
    /// 3. detection
    ///
    /// The environment has to win. A config file is shared by every machine
    /// that checks out the dotfiles, so it is the wrong place to say what one
    /// terminal looks like — and if the file beats the variable, there is no
    /// way to override it for a single session at all. That was the bug: a
    /// config saying `light` made `NINES_THEME=dark` do nothing.
    ///
    /// `light` | `dark` | `neutral` pick a palette outright. `system` (or
    /// `auto`, or nothing) means "work it out", which is [`from_env`].
    ///
    /// A setting is needed because detection cannot be relied on: it reads
    /// `COLORFGBG`, and most terminals — wezterm among them — never set it. A
    /// user on a light terminal therefore gets the Universal compromise palette
    /// rather than the light one, with no way to say otherwise. This is that
    /// way, and every 9s app should offer it.
    ///
    /// `NO_COLOR` still wins over all of it: an explicit request for no colour
    /// is not overridden by a preference for which colours.
    ///
    /// [`from_env`]: Self::from_env
    pub fn from_setting(setting: Option<&str>) -> Self {
        // An explicit variable beats the file. `from_env` would honour it too,
        // but only when the file says "system" — which is exactly the case
        // someone overriding a hard-coded setting is not in.
        let env = std::env::var("NINES_THEME").ok();
        let setting = env
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or(setting);

        let chosen = setting
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter(|s| !matches!(s.to_ascii_lowercase().as_str(), "system" | "auto"))
            .and_then(Appearance::parse);

        match chosen {
            Some(appearance) => Self {
                monochrome: std::env::var_os("NO_COLOR").is_some(),
                ..Self::for_appearance(appearance)
            },
            None => Self::from_env(),
        }
    }

    pub fn from_env() -> Self {
        Self {
            monochrome: std::env::var_os("NO_COLOR").is_some(),
            ..Self::for_appearance(Appearance::detect())
        }
    }

    /// Return `c`, adjusted if it would fall below this appearance's contrast
    /// floor against the background the appearance assumes.
    ///
    /// This is the answer to "can we not just override a colour when contrast
    /// would drop too low", and the limit is worth stating: it CANNOT measure
    /// against the real terminal background, because `text` is `Reset` and a
    /// reset colour has no RGB to measure. That is the same unknown that makes
    /// detection unreliable, not a separate problem.
    ///
    /// What it CAN do is enforce the floor for ANY colour against the assumed
    /// background — including one a tool sets itself, or one a transform like
    /// colour-blind mode produces. That turns the guarantee from "the palette
    /// we ship is fine" into "whatever anyone sets is fine".
    ///
    /// Adjustment blends toward black or white, which keeps the hue
    /// recognisable where scaling channels would not.
    pub fn ensure_legible(&self, c: Color) -> Color {
        if self.monochrome {
            return c;
        }
        let (r, g, b) = match c {
            Color::Rgb(r, g, b) => (r, g, b),
            // Anything without a fixed RGB cannot be measured or adjusted.
            _ => return c,
        };
        let (min, max, _) = self.appearance.legibility_band();
        let l = luminance_rgb(r, g, b);
        if l >= min && l <= max {
            return c;
        }

        let (target, toward) = if l > max { (max, 0u8) } else { (min, 255u8) };
        for step in 1..=100 {
            let f = step as f64 / 100.0;
            let mix = |v: u8| (v as f64 + (toward as f64 - v as f64) * f).round() as u8;
            let (nr, ng, nb) = (mix(r), mix(g), mix(b));
            let nl = luminance_rgb(nr, ng, nb);
            if (toward == 255 && nl >= target) || (toward == 0 && nl <= target) {
                return Color::Rgb(nr, ng, nb);
            }
        }
        if toward == 255 {
            Color::Rgb(255, 255, 255)
        } else {
            Color::Rgb(0, 0, 0)
        }
    }

    /// Run every semantic colour through `ensure_legible`, so a future palette
    /// edit that drops one below its floor is corrected rather than shipped.
    fn enforce(mut self) -> Self {
        self.dim = self.ensure_legible(self.dim);
        self.accent = self.ensure_legible(self.accent);
        self.negative = self.ensure_legible(self.negative);
        self.ok = self.ensure_legible(self.ok);
        self.warn = self.ensure_legible(self.warn);
        self.error = self.ensure_legible(self.error);
        self
    }

    /// Swap red/green for a blue/orange diverging pair.
    pub fn colorblind(mut self) -> Self {
        self.colorblind = true;
        // Blue and amber are the standard diverging pair: distinguishable under
        // every common form of colour blindness, where green and blue are not
        // reliably so. Structure moves to grey so it cannot be confused with
        // either side of the money axis.
        //
        // The values come from the SAME appearance palette rather than from
        // fresh constants, so colour-blind mode keeps the contrast guarantee
        // instead of trading one accessibility problem for another.
        let p = palette(self.appearance);
        let rgb = |c: (u8, u8, u8)| Color::Rgb(c.0, c.1, c.2);
        self.accent = rgb(p.dim);
        self.ok = rgb(p.accent);
        self.negative = rgb(p.warn);
        self.enforce()
    }

    fn paint(&self, color: Color) -> Style {
        if self.monochrome {
            Style::default()
        } else {
            Style::default().fg(color)
        }
    }

    /// Style for the highlighted row.
    ///
    /// Both colours are always set. Under `NO_COLOR` this falls back to a bold
    /// reversed style, which is the one place reverse video is correct: with no
    /// palette to work with, it is the only way to show selection at all.
    ///
    /// The background is DARK and the text is light, which is the opposite of
    /// the obvious choice. Black on a mid-tone background only reads while the
    /// background stays light, and it did not: black on ANSI blue measures
    /// about 1.6:1, far under the 4.5:1 §8 requires. Light on dark is 12:1 and
    /// stays legible whatever the terminal does with the rest of the palette.
    ///
    /// This is the one place "prefer the 16 ANSI colours" is deliberately set
    /// aside: ANSI blue is whatever the user's theme says it is, and a contrast
    /// guarantee cannot be built on a colour that moves.
    /// The style to fill a frame with before drawing anything.
    ///
    /// `None` when the terminal's own background should show through, which is
    /// the default and what CONTRACT.md §8 asks for.
    pub fn canvas(&self) -> Option<Style> {
        (self.paint_background && !self.monochrome).then(|| {
            Style::default()
                .bg(self.background)
                .fg(self.text_on_canvas())
        })
    }

    /// The ordinary text colour to pair with a painted background.
    ///
    /// `Color::Reset` is the terminal's foreground, which is chosen for the
    /// terminal's background — paint over that background and it can vanish.
    fn text_on_canvas(&self) -> Color {
        match self.appearance {
            Appearance::Dark => Color::Rgb(0xe6, 0xed, 0xf3),
            Appearance::Neutral => Color::Rgb(0xf5, 0xf5, 0xf5),
            _ => Color::Rgb(0x1f, 0x23, 0x28),
        }
    }

    pub fn selected(&self) -> Style {
        if self.monochrome {
            return Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD);
        }
        Style::default()
            .bg(self.select)
            // An explicit RGB white, NOT Color::White. `Color::White` is ANSI
            // index 7, and what index 7 looks like is entirely up to the
            // terminal's palette — light schemes routinely remap it to a DARK
            // tone so that "white" text stays readable on a white page. The
            // background here is a fixed RGB navy, so a foreground that moves
            // gives dark-on-dark and the contrast guarantee evaporates.
            //
            // This module already knew the rule and stated it about blue; it
            // just did not apply it to white.
            .fg(SELECTION_TEXT)
            .add_modifier(Modifier::BOLD)
    }

    /// Column headers.
    pub fn header(&self) -> Style {
        self.paint(self.accent).add_modifier(Modifier::BOLD)
    }

    /// De-emphasised text.
    pub fn dim(&self) -> Style {
        self.paint(self.dim)
    }

    pub fn ok(&self) -> Style {
        self.paint(self.ok)
    }

    /// Money going out. Not [`Theme::error`].
    pub fn negative(&self) -> Style {
        self.paint(self.negative)
    }
    pub fn warn(&self) -> Style {
        self.paint(self.warn)
    }
    pub fn error(&self) -> Style {
        self.paint(self.error)
    }

    /// Render a signed quantity with a leading glyph so the sign survives
    /// without colour.
    ///
    /// Accounting convention: negatives in parentheses. The glyph is what a
    /// screen reader, a monochrome terminal, and a colour-blind reader all rely
    /// on — the colour is the redundant part, not the other way round.
    pub fn signed(&self, rendered: &str, negative: bool) -> (String, Style) {
        if negative {
            (format!("({rendered})"), self.negative())
        } else {
            (rendered.to_string(), self.ok())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance, so the ratio below is the number an
    /// accessibility checker would report.
    fn luminance(c: Color) -> f64 {
        let (r, g, b) = match c {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::White => (255, 255, 255),
            Color::Black => (0, 0, 0),
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

    /// CONTRACT §8: selection contrast >= 4.5:1.
    ///
    /// A test rather than a comment, because it was a comment for a while and
    /// the comment was wrong.
    #[test]
    fn selection_meets_the_contrast_requirement() {
        let style = Theme::default().selected();
        let (fg, bg) = (style.fg.unwrap(), style.bg.unwrap());
        let (hi, lo) = {
            let (a, b) = (luminance(fg), luminance(bg));
            (a.max(b), a.min(b))
        };
        let ratio = (hi + 0.05) / (lo + 0.05);
        assert!(
            ratio >= 4.5,
            "selection contrast {ratio:.2}:1, want >= 4.5:1"
        );
    }

    #[test]
    fn selection_never_relies_on_reverse_video_when_colour_exists() {
        let style = Theme::default().selected();
        assert!(
            style.bg.is_some() && style.fg.is_some(),
            "both colours must be explicit — reverse video can resolve to invisible text"
        );
        assert!(!style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn monochrome_selection_falls_back_to_reverse_video() {
        let theme = Theme {
            monochrome: true,
            ..Theme::default()
        };
        let style = theme.selected();
        assert!(
            style.add_modifier.contains(Modifier::REVERSED),
            "with no palette, reverse is the only way to show selection"
        );
    }

    #[test]
    fn monochrome_emits_no_foreground_colour() {
        let theme = Theme {
            monochrome: true,
            ..Theme::default()
        };
        for style in [theme.ok(), theme.warn(), theme.error(), theme.dim()] {
            assert!(style.fg.is_none(), "NO_COLOR must mean no colour");
        }
    }

    #[test]
    fn negatives_are_marked_by_parentheses_not_only_by_colour() {
        let theme = Theme::default();
        let (text, _) = theme.signed("1,234.56", true);
        assert_eq!(text, "(1,234.56)");

        // And the marking survives with colour switched off entirely.
        let mono = Theme {
            monochrome: true,
            ..Theme::default()
        };
        let (text, style) = mono.signed("1,234.56", true);
        assert_eq!(text, "(1,234.56)");
        assert!(style.fg.is_none());
    }

    #[test]
    fn positives_are_not_parenthesised() {
        let (text, _) = Theme::default().signed("10.00", false);
        assert_eq!(text, "10.00");
    }

    #[test]
    fn colorblind_mode_abandons_the_green_blue_money_pair() {
        let plain = Theme::default();
        let cb = Theme::default().colorblind();

        // The intent is unchanged; only the constants moved to RGB. "Green must
        // not survive" now means the green-dominant hue must be gone, rather
        // than that one ANSI name is absent.
        let green_dominant = |c: Color| match c {
            Color::Rgb(r, g, b) => g > r && g > b,
            _ => false,
        };
        assert!(
            green_dominant(plain.ok),
            "the default money-in colour is green"
        );
        assert!(
            !green_dominant(cb.ok),
            "green must not survive colour-blind mode"
        );
        assert_ne!(cb.ok, plain.ok);
        assert_ne!(
            cb.ok, cb.negative,
            "the two sides of the money axis must stay distinguishable"
        );
    }

    #[test]
    fn an_outflow_is_not_styled_as_an_error() {
        // A ledger is mostly negative amounts. Painting them with error() makes
        // ordinary spending look like a fault, and turns the whole screen red.
        let theme = Theme::default();
        let (_, negative) = theme.signed("100.00", true);
        assert_eq!(negative.fg, Some(theme.negative));
        assert_ne!(
            negative.fg,
            Some(theme.error),
            "an outflow must not wear the error colour"
        );
    }

    #[test]
    fn the_money_axis_stays_two_distinguishable_hues() {
        // The axis used to be asserted as the ANSI constants Green and Blue.
        // It is now appearance-tuned RGB (see CONTRACT §8.2), so the meaningful
        // assertion is that the two sides remain distinct and that structure is
        // a third thing — not which constants they happen to be.
        let theme = Theme::default();
        assert_ne!(
            theme.ok, theme.negative,
            "the two sides of the axis must differ"
        );
        assert_ne!(theme.accent, theme.ok);
        assert_ne!(theme.accent, theme.negative);
    }

    #[test]
    fn semantic_colours_are_fixed_rgb_so_contrast_can_be_guaranteed() {
        // This inverts an earlier rule, deliberately. Preferring the 16 ANSI
        // colours let the user's terminal theme tint the app — but an ANSI
        // colour's actual RGB is whatever that theme says, so no ratio can be
        // promised against it. ANSI cyan and yellow measured 1.25:1 and 1.07:1
        // on a white terminal: effectively invisible.
        //
        // A guarantee and a borrowed palette are mutually exclusive. The
        // guarantee wins for the semantic colours; `text` still defers to the
        // terminal, so the app is not painting its own background.
        let t = Theme::default();
        for (name, color) in [
            ("accent", t.accent),
            ("ok", t.ok),
            ("warn", t.warn),
            ("error", t.error),
            ("negative", t.negative),
            ("dim", t.dim),
        ] {
            assert!(
                matches!(color, Color::Rgb(..)),
                "{name} must be fixed RGB, or its contrast cannot be verified"
            );
        }
        assert_eq!(
            t.text,
            Color::Reset,
            "the terminal's own foreground still applies"
        );
    }

    /// The opt-in that makes "dark" mean a dark app rather than a dark-suited
    /// palette. Off unless asked for, because the tool blending into the user's
    /// scheme is the documented default (CONTRACT.md §8).
    #[test]
    fn the_background_is_not_painted_unless_asked_for() {
        let mut env = crate::testenv::Env::acquire();
        env.remove("NINES_PAINT_BACKGROUND");
        assert!(
            Theme::for_appearance(Appearance::Dark).canvas().is_none(),
            "painted over the terminal without being asked"
        );

        env.set("NINES_PAINT_BACKGROUND", "1");
        let theme = Theme::for_appearance(Appearance::Dark);
        let canvas = theme.canvas().expect("asked for, so it should paint");
        assert_eq!(canvas.bg, Some(theme.background));
    }

    /// A painted background needs a foreground chosen for IT. `Color::Reset` is
    /// the terminal's own, picked for the terminal's background — paint over
    /// that and the text can vanish.
    #[test]
    fn painting_supplies_a_foreground_too() {
        let mut env = crate::testenv::Env::acquire();
        env.set("NINES_PAINT_BACKGROUND", "1");
        for appearance in [Appearance::Light, Appearance::Dark, Appearance::Neutral] {
            let canvas = Theme::for_appearance(appearance)
                .canvas()
                .expect("painting");
            let fg = canvas.fg.expect("a painted canvas must set a foreground");
            assert!(
                matches!(fg, Color::Rgb(_, _, _)),
                "{appearance:?} left the foreground to the terminal on a painted background"
            );
            assert_ne!(canvas.fg, canvas.bg, "{appearance:?}: invisible text");
        }
    }

    /// NO_COLOR wins: an explicit request for no colour is not overridden by a
    /// request for a particular one.
    #[test]
    fn no_color_beats_a_request_to_paint() {
        let mut env = crate::testenv::Env::acquire();
        env.set("NINES_PAINT_BACKGROUND", "1");
        env.set("NO_COLOR", "1");
        assert!(Theme::from_env().canvas().is_none());
    }

    /// The selection foreground must be a colour the terminal cannot move.
    ///
    /// A named/indexed colour is remapped by the user's scheme — index 7 is
    /// dark in many light schemes — which turns white-on-navy into
    /// dark-on-navy without a single line of this code changing.
    #[test]
    fn the_selection_foreground_is_not_a_palette_index() {
        for appearance in [Appearance::Light, Appearance::Dark, Appearance::Universal] {
            let theme = Theme::for_appearance(appearance);
            let fg = theme
                .selected()
                .fg
                .expect("a selected row must set a foreground");
            assert!(
                matches!(fg, Color::Rgb(_, _, _)),
                "{appearance:?}: selection foreground is {fg:?}, which the terminal \
                 palette can redefine — that is how dark text lands on a dark bar"
            );
        }
    }

    /// The override that did not work. A config file is shared by every
    /// machine, so it cannot be the last word on what one terminal looks like.
    #[test]
    fn the_environment_beats_a_configured_appearance() {
        let mut env = crate::testenv::Env::acquire();
        env.set("NINES_THEME", "dark");
        assert_eq!(
            Theme::from_setting(Some("light")).appearance,
            Appearance::Dark,
            "a config saying light made NINES_THEME=dark do nothing"
        );

        env.set("NINES_THEME", "light");
        assert_eq!(
            Theme::from_setting(Some("dark")).appearance,
            Appearance::Light
        );
    }

    #[test]
    fn an_empty_environment_variable_does_not_count_as_an_override() {
        let mut env = crate::testenv::Env::acquire();
        env.set("NINES_THEME", "   ");
        assert_eq!(
            Theme::from_setting(Some("dark")).appearance,
            Appearance::Dark
        );
    }

    #[test]
    fn a_configured_appearance_beats_detection() {
        let mut env = crate::testenv::Env::acquire();
        env.remove("NINES_THEME");
        // The point of the setting: a light terminal that never sets
        // COLORFGBG can still ask for the light palette.
        assert_eq!(
            Theme::from_setting(Some("light")).appearance,
            Appearance::Light
        );
        assert_eq!(
            Theme::from_setting(Some("dark")).appearance,
            Appearance::Dark
        );
    }

    #[test]
    fn system_and_nothing_both_mean_work_it_out() {
        let detected = Theme::from_env().appearance;
        assert_eq!(Theme::from_setting(Some("system")).appearance, detected);
        assert_eq!(Theme::from_setting(Some("  ")).appearance, detected);
        assert_eq!(Theme::from_setting(None).appearance, detected);
    }

    #[test]
    fn a_nonsense_setting_falls_back_rather_than_panicking() {
        let detected = Theme::from_env().appearance;
        assert_eq!(Theme::from_setting(Some("chartreuse")).appearance, detected);
    }

    /// A cell's own colour must not survive onto the selection bar.
    ///
    /// Every table in a 9s app colours some column — an amount, a dim id — for
    /// contrast against the BACKGROUND. On the selection bar that same colour
    /// can be unreadable, so the highlight has to win. Reasoning about
    /// ratatui's style-patching order is how you get this wrong; rendering it
    /// is how you find out.
    #[test]
    fn a_cells_own_colour_does_not_survive_onto_the_selection_bar() {
        use ratatui::backend::TestBackend;
        use ratatui::layout::Constraint;
        use ratatui::widgets::{Cell, Row, Table, TableState};
        use ratatui::Terminal;

        for appearance in [Appearance::Light, Appearance::Dark, Appearance::Universal] {
            let theme = Theme::for_appearance(appearance);
            let mut state = TableState::default().with_selected(Some(0));
            let mut terminal = Terminal::new(TestBackend::new(24, 2)).expect("terminal");
            terminal
                .draw(|f| {
                    let rows = vec![Row::new(vec![
                        Cell::from("plain"),
                        Cell::from(ratatui::text::Span::styled("coloured", theme.negative())),
                    ])];
                    let table = Table::new(rows, [Constraint::Length(8), Constraint::Fill(1)])
                        .row_highlight_style(theme.selected());
                    f.render_stateful_widget(table, f.area(), &mut state);
                })
                .expect("draw");

            let buf = terminal.backend().buffer().clone();
            for (x, what) in [(0u16, "a plain cell"), (9u16, "a coloured cell")] {
                let cell = &buf[(x, 0u16)];
                assert_eq!(
                    cell.bg, theme.select,
                    "{what} on the selected row lost the selection background ({appearance:?})"
                );
                assert_eq!(
                    cell.fg, SELECTION_TEXT,
                    "{what} kept its own colour on the selection bar ({appearance:?}) — \
                     that is how dark text ends up on a dark bar"
                );
            }
        }
    }
}
