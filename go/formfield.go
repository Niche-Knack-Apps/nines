// SPDX-License-Identifier: Apache-2.0

package nines

import "github.com/rivo/tview"

// AddField appends a text field to a form under the house keyboard contract:
//
//   - printable runes belong to the field — the global ':' and '/' shortcuts
//     are suppressed while any text input has focus, so paths and oci:// URLs
//     type cleanly
//   - Tab/Backtab and Up/Down are pure navigation between fields, always
//   - Enter commits the text and parks focus (EnhanceForm wires the park)
//   - completion, when given, renders as inline ghost text only — no dropdown
//     to trap the cursor; Right at end (or ctrl-f, word-wise) accepts it
//   - a quick double Right with no ghost pending expands the field full-width
//
// This constructor is the single place the contract lives; forms must not
// attach their own key handling to text fields.
func AddField(app *App, form *tview.Form, label, value string, width int, complete func(string) []string) *PromptField {
	f := &PromptField{
		InputField: tview.NewInputField(),
		theme:      app.Theme(),
	}
	f.SetLabel(label).SetText(value).SetFieldWidth(width)
	// The ghost recomputes on every change — NewPromptField wires this for the
	// command bar; a directly-constructed field must wire it too.
	f.SetChangedFunc(func(text string) { f.recompute(text) })
	f.SetFormMode(func() { app.expandField(f.InputField, label, complete) })
	if complete != nil {
		f.SetSuggestFunc(complete)
	}
	form.AddFormItem(f)
	return f
}

// FieldText reads a text value from either a plain InputField or a PromptField
// form item.
func FieldText(item tview.FormItem) string {
	if g, ok := item.(interface{ GetText() string }); ok {
		return g.GetText()
	}
	return ""
}
