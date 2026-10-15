// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"github.com/gdamore/tcell/v2"
	"github.com/rivo/tview"
)

// EnhanceForm applies the house form behaviour to every field:
//
//   - every text field is drawn in the SELECTION pair — fixed white on fixed
//     navy — so its box and its text are always opposing, whatever the
//     terminal's own colours are; the focused field is the one with the cursor
//   - Up/Down move between fields (unless the field's completion dropdown has
//     candidates, which the arrows then navigate)
//   - Enter commits the typed text and deselects the field — focus parks, and
//     the next Tab or arrow resumes the highlight right where it left off
func EnhanceForm(app *App, form *tview.Form) {
	theme := app.Theme()

	form.SetButtonStyle(tcell.StyleDefault.Background(theme.Bg).Foreground(theme.Dim))
	// Theme.Selected(), not a style hand-rolled from theme.Select. This line
	// used to say Foreground(tcell.ColorBlack) against that same dark
	// background — black on navy, which is unreadable on any terminal and was
	// simply invisible on a light one. The contrast guarantee lives in
	// Selected(); anything that builds its own style from Select opts out of it.
	form.SetButtonActivatedStyle(theme.Selected())

	// The field colours are set on the FORM, not on the fields. tview's
	// Form.Draw pushes the form's own field style onto every item on every
	// draw (SetFormAttributes), so a colour set on an item — including one set
	// from a focus or blur callback — is overwritten before the item paints.
	// This used to set per-field focus/blur colours that way; none of them ever
	// reached the screen, and what did reach it was tview's form default:
	// ContrastBackgroundColor (the navy) under PrimaryTextColor (the
	// terminal's foreground). On a dark terminal that is light-on-navy and
	// looked intended; on a light terminal it is dark-on-navy and unreadable.
	//
	// Selected() is the one pair with a contrast guarantee that does not
	// depend on the terminal (both colours fixed RGB), so it is the pair every
	// field box uses. Bold is dropped: it is a row highlight's emphasis, not a
	// text field's.
	selected := theme.Selected().Bold(false)
	form.SetFieldStyle(selected)
	form.SetLabelColor(theme.Dim)

	for i := 0; i < form.GetFormItemCount(); i++ {
		idx := i
		switch field := form.GetFormItem(i).(type) {
		case *PromptField:
			// The house field: its own handler owns Tab/arrows/ghost; the
			// enhancer only supplies the Enter park.
			field.SetSubmitFunc(func(string) { app.ParkForm(form, idx) })
		case *tview.InputField:
			prev := field.GetInputCapture()
			field.SetInputCapture(func(event *tcell.EventKey) *tcell.EventKey {
				switch event.Key() {
				case tcell.KeyEnter:
					// Commit-and-deselect: text stays, focus parks, and the
					// next Tab/arrow resumes from this very field.
					app.ParkForm(form, idx)
					return nil
				case tcell.KeyDown:
					// Arrows always move; nothing is allowed to trap them.
					return tcell.NewEventKey(tcell.KeyTab, 0, tcell.ModNone)
				case tcell.KeyUp:
					return tcell.NewEventKey(tcell.KeyBacktab, 0, tcell.ModNone)
				}
				if prev != nil {
					return prev(event)
				}
				return event
			})
		}
	}
}

// ParkForm deselects the given form's fields while remembering position; the
// next Tab, Backtab, Enter, or vertical arrow resumes focus at the same field.
func (a *App) ParkForm(form *tview.Form, index int) {
	a.parkedForm, a.parkedIndex = form, index
	a.SetFocus(a.focusSink)
}

// resumeParkedForm restores focus to the parked field. Reports whether a park
// was active.
func (a *App) resumeParkedForm() bool {
	if a.parkedForm == nil {
		return false
	}
	form, idx := a.parkedForm, a.parkedIndex
	a.parkedForm = nil
	form.SetFocus(idx)
	a.SetFocus(form)
	return true
}

// clearParkedForm drops any parked state, for view changes.
func (a *App) clearParkedForm() { a.parkedForm = nil }
