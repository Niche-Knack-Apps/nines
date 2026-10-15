// SPDX-License-Identifier: Apache-2.0

package nines

import "github.com/gdamore/tcell/v2"

// Health is the state vocabulary shared by every 9s tool — CONTRACT.md §8.1.
//
// Colour encodes STATE, never category, so a reader who learned the grammar in
// k9s or p9s can read z9s without learning anything new.
//
// The Idle/Failed split is the rule that carries the weight. Painting every
// stopped thing red is the easy mistake and it destroys the signal: on a
// working machine most stopped things stopped on purpose, so red stops meaning
// "look at this" and becomes wallpaper. A container that exited 0, or a package
// on disk that was never deployed, is Idle — inert, not broken.
//
// The corollary costs something and is the point: green is spent only on the
// live thing.
type Health int

const (
	// HealthIdle is inert: nothing is happening, and nothing is wrong.
	HealthIdle Health = iota
	// HealthOK is working as intended — running, ready, deployed.
	HealthOK
	// HealthTransitional is on its way somewhere and expected to settle.
	HealthTransitional
	// HealthFailed is broken and wants attention.
	HealthFailed
)

// String names the state.
func (h Health) String() string {
	switch h {
	case HealthOK:
		return "ok"
	case HealthTransitional:
		return "transitional"
	case HealthFailed:
		return "failed"
	}
	return "idle"
}

// Glyph is the mark that carries the state when colour cannot.
//
// Per §8, colour never carries meaning alone: roughly 8% of men cannot separate
// red from green, which is exactly the pair a status column reaches for, and
// under NO_COLOR there is no palette at all. A row that already spells the state
// in words satisfies the rule without this; a row that would otherwise be
// colour alone must show it.
func (h Health) Glyph() string {
	switch h {
	case HealthOK:
		return "●"
	case HealthTransitional:
		return "◐"
	case HealthFailed:
		return "✖"
	}
	return "·"
}

// HealthColor maps a state onto the palette.
//
// Under a monochrome theme every branch resolves to the default colour, because
// the palette itself was emptied — the glyph is what survives.
func (t Theme) HealthColor(h Health) tcell.Color {
	switch h {
	case HealthOK:
		return t.OK
	case HealthTransitional:
		return t.Warn
	case HealthFailed:
		return t.Error
	}
	return t.Dim
}

// HealthLabel renders a state as text plus its colour, prefixing the glyph when
// the caller has no word of its own to show.
//
// It mirrors Theme.signed in the Rust implementation: the marking is in the
// TEXT, and the colour is the redundant part rather than the other way round.
func (t Theme) HealthLabel(h Health, word string) (string, tcell.Color) {
	if word == "" {
		return h.Glyph(), t.HealthColor(h)
	}
	return word, t.HealthColor(h)
}
