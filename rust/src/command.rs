//! The `:` command bar — CONTRACT.md §5.
//!
//! Ranking comes from [`crate::complete`], the same engine that ranks a
//! command's argument and a form field. One implementation, so `:reg ass` and an
//! account field cannot disagree about which account was meant.

use std::collections::HashMap;

use crate::complete::Completer;

/// A registered command.
#[derive(Debug, Clone)]
pub struct Command {
    /// Every name that invokes this command. The first is canonical and is what
    /// completion offers.
    pub aliases: Vec<String>,
    pub desc: String,
}

impl Command {
    pub fn new<S: Into<String>>(
        aliases: impl IntoIterator<Item = S>,
        desc: impl Into<String>,
    ) -> Self {
        Self {
            aliases: aliases.into_iter().map(Into::into).collect(),
            desc: desc.into(),
        }
    }

    pub fn canonical(&self) -> &str {
        self.aliases.first().map(String::as_str).unwrap_or("")
    }
}

/// What the user typed, split into a command and its argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub name: String,
    pub args: String,
}

/// The command registry and the bar's state.
#[derive(Debug, Default)]
pub struct CommandBar {
    commands: Vec<Command>,
    by_alias: HashMap<String, usize>,
    completer: Completer,
    /// Current input. `None` when the bar is closed.
    input: Option<String>,
}

impl CommandBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, command: Command) {
        let index = self.commands.len();
        for alias in &command.aliases {
            self.by_alias.insert(alias.to_lowercase(), index);
        }
        self.commands.push(command);
    }

    pub fn is_open(&self) -> bool {
        self.input.is_some()
    }

    pub fn input(&self) -> Option<&str> {
        self.input.as_deref()
    }

    pub fn open(&mut self) {
        self.input = Some(String::new());
    }

    pub fn close(&mut self) {
        self.input = None;
    }

    pub fn push(&mut self, c: char) {
        if let Some(input) = self.input.as_mut() {
            input.push(c);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(input) = self.input.as_mut() {
            input.pop();
        }
    }

    /// Split the input into a command name and its argument.
    pub fn invocation(&self) -> Option<Invocation> {
        let input = self.input.as_ref()?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }
        let (name, args) = match trimmed.split_once(char::is_whitespace) {
            Some((n, a)) => (n, a.trim()),
            None => (trimmed, ""),
        };
        Some(Invocation {
            name: name.to_lowercase(),
            args: args.to_string(),
        })
    }

    /// Look up a command by any alias.
    pub fn resolve(&self, name: &str) -> Option<&Command> {
        self.by_alias
            .get(&name.to_lowercase())
            .and_then(|i| self.commands.get(*i))
    }

    /// Record a successful invocation, so it ranks higher next time.
    pub fn record_use(&mut self, name: &str) {
        self.completer.record(name);
    }

    /// Candidates for the current input, best first.
    pub fn candidates(&self) -> Vec<&Command> {
        let Some(input) = self.input.as_deref() else {
            return Vec::new();
        };
        // Only the command word completes; once there is an argument the user
        // has chosen, and re-ranking under them would be noise.
        if input.contains(char::is_whitespace) {
            return Vec::new();
        }
        self.rank(input)
    }

    /// The inline ghost suffix — the rest of the best candidate.
    ///
    /// Only ever an *extension* of what was typed. Suggesting a completion that
    /// would replace typed characters makes Tab destructive.
    pub fn ghost(&self) -> Option<String> {
        let input = self.input.as_deref()?;
        if input.is_empty() {
            return None;
        }
        let best = self.candidates().first().copied()?;
        let name = best.canonical();
        name.strip_prefix(input).map(str::to_string)
    }

    /// Accept the ghost, completing to the best candidate.
    pub fn accept_ghost(&mut self) {
        if let Some(ghost) = self.ghost() {
            if let Some(input) = self.input.as_mut() {
                input.push_str(&ghost);
            }
        }
    }

    /// Rank commands by their best-matching alias.
    ///
    /// A command is offered under its canonical name however it was reached, so
    /// typing an alias does not complete to something the user has never seen.
    fn rank(&self, input: &str) -> Vec<&Command> {
        let mut scored: Vec<(crate::complete::Tier, std::cmp::Reverse<u32>, &Command)> = self
            .commands
            .iter()
            .filter_map(|c| {
                let best = c
                    .aliases
                    .iter()
                    .filter_map(|a| crate::complete::tier(input, a))
                    .min()?;
                Some((
                    best,
                    std::cmp::Reverse(self.completer.uses(c.canonical())),
                    c,
                ))
            })
            .collect();
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.canonical().cmp(b.2.canonical()))
        });
        scored.into_iter().map(|(_, _, c)| c).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> CommandBar {
        let mut bar = CommandBar::new();
        bar.register(Command::new(["balances", "bal", "b"], "account balances"));
        bar.register(Command::new(["register", "reg"], "transactions"));
        bar.register(Command::new(["dashboard", "dash"], "overview"));
        bar.register(Command::new(["ledger"], "run ledger directly"));
        bar
    }

    fn open_with(input: &str) -> CommandBar {
        let mut bar = bar();
        bar.open();
        for c in input.chars() {
            bar.push(c);
        }
        bar
    }

    #[test]
    fn any_alias_resolves_to_the_command() {
        let bar = bar();
        for alias in ["balances", "bal", "b", "BAL"] {
            assert_eq!(
                bar.resolve(alias).map(Command::canonical),
                Some("balances"),
                "alias {alias} did not resolve"
            );
        }
        assert!(bar.resolve("nonsense").is_none());
    }

    #[test]
    fn an_exact_prefix_outranks_a_subsequence() {
        let bar = open_with("reg");
        assert_eq!(
            bar.candidates().first().map(|c| c.canonical()),
            Some("register")
        );
    }

    #[test]
    fn a_subsequence_still_finds_the_command() {
        // 'dsh' is not a prefix of anything but is a subsequence of dashboard.
        let bar = open_with("dsh");
        assert_eq!(
            bar.candidates().first().map(|c| c.canonical()),
            Some("dashboard")
        );
    }

    #[test]
    fn recent_use_breaks_ties_between_equal_tiers() {
        let mut bar = bar();
        // 'b' prefixes 'balances' only, so add a competitor.
        bar.register(Command::new(["budget", "bud"], "envelopes"));

        bar.open();
        bar.push('b');
        let before = bar.candidates().first().map(|c| c.canonical().to_string());

        bar.record_use("budget");
        bar.record_use("budget");
        let after = bar.candidates().first().map(|c| c.canonical().to_string());

        assert_ne!(before, after, "use should promote a command");
        assert_eq!(after.as_deref(), Some("budget"));
    }

    #[test]
    fn the_ghost_only_ever_extends_what_was_typed() {
        let bar = open_with("bal");
        assert_eq!(bar.ghost().as_deref(), Some("ances"));

        // A subsequence match must not offer a ghost, because accepting it would
        // rewrite characters the user typed.
        let bar = open_with("dsh");
        assert!(
            bar.ghost().is_none(),
            "ghost must not replace typed characters"
        );
    }

    #[test]
    fn accepting_the_ghost_completes_the_command() {
        let mut bar = open_with("bal");
        bar.accept_ghost();
        assert_eq!(bar.input(), Some("balances"));
    }

    #[test]
    fn input_splits_into_a_command_and_its_argument() {
        let bar = open_with("ledger reg --monthly ^Expenses");
        let inv = bar.invocation().expect("parses");
        assert_eq!(inv.name, "ledger");
        assert_eq!(inv.args, "reg --monthly ^Expenses");
    }

    #[test]
    fn a_bare_command_has_empty_arguments() {
        let inv = open_with("bal").invocation().expect("parses");
        assert_eq!(inv.name, "bal");
        assert_eq!(inv.args, "");
    }

    #[test]
    fn completion_stops_once_an_argument_is_being_typed() {
        let bar = open_with("ledger re");
        assert!(
            bar.candidates().is_empty(),
            "re-ranking commands while typing an argument would be noise"
        );
        assert!(bar.ghost().is_none());
    }

    #[test]
    fn an_empty_or_closed_bar_offers_nothing() {
        let bar = bar();
        assert!(!bar.is_open());
        assert!(bar.invocation().is_none());
        assert!(bar.candidates().is_empty());

        let bar = open_with("");
        assert!(bar.is_open());
        assert!(bar.invocation().is_none(), "whitespace is not a command");
    }

    #[test]
    fn backspace_walks_back_through_the_input() {
        let mut bar = open_with("bal");
        bar.backspace();
        assert_eq!(bar.input(), Some("ba"));
        bar.backspace();
        bar.backspace();
        assert_eq!(bar.input(), Some(""));
        bar.backspace();
        assert_eq!(bar.input(), Some(""), "backspace on empty is not an error");
    }
}
