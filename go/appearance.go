// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"math"
	"os"
	"strconv"
	"strings"

	"github.com/gdamore/tcell/v2"
	"github.com/rivo/tview"
)

// Appearance is the terminal background a palette is tuned for — CONTRACT §8.2.
//
// It exists because of a hard limit rather than a preference. For one colour to
// reach 3:1 against a white background its luminance must be <= 0.300; against
// black it must be >= 0.100; against a mid-grey it must be <= 0.039 or >= 0.748.
// White and black intersect in [0.100, 0.300], which fits a palette. Adding
// mid-grey empties the intersection completely.
//
// So a single fixed palette CANNOT serve light, dark and neutral backgrounds.
// The background has to be known, or chosen for.
type Appearance int

const (
	// AppearanceUniversal is the safe default when the background is unknown.
	// Its colours sit in the [0.100, 0.300] band, so nothing is ever
	// invisible on a light or a dark terminal — at the cost of being less
	// vivid on dark than a dark-tuned palette.
	AppearanceUniversal Appearance = iota
	// AppearanceLight is tuned for a near-white background.
	AppearanceLight
	// AppearanceDark is tuned for a near-black background.
	AppearanceDark
	// AppearanceNeutral is tuned for a mid-grey background. It is opt-in only:
	// its colours are near-white and would be invisible on a light terminal.
	AppearanceNeutral
)

// String names the appearance.
func (a Appearance) String() string {
	switch a {
	case AppearanceLight:
		return "light"
	case AppearanceDark:
		return "dark"
	case AppearanceNeutral:
		return "neutral"
	}
	return "universal"
}

// ParseAppearance reads a name, falling back to universal.
func ParseAppearance(s string) (Appearance, bool) {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "light":
		return AppearanceLight, true
	case "dark":
		return AppearanceDark, true
	case "neutral", "grey", "gray":
		return AppearanceNeutral, true
	case "universal", "auto":
		return AppearanceUniversal, true
	}
	return AppearanceUniversal, false
}

// DetectAppearance decides which palette to use.
//
// NINES_THEME is authoritative, because detection cannot be relied on. The
// COLORFGBG convention is honoured when present but is frequently unset, and
// the OSC 11 query that would ask the terminal directly is deliberately NOT
// used: many terminals answer it wrongly or not at all, tmux sits in the way,
// and issuing it means writing to and reading from a terminal the UI library
// already owns. A wrong answer there is worse than no answer, because the
// universal palette is legible either way.
func DetectAppearance() Appearance {
	// An empty value means "not set", not "universal" — otherwise exporting
	// the variable blank would silently suppress COLORFGBG.
	if v := strings.TrimSpace(os.Getenv("NINES_THEME")); v != "" {
		if a, valid := ParseAppearance(v); valid {
			return a
		}
	}
	if a, ok := appearanceFromCOLORFGBG(os.Getenv("COLORFGBG")); ok {
		return a
	}
	return AppearanceUniversal
}

// appearanceFromCOLORFGBG reads the "fg;bg" convention, where the last field is
// an ANSI colour index for the background.
func appearanceFromCOLORFGBG(v string) (Appearance, bool) {
	parts := strings.Split(strings.TrimSpace(v), ";")
	if len(parts) < 2 {
		return AppearanceUniversal, false
	}
	bg, err := strconv.Atoi(parts[len(parts)-1])
	if err != nil {
		return AppearanceUniversal, false
	}
	switch bg {
	case 0, 8:
		return AppearanceDark, true
	case 7, 15:
		return AppearanceLight, true
	}
	// Any other index is a colour we cannot classify without knowing the
	// terminal's palette, so say nothing rather than guess.
	return AppearanceUniversal, false
}

// palettes holds the semantic colours per appearance.
//
// Every value is verified by test against the background it is meant for:
// light and dark reach 4.5:1, neutral reaches 3:1 — the most a mid-grey
// background allows anything to reach.
var palettes = map[Appearance]struct {
	Dim, Accent, Header, OK, Warn, Error, Sensitive int32
}{
	// Universal doubles as the light palette. These sit in the band that works
	// on both extremes: 4.55:1 at worst on white, 3.84:1 at worst on black.
	AppearanceUniversal: {0x6e7781, 0x0969da, 0x0f766e, 0x1a7f37, 0x9a6700, 0xcf222e, 0x8250df},
	AppearanceLight:     {0x6e7781, 0x0969da, 0x0f766e, 0x1a7f37, 0x9a6700, 0xcf222e, 0x8250df},
	// Light foregrounds: 6.26:1 at worst on black.
	AppearanceDark: {0x8b949e, 0x58a6ff, 0x39c5cf, 0x3fb950, 0xd29922, 0xf85149, 0xbc8cff},
	// Near-white: a mid-grey background permits nothing else. 3.16:1 at worst.
	AppearanceNeutral: {0xe5e6e8, 0xd4e8ff, 0xc4eef1, 0xcfeed3, 0xf4e6c8, 0xfee0de, 0xeee1ff},
}

// ThemeFor builds the palette for a background.
//
// Bg and Fg stay ColorDefault in every appearance: the terminal's own
// background and foreground are already correct, and overriding them would
// fight the user's theme rather than fit into it.
func ThemeFor(a Appearance) Theme {
	p, ok := palettes[a]
	if !ok {
		p = palettes[AppearanceUniversal]
		a = AppearanceUniversal
	}
	return Theme{
		Appearance: a,
		// The terminal's own background by default — CONTRACT §8. PaintBg
		// swaps in Canvas, which is the colour these foregrounds were
		// measured against.
		Bg:        tcell.ColorDefault,
		Canvas:    canvasFor(a),
		PaintBg:   paintBackgroundRequested(),
		Fg:        tcell.ColorDefault,
		Dim:       tcell.NewHexColor(p.Dim),
		Accent:    tcell.NewHexColor(p.Accent),
		Header:    tcell.NewHexColor(p.Header),
		OK:        tcell.NewHexColor(p.OK),
		Warn:      tcell.NewHexColor(p.Warn),
		Error:     tcell.NewHexColor(p.Error),
		Sensitive: tcell.NewHexColor(p.Sensitive),
		// The selection sets both of its own colours, so it is the one element
		// that does not depend on the terminal background at all.
		Select: tcell.NewHexColor(0x1f3350),
	}.enforce()
}

// legibilityBand returns the luminance range a foreground must sit in to meet
// the floor for this appearance, and the floor itself.
//
// Universal has to satisfy white AND black at once, which is why its band is
// the intersection rather than a single bound.
func (a Appearance) legibilityBand() (min, max, floor float64) {
	const (
		white = 1.0
		black = 0.0
	)
	switch a {
	case AppearanceLight:
		// 4.5:1 against white: L <= 1.05/4.5 - 0.05
		return 0, 1.05/4.5 - 0.05, 4.5
	case AppearanceDark:
		// 4.5:1 against black: L >= 0.05*4.5 - 0.05
		return 0.05*4.5 - 0.05, 1, 4.5
	case AppearanceNeutral:
		// Mid-grey permits only the extremes; the light half is the only one
		// where hues stay apart, so that is the half used.
		g := 0.2158605001139457 // luminance of #808080
		return 3.0*(g+0.05) - 0.05, 1, 3.0
	default:
		// Universal: dark enough for white, light enough for black.
		return 0.05*3.0 - 0.05, 1.05/3.0 - 0.05, 3.0
	}
}

// EnsureLegible returns c, adjusted if it would fall below this appearance's
// contrast floor against the background the appearance assumes.
//
// This is the answer to "can we not just override a colour when contrast would
// drop too low", and it is worth being precise about what it can and cannot do.
// It CANNOT measure contrast against the real terminal background: Bg is
// ColorDefault, which has no RGB at all — asking tcell for it returns
// (-1,-1,-1). There is nothing to measure against, which is the same reason
// detection is unreliable rather than a separate problem.
//
// What it CAN do is enforce the floor against the background the chosen
// appearance assumes, for ANY colour — including one a tool sets itself, or one
// a transform like colour-blind mode produces. That turns the guarantee from
// "the palette we ship is fine" into "whatever anyone sets is fine", which is
// strictly stronger and covers the cases the palette tests cannot see.
//
// Adjustment blends toward black or white, which preserves hue far better than
// scaling channels and keeps a colour recognisably itself.
func (t Theme) EnsureLegible(c tcell.Color) tcell.Color {
	if t.Monochrome || c == tcell.ColorDefault || !c.Valid() {
		return c
	}
	min, max, _ := t.Appearance.legibilityBand()

	l := luminanceOf(c)
	if l >= min && l <= max {
		return c
	}

	target := min
	toward := int32(255) // blend toward white to raise luminance
	if l > max {
		target, toward = max, 0 // blend toward black to lower it
	}

	r, g, b := c.RGB()
	// 100 steps is finer than the 256-colour palette most terminals will
	// quantise this to, so the loop cannot overshoot visibly.
	for step := 1; step <= 100; step++ {
		f := float64(step) / 100
		nr := int32(float64(r) + (float64(toward)-float64(r))*f)
		ng := int32(float64(g) + (float64(toward)-float64(g))*f)
		nb := int32(float64(b) + (float64(toward)-float64(b))*f)
		cand := tcell.NewRGBColor(nr, ng, nb)
		cl := luminanceOf(cand)
		if (toward == 255 && cl >= target) || (toward == 0 && cl <= target) {
			return cand
		}
	}
	if toward == 255 {
		return tcell.ColorWhite
	}
	return tcell.ColorBlack
}

// luminanceOf is the WCAG relative luminance of a colour.
func luminanceOf(c tcell.Color) float64 {
	r, g, b := c.RGB()
	ch := func(v int32) float64 {
		f := float64(v) / 255
		if f <= 0.03928 {
			return f / 12.92
		}
		return math.Pow((f+0.055)/1.055, 2.4)
	}
	return 0.2126*ch(r) + 0.7152*ch(g) + 0.0722*ch(b)
}

// enforce runs every semantic colour through EnsureLegible.
//
// Applied to the shipped palettes too, so a future edit that drops one below
// its floor is corrected rather than shipped.
func (t Theme) enforce() Theme {
	t.Dim = t.EnsureLegible(t.Dim)
	t.Accent = t.EnsureLegible(t.Accent)
	t.Header = t.EnsureLegible(t.Header)
	t.OK = t.EnsureLegible(t.OK)
	t.Warn = t.EnsureLegible(t.Warn)
	t.Error = t.EnsureLegible(t.Error)
	t.Sensitive = t.EnsureLegible(t.Sensitive)
	return t
}

// ApplyGlobalStyles points tview's package-level palette at this theme.
//
// tview widgets that are not given explicit colours fall back to tview.Styles,
// which defaults to a black background, white borders and titles, and yellow
// secondary text. On a light terminal white borders are invisible, yellow
// measures 1.07:1, and every modal is a black box — none of which any amount of
// per-view colouring fixes, because those widgets never consult our theme.
//
// Called once when the shell is built. It is a global, which is acceptable
// because a process runs one shell.
func ApplyGlobalStyles(t Theme) {
	selFg, selBg, _ := t.Selected().Decompose()

	// One hook for the whole toolkit: every tview widget without explicit
	// colours reads these, so opting into a painted background here reaches
	// every screen without any of them knowing about it.
	bg, fg := t.Bg, t.Fg
	if canvas, painting := t.CanvasStyle(); painting {
		fg, bg, _ = canvas.Decompose()
	}
	tview.Styles.PrimitiveBackgroundColor = bg
	tview.Styles.PrimaryTextColor = fg
	// Field and highlight backgrounds share the selection's pair, so a focused
	// field and a selected row cannot drift apart again.
	tview.Styles.ContrastBackgroundColor = selBg
	tview.Styles.InverseTextColor = selFg
	tview.Styles.MoreContrastBackgroundColor = selBg
	tview.Styles.BorderColor = t.Dim
	tview.Styles.TitleColor = t.Accent
	tview.Styles.GraphicsColor = t.Dim
	tview.Styles.SecondaryTextColor = t.Accent
	tview.Styles.TertiaryTextColor = t.Dim
	tview.Styles.ContrastSecondaryTextColor = selFg
}

// canvasFor is the background a palette's foregrounds were measured against.
//
// Only painted when asked for. It must stay the colour the contrast ratios were
// computed against, or those ratios become claims about nothing.
func canvasFor(a Appearance) tcell.Color {
	switch a {
	case AppearanceDark:
		return tcell.NewHexColor(0x0d1117)
	case AppearanceNeutral:
		return tcell.NewHexColor(0x6a6a6a)
	default:
		return tcell.NewHexColor(0xffffff)
	}
}

// CanvasText is the ordinary text colour to pair with a painted background.
//
// tcell.ColorDefault is the terminal's foreground, chosen for the terminal's
// background — paint over that background and it can vanish.
func CanvasText(a Appearance) tcell.Color {
	switch a {
	case AppearanceDark:
		return tcell.NewHexColor(0xe6edf3)
	case AppearanceNeutral:
		return tcell.NewHexColor(0xf5f5f5)
	default:
		return tcell.NewHexColor(0x1f2328)
	}
}

// paintBackgroundRequested reports whether the user asked the tool to paint its
// own background rather than defer to the terminal.
//
// An environment variable rather than a config field so it reaches every 9s
// tool at once, including those with no config file of their own.
func paintBackgroundRequested() bool {
	switch strings.ToLower(strings.TrimSpace(os.Getenv("NINES_PAINT_BACKGROUND"))) {
	case "1", "true", "yes", "on":
		return true
	}
	return false
}
