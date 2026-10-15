// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"testing"

	"github.com/gdamore/tcell/v2"
)

func TestHealthColors(t *testing.T) {
	theme := DefaultTheme()
	cases := map[Health]tcell.Color{
		HealthOK:           theme.OK,
		HealthTransitional: theme.Warn,
		HealthFailed:       theme.Error,
		HealthIdle:         theme.Dim,
	}
	for h, want := range cases {
		if got := theme.HealthColor(h); got != want {
			t.Errorf("%v: got %v want %v", h, got, want)
		}
	}
}

// CONTRACT §8: colour never carries meaning alone. Every state must be
// distinguishable with no palette at all.
func TestEveryHealthHasADistinctGlyph(t *testing.T) {
	seen := map[string]Health{}
	for _, h := range []Health{HealthIdle, HealthOK, HealthTransitional, HealthFailed} {
		g := h.Glyph()
		if g == "" {
			t.Errorf("%v has no glyph", h)
		}
		if prev, dup := seen[g]; dup {
			t.Errorf("%v and %v share the glyph %q", prev, h, g)
		}
		seen[g] = h
	}
}

// A caller with a word of its own keeps it; the colour is the redundant part.
func TestHealthLabelPrefersTheCallersWord(t *testing.T) {
	theme := DefaultTheme()

	text, color := theme.HealthLabel(HealthOK, "running")
	if text != "running" {
		t.Errorf("got %q, want the caller's word", text)
	}
	if color != theme.OK {
		t.Error("colour should still be applied")
	}

	// With no word, the glyph is what carries the state.
	text, _ = theme.HealthLabel(HealthFailed, "")
	if text != HealthFailed.Glyph() {
		t.Errorf("got %q, want the glyph", text)
	}
}

// CONTRACT §9.8: the tool must be usable with NO_COLOR=1.
func TestNoColorEmptiesThePalette(t *testing.T) {
	t.Setenv("NO_COLOR", "1")
	theme := ThemeFromEnv()

	if !theme.Monochrome {
		t.Fatal("NO_COLOR was not detected")
	}
	// Emptying the palette is what makes every existing `theme.OK` call site
	// correct without being touched.
	for name, c := range map[string]tcell.Color{
		"OK": theme.OK, "Warn": theme.Warn, "Error": theme.Error,
		"Dim": theme.Dim, "Accent": theme.Accent, "Header": theme.Header,
	} {
		if c != tcell.ColorDefault {
			t.Errorf("%s still carries colour under NO_COLOR", name)
		}
	}
	// Every state now resolves to the same colour, so only the glyph separates
	// them — which is exactly why the glyph is required.
	if theme.HealthColor(HealthOK) != theme.HealthColor(HealthFailed) {
		t.Error("under NO_COLOR the states should be colour-identical")
	}
	if HealthOK.Glyph() == HealthFailed.Glyph() {
		t.Error("with no colour left, the glyphs must still differ")
	}
}

// Per the NO_COLOR convention, an EMPTY value still disables colour.
func TestNoColorEmptyValueStillDisables(t *testing.T) {
	t.Setenv("NO_COLOR", "")
	if !ThemeFromEnv().Monochrome {
		t.Error("an empty NO_COLOR must still disable colour")
	}
}

func TestColorEnabledWhenUnset(t *testing.T) {
	t.Setenv("NO_COLOR", "")
	// t.Setenv cannot unset, so verify against the default directly.
	if DefaultTheme().Monochrome {
		t.Error("the default theme must not be monochrome")
	}
	// The palette is appearance-tuned hex now, not a bare ANSI constant; what
	// matters is that colour is present at all.
	if DefaultTheme().OK == tcell.ColorDefault {
		t.Error("the default palette should carry colour")
	}
}

// With no palette, reverse video is the only way left to show selection — and
// it is the one place the §8 warning against it does not apply.
func TestMonochromeSelectionFallsBackToReverse(t *testing.T) {
	t.Setenv("NO_COLOR", "1")
	style := ThemeFromEnv().Selected()
	_, _, attrs := style.Decompose()
	if attrs&tcell.AttrReverse == 0 {
		t.Error("monochrome selection must use reverse video")
	}

	// With a palette, it must NOT: reverse can resolve to invisible text.
	_, _, attrs = DefaultTheme().Selected().Decompose()
	if attrs&tcell.AttrReverse != 0 {
		t.Error("a coloured selection must set both colours explicitly instead")
	}
}
