// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"math"
	"testing"

	"github.com/gdamore/tcell/v2"
	"github.com/rivo/tview"
)

// relativeLuminance is the WCAG definition, so the ratios below are the same
// numbers an accessibility checker would report.
func relativeLuminance(c tcell.Color) float64 {
	r, g, b := c.RGB()
	channel := func(v int32) float64 {
		f := float64(v) / 255
		if f <= 0.03928 {
			return f / 12.92
		}
		return math.Pow((f+0.055)/1.055, 2.4)
	}
	return 0.2126*channel(r) + 0.7152*channel(g) + 0.0722*channel(b)
}

func contrastRatio(a, b tcell.Color) float64 {
	la, lb := relativeLuminance(a), relativeLuminance(b)
	hi, lo := math.Max(la, lb), math.Min(la, lb)
	return (hi + 0.05) / (lo + 0.05)
}

// The three backgrounds a terminal actually presents.
var (
	bgWhite = tcell.NewHexColor(0xffffff)
	bgBlack = tcell.NewHexColor(0x000000)
	bgGrey  = tcell.NewHexColor(0x808080)
)

// tokens returns every semantic colour by name, so a new one cannot be added
// without this file having an opinion about it.
func tokens(t Theme) map[string]tcell.Color {
	return map[string]tcell.Color{
		"Dim": t.Dim, "Accent": t.Accent, "Header": t.Header,
		"OK": t.OK, "Warn": t.Warn, "Error": t.Error, "Sensitive": t.Sensitive,
	}
}

// EVERY element, against the background its palette is for.
func TestEveryTokenMeetsContrastOnItsOwnBackground(t *testing.T) {
	cases := []struct {
		appearance Appearance
		bg         tcell.Color
		floor      float64
		why        string
	}{
		{AppearanceLight, bgWhite, 4.5, "tuned for a white terminal"},
		{AppearanceDark, bgBlack, 4.5, "tuned for a black terminal"},
		// 3:1 is not a lowered standard here, it is the ceiling: a mid-grey
		// background leaves nothing further to reach for.
		{AppearanceNeutral, bgGrey, 3.0, "mid-grey allows no more than this"},
	}
	for _, c := range cases {
		theme := ThemeFor(c.appearance)
		for name, col := range tokens(theme) {
			if got := contrastRatio(col, c.bg); got < c.floor {
				t.Errorf("%s/%s = %.2f:1, want >= %.1f:1 (%s)",
					c.appearance, name, got, c.floor, c.why)
			}
		}
	}
}

// The universal palette is the default, so it must never be unreadable on
// either extreme — that is the whole reason it exists.
func TestUniversalPaletteWorksOnLightAndDark(t *testing.T) {
	theme := ThemeFor(AppearanceUniversal)
	for name, col := range tokens(theme) {
		onWhite := contrastRatio(col, bgWhite)
		onBlack := contrastRatio(col, bgBlack)
		if onWhite < 4.5 {
			t.Errorf("universal/%s on white = %.2f:1, want >= 4.5:1", name, onWhite)
		}
		if onBlack < 3.0 {
			t.Errorf("universal/%s on black = %.2f:1, want >= 3.0:1", name, onBlack)
		}
	}
}

// The constraint that forces the whole design — CONTRACT §8.2.
//
// If this ever stops holding, a single palette could serve every background and
// appearances would be unnecessary. It does not stop holding; it is arithmetic.
func TestNoSingleColourCanServeAllThreeBackgrounds(t *testing.T) {
	const floor = 3.0
	// Against white a colour must be dark; against black it must be light.
	whiteMax := 1.05/floor - 0.05 // L <= 0.300
	blackMin := 0.05*floor - 0.05 // L >= 0.100
	if whiteMax <= blackMin {
		t.Fatal("white and black should still intersect")
	}
	// Mid-grey allows only the extremes.
	greyL := relativeLuminance(bgGrey)
	greyDarkMax := (greyL+0.05)/floor - 0.05
	greyLightMin := floor*(greyL+0.05) - 0.05

	if greyDarkMax >= blackMin {
		t.Errorf("the dark half of mid-grey (L <= %.3f) now overlaps black's floor (L >= %.3f)",
			greyDarkMax, blackMin)
	}
	if greyLightMin <= whiteMax {
		t.Errorf("the light half of mid-grey (L >= %.3f) now overlaps white's ceiling (L <= %.3f)",
			greyLightMin, whiteMax)
	}
}

// CONTRACT §8: selection sets both colours explicitly, contrast >= 4.5:1.
//
// A test rather than a comment, because it was a comment for a while and the
// comment was wrong.
func TestSelectionMeetsContrastRequirement(t *testing.T) {
	for _, a := range []Appearance{AppearanceUniversal, AppearanceLight, AppearanceDark, AppearanceNeutral} {
		style := ThemeFor(a).Selected()
		fg, bg, _ := style.Decompose()
		if fg == tcell.ColorDefault || bg == tcell.ColorDefault {
			t.Errorf("%s: selection must set BOTH colours — reverse video can vanish", a)
			continue
		}
		if got := contrastRatio(fg, bg); got < 4.5 {
			t.Errorf("%s: selection contrast %.2f:1, want >= 4.5:1", a, got)
		}
	}
}

// The selection carries its own background, so unlike everything else it does
// not depend on the terminal's at all.
func TestSelectionIsIndependentOfAppearance(t *testing.T) {
	want, _, _ := ThemeFor(AppearanceLight).Selected().Decompose()
	for _, a := range []Appearance{AppearanceDark, AppearanceNeutral, AppearanceUniversal} {
		got, _, _ := ThemeFor(a).Selected().Decompose()
		if got != want {
			t.Errorf("%s selection foreground differs; it should not need to", a)
		}
	}
}

func TestSelectionDoesNotUseReverseVideoWhenAPaletteExists(t *testing.T) {
	_, _, attrs := DefaultTheme().Selected().Decompose()
	if attrs&tcell.AttrReverse != 0 {
		t.Error("a coloured selection must set both colours instead of reversing")
	}
}

// Bg and Fg stay default in every appearance: the terminal's own are already
// right, and overriding them fights the user's theme instead of fitting it.
func TestAppearancesNeverOverrideTerminalBackground(t *testing.T) {
	for _, a := range []Appearance{AppearanceUniversal, AppearanceLight, AppearanceDark, AppearanceNeutral} {
		th := ThemeFor(a)
		if th.Bg != tcell.ColorDefault || th.Fg != tcell.ColorDefault {
			t.Errorf("%s overrides the terminal's own colours", a)
		}
	}
}

func TestDetectAppearance(t *testing.T) {
	// The setting is authoritative, because detection cannot be relied on.
	t.Setenv("COLORFGBG", "15;0")
	t.Setenv("NINES_THEME", "light")
	if got := DetectAppearance(); got != AppearanceLight {
		t.Errorf("NINES_THEME must win over COLORFGBG, got %s", got)
	}

	t.Setenv("NINES_THEME", "")
	for value, want := range map[string]Appearance{
		"15;0":  AppearanceDark,
		"0;15":  AppearanceLight,
		"7;0":   AppearanceDark,
		"0;7":   AppearanceLight,
		"":      AppearanceUniversal,
		"15;12": AppearanceUniversal, // an index we cannot classify
		"junk":  AppearanceUniversal,
	} {
		t.Setenv("COLORFGBG", value)
		if got := DetectAppearance(); got != want {
			t.Errorf("COLORFGBG=%q gave %s, want %s", value, got, want)
		}
	}
}

// An unreadable setting must fall back rather than fail, and must never leave
// the caller with a palette that does not exist.
func TestParseAppearanceFallsBack(t *testing.T) {
	if a, ok := ParseAppearance("chartreuse"); ok || a != AppearanceUniversal {
		t.Errorf("nonsense should fall back to universal, got %s ok=%v", a, ok)
	}
	for _, name := range []string{"light", "Dark", " neutral ", "grey", "gray"} {
		if _, ok := ParseAppearance(name); !ok {
			t.Errorf("%q should parse", name)
		}
	}
}

// NO_COLOR wins over every appearance: with no palette the ratios are moot and
// the glyphs are what carry meaning.
func TestNoColorAppliesWhateverTheAppearance(t *testing.T) {
	t.Setenv("NINES_THEME", "dark")
	t.Setenv("NO_COLOR", "1")
	theme := ThemeFromEnv()
	if !theme.Monochrome {
		t.Fatal("NO_COLOR was not detected")
	}
	for name, c := range tokens(theme) {
		if c != tcell.ColorDefault {
			t.Errorf("%s still carries colour under NO_COLOR", name)
		}
	}
}

// EnsureLegible is the guard for colours the palette tests cannot see: one a
// tool sets itself, or one a transform produces.
func TestEnsureLegibleRescuesABadColour(t *testing.T) {
	cases := []struct {
		appearance Appearance
		bad        int32
		bg         tcell.Color
		floor      float64
		what       string
	}{
		// ANSI-yellow-alike on white: 1.07:1, the exact failure that was
		// reported.
		{AppearanceLight, 0xffff00, bgWhite, 4.5, "bright yellow on a light terminal"},
		{AppearanceLight, 0x00ffff, bgWhite, 4.5, "bright cyan on a light terminal"},
		// Near-black on a dark terminal.
		{AppearanceDark, 0x101010, bgBlack, 4.5, "near-black on a dark terminal"},
		{AppearanceNeutral, 0x707070, bgGrey, 3.0, "mid-grey on a mid-grey terminal"},
	}
	for _, c := range cases {
		theme := ThemeFor(c.appearance)
		bad := tcell.NewHexColor(c.bad)

		if before := contrastRatio(bad, c.bg); before >= c.floor {
			t.Fatalf("%s: test colour already passes (%.2f:1)", c.what, before)
		}
		fixed := theme.EnsureLegible(bad)
		if got := contrastRatio(fixed, c.bg); got < c.floor {
			t.Errorf("%s: still %.2f:1 after rescue, want >= %.1f:1", c.what, got, c.floor)
		}
	}
}

// A colour that already clears its floor must be returned untouched — the guard
// is a floor, not a filter that repaints everything.
func TestEnsureLegibleLeavesGoodColoursAlone(t *testing.T) {
	for _, a := range []Appearance{AppearanceUniversal, AppearanceLight, AppearanceDark, AppearanceNeutral} {
		theme := ThemeFor(a)
		for name, col := range tokens(theme) {
			if got := theme.EnsureLegible(col); got != col {
				t.Errorf("%s/%s was adjusted despite already passing", a, name)
			}
		}
	}
}

// Under NO_COLOR there is no palette to fix, and forcing a colour back in would
// undo the whole point.
func TestEnsureLegibleIsInertUnderNoColor(t *testing.T) {
	t.Setenv("NO_COLOR", "1")
	theme := ThemeFromEnv()
	if got := theme.EnsureLegible(tcell.NewHexColor(0xffff00)); got != tcell.NewHexColor(0xffff00) {
		t.Error("NO_COLOR should leave EnsureLegible inert")
	}
	if got := theme.EnsureLegible(tcell.ColorDefault); got != tcell.ColorDefault {
		t.Error("a default colour has no RGB to adjust and must pass through")
	}
}

// Universal has to satisfy white and black at once, so its guard clamps into
// the intersection rather than pushing to one extreme.
func TestEnsureLegibleUniversalSatisfiesBothExtremes(t *testing.T) {
	theme := ThemeFor(AppearanceUniversal)
	for _, bad := range []int32{0xffff00, 0x00ffff, 0x000000, 0xffffff} {
		fixed := theme.EnsureLegible(tcell.NewHexColor(bad))
		onWhite := contrastRatio(fixed, bgWhite)
		onBlack := contrastRatio(fixed, bgBlack)
		if onWhite < 3.0 || onBlack < 3.0 {
			t.Errorf("#%06x rescued to white=%.2f:1 black=%.2f:1, want both >= 3:1",
				bad, onWhite, onBlack)
		}
	}
}

// The focused field is a highlight and must use the SAME pair as a selected
// row. They came apart once — Select moved from a light blue to a dark slate,
// Selected() followed, and the field's hardcoded black did not, leaving text at
// 1.65:1. Deriving both from one place is the fix; this asserts it holds.
func TestFocusedFieldUsesTheSelectionPair(t *testing.T) {
	for _, a := range []Appearance{AppearanceUniversal, AppearanceLight, AppearanceDark, AppearanceNeutral} {
		theme := ThemeFor(a)
		ApplyGlobalStyles(theme)

		selFg, selBg, _ := theme.Selected().Decompose()
		if tview.Styles.ContrastBackgroundColor != selBg {
			t.Errorf("%s: field background is not the selection background", a)
		}
		if tview.Styles.InverseTextColor != selFg {
			t.Errorf("%s: text on that background is not the selection foreground", a)
		}
		if got := contrastRatio(selFg, selBg); got < 4.5 {
			t.Errorf("%s: focused field %.2f:1, want >= 4.5:1", a, got)
		}
	}
}

// tview's own defaults assume a black terminal: white borders and titles are
// invisible on a light one, and secondary text is yellow at 1.07:1. Nothing in
// a view can fix that, because those widgets never consult our theme.
func TestGlobalStylesAreThemedNotTviewDefaults(t *testing.T) {
	theme := ThemeFor(AppearanceLight)
	ApplyGlobalStyles(theme)

	if tview.Styles.BorderColor == tcell.ColorWhite {
		t.Error("borders are still tview's white — invisible on a light terminal")
	}
	if tview.Styles.SecondaryTextColor == tcell.ColorYellow {
		t.Error("secondary text is still tview's yellow — 1.07:1 on white")
	}
	if tview.Styles.PrimitiveBackgroundColor != theme.Bg {
		t.Error("widget backgrounds do not inherit the terminal's own")
	}

	// Every global that is drawn as text must clear the floor against the
	// background it lands on.
	for name, fg := range map[string]tcell.Color{
		"BorderColor": tview.Styles.BorderColor,
		"TitleColor":  tview.Styles.TitleColor,
		"Secondary":   tview.Styles.SecondaryTextColor,
		"Tertiary":    tview.Styles.TertiaryTextColor,
	} {
		if got := contrastRatio(fg, bgWhite); got < 3.0 {
			t.Errorf("%s on a white terminal = %.2f:1, want >= 3:1", name, got)
		}
	}
}
