// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"sort"
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/mattn/go-runewidth"
	"github.com/rivo/tview"
)

// SuggestFunc returns ranked completion candidates for the current input.
type SuggestFunc func(input string) []string

// PromptField is an InputField that shows its best completion inline as dim
// "ghost" text after the cursor, accepted with Right-arrow or Tab.
//
// tview's InputField.SetAutocompleteFunc only renders a dropdown list; it has no
// inline suggestion support, so Draw is overridden to paint the un-typed
// remainder itself. Both mechanisms are used together: the ghost shows the top
// match without stealing focus, the dropdown shows the rest.
type PromptField struct {
	*tview.InputField

	theme   Theme
	suggest SuggestFunc

	// ghost is the full candidate text, not just the remainder.
	ghost string
	// lastRight times the double-Right expand chord in form mode.
	lastRight time.Time
	// labelWidth is the form-imposed label column width, captured from
	// SetFormAttributes; a form aligns all labels to it, so ghost drawing must
	// offset by it rather than by the label's own width.
	labelWidth int

	onSubmit func(text string)
	onCancel func()
	onChange func(text string)

	// formMode changes the key contract for use as a form item: Tab/Backtab
	// and Up/Down are pure navigation (handed to the form), Enter submits
	// (parks), and the ghost is accepted with Right at end or ctrl-f — never
	// with keys that mean "move".
	formMode func() // non-nil marks form mode; called to signal expansion
}

// SetFormMode marks this field as a form item and installs the expand hook
// fired on a double Right-arrow when no ghost is pending.
func (p *PromptField) SetFormMode(expand func()) *PromptField {
	p.formMode = expand
	return p
}

// SetChangeFunc installs a live-typing hook, fired on every text change after
// ghost recomputation. This is what makes '/' filtering apply as the user
// types instead of on Enter.
func (p *PromptField) SetChangeFunc(fn func(text string)) *PromptField {
	p.onChange = fn
	return p
}

// SetSubmitFunc installs the Enter handler.
//
// tview's own SetDoneFunc is deliberately not used: when the autocomplete
// dropdown is open, InputField routes Enter into the list to pick an item and
// never fires done, so a command typed with a live suggestion would need two
// Enters. Enter is intercepted in InputHandler instead.
func (p *PromptField) SetSubmitFunc(fn func(text string)) *PromptField {
	p.onSubmit = fn
	return p
}

// SetCancelFunc installs the Esc handler.
func (p *PromptField) SetCancelFunc(fn func()) *PromptField {
	p.onCancel = fn
	return p
}

// NewPromptField builds a prompt with the given label.
func NewPromptField(theme Theme, label string) *PromptField {
	p := &PromptField{
		InputField: tview.NewInputField(),
		theme:      theme,
	}
	p.SetLabel(label).
		SetFieldBackgroundColor(theme.Bg).
		SetFieldTextColor(theme.Fg).
		SetLabelColor(theme.Accent)

	p.SetChangedFunc(func(text string) {
		p.recompute(text)
		if p.onChange != nil {
			p.onChange(text)
		}
	})
	return p
}

// SetSuggestFunc installs the candidate source. It also wires tview's own
// dropdown so both affordances stay in sync.
func (p *PromptField) SetSuggestFunc(fn SuggestFunc) *PromptField {
	p.suggest = fn
	if p.formMode != nil {
		// Form fields use ghost text only. tview's dropdown steals Enter and
		// the arrow keys, which a form cannot tolerate.
		return p
	}
	p.SetAutocompleteFunc(func(currentText string) []string {
		if fn == nil || currentText == "" {
			return nil
		}
		return fn(currentText)
	})
	return p
}

// recompute refreshes the ghost candidate for the current text.
func (p *PromptField) recompute(text string) {
	p.ghost = ""
	if p.suggest == nil || text == "" {
		return
	}
	for _, c := range p.suggest(text) {
		// Only a candidate that literally extends what was typed can be shown
		// inline; fuzzy matches that reorder characters go to the dropdown.
		if len(c) > len(text) && strings.HasPrefix(strings.ToLower(c), strings.ToLower(text)) {
			p.ghost = text + c[len(text):]
			return
		}
	}
}

// Ghost returns the current inline suggestion, or "" if there is none.
func (p *PromptField) Ghost() string { return p.ghost }

// AcceptGhost commits the inline suggestion. It reports whether anything was
// accepted, so callers can fall through to normal cursor movement.
func (p *PromptField) AcceptGhost() bool {
	if p.ghost == "" || p.ghost == p.GetText() {
		return false
	}
	p.SetText(p.ghost)
	return true
}

// acceptGhostWord commits the suggestion up to the next word boundary, so a long
// completion can be taken in pieces.
func (p *PromptField) acceptGhostWord() bool {
	text := p.GetText()
	if p.ghost == "" || len(p.ghost) <= len(text) {
		return false
	}
	rest := p.ghost[len(text):]
	// Advance past any leading separators, then to the end of the next word.
	i := 0
	for i < len(rest) && isSep(rest[i]) {
		i++
	}
	for i < len(rest) && !isSep(rest[i]) {
		i++
	}
	p.SetText(text + rest[:i])
	return true
}

func isSep(b byte) bool {
	return b == ' ' || b == '-' || b == '/' || b == '.' || b == ':' || b == '_' || b == ','
}

// InputHandler intercepts the accept keys before InputField sees them.
//
// Right-arrow only accepts when the cursor is at the end of the text; otherwise
// it must still move the cursor, or in-line editing becomes impossible.
func (p *PromptField) InputHandler() func(*tcell.EventKey, func(tview.Primitive)) {
	return p.WrapInputHandler(func(event *tcell.EventKey, setFocus func(tview.Primitive)) {
		switch event.Key() {
		case tcell.KeyEnter:
			// Taken before InputField sees it, so an open dropdown cannot eat it.
			if p.onSubmit != nil {
				p.onSubmit(p.GetText())
			}
			return
		case tcell.KeyEsc:
			if p.onCancel != nil {
				p.onCancel()
				return
			}
		case tcell.KeyTab, tcell.KeyBacktab:
			// In a form, Tab is navigation and nothing else; the command bar
			// keeps Tab-accepts-ghost.
			if p.formMode == nil && event.Key() == tcell.KeyTab && p.AcceptGhost() {
				return
			}
			if p.formMode != nil {
				if h := p.InputField.InputHandler(); h != nil {
					h(event, setFocus) // finished() -> the form moves focus
				}
				return
			}
		case tcell.KeyDown, tcell.KeyUp:
			// Arrows are never blocked: in a form they move fields, always.
			if p.formMode != nil {
				key := tcell.KeyTab
				if event.Key() == tcell.KeyUp {
					key = tcell.KeyBacktab
				}
				if h := p.InputField.InputHandler(); h != nil {
					h(tcell.NewEventKey(key, 0, tcell.ModNone), setFocus)
				}
				return
			}
		case tcell.KeyRight:
			// Right accepts a live ghost; with none pending, a quick double
			// Right expands a cramped form field.
			if p.AcceptGhost() {
				p.lastRight = time.Time{}
				return
			}
			if p.formMode != nil {
				now := time.Now()
				if now.Sub(p.lastRight) < 450*time.Millisecond {
					p.lastRight = time.Time{}
					p.formMode()
					return
				}
				p.lastRight = now
			}
		case tcell.KeyCtrlF:
			if p.acceptGhostWord() {
				return
			}
		}
		if h := p.InputField.InputHandler(); h != nil {
			h(event, setFocus)
		}
	})
}

// SetFormAttributes captures the form's label column width for ghost drawing,
// then defers to the embedded field.
func (p *PromptField) SetFormAttributes(labelWidth int, labelColor, bgColor, fieldTextColor, fieldBgColor tcell.Color) tview.FormItem {
	p.labelWidth = labelWidth
	return p.InputField.SetFormAttributes(labelWidth, labelColor, bgColor, fieldTextColor, fieldBgColor)
}

// Draw renders the field, then paints the un-typed remainder of the suggestion
// in dim text immediately after the cursor.
func (p *PromptField) Draw(screen tcell.Screen) {
	p.InputField.Draw(screen)

	text := p.GetText()
	if p.ghost == "" || len(p.ghost) <= len(text) {
		return
	}
	suffix := p.ghost[len(text):]

	x, y, w, h := p.GetInnerRect()
	if h <= 0 || w <= 0 {
		return
	}
	// In a form the label column is form-imposed; standalone, it is the
	// label's own width (tag-aware — the axolotl mark occupies no cells).
	labelW := p.labelWidth
	if labelW == 0 {
		labelW = tview.TaggedStringWidth(p.GetLabel())
	}
	offset := labelW + runewidth.StringWidth(text)
	startX := x + offset
	avail := w - offset
	if avail <= 0 {
		return
	}

	style := tcell.StyleDefault.
		Background(p.theme.Bg).
		Foreground(p.theme.Dim).
		Dim(true)

	col := startX
	for _, r := range suffix {
		rw := runewidth.RuneWidth(r)
		if col+rw > x+w {
			break
		}
		screen.SetContent(col, y, r, nil, style)
		col += rw
	}
}

// RankCandidates orders candidates for completion: exact prefix first, then
// case-insensitive prefix, then subsequence (so "dpl" reaches "deploy"), with
// shorter matches ahead of longer ones inside each tier.
//
// recent, if non-nil, promotes previously used values -- this is what makes a
// variable field suggest the value you actually used last time.
func RankCandidates(input string, candidates []string, recent map[string]int) []string {
	if input == "" {
		return nil
	}
	lower := strings.ToLower(input)

	type scored struct {
		s    string
		tier int
		used int
	}
	var out []scored

	for _, c := range candidates {
		lc := strings.ToLower(c)
		tier := -1
		switch {
		case strings.HasPrefix(c, input):
			tier = 0
		case strings.HasPrefix(lc, lower):
			tier = 1
		case isSubsequence(lower, lc):
			tier = 2
		}
		if tier >= 0 {
			out = append(out, scored{c, tier, recent[c]})
		}
	}

	sort.SliceStable(out, func(i, j int) bool {
		if out[i].tier != out[j].tier {
			return out[i].tier < out[j].tier
		}
		if out[i].used != out[j].used {
			return out[i].used > out[j].used // more recently used first
		}
		return len(out[i].s) < len(out[j].s)
	})

	res := make([]string, 0, len(out))
	for _, s := range out {
		res = append(res, s.s)
	}
	return res
}

// isSubsequence reports whether every rune of need appears in hay in order.
func isSubsequence(need, hay string) bool {
	if need == "" {
		return true
	}
	i := 0
	for _, r := range hay {
		if rune(need[i]) == r {
			i++
			if i == len(need) {
				return true
			}
		}
	}
	return false
}
