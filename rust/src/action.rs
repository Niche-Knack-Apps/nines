//! Actions, capabilities, and the guard.
//!
//! One object drives both what the user is *offered* and what the program will
//! *do*. That is the entire idea, and it exists because the obvious alternative
//! is broken: filtering an action out of the hint list while leaving its key
//! handler live means the binding still fires for anyone who knows it. The
//! action is hidden, not disabled — which is worse than showing it, because now
//! the tool looks safe and isn't.
//!
//! See CONTRACT.md §3.

use std::collections::BTreeSet;

/// The authority an action requires.
///
/// Deliberately an owned string rather than an enum: the set of capabilities is
/// a property of the *tool*, not of the shell. z9s has deploy/remove/init; a
/// ledger client has write/reconcile. The shell only needs to compare them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Capability(pub String);

impl Capability {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Capability {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// How much an action can hurt, independent of what it is called or bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Danger {
    /// Reads. Always available.
    Safe,
    /// Changes data that can be undone or re-derived.
    Mutation,
    /// Removes or overwrites something not trivially recoverable.
    Destructive,
}

impl Danger {
    /// Whether this action changes anything at all.
    pub fn writes(self) -> bool {
        !matches!(self, Danger::Safe)
    }
}

/// A classified operation, declared once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// Key as advertised, e.g. `d` or `ctrl-d`.
    pub key: String,
    /// Short description for the hint line.
    pub desc: String,
    pub capability: Capability,
    pub danger: Danger,
}

impl Action {
    pub fn new(
        key: impl Into<String>,
        desc: impl Into<String>,
        capability: impl Into<Capability>,
        danger: Danger,
    ) -> Self {
        Self {
            key: key.into(),
            desc: desc.into(),
            capability: capability.into(),
            danger,
        }
    }
}

/// One advertised binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub key: String,
    pub desc: String,
}

impl Hint {
    pub fn new(key: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            desc: desc.into(),
        }
    }
}

/// Why an action was refused. Carrying the reason is the point — a keypress that
/// silently does nothing reads as a bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The session is read-only, so nothing may be changed.
    ReadOnly,
    /// The session lacks this specific capability.
    MissingCapability(Capability),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::ReadOnly => write!(f, "read-only session — nothing can be changed"),
            Refusal::MissingCapability(c) => {
                write!(f, "not permitted here (needs {})", c.as_str())
            }
        }
    }
}

/// What this session is allowed to do.
///
/// Both the hint renderer and the key handler consult this, through the same
/// two methods, so the two can never drift apart.
#[derive(Debug, Clone, Default)]
pub struct Authority {
    granted: BTreeSet<Capability>,
    read_only: bool,
}

impl Authority {
    /// A session that may do anything.
    pub fn full() -> Self {
        Self {
            granted: BTreeSet::new(),
            read_only: false,
        }
    }

    /// A session that may read but change nothing.
    pub fn read_only() -> Self {
        Self {
            granted: BTreeSet::new(),
            read_only: true,
        }
    }

    /// Restrict to an explicit capability set. Without this, every capability is
    /// considered granted and only `read_only` constrains anything.
    pub fn with_capabilities<I, C>(mut self, caps: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Capability>,
    {
        self.granted = caps.into_iter().map(Into::into).collect();
        self
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Whether `action` may run. The single source of truth.
    pub fn permits(&self, action: &Action) -> Result<(), Refusal> {
        if self.read_only && action.danger.writes() {
            return Err(Refusal::ReadOnly);
        }
        // An empty capability set means "unrestricted"; a non-empty one is an
        // allow-list. This keeps the common case — a tool that only cares about
        // read-only — free of ceremony.
        if !self.granted.is_empty() && !self.granted.contains(&action.capability) {
            return Err(Refusal::MissingCapability(action.capability.clone()));
        }
        Ok(())
    }

    /// Render only the actions this session may actually run.
    ///
    /// Uses `permits`, so an action can never be advertised and then refused, or
    /// refused and yet still bound.
    pub fn hints<'a, I>(&self, actions: I) -> Vec<Hint>
    where
        I: IntoIterator<Item = &'a Action>,
    {
        actions
            .into_iter()
            .filter(|a| self.permits(a).is_ok())
            .map(|a| Hint::new(&a.key, &a.desc))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deploy() -> Action {
        Action::new("d", "deploy", "deploy", Danger::Mutation)
    }
    fn remove() -> Action {
        Action::new("ctrl-d", "remove", "remove", Danger::Destructive)
    }
    fn inspect() -> Action {
        Action::new("enter", "inspect", "inspect", Danger::Safe)
    }

    #[test]
    fn full_authority_permits_everything() {
        let a = Authority::full();
        assert!(a.permits(&deploy()).is_ok());
        assert!(a.permits(&remove()).is_ok());
        assert!(a.permits(&inspect()).is_ok());
    }

    #[test]
    fn read_only_refuses_writes_but_never_reads() {
        let a = Authority::read_only();
        assert_eq!(a.permits(&deploy()), Err(Refusal::ReadOnly));
        assert_eq!(a.permits(&remove()), Err(Refusal::ReadOnly));
        assert!(
            a.permits(&inspect()).is_ok(),
            "reads must survive read-only"
        );
    }

    #[test]
    fn capability_allow_list_is_enforced() {
        let a = Authority::full().with_capabilities(["inspect", "deploy"]);
        assert!(a.permits(&deploy()).is_ok());
        assert_eq!(
            a.permits(&remove()),
            Err(Refusal::MissingCapability(Capability::new("remove")))
        );
    }

    /// The hole this whole module exists to close.
    #[test]
    fn nothing_is_hidden_yet_still_executable() {
        let actions = vec![inspect(), deploy(), remove()];
        for authority in [
            Authority::full(),
            Authority::read_only(),
            Authority::full().with_capabilities(["inspect"]),
            Authority::read_only().with_capabilities(["inspect", "deploy"]),
        ] {
            let advertised = authority.hints(&actions);
            for action in &actions {
                let offered = advertised.iter().any(|h| h.key == action.key);
                let allowed = authority.permits(action).is_ok();
                assert_eq!(
                    offered, allowed,
                    "'{}' offered={offered} allowed={allowed} — hints and enforcement disagree",
                    action.key
                );
            }
        }
    }

    #[test]
    fn refusals_explain_themselves() {
        assert!(Refusal::ReadOnly.to_string().contains("read-only"));
        assert!(Refusal::MissingCapability(Capability::new("remove"))
            .to_string()
            .contains("remove"));
    }

    #[test]
    fn danger_classifies_writes() {
        assert!(!Danger::Safe.writes());
        assert!(Danger::Mutation.writes());
        assert!(Danger::Destructive.writes());
    }
}
