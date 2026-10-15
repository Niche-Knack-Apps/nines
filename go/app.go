// SPDX-License-Identifier: Apache-2.0

package nines

import (
	"fmt"
	"sort"
	"strings"

	"github.com/gdamore/tcell/v2"
	"github.com/mattn/go-runewidth"
	"github.com/rivo/tview"
)

// Screen is a view that can be pushed onto the page stack.
type Screen interface {
	tview.Primitive
	// Name is the breadcrumb label.
	Name() string
	// Hints are the context-specific key bindings shown in the status bar.
	Hints() []Hint
}

// Hint is one key binding advertised to the user.
type Hint struct {
	Key, Desc string
}

// Art is the header artwork a tool supplies. The shell never owns a logo — it
// only measures one, so a tool can brand itself without the shell knowing
// anything about it.
//
// Both fields may be empty; the columns then collapse to nothing and the rest of
// the header takes the space.
type Art struct {
	// Mascot sits at the far right. Its height sets the header height.
	Mascot string
	// Logotype is the block wordmark to the mascot's left.
	Logotype string
}

// Capability identifies the authority an action needs.
type Capability string

// Capabilities themselves are declared by the consuming tool: they name that
// tool's verbs, not the shell's. z9s has deploy/remove/init; a ledger client has
// write/reconcile. The shell only ever compares them.

// Danger classifies action impact independently of its key binding.
type Danger string

const (
	DangerSafe        Danger = "safe"
	DangerMutation    Danger = "mutation"
	DangerDestructive Danger = "destructive"
)

// Action is a classified UI operation. Hints and enforcement consume the same
// object so a hidden read-only action cannot remain executable by key.
type Action struct {
	Key        string
	Desc       string
	Capability Capability
	Danger     Danger
}

// Command is a ':' command.
type Command struct {
	// Aliases are all the names that invoke this command; the first is canonical.
	Aliases []string
	Desc    string
	Run     func(args string) error
}

// infoField is one row of the header's info panel.
type infoField struct {
	val   string
	color tcell.Color
}

// App is the z9s application shell: header, page stack, and command bar.
type App struct {
	*tview.Application

	theme     Theme
	art       Art
	pages     *tview.Pages
	layout    *tview.Flex
	top       *tview.Flex
	topHeight int
	header    *tview.TextView
	logotype  *tview.TextView
	crumbs    *tview.TextView
	legend    *tview.TextView
	info      *tview.TextView
	status    *tview.TextView
	prompt    *PromptField
	readOnly  bool
	infoKeys  []string
	infoVals  map[string]infoField

	stack    []Screen
	commands []Command
	byAlias  map[string]*Command

	// promptMode is "" when hidden, ":" for commands, "/" for filter.
	promptMode string
	// recent tracks command usage so completion promotes what you actually use.
	recent map[string]int
	// afterStop runs once when the app stops, for task-runner teardown.
	afterStop func()

	// headerHidden collapses the top panel to give small terminals room.
	headerHidden bool
	// focusSink holds focus while a form is "parked" (Enter deselected a
	// field); parkedForm/parkedIndex remember where to resume.
	focusSink   *tview.Box
	parkedForm  *tview.Form
	parkedIndex int
}

// NewApp builds the shell. Screens are pushed separately.
func NewApp(theme Theme, art Art) *App {
	// tview widgets without explicit colours read the package-level palette,
	// not ours; point it at the theme before any of them are built.
	ApplyGlobalStyles(theme)

	a := &App{
		Application: tview.NewApplication(),
		theme:       theme,
		art:         art,
		pages:       tview.NewPages(),
		byAlias:     map[string]*Command{},
		recent:      map[string]int{},
		infoVals:    map[string]infoField{},
	}

	// The viewing area gets a clean window: a box with a live title naming the
	// view, its active filter, and its row count.
	a.pages.SetBorder(true)
	a.pages.SetBorderColor(theme.Dim)
	a.pages.SetTitleColor(theme.Accent)

	a.header = tview.NewTextView().SetDynamicColors(true)
	// The logo carries its own colour tags (the icon's palette); no accent wrap.
	a.header.SetText(a.art.Mascot)
	a.logotype = tview.NewTextView().SetDynamicColors(true)
	a.logotype.SetText(fmt.Sprintf("[%s]%s", colorName(theme.Accent), a.art.Logotype))

	a.focusSink = tview.NewBox()
	a.crumbs = tview.NewTextView().SetDynamicColors(true)
	a.legend = tview.NewTextView().SetDynamicColors(true)
	a.info = tview.NewTextView().SetDynamicColors(true)
	a.status = tview.NewTextView().SetDynamicColors(true)

	a.prompt = NewPromptField(theme, ":")
	a.prompt.SetSuggestFunc(func(input string) []string {
		if a.promptMode != ":" {
			return nil
		}
		// ":cd /some/pa" completes the path argument zsh-style; bare input
		// completes command names.
		if cmd, arg, ok := strings.Cut(input, " "); ok {
			switch cmd {
			case "cd", "fs", "dir", "new", "scaffold":
				out := make([]string, 0, 8)
				for _, p := range PathCandidates(arg) {
					out = append(out, cmd+" "+p)
				}
				return out
			}
			return nil
		}
		return RankCandidates(input, a.aliasNames(), a.recent)
	})
	a.prompt.SetSubmitFunc(func(raw string) {
		text := strings.TrimSpace(raw)
		mode := a.promptMode
		a.hidePrompt()
		switch mode {
		case ":":
			if text != "" {
				a.dispatch(text)
			}
		case "/":
			a.applyFilter(text)
		}
	})
	a.prompt.SetChangeFunc(func(text string) {
		// '/' filters live as the user types; ':' stays submit-on-enter.
		if a.promptMode == "/" {
			a.applyFilterLive(strings.TrimSpace(text))
		}
	})
	a.prompt.SetCancelFunc(func() {
		mode := a.promptMode
		a.hidePrompt()
		if mode == "/" {
			a.applyFilter("") // ESC clears an active filter
		}
	})

	// Middle of the header: breadcrumbs over a persistent key legend. The legend
	// is the always-visible reference for the current view's bindings; the bottom
	// status line still carries the same hints but is overwritten by flash
	// messages, so it cannot be relied on as a reference.
	mid := tview.NewFlex().SetDirection(tview.FlexRow).
		AddItem(a.crumbs, 1, 0, false).
		AddItem(a.legend, 0, 1, false)

	// k9s header order: info panel left, key legend middle, then the block
	// logotype with the mascot floating at the far right. All parts size
	// themselves from the art so redrawing a logo never touches layout numbers.
	logoW, logoH := artSize(a.art.Mascot)
	typeW, _ := artSize(a.art.Logotype)

	a.top = tview.NewFlex().SetDirection(tview.FlexColumn).
		AddItem(a.info, 0, 2, false).
		AddItem(mid, 0, 3, false).
		AddItem(a.logotype, typeW+2, 0, false).
		AddItem(a.header, logoW+1, 0, false)
	a.topHeight = logoH

	a.layout = tview.NewFlex().SetDirection(tview.FlexRow)
	a.rebuildLayout(false)

	a.SetRoot(a.layout, true).EnableMouse(false)
	a.SetInputCapture(a.globalKeys)
	return a
}

// Theme exposes the palette to views.
func (a *App) Theme() Theme { return a.theme }

// SetReadOnly controls whether mutating and destructive actions are advertised
// or executable. Safe inspection, filtering, and local draft editing remain
// available.
func (a *App) SetReadOnly(readOnly bool) {
	a.readOnly = readOnly
	a.renderInfo()
	a.showHints()
}

func (a *App) ReadOnly() bool { return a.readOnly }

// RefreshHints redraws context actions after a row selection changes.
func (a *App) RefreshHints() { a.showHints() }

// ActionHints returns only actions allowed by the current policy.
func (a *App) ActionHints(actions ...Action) []Hint {
	hints := make([]Hint, 0, len(actions))
	for _, action := range actions {
		if a.Allows(action) {
			hints = append(hints, Hint{Key: action.Key, Desc: action.Desc})
		}
	}
	return hints
}

// Allows applies the current capability/danger policy.
func (a *App) Allows(action Action) bool {
	return !a.readOnly || action.Danger == DangerSafe
}

// RequireAction enforces an action at its handler boundary and explains policy
// refusal in the status line.
func (a *App) RequireAction(action Action) bool {
	if a.Allows(action) {
		return true
	}
	a.Flash(fmt.Sprintf("read-only mode disables %s (%s)", action.Desc, action.Capability), a.theme.Warn)
	return false
}

// AddCommand registers a ':' command.
func (a *App) AddCommand(c Command) {
	a.commands = append(a.commands, c)
	cp := &a.commands[len(a.commands)-1]
	for _, al := range c.Aliases {
		a.byAlias[al] = cp
	}
}

// Commands returns registered commands, for the help screen.
func (a *App) Commands() []Command { return a.commands }

func (a *App) aliasNames() []string {
	out := make([]string, 0, len(a.byAlias))
	for k := range a.byAlias {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

// Dispatch runs a ':' command line programmatically.
func (a *App) Dispatch(line string) { a.dispatch(line) }

// dispatch runs a ':' command line.
func (a *App) dispatch(line string) {
	name, args, _ := strings.Cut(line, " ")
	cmd, ok := a.byAlias[name]
	if !ok {
		a.Flash(fmt.Sprintf("unknown command %q — ctrl-a for aliases", name), a.theme.Error)
		return
	}
	a.recent[name]++
	if err := cmd.Run(strings.TrimSpace(args)); err != nil {
		a.Flash(err.Error(), a.theme.Error)
	}
}

// Push adds a screen to the stack and shows it.
func (a *App) Push(s Screen) {
	a.stack = append(a.stack, s)
	name := fmt.Sprintf("page-%d", len(a.stack))
	a.pages.AddPage(name, s, true, true)
	a.refreshChrome()
	a.SetFocus(s)
}

// Pop removes the top screen. The last screen is never popped, so ESC at the
// root does nothing rather than emptying the display. A popped screen that
// holds resources (a registry tunnel, say) is closed.
func (a *App) Pop() {
	a.clearParkedForm()
	if len(a.stack) <= 1 {
		return
	}
	if c, ok := a.Top().(interface{ Close() }); ok {
		c.Close()
	}
	a.pages.RemovePage(fmt.Sprintf("page-%d", len(a.stack)))
	a.stack = a.stack[:len(a.stack)-1]
	a.refreshChrome()
	a.SetFocus(a.stack[len(a.stack)-1])
}

// Replace swaps the whole stack for a single screen, closing any screens that
// hold resources.
func (a *App) Replace(s Screen) {
	for i := range a.stack {
		if c, ok := a.stack[i].(interface{ Close() }); ok {
			c.Close()
		}
		a.pages.RemovePage(fmt.Sprintf("page-%d", i+1))
	}
	a.stack = nil
	a.Push(s)
}

// Top returns the current screen, or nil.
func (a *App) Top() Screen {
	if len(a.stack) == 0 {
		return nil
	}
	return a.stack[len(a.stack)-1]
}

// Helper lets a screen supply context help: '?' anywhere shows what the
// current screen is and does, and esc returns.
type Helper interface {
	HelpText() string
}

// Titled lets a screen drive the viewing-area window title with its state:
// what is showing, the active filter, and the row count — "packages(/har)[3]".
type Titled interface {
	TitleInfo() string
}

func (a *App) refreshChrome() {
	var names []string
	for _, s := range a.stack {
		names = append(names, s.Name())
	}
	a.crumbs.SetText(fmt.Sprintf("[%s]%s", colorName(a.theme.Accent), strings.Join(names, " › ")))

	title := ""
	if top := a.Top(); top != nil {
		if t, ok := top.(Titled); ok {
			title = t.TitleInfo()
		} else {
			title = top.Name()
		}
	}
	if title != "" {
		a.pages.SetTitle(fmt.Sprintf(" %s ", title))
	}
	a.showHints()
}

func (a *App) showHints() {
	hints := []Hint{{":", "cmd"}, {"/", "filter"}, {"?", "help"}, {"esc", "back"}}
	if top := a.Top(); top != nil {
		hints = append(top.Hints(), hints...)
	}
	// The legend always reflects the current view, even while the prompt is open
	// or a flash message owns the status line.
	a.renderLegend(hints)

	if a.promptMode != "" {
		return
	}
	var parts []string
	if a.readOnly {
		parts = append(parts, fmt.Sprintf("[%s]READ ONLY", colorName(a.theme.Warn)))
	}
	for _, h := range hints {
		parts = append(parts, fmt.Sprintf("[%s]<%s>[%s] %s",
			colorName(a.theme.Accent), h.Key, colorName(a.theme.Dim), h.Desc))
	}
	a.status.SetText(strings.Join(parts, "  "))
}

// renderLegend lays hints out k9s-style: a compact column-major grid in the
// header, three rows tall, so every binding for the current view is visible at
// a glance without opening help.
func (a *App) renderLegend(hints []Hint) {
	if len(hints) == 0 {
		a.legend.SetText("")
		return
	}
	// Use the vertical room the logo grants: one hint per line reads like a
	// k9s menu, spilling into a second column only when the list outgrows it.
	_, logoH := artSize(a.art.Mascot)
	rows := logoH - 1 // minus the breadcrumb line
	if rows < 3 {
		rows = 3
	}
	if len(hints) < rows {
		rows = len(hints)
	}
	cols := (len(hints) + rows - 1) / rows

	// Column widths from the widest cell in each column, so columns align.
	widths := make([]int, cols)
	for i, h := range hints {
		if w := len(h.Key) + len(h.Desc) + 3; w > widths[i/rows] {
			widths[i/rows] = w
		}
	}

	// Filling lines in index order lands each row's cells in increasing column
	// order, because consecutive indices walk down one column before moving on.
	lines := make([]string, rows)
	for i, h := range hints {
		col := i / rows
		pad := widths[col] - (len(h.Key) + len(h.Desc) + 3) + 2
		lines[i%rows] += fmt.Sprintf("[%s]<%s>[%s] %s%s",
			colorName(a.theme.Accent), h.Key, colorName(a.theme.Dim), h.Desc,
			strings.Repeat(" ", pad))
	}
	a.legend.SetText(strings.Join(lines, "\n"))
}

// RefreshChrome re-renders breadcrumbs and the legend. Views whose Name changes
// with state (a selection count, say) call this after mutating.
func (a *App) RefreshChrome() { a.refreshChrome() }

// SetInfoField upserts one row of the header info panel (Folder, Package,
// Context, Zarf, ...). Rows render in first-set order, k9s-style.
func (a *App) SetInfoField(key, val string, color tcell.Color) {
	if _, ok := a.infoVals[key]; !ok {
		a.infoKeys = append(a.infoKeys, key)
	}
	a.infoVals[key] = infoField{val: val, color: color}
	a.renderInfo()
}

func (a *App) renderInfo() {
	width := 0
	for _, k := range a.infoKeys {
		if len(k) > width {
			width = len(k)
		}
	}
	var b strings.Builder
	for _, k := range a.infoKeys {
		f := a.infoVals[k]
		val := f.val
		color := f.color
		if k == "Context" {
			// k9s marks write authority on the context line.
			if a.readOnly {
				val += " [RO]"
				color = a.theme.Warn
			} else {
				val += " [RW]"
			}
		}
		// One line per field: long values (paths, error text) truncate rather
		// than wrap and shove every row below them out of alignment.
		if len(val) > 48 {
			val = val[:23] + "…" + val[len(val)-24:]
		}
		fmt.Fprintf(&b, " [%s]%-*s[%s] %s\n",
			colorName(a.theme.Accent), width+1, k+":", colorName(color), val)
	}
	a.info.SetText(b.String())
}

// ResetCommands clears registered ':' commands so the main UI can be rebuilt
// against a new catalog root.
func (a *App) ResetCommands() {
	a.commands = nil
	a.byAlias = map[string]*Command{}
}

// TeardownMain runs and clears any registered cleanup without stopping the
// application — used when rebuilding the main UI in place.
func (a *App) TeardownMain() {
	if a.afterStop != nil {
		a.afterStop()
		a.afterStop = nil
	}
}

// SetAfterStop registers cleanup run when the application stops.
func (a *App) SetAfterStop(fn func()) { a.afterStop = fn }

// Stop shuts the application down, running any registered cleanup first.
func (a *App) Stop() {
	if a.afterStop != nil {
		a.afterStop()
		a.afterStop = nil
	}
	a.Application.Stop()
}

// ShowSplash replaces the whole screen with the given primitive; ShowMain
// restores the normal chrome. Used for the startup splash while the first
// catalog scan runs.
func (a *App) ShowSplash(p tview.Primitive) {
	a.SetRoot(p, true)
}

// ShowMain swaps from the splash to the standard layout.
func (a *App) ShowMain() {
	a.SetRoot(a.layout, true)
	if top := a.Top(); top != nil {
		a.SetFocus(top)
	}
}

// Flash shows a transient message in the status bar.
func (a *App) Flash(msg string, color tcell.Color) {
	a.status.SetText(fmt.Sprintf("[%s]%s", colorName(color), msg))
}

// promptLabel is the command-bar mark: a tiny axolotl in the mascot palette
// (gills–body–gills) in place of k9s's dog.
func (a *App) promptLabel(mode string) string {
	return fmt.Sprintf("[#9a7fd0]}[#4aa5dc]o[#9a7fd0]{[-] %s ", mode)
}

// showPrompt drops the command bar in between the header and the viewing
// window, k9s-style, and removes it again on esc/enter.
func (a *App) showPrompt(mode string) {
	a.promptMode = mode
	a.prompt.SetLabel(a.promptLabel(mode)).SetText("")
	a.rebuildLayout(true)
	a.SetFocus(a.prompt)
}

func (a *App) hidePrompt() {
	if a.promptMode == "" {
		return
	}
	a.promptMode = ""
	a.rebuildLayout(false)
	a.showHints()
	if top := a.Top(); top != nil {
		a.SetFocus(top)
	}
}

// rebuildLayout lays the root out with or without the drop-down command bar.
func (a *App) rebuildLayout(withPrompt bool) {
	a.layout.Clear()
	if !a.headerHidden {
		a.layout.AddItem(a.top, a.topHeight, 0, false)
	}
	if withPrompt {
		a.layout.AddItem(a.prompt, 1, 0, true)
	}
	a.layout.AddItem(a.pages, 0, 1, !withPrompt)
	a.layout.AddItem(a.status, 1, 0, false)
	a.layout.AddItem(a.focusSink, 0, 0, false)
}

// Filterable is implemented by screens that support '/' filtering.
type Filterable interface {
	SetFilter(string)
	// Filter reports the active filter, so esc can clear it before popping.
	Filter() string
}

// applyFilterLive updates the filter without status-line noise; the window
// title reflects the active filter, so per-keystroke flashes would only spam.
func (a *App) applyFilterLive(q string) {
	if f, ok := a.Top().(Filterable); ok {
		f.SetFilter(q)
	}
}

// applyFilter forwards a '/' query to the current screen if it supports one.
func (a *App) applyFilter(q string) {
	f, ok := a.Top().(Filterable)
	if !ok {
		if q != "" {
			a.Flash("this view cannot be filtered", a.theme.Dim)
		}
		return
	}
	f.SetFilter(q)
	if q != "" {
		a.Flash("filter: "+q+"  (esc clears)", a.theme.Accent)
	}
}

// globalKeys handles bindings that apply everywhere. Keys are only intercepted
// when the prompt is closed, so typing ':' inside the prompt is literal.
func (a *App) globalKeys(event *tcell.EventKey) *tcell.EventKey {
	if a.promptMode != "" {
		return event
	}

	switch event.Key() {
	case tcell.KeyTab, tcell.KeyBacktab, tcell.KeyDown, tcell.KeyUp, tcell.KeyEnter:
		if a.resumeParkedForm() {
			return nil
		}
	}

	switch event.Key() {
	case tcell.KeyCtrlE:
		// k9s's header toggle: trade chrome for rows on small terminals. In a
		// text field ctrl-e keeps its editing meaning (end of line).
		if !a.textFocused() {
			a.ToggleHeader()
			return nil
		}
	case tcell.KeyEsc:
		a.clearParkedForm()
		// esc peels state in order: an active filter first, then the view.
		if f, ok := a.Top().(Filterable); ok && f.Filter() != "" {
			f.SetFilter("")
			a.Flash("filter cleared", a.theme.Dim)
			return nil
		}
		a.Pop()
		return nil
	case tcell.KeyCtrlC:
		a.Stop()
		return nil
	}

	// While a text input has focus, ':' and '/' are ordinary characters —
	// paths and oci:// references must type cleanly. '?' stays global help by
	// design. In list views the shortcuts behave as before.
	if !a.textFocused() {
		switch event.Rune() {
		case ':':
			a.showPrompt(":")
			return nil
		case '/':
			a.showPrompt("/")
			return nil
		}
	}
	if event.Rune() == '?' {
		a.dispatch("help")
		return nil
	}
	return event
}

// ToggleHeader hides or restores the top panel, trading chrome for rows on
// small terminals.
func (a *App) ToggleHeader() {
	a.headerHidden = !a.headerHidden
	a.rebuildLayout(a.promptMode != "")
	if a.headerHidden {
		a.Flash("header hidden — ctrl-e brings it back", a.theme.Dim)
	} else {
		a.showHints()
	}
}

// textFocused reports whether the focused primitive consumes typed runes.
func (a *App) textFocused() bool {
	switch a.GetFocus().(type) {
	case *PromptField, *tview.InputField, *tview.TextArea:
		return true
	}
	return false
}

// ArtSize measures art in terminal cells, ignoring colour markup.
func ArtSize(art string) (w, h int) { return artSize(art) }

// artSize measures art in terminal cells, ignoring colour markup.
//
// The semantics are fixed by CONTRACT.md so the Rust implementation measures the
// same art identically — shared artwork that lays out differently in each
// implementation is exactly the drift the contract exists to prevent. Three
// details matter, and the original of this function got all three wrong because
// z9s's own art never exercised them:
//
//   - Empty art is 0x0, not 0x1. strings.Split("", "\n") returns one empty
//     element, which would reserve a header row for artwork that is not there.
//   - A trailing newline terminates a line rather than starting a new one, so
//     art written as a heredoc does not gain a blank row.
//   - Width is display cells, not bytes. A CJK glyph occupies two columns and
//     len() counts its three UTF-8 bytes, so a logo would overflow its column.
//   - Any [...] tag is markup, not just [#rrggbb]; a literal bracket is escaped
//     [[] and that escape swallows its closing bracket.
func artSize(art string) (w, h int) {
	if art == "" {
		return 0, 0
	}
	// A trailing newline terminates the last line rather than starting an
	// empty one. strings.Split would report an extra row and the header would
	// carry a blank line under the art.
	for _, line := range strings.Split(strings.TrimSuffix(art, "\n"), "\n") {
		h++
		if n := runewidth.StringWidth(stripMarkup(line)); n > w {
			w = n
		}
	}
	return w, h
}

// stripMarkup removes tview colour tags so they do not count as visible cells.
func stripMarkup(line string) string {
	var out strings.Builder
	runes := []rune(line)
	for i := 0; i < len(runes); i++ {
		if runes[i] != '[' {
			out.WriteRune(runes[i])
			continue
		}
		if i+1 < len(runes) && runes[i+1] == '[' {
			// "[[]" is one escape for a literal '['; consume its closing bracket
			// too, or the ']' is counted as a visible cell.
			out.WriteRune('[')
			i++
			if i+1 < len(runes) && runes[i+1] == ']' {
				i++
			}
			continue
		}
		// Skip to the closing bracket. An unterminated tag runs to end of line,
		// which is what a renderer would do with it anyway.
		for i++; i < len(runes) && runes[i] != ']'; i++ {
		}
	}
	return out.String()
}

// colorName renders a tcell colour for tview's dynamic-colour markup.
func colorName(c tcell.Color) string {
	if c == tcell.ColorDefault {
		return "-"
	}
	return fmt.Sprintf("#%06x", c.Hex())
}
