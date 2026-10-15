// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"reflect"
	"testing"
)

func TestRankCandidates(t *testing.T) {
	cmds := []string{"deploy", "describe", "packages", "pkg", "quit", "help"}

	tests := []struct {
		name   string
		input  string
		cands  []string
		recent map[string]int
		want   []string
	}{
		{
			// "help" and "deploy" both contain a 'p', so they match as
			// subsequences and rank below the real prefixes, shorter first.
			name:  "exact prefix beats subsequence, shorter first",
			input: "p",
			cands: cmds,
			want:  []string{"pkg", "packages", "help", "deploy"},
		},
		{
			name:  "fuzzy subsequence reaches deploy",
			input: "dpl",
			cands: cmds,
			want:  []string{"deploy"},
		},
		{
			name:  "case-insensitive prefix ranks below exact prefix",
			input: "De",
			cands: []string{"describe", "Deploy"},
			want:  []string{"Deploy", "describe"},
		},
		{
			name:   "recency promotes within the same tier",
			input:  "p",
			cands:  []string{"pkg", "packages"},
			recent: map[string]int{"packages": 5},
			want:   []string{"packages", "pkg"},
		},
		{
			name:  "empty input yields nothing",
			input: "",
			cands: cmds,
			want:  nil,
		},
		{
			name:  "no match yields nothing",
			input: "zzzz",
			cands: cmds,
			want:  nil,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := RankCandidates(tc.input, tc.cands, tc.recent)
			if len(got) == 0 && len(tc.want) == 0 {
				return
			}
			if !reflect.DeepEqual(got, tc.want) {
				t.Errorf("RankCandidates(%q) = %v, want %v", tc.input, got, tc.want)
			}
		})
	}
}

func TestIsSubsequence(t *testing.T) {
	yes := [][2]string{{"dpl", "deploy"}, {"", "anything"}, {"abc", "abc"}, {"ac", "abc"}}
	no := [][2]string{{"cba", "abc"}, {"abcd", "abc"}, {"z", "abc"}}

	for _, p := range yes {
		if !isSubsequence(p[0], p[1]) {
			t.Errorf("isSubsequence(%q, %q) = false, want true", p[0], p[1])
		}
	}
	for _, p := range no {
		if isSubsequence(p[0], p[1]) {
			t.Errorf("isSubsequence(%q, %q) = true, want false", p[0], p[1])
		}
	}
}

// The ghost is only shown for candidates that literally extend what was typed;
// a fuzzy match that reorders characters would make the inline text lie.
func TestGhostOnlyForPrefixExtension(t *testing.T) {
	p := NewPromptField(DefaultTheme(), ":")
	p.SetSuggestFunc(func(input string) []string {
		return RankCandidates(input, []string{"deploy", "packages"}, nil)
	})

	p.recompute("dep")
	if got := p.Ghost(); got != "deploy" {
		t.Errorf("ghost for %q = %q, want %q", "dep", got, "deploy")
	}

	// "dpl" reaches deploy only as a subsequence, so there is no honest inline
	// completion -- it belongs in the dropdown instead.
	p.recompute("dpl")
	if got := p.Ghost(); got != "" {
		t.Errorf("ghost for %q = %q, want empty (subsequence match)", "dpl", got)
	}

	p.recompute("")
	if got := p.Ghost(); got != "" {
		t.Errorf("ghost for empty input = %q, want empty", got)
	}
}

func TestAcceptGhostWord(t *testing.T) {
	p := NewPromptField(DefaultTheme(), ":")
	p.SetSuggestFunc(func(input string) []string {
		return RankCandidates(input, []string{"web-app-commit"}, nil)
	})

	p.SetText("web")
	if !p.acceptGhostWord() {
		t.Fatal("acceptGhostWord() = false, want true")
	}
	if got := p.GetText(); got != "web-app" {
		t.Errorf("after word accept = %q, want %q", got, "web-app")
	}

	if !p.acceptGhostWord() {
		t.Fatal("second acceptGhostWord() = false, want true")
	}
	if got := p.GetText(); got != "web-app-commit" {
		t.Errorf("after second word accept = %q, want %q", got, "web-app-commit")
	}

	if p.acceptGhostWord() {
		t.Error("acceptGhostWord() at end = true, want false")
	}
}
