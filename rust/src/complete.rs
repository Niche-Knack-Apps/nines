//! Reusable completion — one ranking implementation for every input.
//!
//! The command bar, a command's argument, and a form field all answer the same
//! question: given what has been typed, which of these candidates did the user
//! mean? Three copies of that would rank differently, and a tool where `:reg ass`
//! and a form field disagree about which account is meant is a tool nobody
//! trusts.
//!
//! # Ranking
//!
//! Tiered, not fuzzy-everything:
//!
//! 1. exact prefix
//! 2. case-insensitive prefix
//! 3. subsequence — `aubc` reaches `Assets:US Bank:Checking`
//! 4. substring anywhere — `check` reaches `Assets:US Bank:Checking`
//!
//! Flat fuzzy matching ranks `Assets:Cash` above `Assets:CashApp` about half the
//! time for the input `cash`, and the thing you typed the prefix of should win.
//! Ties break on recent use, then on length (shorter is likelier to be what a
//! short input meant), then alphabetically so the order is stable.
//!
//! # Ghost text
//!
//! Only ever an *extension* of what was typed. A completion that would rewrite
//! typed characters makes Tab destructive, so a subsequence or substring match
//! ranks but offers no ghost.

use std::collections::HashMap;

/// How well a candidate matches. Lower is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    ExactPrefix,
    CasePrefix,
    Subsequence,
    Substring,
}

/// Classify `candidate` against `input`. `None` when it does not match at all.
///
/// An empty input matches everything at the best tier, so an untouched field
/// offers its candidates in frequency order rather than nothing.
pub fn tier(input: &str, candidate: &str) -> Option<Tier> {
    if input.is_empty() {
        return Some(Tier::ExactPrefix);
    }
    if candidate.starts_with(input) {
        return Some(Tier::ExactPrefix);
    }
    let lower_input = input.to_lowercase();
    let lower_candidate = candidate.to_lowercase();
    if lower_candidate.starts_with(&lower_input) {
        return Some(Tier::CasePrefix);
    }
    if is_subsequence(&lower_input, &lower_candidate) {
        return Some(Tier::Subsequence);
    }
    if lower_candidate.contains(&lower_input) {
        return Some(Tier::Substring);
    }
    None
}

/// Is `needle` a subsequence of `haystack`? `dpl` matches `deploy`.
pub fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle.chars().all(|n| chars.any(|h| h == n))
}

/// Ranks candidates and remembers what gets chosen.
///
/// The memory is the point: an account picker that puts the account you use
/// every day at the top after a week is worth more than any matching cleverness.
#[derive(Debug, Default, Clone)]
pub struct Completer {
    chosen: HashMap<String, u32>,
}

impl Completer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `choice` was accepted, so it ranks higher next time.
    pub fn record(&mut self, choice: &str) {
        *self.chosen.entry(choice.to_string()).or_insert(0) += 1;
    }

    /// How often `choice` has been accepted.
    pub fn uses(&self, choice: &str) -> u32 {
        self.chosen.get(choice).copied().unwrap_or(0)
    }

    /// Candidates matching `input`, best first.
    pub fn rank<'a, I, S>(&self, input: &str, candidates: I) -> Vec<&'a str>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        let mut scored: Vec<(Tier, std::cmp::Reverse<u32>, usize, &str)> = candidates
            .into_iter()
            .map(AsRef::as_ref)
            .filter_map(|c| {
                tier(input, c).map(|t| (t, std::cmp::Reverse(self.uses(c)), c.chars().count(), c))
            })
            .collect();
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
                .then(a.3.cmp(b.3))
        });
        scored.into_iter().map(|(_, _, _, c)| c).collect()
    }

    /// The inline suffix completing `input` to the best candidate.
    ///
    /// Matching ignores case; the completion does not. Typing `assets:cash`
    /// should reach `Assets:Cash` — but what lands in the field must be the
    /// account as the ledger spells it, or the write is rejected for an
    /// undeclared account whose only fault is a lower-case A. Use
    /// [`Completer::completion`] to get the corrected whole value.
    ///
    /// `None` unless the best match starts with what was typed, case aside:
    /// accepting a ghost must never delete a character the user entered.
    pub fn ghost<'a, I, S>(&self, input: &str, candidates: I) -> Option<String>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        let (_, suffix) = self.completion_parts(input, candidates)?;
        Some(suffix)
    }

    /// The whole value `input` should become, with the candidate's own casing.
    ///
    /// Returns the full string rather than a suffix, because fixing the case of
    /// what was already typed means replacing it.
    pub fn completion<'a, I, S>(&self, input: &str, candidates: I) -> Option<String>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        let (whole, _) = self.completion_parts(input, candidates)?;
        Some(whole)
    }

    fn completion_parts<'a, I, S>(&self, input: &str, candidates: I) -> Option<(String, String)>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        if input.is_empty() {
            return None;
        }
        let best = *self.rank(input, candidates).first()?;
        // Case-insensitive prefix, so `assets:` completes `Assets:Cash`.
        if best.len() < input.len() || !best[..input.len()].eq_ignore_ascii_case(input) {
            return None;
        }
        Some((best.to_string(), best[input.len()..].to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accounts() -> Vec<String> {
        [
            "Assets:Cash",
            "Assets:CashApp",
            "Assets:CashApp:Savings",
            "Assets:US Bank:Checking",
            "Expenses:Food:Dining",
            "Expenses:Food:Groceries",
            "Liabilities:SoFi Personal Loan",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[test]
    fn an_exact_prefix_beats_everything_else() {
        let all = accounts();
        let c = Completer::new();
        let ranked = c.rank("Assets:Cash", &all);
        assert_eq!(ranked[0], "Assets:Cash");
    }

    #[test]
    fn the_thing_you_typed_the_prefix_of_wins_over_a_longer_sibling() {
        let all = accounts();
        // Flat fuzzy ranking gets this wrong about half the time.
        let c = Completer::new();
        let ranked = c.rank("Assets:CashA", &all);
        assert_eq!(ranked[0], "Assets:CashApp");
    }

    #[test]
    fn case_is_forgiven_but_ranked_below_an_exact_prefix() {
        let c = Completer::new();
        assert_eq!(tier("assets:cash", "Assets:Cash"), Some(Tier::CasePrefix));
        assert_eq!(tier("Assets:Cash", "Assets:Cash"), Some(Tier::ExactPrefix));
        assert!(Tier::ExactPrefix < Tier::CasePrefix);
    }

    #[test]
    fn initials_reach_a_long_account_name() {
        let all = accounts();
        let c = Completer::new();
        let ranked = c.rank("aubc", &all);
        assert_eq!(ranked[0], "Assets:US Bank:Checking");
    }

    #[test]
    fn a_word_from_the_middle_finds_the_account() {
        let all = accounts();
        let c = Completer::new();
        let ranked = c.rank("groceries", &all);
        assert_eq!(ranked[0], "Expenses:Food:Groceries");
    }

    #[test]
    fn nothing_matches_returns_nothing() {
        let all = accounts();
        let c = Completer::new();
        assert!(c.rank("zzzzz", &all).is_empty());
    }

    #[test]
    fn an_empty_input_offers_everything() {
        let all = accounts();
        let c = Completer::new();
        assert_eq!(c.rank("", &all).len(), all.len());
    }

    #[test]
    fn use_promotes_a_candidate_within_its_tier() {
        let all = accounts();
        let mut c = Completer::new();
        let before = c.rank("Assets:Cash", &all)[0].to_string();

        c.record("Assets:CashApp");
        c.record("Assets:CashApp");
        let after = c.rank("Assets:Cash", &all)[0];

        assert_eq!(before, "Assets:Cash", "shortest exact-prefix match first");
        assert_eq!(after, "Assets:CashApp", "use should promote it");
    }

    #[test]
    fn use_never_promotes_across_tiers() {
        let all = accounts();
        // A heavily-used subsequence match must not outrank a prefix match, or
        // typing the beginning of a name stops working.
        let mut c = Completer::new();
        for _ in 0..50 {
            c.record("Liabilities:SoFi Personal Loan");
        }
        let ranked = c.rank("Assets:Cash", &all);
        assert_eq!(ranked[0], "Assets:Cash");
    }

    #[test]
    fn shorter_wins_when_tier_and_use_are_equal() {
        let all = accounts();
        let c = Completer::new();
        let ranked = c.rank("Assets:CashApp", &all);
        assert_eq!(ranked[0], "Assets:CashApp");
        assert_eq!(ranked[1], "Assets:CashApp:Savings");
    }

    #[test]
    fn ranking_is_stable_for_otherwise_equal_candidates() {
        let all = accounts();
        let c = Completer::new();
        let a = c.rank("Expenses:Food:", &all);
        let b = c.rank("Expenses:Food:", &all);
        assert_eq!(a, b);
        assert_eq!(a[0], "Expenses:Food:Dining", "alphabetical settles the tie");
    }

    #[test]
    fn matching_ignores_case_but_the_completion_corrects_it() {
        let all = accounts();
        let c = Completer::new();
        // The ledger spells it Assets:Cash. Landing "assets:Cash" in the field
        // would be rejected as an undeclared account over a lower-case A.
        assert_eq!(
            c.completion("assets:cas", &all).as_deref(),
            Some("Assets:Cash")
        );
        assert_eq!(c.ghost("assets:cas", &all).as_deref(), Some("h"));
    }

    #[test]
    fn a_correctly_cased_input_completes_unchanged() {
        let all = accounts();
        let c = Completer::new();
        assert_eq!(
            c.completion("Assets:Cas", &all).as_deref(),
            Some("Assets:Cash")
        );
    }

    #[test]
    fn a_subsequence_match_still_offers_no_completion() {
        // Completing "aubc" would delete what was typed, which makes accepting
        // a suggestion destructive.
        let all = accounts();
        let c = Completer::new();
        assert!(c.completion("aubc", &all).is_none());
        assert!(c.ghost("aubc", &all).is_none());
    }

    #[test]
    fn the_ghost_extends_and_never_replaces() {
        let all = accounts();
        let c = Completer::new();
        assert_eq!(
            c.ghost("Assets:US", &all).as_deref(),
            Some(" Bank:Checking")
        );
        // A subsequence match ranks, but completing it would rewrite what was
        // typed, so there is no ghost.
        assert!(c.ghost("aubc", &all).is_none());
        // A case-insensitive prefix DOES complete now, correcting the case.
        assert_eq!(
            c.ghost("assets:us", &all).as_deref(),
            Some(" Bank:Checking")
        );
    }

    #[test]
    fn an_empty_input_offers_no_ghost() {
        let all = accounts();
        let c = Completer::new();
        assert!(c.ghost("", &all).is_none());
    }

    #[test]
    fn subsequence_respects_order() {
        assert!(is_subsequence("abc", "aXbXc"));
        assert!(!is_subsequence("cba", "aXbXc"));
        assert!(!is_subsequence("abcd", "abc"), "needle cannot be longer");
    }

    #[test]
    fn works_over_borrowed_strs_as_well_as_owned() {
        // Callers hold candidates in whatever they already have; the completer
        // should not force an allocation to ask a question.
        let c = Completer::new();
        let borrowed: Vec<&str> = vec!["alpha", "beta"];
        assert_eq!(c.rank("al", &borrowed)[0], "alpha");
    }
}
