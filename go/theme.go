// SPDX-License-Identifier: Apache-2.0

// Package nines is the tview/tcell implementation of the 9s UI contract: the
// application shell, the page stack, key handling, and the ghost-text command
// prompt.
//
// CONTRACT.md in the repository root is normative. Where this package and the
// contract disagree, this package is wrong. A Rust implementation of the same
// contract lives in rust/.
package nines

import (
	"os"
	"strings"

	"github.com/gdamore/tcell/v2"
)

// Theme is the colour palette. Values are deliberately few: a TUI reads best
// when colour carries meaning rather than decoration.
type Theme struct {
	Bg        tcell.Color
	Fg        tcell.Color
	Dim       tcell.Color // ghost text, secondary columns, hints
	Accent    tcell.Color // logo, headers, hints
	Canvas    tcell.Color // background these foregrounds were measured against
	PaintBg   bool        // paint Canvas instead of deferring to the terminal
	Select    tcell.Color // selected row / focused field background
	Header    tcell.Color // table header row
	OK        tcell.Color
	Warn      tcell.Color
	Error     tcell.Color
	Sensitive tcell.Color

	// Appearance is the background this palette was tuned for.
	Appearance Appearance

	// Monochrome reports that the palette was emptied for NO_COLOR. Callers
	// generally need not check it — the colours already resolve to default —
	// but Selected does, since with no palette reverse video is the only way
	// left to show a selection.
	Monochrome bool
}

// DefaultTheme is the universal palette: legible on a light or a dark terminal
// without knowing which — see CONTRACT §8.2. A tool that knows better should
// use ThemeFromEnv, which honours the user's own setting.
//
// The terminal's own background and foreground are inherited rather than
// overridden, so the tool fits into the user's colour scheme instead of
// fighting it. A tool with its own brand colour overrides the individual field
// rather than replacing the theme.
func DefaultTheme() Theme { return ThemeFor(AppearanceUniversal) }

// ThemeFromEnv builds the theme for this terminal: the appearance the user
// asked for (or that could be detected), honouring NO_COLOR — CONTRACT §9.8.
//
// Per the NO_COLOR convention any value disables colour, including an empty
// one; only absence enables it.
//
// Colour is removed by emptying the PALETTE rather than by branching at every
// call site: every `theme.OK` in every tool then resolves to the terminal's own
// default with no code change, and it becomes impossible to add a call site
// that forgets to check. The Rust implementation carries a `monochrome` flag
// instead because its accessors already funnel through one paint().
// ThemeFromSetting builds a theme from a configured setting.
//
// Precedence, highest first:
//
//  1. NINES_THEME in the environment
//  2. the setting passed in, from a config file
//  3. detection
//
// The environment has to win. A config file is shared by every machine that
// checks out the dotfiles, so it is the wrong place to say what one terminal
// looks like — and if the file beats the variable, there is no way to override
// it for a single session at all.
//
// "light" | "dark" | "neutral" pick a palette outright. "system" (or "auto", or
// an empty string) means work it out, which is ThemeFromEnv.
//
// A setting is needed because detection cannot be relied on: it reads
// COLORFGBG, and most terminals never set it. A user on a light terminal
// therefore gets the Universal compromise palette rather than the light one,
// with no way to say otherwise. This is that way, and every 9s app should
// offer it.
//
// NO_COLOR still wins: an explicit request for no colour is not overridden by
// a preference for which colours.
func ThemeFromSetting(setting string) Theme {
	// An explicit variable beats the file. ThemeFromEnv honours it too, but
	// only when the file says "system" — which is exactly the case someone
	// overriding a hard-coded setting is not in.
	if env := strings.TrimSpace(os.Getenv("NINES_THEME")); env != "" {
		setting = env
	}
	switch strings.ToLower(strings.TrimSpace(setting)) {
	case "", "system", "auto":
		return ThemeFromEnv()
	}
	a, ok := ParseAppearance(setting)
	if !ok {
		// A setting nobody can read is not a reason to fail to start.
		return ThemeFromEnv()
	}
	return honourNoColor(ThemeFor(a))
}

func ThemeFromEnv() Theme {
	return honourNoColor(ThemeFor(DetectAppearance()))
}

// honourNoColor strips the palette when NO_COLOR is set.
//
// One definition, because both entry points have to obey it and a second copy
// would eventually only strip some of the colours.
func honourNoColor(t Theme) Theme {
	if _, ok := os.LookupEnv("NO_COLOR"); !ok {
		return t
	}
	t.Monochrome = true
	for _, c := range []*tcell.Color{
		&t.Bg, &t.Fg, &t.Dim, &t.Accent, &t.Select,
		&t.Header, &t.OK, &t.Warn, &t.Error, &t.Sensitive,
	} {
		*c = tcell.ColorDefault
	}
	return t
}

// Selected is the style for the highlighted table row.
//
// It is explicit on purpose. tview's default selection is reverse-video, and
// with ColorDefault foregrounds/backgrounds reverse-video can resolve to
// invisible text (default-on-default) depending on the terminal — the selected
// row's words literally disappear.
//
// The background is DARK and the text is light, which is the opposite of the
// obvious choice and the reason matters. A light selection background only
// works if every cell's foreground is forced to black, and any colour that
// survives onto it is then unreadable: against the mid-blue this used to use,
// dim grey measured 1.4:1, green 2.0:1 and yellow 2.5:1 — all far below the
// 4.5:1 §8 requires. Against a dark background the same colours measure 3.2:1
// to 11.9:1, so the row stays legible even where a semantic colour shows
// through, and white-on-dark is 12.7:1.
//
// This is the one place the "prefer the 16 ANSI colours" guidance is
// deliberately set aside: ANSI blue is whatever the user's theme says it is,
// and a contrast guarantee cannot be built on a colour that moves.
func (t Theme) Selected() tcell.Style {
	if t.Monochrome {
		// The one place reverse video is correct: with no palette to work
		// with, it is the only way to show selection at all.
		return tcell.StyleDefault.Reverse(true).Bold(true)
	}
	return tcell.StyleDefault.
		Background(t.Select).
		// An explicit RGB white, NOT tcell.ColorWhite. ColorWhite is ANSI
		// index 7, and what index 7 looks like is up to the terminal's
		// palette — light schemes routinely remap it to a DARK tone so that
		// "white" text stays readable on a white page. The background here is
		// a fixed RGB navy, so a foreground that moves gives dark-on-dark.
		//
		// The comment above already made this argument about blue. It applies
		// just as much to white.
		Foreground(SelectionText).
		Bold(true)
}

// SelectionText is the foreground of a selected row: fixed RGB, never a
// palette index, so the terminal's scheme cannot redefine it out from under the
// contrast guarantee in Selected.
var SelectionText = tcell.NewHexColor(0xffffff)

// CanvasStyle is the style to fill the screen with before drawing anything.
//
// ok is false when the terminal's own background should show through, which is
// the default and what CONTRACT §8 asks for.
func (t Theme) CanvasStyle() (tcell.Style, bool) {
	if !t.PaintBg || t.Monochrome {
		return tcell.StyleDefault, false
	}
	return tcell.StyleDefault.
		Background(t.Canvas).
		Foreground(CanvasText(t.Appearance)), true
}

// StatusColor maps a package load status string to its colour.
func (t Theme) StatusColor(status string) tcell.Color {
	switch status {
	case "ok":
		return t.OK
	case "raw":
		return t.Warn
	case "broken":
		return t.Error
	case "artifact":
		return t.Dim
	}
	return t.Fg
}
