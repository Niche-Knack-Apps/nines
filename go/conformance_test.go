// SPDX-License-Identifier: Apache-2.0

package nines

import "testing"

// Art measurement is the one piece of geometry both implementations must agree
// on exactly: shared artwork that lays out differently in Go and Rust is the
// drift this repository exists to prevent.
//
// The same table lives in rust/src/layout.rs. Changing a case here without
// changing it there is a contract break.
func TestArtSizeMatchesTheContract(t *testing.T) {
	cases := []struct {
		name string
		art  string
		w, h int
	}{
		{"empty reserves nothing", "", 0, 0},
		{"single line", "abc", 3, 1},
		{"width is the widest line", "ab\nabcd\nabc", 4, 3},
		{"colour markup is invisible", "[#ff0000]X[-]", 1, 1},
		{"any tag is markup, not just hex", "[red]ab[-]c", 3, 1},
		{"escaped bracket is one cell", "[[]", 1, 1},
		{"wide glyphs take two columns", "漢字", 4, 1},
		{"unterminated tag runs to end of line", "ab[unclosed", 2, 1},
		{"trailing newline terminates, not adds", "a\n", 1, 1},
		{"a blank line between art IS a line", "a\n\n", 1, 2},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			w, h := artSize(c.art)
			if w != c.w || h != c.h {
				t.Fatalf("artSize(%q) = %dx%d, contract says %dx%d", c.art, w, h, c.w, c.h)
			}
		})
	}
}
