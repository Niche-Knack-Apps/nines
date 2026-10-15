// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"testing"

	"github.com/gdamore/tcell/v2"
	"github.com/rivo/tview"
)

// A form field's box and its text must be opposing on EVERY terminal. tview's
// form default paints the field navy under the terminal's own foreground,
// which on a light terminal is dark-on-dark; and Form.Draw re-applies the
// form's field style to each item on every draw, so a colour set on the field
// itself never reaches the screen. The only fix that survives Draw is the
// form-level style, and the only pair with a terminal-independent guarantee is
// the selection pair. This renders a form and reads the cell back.
func TestEnhancedFormFieldsAreDrawnInTheSelectionPair(t *testing.T) {
	for _, a := range []Appearance{AppearanceUniversal, AppearanceLight, AppearanceDark, AppearanceNeutral} {
		theme := ThemeFor(a)
		app := NewApp(theme, Art{})

		form := tview.NewForm()
		form.AddInputField("name", "value", 0, nil, nil)
		AddField(app, form, "path", "/tmp", 0, nil)
		EnhanceForm(app, form)

		screen := tcell.NewSimulationScreen("")
		if err := screen.Init(); err != nil {
			t.Fatal(err)
		}
		screen.SetSize(40, 6)
		form.SetRect(0, 0, 40, 6)
		form.Draw(screen)

		selFg, selBg, _ := theme.Selected().Decompose()
		for _, row := range []int{1, 3} { // plain field, then the house field
			// A row/column of padding, then "name "/"path ": the box starts at column 6.
			_, _, style, _ := screen.GetContent(6, row)
			fg, bg, _ := style.Decompose()
			if bg != selBg {
				t.Errorf("%s row %d: field background %v, want the selection background %v", a, row, bg, selBg)
			}
			if fg != selFg {
				t.Errorf("%s row %d: field text %v, want the selection foreground %v", a, row, fg, selFg)
			}
		}
	}
}

// The defect this guards against: without the form-level style, the box is
// navy under the terminal's own foreground. On a light terminal that
// foreground is dark, so the cell is dark-on-dark. The test cannot see the
// terminal's real foreground (it is ColorDefault), so it asserts the shape of
// the defect: a fixed background paired with a foreground that is left to the
// terminal — the pair that CONTRACT §8 says can never be promised legible.
func TestUnenhancedFormLeavesTheFieldTextToTheTerminal(t *testing.T) {
	ApplyGlobalStyles(ThemeFor(AppearanceLight))
	form := tview.NewForm()
	form.AddInputField("name", "value", 0, nil, nil)

	screen := tcell.NewSimulationScreen("")
	if err := screen.Init(); err != nil {
		t.Fatal(err)
	}
	screen.SetSize(40, 6)
	form.SetRect(0, 0, 40, 6)
	form.Draw(screen)

	_, _, style, _ := screen.GetContent(6, 1)
	fg, bg, _ := style.Decompose()
	if bg == tcell.ColorDefault || fg != tcell.ColorDefault {
		t.Skipf("tview's form default changed (fg %v on bg %v); the guard above may be moot", fg, bg)
	}
}
