package nines

import (
	"testing"

	"github.com/gdamore/tcell/v2"
)

// The opt-in that makes "dark" mean a dark app rather than a dark-suited
// palette. Off unless asked for: blending into the user's scheme is the
// documented default (CONTRACT §8).
func TestBackgroundIsNotPaintedUnlessAskedFor(t *testing.T) {
	t.Setenv("NINES_PAINT_BACKGROUND", "")
	if _, painting := ThemeFor(AppearanceDark).CanvasStyle(); painting {
		t.Fatal("painted over the terminal without being asked")
	}

	t.Setenv("NINES_PAINT_BACKGROUND", "1")
	theme := ThemeFor(AppearanceDark)
	style, painting := theme.CanvasStyle()
	if !painting {
		t.Fatal("asked for, but not painting")
	}
	if _, bg, _ := style.Decompose(); bg != theme.Canvas {
		t.Fatalf("painted %v, want the palette's canvas %v", bg, theme.Canvas)
	}
}

// A painted background needs a foreground chosen for IT. ColorDefault is the
// terminal's own, picked for the terminal's background — paint over that and
// the text can vanish.
func TestPaintingSuppliesAForegroundToo(t *testing.T) {
	t.Setenv("NINES_PAINT_BACKGROUND", "1")
	for _, a := range []Appearance{AppearanceLight, AppearanceDark, AppearanceNeutral} {
		style, painting := ThemeFor(a).CanvasStyle()
		if !painting {
			t.Fatalf("%v: not painting", a)
		}
		fg, bg, _ := style.Decompose()
		if fg == tcell.ColorDefault {
			t.Fatalf("%v: left the foreground to the terminal on a painted background", a)
		}
		if fg == bg {
			t.Fatalf("%v: invisible text, fg == bg == %v", a, fg)
		}
	}
}

// NO_COLOR wins: an explicit request for no colour is not overridden by a
// request for a particular one.
func TestNoColorBeatsARequestToPaint(t *testing.T) {
	t.Setenv("NINES_PAINT_BACKGROUND", "1")
	t.Setenv("NO_COLOR", "1")
	if _, painting := ThemeFromEnv().CanvasStyle(); painting {
		t.Fatal("painted despite NO_COLOR")
	}
}
