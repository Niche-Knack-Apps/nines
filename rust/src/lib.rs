//! `nines` — a k9s-shaped terminal shell for ratatui.
//!
//! This crate implements [`CONTRACT.md`], the language-neutral specification of
//! the 9s interface. A Go implementation of the same contract lives in `go/`.
//! Where this crate and the contract disagree, this crate is wrong.
//!
//! [`CONTRACT.md`]: https://github.com/Niche-Knack-Apps/nines/blob/main/CONTRACT.md
//!
//! # What the shell gives you
//!
//! A page stack, a command bar, a header with breadcrumbs and a persistent key
//! legend, a status line, and — the part that matters — one place where an
//! action's key binding and its authority check come from the same object.
//!
//! # What it deliberately does not give you
//!
//! A base class. A screen implements [`Screen`], which is three methods, and
//! opts in to anything further by implementing a capability trait. Everything
//! optional is discovered at runtime rather than stubbed out by every screen
//! that does not need it.

pub mod action;
pub mod appearance;
pub mod chrome;
pub mod command;
pub mod complete;
pub mod confirm;
pub mod form;
pub mod health;
pub mod layout;
pub mod screen;
pub mod terminal;
pub mod theme;

// Serialising tests that touch process-global state — notably NINES_THEME,
// which several tests must set to prove precedence.
#[cfg(test)]
pub(crate) mod testenv;

pub use action::{Action, Authority, Capability, Danger, Hint, Refusal};
pub use chrome::{render_breadcrumbs, render_legend, InfoPanel, Status};
pub use command::{Command, CommandBar, Invocation};
pub use complete::{Completer, Tier};
pub use confirm::{Answer, Confirm};
pub use form::{Field, Form, Outcome};
pub use layout::{art_size, chrome, header, Chrome, Header};
pub use screen::{Closable, Filterable, Helper, Navigable, Screen, Stack, Titled};
pub use terminal::{Capabilities, Session};
pub use theme::Theme;
