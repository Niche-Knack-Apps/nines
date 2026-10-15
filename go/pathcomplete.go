// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/rivo/tview"
)

// PathCandidates completes a filesystem path the way zsh does: the directory
// part is taken as typed (with ~ expanded), and entries matching the base
// prefix come back as full paths, directories with a trailing slash.
func PathCandidates(input string) []string {
	if input == "" {
		return nil
	}
	expanded := input
	if home, err := os.UserHomeDir(); err == nil {
		if expanded == "~" {
			expanded = home
		} else if strings.HasPrefix(expanded, "~/") {
			expanded = filepath.Join(home, expanded[2:])
		}
	}

	dir, base := filepath.Split(expanded)
	if dir == "" {
		dir = "."
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}

	var out []string
	for _, e := range entries {
		name := e.Name()
		if !strings.HasPrefix(name, base) {
			continue
		}
		// Hidden entries only complete when explicitly asked for.
		if strings.HasPrefix(name, ".") && !strings.HasPrefix(base, ".") {
			continue
		}
		full := filepath.Join(dir, name)
		if e.IsDir() {
			full += string(os.PathSeparator)
		}
		out = append(out, full)
	}
	sort.Strings(out)
	return out
}

// expandedField is the full-width editor a cramped field expands into.
type expandedField struct {
	*tview.Flex
	title string
}

func (e *expandedField) Name() string { return e.title }
func (e *expandedField) Hints() []Hint {
	return []Hint{
		{Key: "enter/←←", Desc: "apply + collapse"},
		{Key: "esc", Desc: "discard"},
	}
}

func (a *App) expandField(src *tview.InputField, title string, complete func(string) []string) {
	big := tview.NewInputField().
		SetLabel(" " + title + " > ").
		SetText(src.GetText()).
		SetFieldBackgroundColor(a.theme.Bg).
		SetFieldTextColor(a.theme.Fg).
		SetLabelColor(a.theme.Accent)
	if complete != nil {
		big.SetAutocompleteFunc(complete)
	}

	commit := func() {
		src.SetText(big.GetText())
		a.Pop()
	}
	var lastLeft time.Time
	big.SetInputCapture(func(event *tcell.EventKey) *tcell.EventKey {
		switch event.Key() {
		case tcell.KeyEnter:
			commit()
			return nil
		case tcell.KeyEsc:
			a.Pop()
			return nil
		case tcell.KeyLeft:
			now := time.Now()
			if now.Sub(lastLeft) < 450*time.Millisecond {
				commit()
				return nil
			}
			lastLeft = now
		case tcell.KeyTab:
			if complete != nil {
				if cands := complete(big.GetText()); len(cands) > 0 {
					big.SetText(cands[0])
					return nil
				}
			}
		}
		return event
	})

	wrap := tview.NewFlex().SetDirection(tview.FlexRow).
		AddItem(tview.NewBox(), 0, 1, false).
		AddItem(big, 1, 0, true).
		AddItem(tview.NewBox(), 0, 1, false)
	a.Push(&expandedField{Flex: wrap, title: title})
	a.SetFocus(big)
}
