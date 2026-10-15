// SPDX-License-Identifier: Apache-2.0

package nines

import "os"

// The TUI owns the terminal's alternate screen; anything that writes to
// stdout/stderr while it runs — Zarf's progress spinners during a package
// create, syft's SBOM output, a stray fmt.Print — paints over the interface
// and wrecks it. While the TUI runs, both are redirected into a drain, and the
// real handles are kept here for the few places that legitimately need the
// terminal (suspending into $EDITOR).
var (
	realStdout = os.Stdout
	realStderr = os.Stderr
)

// SetRealStdio records the true terminal handles before redirection.
func SetRealStdio(out, err *os.File) {
	realStdout, realStderr = out, err
}

// RealStdout is the terminal's actual stdout, regardless of redirection.
func RealStdout() *os.File { return realStdout }

// RealStderr is the terminal's actual stderr, regardless of redirection.
func RealStderr() *os.File { return realStderr }
