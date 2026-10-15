# nines

[![ci](https://github.com/Niche-Knack-Apps/nines/actions/workflows/ci.yml/badge.svg)](https://github.com/Niche-Knack-Apps/nines/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/Niche-Knack-Apps/nines)](https://github.com/Niche-Knack-Apps/nines/releases)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

A k9s-shaped terminal shell, specified once and implemented twice — so a tool can
be written in whichever language suits its problem and still present the same
interface.

Made by [Niche-Knack](https://nk.tools) — Smart Tools. Sharp Solutions.

```
nines/
├── CONTRACT.md    the specification. normative.
├── rust/          ratatui implementation
└── go/            tview/tcell implementation
```

## Install

**Go** (tview/tcell):

```sh
go get github.com/Niche-Knack-Apps/nines/go@latest
```

```go
import nines "github.com/Niche-Knack-Apps/nines/go"
```

**Rust** (ratatui 0.30, crossterm 0.29). The crate is not on crates.io; depend on
a release tag:

```toml
[dependencies]
nines = { git = "https://github.com/Niche-Knack-Apps/nines", tag = "v0.1.0" }
```

Both implementations follow the same version. The Go module is tagged
`go/vX.Y.Z` alongside each `vX.Y.Z` release, because it lives in a subdirectory.

## Why

[`z9s`](https://github.com/Niche-Knack-Apps/z9s) proved the shape. But the shell
lived inside one Go binary, so the next tool faced a bad choice: copy it by hand
and drift, or pick the language for the shell instead of for the problem.

`z9s` links Zarf as a Go library. Another tool might share a Rust engine with a
desktop app. Neither should have to change language to keep the interface, and a
user who learns one should have learned both.

So the contract is the artifact. `rust/` and `go/` are implementations of it, and
where either disagrees with `CONTRACT.md`, the implementation is wrong.

## What it gives you

- A page stack with breadcrumbs, where the root is never popped and a screen
  holding resources releases them when the user navigates away.
- A header with an info panel, a persistent key legend, and art whose columns are
  measured rather than hard-coded.
- A command bar that is inserted into the layout, not overlaid.
- **One object per action, driving both the hint list and the authority check.**
  An action that is not offered cannot be invoked — the two cannot drift.
- A key-ownership rule that stops the three-layers-fighting-over-Enter failure
  that prompted this in the first place. See `CONTRACT.md` §4.1.
- Selection colours that cannot resolve to invisible text, and no meaning carried
  by colour alone.

## What it does not give you

A base class. A screen is three methods — render, name, hints — and opts into
anything further. A screen that filters says so; one that does not stays three
methods long.

## Status

`CONTRACT.md` is complete.

`go/` is the extraction of z9s's UI layer, and **z9s runs on it** — which is the
only real proof it is faithful.

`rust/` implements actions, layout, screens and theme against ratatui 0.30.

Art measurement is covered by an identical table in both (`go/conformance_test.go`,
`rust/src/layout.rs`). Writing it found three divergences on the first run — empty
art, trailing newlines, and bytes-versus-cells — none of which z9s's own artwork
exercised.

## Theme

`NINES_THEME=light|dark|neutral` picks the palette when terminal detection cannot
(it never can under tmux). `NO_COLOR` is honoured. See `CONTRACT.md` §8.

## Testing

The contract's most important rule is that a screen's `apply` is callable without
a running app, so screens are tested by feeding them a result and asserting on
rendered cells — not by driving a live terminal. A shell whose screens can only
be tested end-to-end will not be tested.

```sh
cd rust && cargo test
cd go   && go test ./...
```

## Releases

Every merge to `main` runs CI for both implementations. When the merge carries a
new version in `VERSION` (kept equal to `rust/Cargo.toml`), the release workflow
tags `vX.Y.Z` and `go/vX.Y.Z` and publishes a GitHub release with notes. A pull
request that does not bump `VERSION` fails its check, so every merge ships.

## Support

nines is free and open. Niche-Knack also makes small, sharp, offline-first apps —
see them at [nk.tools](https://nk.tools).

Really believe in this project's potential and want to contribute to its
development? [Buy Me a Coffee](https://buymeacoffee.com/nicheknack).

## License

[Apache License 2.0](LICENSE). See [NOTICE](NOTICE).
