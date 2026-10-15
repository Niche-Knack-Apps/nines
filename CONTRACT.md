# The 9s UI Contract

A k9s-shaped terminal shell, specified once so a Go implementation and a Rust
implementation are indistinguishable on screen.

This document is normative. `go/` and `rust/` are implementations of it. If they
disagree with this file, they are wrong.

## Why this exists

`z9s` proved the shape works, and everything here is drawn from it rather than
invented. But the shell was trapped inside one Go binary, so the next tool had a
choice between copying it by hand and diverging, or picking the language for the
shell rather than for the problem.

Splitting the *specification* from the *implementation* means a tool can be
written in whichever language suits its domain — `z9s` links Zarf, `e9s` shares a
Rust ledger engine — and still present the same interface. A user who learns one
has learned both.

---

## 1. Layout

Rows, top to bottom. Only `body` flexes; everything else is fixed height.

```
┌────────────────────────────────────────────────────────────────────┐
│ header            (height = logo height; hidden by ctrl-e)         │
├────────────────────────────────────────────────────────────────────┤
│ command bar       (1 row; present ONLY while ':' or '/' is open)   │
├────────────────────────────────────────────────────────────────────┤
│ body              (flex — the current screen)                      │
├────────────────────────────────────────────────────────────────────┤
│ status / flash    (1 row, always)                                  │
└────────────────────────────────────────────────────────────────────┘
```

**The command bar is inserted into the layout, not overlaid.** It pushes the body
down by one row. An overlay would cover content the user is mid-way through
reading, and it makes focus ambiguous.

### 1.1 Header

Four columns:

| Column | Width | Contents |
|---|---|---|
| info | flex 2 | ordered `key: value` rows |
| middle | flex 3 | breadcrumbs (1 row) over the legend (rest) |
| logotype | fixed, `width(logotype) + 2` | block wordmark |
| mascot | fixed, `width(mascot) + 1` | ASCII art |

Header height is the **mascot's** height.

**Every fixed width is measured from the art itself, never hard-coded.** Changing
a logo must never require touching a layout constant. Measurement ignores colour
markup, so `[#ff0000]X[-]` is one cell wide.

### 1.2 The legend is not the status line

Both show the current screen's bindings. They are separate because **flash
messages overwrite the status line**, and a reference the user can lose mid-task
is not a reference.

- **Legend** — persistent grid in the header, column-major, sized from the header
  height. Always shows the current screen's bindings.
- **Status line** — the same hints *until* something flashes, then the message.

### 1.3 Measuring art

Both implementations must measure the same art identically, or shared artwork
lays out differently in each and the contract is worthless. Every rule below was
a real divergence found by the conformance table, not a hypothetical:

| Input | Size | Why |
|---|---|---|
| `""` | 0 × 0 | Empty art reserves nothing. Splitting an empty string on newline yields one empty element in most languages, which would reserve a header row for art that is not there. |
| `"abc"` | 3 × 1 | |
| `"ab\nabcd"` | 4 × 2 | Width is the widest line. |
| `"a\n"` | 1 × 1 | **A trailing newline terminates a line, it does not start one.** Art written as a heredoc ends with one, and counting it adds a blank row under every logo. |
| `"a\n\n"` | 1 × 2 | A genuinely blank line is a line. |
| `"漢字"` | 4 × 1 | **Width is display cells, not bytes and not chars.** A CJK glyph is three bytes, one char, and two columns. |
| `"[#ff0000]X[-]"` | 1 × 1 | Colour markup is invisible. |
| `"[red]ab[-]c"` | 3 × 1 | **Any** `[...]` is markup, not only `[#rrggbb]`. |
| `"[[]"` | 1 × 1 | `[[]` is one escape for a literal `[`, and it swallows its closing bracket. |
| `"ab[unclosed"` | 2 × 1 | An unterminated tag runs to end of line. |

Implementations keep this exact table as a test. Changing a row without changing
it in both is a contract break.

### 1.4 Info panel

Ordered `key: value` rows. Setting a key that already exists updates it in place
and keeps its original position; a new key appends. Order is set-order, never
sorted — layout must not shuffle when a value arrives late.

The read-only indicator is a suffix on the context row: `[RO]` or `[RW]`.

---

## 2. Screens

A screen is the unit of navigation. The required surface is deliberately tiny:

```
Screen:
    render(area)          draw yourself
    name() -> String      breadcrumb label
    hints() -> [Hint]     bindings valid right now
```

Everything else is an **optional capability**, discovered at runtime by asking
whether a screen implements it. Not a base class, not fields on a struct that
half the screens ignore.

| Capability | Surface | Effect |
|---|---|---|
| `Filterable` | `set_filter(s)`, `filter() -> String` | `/` opens the filter; esc clears it before popping |
| `Titled` | `title_info() -> String` | contributes to the window title |
| `Helper` | `help_text() -> String` | `?` shows help for *this* screen |
| `Closable` | `close()` | called when popped, for screens holding resources |

A screen that implements none of these still works.

### 2.1 The stack

Push, pop, replace, top.

- **The last screen is never popped.** Esc on the root is a no-op, not an exit.
  Quitting is explicit.
- **Popping a `Closable` screen closes it.** Tunnels, watchers and handles are
  released at the moment the user navigates away, not at process exit.
- Focus follows the new top.

### 2.2 Refresh must be split from apply

```
refresh()       start work; paint a loading state; return immediately
apply(result)   take a finished result and update state
```

Two reasons, and the second is the one that matters:

1. The UI never blocks on I/O.
2. **`apply` is callable directly, so screens are testable without a running
   app, an event loop, or a scheduler.** Hand it a result, assert on the rendered
   cells. This is the single highest-leverage rule in the contract; a shell whose
   screens can only be tested by driving a live terminal will not be tested.

A refresh already in flight is dropped, not queued — re-pressing `r` must not
stack requests.

---

## 3. Actions and authority

An action is declared **once** and consumed by both the hint renderer and the
handler guard:

```
Action { key, desc, capability, danger }

danger: safe | mutation | destructive
```

This is the whole point. Filtering an action out of the hints while leaving its
key live is a hole — the binding still fires for anyone who knows it. Both paths
consult the same object, so an action that is not offered cannot be invoked.

Guards are checked at the handler boundary and **explain the refusal**. "Not
permitted in read-only mode" beats a keypress that silently does nothing.

### 3.1 Confirmation

An action whose danger is `mutation` or `destructive` **and** whose effect
reaches beyond the selected row must be confirmed before it runs.

The confirmation is not "are you sure". It states, in the user's terms, what is
about to happen — how many records, in how many files — because the phrasing *is*
the safety feature. A dialog that only repeats the verb has told the user nothing
they did not already know when they pressed the key.

| Rule | Why |
|---|---|
| One fact per line, listed | A paragraph gets skimmed; a list gets read |
| `y` accepts; `n`, `q`, `esc` decline | The affirmative key is the answer |
| **Enter does not accept** | Enter is what people press to dismiss things |
| Any other key is ignored, not a decline | Otherwise the next keystroke lands on whatever was behind |
| The screen behind stays visible around it | The decision keeps its context |

While it is open the confirmation owns every key, exactly as a form does (§4.1).

---

## 4. Keys

### 4.1 Ownership — the rule that matters most

> Every key has exactly one owner, decided by focus type, enforced in one place.

This is the hard-won half of the contract. In z9s, key handling grew into three
uncoordinated layers — app-global runes, an autocomplete dropdown owning Enter
and the arrows, and ad-hoc per-field captures — each patch fighting the last.
The failure mode was not any single bug: it was the absence of an ownership rule.

| Key | List view | Text field | Command bar | Confirmation |
|---|---|---|---|---|
| printable runes | view shortcuts | **the field, always** (incl. `:` `/`) | the input | `y`/`n` only; the rest ignored |
| `?` | help | help (by design) | literal `?` | ignored |
| tab / backtab | — | move between fields, nothing else | accept completion | ignored |
| ↓ / ↑ | move rows | move between fields — **never blocked** | completion list | ignored |
| enter | open row | commit + release focus | submit | **ignored** (§3.1) |
| esc | clear filter, then pop | leave the form | close the bar | decline |

Rules:

- Field key handling lives in **exactly two places**: the field constructor and
  the form decorator. Never attach a capture to an individual field.
- **Inline ghost-text completion only.** A dropdown that owns Enter and the
  arrows traps the cursor in a field the user cannot leave.
- **Navigation keys never depend on completion state, dropdown state, or any
  other mode.** Arrows always move.
- Global `:` and `/` are suppressed whenever the focused widget consumes text, so
  a path or a `scheme://` reference can be typed literally. `?` stays global.

A deliberate deviation is allowed where a widget is genuinely a command line
rather than a form field — enter applies, esc reverts to the last good value —
but it must be documented as one.

### 4.2 Global bindings

| Key | Action |
|---|---|
| `:` | command bar |
| `/` | filter |
| `?` | help for the current screen |
| `ctrl-e` | toggle the header (trade chrome for rows) |
| `esc` | clear filter, else pop |
| `ctrl-c` | quit |

### 4.3 List motions

`j`/`k` or ↓/↑ move. `gg` top, `G` bottom, `^` first column, `$` last column.
The `gg` chord window is **600 ms**; a lone `g` inside it is not an error.

Vi keys are first-class and arrows are aliases, not the reverse.

### 4.4 Period navigation (tools with a time axis)

Borrowed from `hledger-ui`, which has no k9s equivalent and is the best idea in
its problem space:

| Key | Action |
|---|---|
| `shift-←` / `shift-→` | previous / next period |
| `shift-↑` / `shift-↓` | widen / narrow granularity |
| `T` | jump to today |

Left and right travel; up and down zoom.

---

## 5. Command bar

`:` opens it. The first word selects a command, the rest is its argument.

- Commands declare **aliases**; the first is canonical. `:packages`, `:pkg`, `:p`.
- Completion is tiered: exact prefix, then case-insensitive prefix, then
  subsequence (`dpl` reaches `deploy`), with recent usage breaking ties.
- **An unknown command flashes an error and leaves the bar open.** It must not
  clear what the user typed.

---

## 6. Redraw

Work arriving faster than the terminal can paint must collapse, without dropping
the final state:

- Leading edge fires immediately.
- Further updates during a paint set a dirty flag rather than queueing.
- One trailing repaint runs if anything arrived while painting.

Never block the producer on the renderer. A tool that stutters while streaming
logs has this wrong.

---

## 7. Configuration

Three directories, distinct on purpose:

| Kind | Location | Mode |
|---|---|---|
| config | `$XDG_CONFIG_HOME/<app>/config.yaml` | `0644` |
| cache | `$XDG_CACHE_HOME/<app>/` | `0600` |
| state | `$XDG_STATE_HOME/<app>/` | `0600` |

Cache is disposable; state is not; config is the user's. An `<APP>_CONFIG_DIR`
environment variable overrides the config location, mirroring `K9S_CONFIG_DIR`.

- **Load is total.** A missing *or unreadable* config yields defaults, never an
  error. First run and corrupt-file recovery take the same path.
- **Saves are atomic** — temp file, then rename.
- **Pick-once writes.** When the user makes a choice the tool would otherwise ask
  about again, persist it there and then and say so: `remembered in <path>`.
  Most tools do not need a settings screen.
- The in-app help must document every config key, and a new key is not finished
  until it is described there.

---

## 8. Colour and legibility

- **Never rely on reverse video for selection.** With default foreground and
  background it can resolve to invisible text. Selection sets both colours
  explicitly, contrast ≥ 4.5:1.
- **Never encode meaning in colour alone.** Colour accompanies a glyph, a sign or
  a word — never replaces one. Roughly 8% of men cannot separate red from green,
  and red/green is the worst available pair for exactly the states tools most
  like to colour.
- **Semantic colours are fixed RGB, not ANSI indices.** This inverts an earlier
  rule and the reason matters: preferring the 16 ANSI colours let the user's
  terminal theme tint the app, but an ANSI colour's actual RGB is whatever that
  theme says it is, so no ratio can be promised against it. ANSI cyan and yellow
  measure **1.25:1** and **1.07:1** on a white terminal — effectively invisible.
  A contrast guarantee and a borrowed palette are mutually exclusive; the
  guarantee wins. Foreground and background still defer to the terminal, so the
  tool fits into the user's scheme rather than painting over it.
- Honour `NO_COLOR`. The tool must be fully usable with no colour at all.
- **A text field's box and its text are both fixed.** A field is drawn in the
  selection pair — fixed white on fixed navy — never a fixed background under
  the terminal's own foreground. That pair is dark-on-dark on a light terminal:
  the toolkit's form default does exactly this, and the form decorator (§4.1)
  is where it is overridden, at the form level, because the toolkit re-applies
  the form's field style to every field on every draw.

### 8.1 Painting the background — opt in only

Deferring to the terminal has a consequence worth stating, because it surprises
people: **the appearance setting decides foregrounds, not the look.** Set `dark`
in a light terminal and the palette really does change — and the app still looks
light, because the background was never the tool's to paint. "It ignored my
setting" is the usual report, and the setting was obeyed.

`NINES_PAINT_BACKGROUND=1` opts out of that. The tool fills its frame with the
background its foregrounds were measured against, so `dark` means a dark app
anywhere. It is **off by default** — fitting into the user's scheme is the
better default, and it is the one this contract asks for.

Two rules for implementations:

- Paint the palette's own background, not an arbitrary one. It has to stay the
  colour the contrast ratios were computed against, or those ratios become
  claims about nothing.
- Painting a background **must** supply a foreground with it. The terminal's
  default foreground is chosen for the terminal's background; paint over that
  and the text can vanish.

`NO_COLOR` still wins. An explicit request for no colour is not overridden by a
request for a particular one.

### 8.2 Appearances

**No single palette can serve every terminal.** This is arithmetic, not taste.
For a colour to reach 3:1 against a background, its relative luminance must be:

| Background | Required luminance |
| --- | --- |
| white `#ffffff` | `L <= 0.300` |
| black `#000000` | `L >= 0.100` |
| mid-grey `#808080` | `L <= 0.039` **or** `L >= 0.748` |

White and black intersect in `[0.100, 0.300]` — room enough for a palette.
Adding mid-grey empties the intersection completely. And note that reaching
**4.5:1** against both white and black requires `L` in `[0.1750, 0.1833]`, a band
0.008 wide: not a palette, a coincidence.

So the background must be known, or chosen for. Implementations provide four
appearances:

| Appearance | For | Floor |
| --- | --- | --- |
| `universal` | unknown — **the default** | 4.5:1 on white, 3:1 on black |
| `light` | near-white terminal | 4.5:1 |
| `dark` | near-black terminal | 4.5:1 |
| `neutral` | mid-grey terminal, **opt-in only** | 3:1 — the most mid-grey allows |

`universal` is deliberately the default: it is never invisible anywhere, at the
cost of being less vivid on dark than a dark-tuned palette. `neutral` is never a
default because its near-white colours would vanish on a light terminal.

**Selection is the exception** — it sets both of its own colours, so it does not
depend on the terminal background and is identical in every appearance.

#### 8.2.1 Choosing

`NINES_THEME=light|dark|neutral|universal` is authoritative. `COLORFGBG` is
honoured when present, classifying only background indices 0/8 (dark) and 7/15
(light) and staying silent otherwise.

**The OSC 11 query is deliberately not used.** Many terminals answer it wrongly
or not at all, multiplexers sit in the way, and issuing it means writing to and
reading from a terminal the UI library already owns. A wrong answer is worse
than none, because the universal palette is legible either way.

#### 8.2.2 The contrast guard

Palette values are the *intent*; the guard is the *guarantee*. Every colour
entering a theme is checked against its appearance's floor and, if it falls
short, blended toward black or white until it clears — which preserves hue where
scaling channels would not.

This covers what the palette tests cannot see: a colour a tool sets itself, one
a transform such as colour-blind mode produces, or a future palette edit that
regresses.

**What it cannot do is measure the real terminal background.** Foreground and
background are left as the terminal's own, and a default colour has no RGB to
measure — it is the same unknown that makes detection unreliable, not a separate
problem. The guard enforces the floor against the background the chosen
appearance *assumes*.

Under `NO_COLOR` the guard is inert: there is no palette to correct, and forcing
a colour back in would undo the point.

#### 8.2.3 Verification

Every semantic colour is asserted against the background its appearance targets,
using WCAG relative luminance so the numbers match what an accessibility checker
reports. Adding a colour without a ratio is a contract break — these were
comments once, and the comments were wrong.

### 8.1 The state grammar

Every tool colours *things that are in a state*: a container, a package, a task,
a deployment. They use **one vocabulary**, so a reader who learned it in k9s or
p9s can read z9s without learning anything new.

| State | Colour | Glyph | Means |
| --- | --- | --- | --- |
| `Ok` | green | `●` | working as intended — running, ready, deployed |
| `Transitional` | yellow | `◐` | on its way somewhere — starting, deploying, partially up |
| `Failed` | red | `✖` | something is wrong and wants attention |
| `Idle` | dim | `·` | nothing is happening, and nothing is wrong |

**The Idle/Failed split is the rule that carries the weight.** Painting every
stopped thing red is the easy mistake and it destroys the signal: on a working
machine most stopped things stopped on purpose, so red stops meaning "look at
this" and becomes wallpaper. A container that exited 0, or a package sitting on
disk that was never deployed, is **Idle** — inert, not broken.

The corollary costs something and is the point: **green is spent only on the
live thing.** A package whose definition merely parses is not green. Readable is
not deployed.

**Transitional is not decoration.** It is reserved for states expected to settle
on their own — `created`, `paused`, `restarting`, `deploying`, partially-up. A
state that never settles is not transitional; classify it Ok, Idle or Failed.

**Accent is identity, never status.** Accent marks a name, a grouping, a
namespace. Spending it on a state is the category error this section exists to
prevent, and it silently breaks §8's rule that colour means one thing.

Per §8, the glyph is not optional decoration: it is what keeps the state legible
when the colour is absent or indistinguishable. A row that already spells the
state in words ("running", "Succeeded") satisfies the rule without a glyph; a
row that would otherwise carry colour alone must show one.

---

## 9. Conformance

An implementation conforms when:

1. The layout rows and header columns match §1, with widths measured from art.
2. A screen needs only render/name/hints, and optional capabilities are
   discovered rather than required.
3. `apply` is callable without a running app, and screens are tested that way.
4. One `Action` object drives both hints and enforcement.
5. Key ownership follows §4.1, with field handling in exactly two places.
6. Redraw coalesces per §6 and loses no final state.
7. Config load is total and saves are atomic.
8. The tool is usable with `NO_COLOR=1`.
9. States are classified with the §8.1 vocabulary, and no state is shown by
   colour alone.
10. Every semantic colour is verified by test against the background its
    appearance targets, per §8.2.2.

Conformance is checked by rendering fixed inputs in both implementations and
diffing the resulting cell grids — see `docs/CONFORMANCE.md`.
