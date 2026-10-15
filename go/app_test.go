// SPDX-License-Identifier: Apache-2.0

package nines

import "testing"

// Capabilities used only by these tests. The shell ships none of its own — they
// name a tool's verbs, not the shell's.
const (
	capInspect Capability = "inspect"
	capDeploy  Capability = "deploy"
	capRemove  Capability = "remove"
)

func testApp() *App {
	return NewApp(DefaultTheme(), Art{})
}

func TestReadOnlyPolicyFiltersAndRejectsMutatingActions(t *testing.T) {
	app := testApp()
	inspect := Action{Key: "enter", Desc: "inspect", Capability: capInspect, Danger: DangerSafe}
	deploy := Action{Key: "D", Desc: "deploy", Capability: capDeploy, Danger: DangerMutation}
	remove := Action{Key: "ctrl-d", Desc: "remove", Capability: capRemove, Danger: DangerDestructive}
	app.SetReadOnly(true)

	hints := app.ActionHints(inspect, deploy, remove)
	if len(hints) != 1 || hints[0].Desc != "inspect" {
		t.Fatalf("read-only hints = %#v", hints)
	}
	if !app.RequireAction(inspect) {
		t.Fatal("read-only policy rejected safe inspection")
	}
	if app.RequireAction(deploy) || app.RequireAction(remove) {
		t.Fatal("read-only policy allowed a cluster mutation")
	}
}

// The hole the Action type exists to close: an action that is not advertised
// must also not be executable. Hiding a binding while leaving its handler live
// is worse than showing it, because the tool then looks safe and is not.
func TestAdvertisedAndPermittedNeverDisagree(t *testing.T) {
	actions := []Action{
		{Key: "enter", Desc: "inspect", Capability: capInspect, Danger: DangerSafe},
		{Key: "D", Desc: "deploy", Capability: capDeploy, Danger: DangerMutation},
		{Key: "ctrl-d", Desc: "remove", Capability: capRemove, Danger: DangerDestructive},
	}

	for _, readOnly := range []bool{false, true} {
		app := testApp()
		app.SetReadOnly(readOnly)
		hints := app.ActionHints(actions...)

		for _, action := range actions {
			offered := false
			for _, h := range hints {
				if h.Key == action.Key {
					offered = true
					break
				}
			}
			allowed := app.RequireAction(action)
			if offered != allowed {
				t.Fatalf("readOnly=%v %q offered=%v allowed=%v — hints and enforcement disagree",
					readOnly, action.Key, offered, allowed)
			}
		}
	}
}

// The shell must not carry a logo. An empty Art is legal and collapses the
// columns rather than reserving space for art that does not exist.
func TestEmptyArtIsLegal(t *testing.T) {
	app := NewApp(DefaultTheme(), Art{})
	if app == nil {
		t.Fatal("a tool without artwork must still get an app")
	}
	if w, h := artSize(""); w != 0 || h != 0 {
		t.Fatalf("empty art measured %dx%d, want 0x0", w, h)
	}
}

// Header widths come from measuring the art, so a tool can rebrand without the
// shell knowing anything about it.
func TestArtIsMeasuredNotAssumed(t *testing.T) {
	narrow := Art{Mascot: "ab", Logotype: "x"}
	wide := Art{Mascot: "abcdefgh", Logotype: "xxxxxxxx"}

	nw, _ := artSize(narrow.Mascot)
	ww, _ := artSize(wide.Mascot)
	if ww <= nw {
		t.Fatalf("wider mascot measured %d, narrow %d", ww, nw)
	}

	// Height drives the header height, so a taller mascot means a taller header.
	_, h1 := artSize("one")
	_, h3 := artSize("one\ntwo\nthree")
	if h1 != 1 || h3 != 3 {
		t.Fatalf("art heights = %d, %d; want 1, 3", h1, h3)
	}
}
