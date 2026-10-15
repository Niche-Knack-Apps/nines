//! Terminal setup and teardown.
//!
//! Two things here are worth more than they look.
//!
//! # The panic hook
//!
//! A TUI that panics while holding raw mode and the alternate screen leaves the
//! user with a dead terminal: no echo, no line editing, and the panic message
//! scrolled away with the alternate screen. The hook restores the terminal
//! *before* the default handler prints, so a crash is legible and the shell is
//! usable afterwards. This is not politeness — without it, the first real panic
//! costs someone a `reset` and the backtrace they needed.
//!
//! # Keyboard enhancement
//!
//! Legacy terminal encoding cannot express `shift-↑`: it arrives as a bare `↑`,
//! or as nothing. The contract's period navigation (§4.4) is built on exactly
//! those chords, so where the terminal speaks the Kitty keyboard protocol we ask
//! for disambiguation, and where it does not we fall back rather than break.
//! [`Capabilities`] reports what was actually negotiated so a tool can advertise
//! the bindings that will really work.

use std::io::{self, Stdout, Write};

use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

/// What the host terminal actually agreed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    /// Key events are disambiguated, so modified arrows and key releases are
    /// distinguishable. Without this, `shift-↑` is indistinguishable from `↑`.
    pub enhanced_keys: bool,
    /// Mouse reporting is on.
    pub mouse: bool,
}

impl Capabilities {
    /// Whether chorded navigation (`shift-←/→/↑/↓`) can be relied on.
    ///
    /// A tool should consult this before advertising those bindings: offering a
    /// key that silently does nothing is worse than not offering it.
    pub fn has_modified_arrows(&self) -> bool {
        self.enhanced_keys
    }
}

/// An initialised terminal. Restores itself on drop.
pub struct Session {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    capabilities: Capabilities,
    restored: bool,
}

impl Session {
    /// Take over the terminal.
    ///
    /// `mouse` requests mouse reporting; it is a preference, not a guarantee.
    pub fn enter(mouse: bool) -> io::Result<Self> {
        install_panic_hook();

        enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;

        // Ask, then record what we got. Terminals that do not understand the
        // request ignore it, so this must never be assumed to have worked.
        let enhanced = supports_keyboard_enhancement().unwrap_or(false);
        if enhanced {
            execute!(
                out,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                )
            )?;
        }

        if mouse {
            execute!(out, EnableMouseCapture)?;
        }

        let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

        Ok(Self {
            terminal,
            capabilities: Capabilities {
                enhanced_keys: enhanced,
                mouse,
            },
            restored: false,
        })
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// Hand the terminal back. Idempotent.
    pub fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        restore_terminal(self.capabilities)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Best effort: a failure here cannot be reported anywhere useful, and
        // leaving raw mode on is worse than swallowing the error.
        let _ = self.restore();
    }
}

/// Undo everything [`Session::enter`] did, in reverse.
fn restore_terminal(caps: Capabilities) -> io::Result<()> {
    let mut out = io::stdout();
    if caps.mouse {
        let _ = execute!(out, DisableMouseCapture);
    }
    if caps.enhanced_keys {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(out, DisableBracketedPaste, LeaveAlternateScreen);
    disable_raw_mode()?;
    out.flush()
}

/// Restore the terminal before the default panic handler prints.
///
/// Registered once. The flags passed here are the worst case rather than what
/// was negotiated, because a panic may happen before that is known and undoing
/// a mode that was never set is harmless.
fn install_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore_terminal(Capabilities {
                enhanced_keys: true,
                mouse: true,
            });
            previous(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_gate_the_bindings_that_depend_on_them() {
        let legacy = Capabilities {
            enhanced_keys: false,
            mouse: false,
        };
        let modern = Capabilities {
            enhanced_keys: true,
            mouse: false,
        };
        assert!(
            !legacy.has_modified_arrows(),
            "without disambiguation shift-arrow is indistinguishable from arrow, \
             so a tool must not advertise it"
        );
        assert!(modern.has_modified_arrows());
    }

    #[test]
    fn installing_the_panic_hook_is_idempotent() {
        // Registering per-Session would chain a new hook onto the previous one
        // every time, and a tool that enters the terminal twice would restore
        // twice per panic.
        install_panic_hook();
        install_panic_hook();
        install_panic_hook();
    }
}
