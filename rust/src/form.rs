//! Forms — CONTRACT.md §4.1, the part that was learned the hard way.
//!
//! z9s's form keyboard collapsed once already. Key handling had grown into three
//! uncoordinated layers, each grabbing keys without knowing the others existed:
//! app-global runes fired regardless of focus, an autocomplete dropdown owned
//! Enter and the arrows, and ad-hoc per-field captures were chained on top. The
//! failure was not any single bug but the absence of an ownership rule.
//!
//! So: **every key a focused field could receive is resolved in
//! [`Form::on_key`], and nowhere else.** There is no second handler. A screen
//! embedding a form forwards the key and acts on the outcome.
//!
//! # Consequences worth stating
//!
//! - **Printable characters always belong to the field**, including `:` and `/`.
//!   An account named `Assets:Cash` cannot be typed if `:` opens a command bar.
//! - **Arrows always move between fields.** Never conditional on completion
//!   state — that is what trapped the cursor in z9s.
//! - **Ghost text, never a dropdown.** A dropdown owns Enter and the arrows and
//!   leaves the user in a field they cannot leave.
//! - **Enter advances, and submits from the last field.** A sanctioned
//!   deviation from "commit and park": a form with no submit key is unusable,
//!   and this is the shape every terminal form has. `ctrl-s` submits from
//!   anywhere for people who would rather not tab to the end.

use crate::complete::Completer;

/// One text field.
#[derive(Debug, Clone)]
pub struct Field {
    pub label: String,
    pub value: String,
    /// Completion candidates. Empty means a free-text field.
    pub candidates: Vec<String>,
    /// Refuse to submit while this is empty.
    pub required: bool,
    /// Shown when the value is empty.
    pub placeholder: String,
}

impl Field {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            candidates: Vec::new(),
            required: false,
            placeholder: String::new(),
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn with_candidates(mut self, candidates: Vec<String>) -> Self {
        self.candidates = candidates;
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
}

/// What a keystroke did to the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Still editing. Redraw.
    Editing,
    /// The user asked to submit and every required field is filled.
    Submit,
    /// Submission was refused; the message names the offending field.
    Incomplete(String),
    /// The user backed out.
    Cancel,
}

/// A stack of fields with exactly one key handler.
#[derive(Debug)]
pub struct Form {
    fields: Vec<Field>,
    focused: usize,
    completer: Completer,
}

impl Form {
    pub fn new(fields: Vec<Field>) -> Self {
        Self {
            fields,
            focused: 0,
            completer: Completer::new(),
        }
    }

    /// Share a completer, so what is learned in a form is known everywhere.
    pub fn with_completer(mut self, completer: Completer) -> Self {
        self.completer = completer;
        self
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub fn focused(&self) -> usize {
        self.focused
    }

    pub fn value(&self, label: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.label == label)
            .map(|f| f.value.as_str())
    }

    /// The completion suffix for the focused field, if any.
    pub fn ghost(&self) -> Option<String> {
        let field = self.fields.get(self.focused)?;
        if field.candidates.is_empty() {
            return None;
        }
        self.completer.ghost(&field.value, &field.candidates)
    }

    /// Record every completed field, so the next form ranks better.
    pub fn record_choices(&mut self) {
        let chosen: Vec<String> = self
            .fields
            .iter()
            .filter(|f| !f.candidates.is_empty() && !f.value.is_empty())
            .map(|f| f.value.clone())
            .collect();
        for value in chosen {
            self.completer.record(&value);
        }
    }

    pub fn into_completer(self) -> Completer {
        self.completer
    }

    /// **The only place a form key is interpreted.**
    ///
    /// Adding a second one is how z9s's keyboard collapsed; if a field needs new
    /// behaviour, it goes here.
    pub fn on_key(&mut self, key: crossterm::event::KeyEvent) -> Outcome {
        use crossterm::event::{KeyCode, KeyModifiers};

        // ctrl-s submits from anywhere.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            return self.submit();
        }

        match key.code {
            KeyCode::Esc => Outcome::Cancel,

            // Advance, and submit from the last field.
            KeyCode::Enter => {
                if self.focused + 1 < self.fields.len() {
                    self.focused += 1;
                    Outcome::Editing
                } else {
                    self.submit()
                }
            }

            // Field navigation. Never conditional on completion state.
            KeyCode::Tab | KeyCode::Down => {
                self.focus_next();
                Outcome::Editing
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focus_prev();
                Outcome::Editing
            }

            // Right accepts the ghost. It is the only completion key besides
            // Tab-at-end, and it never replaces typed characters.
            KeyCode::Right => {
                self.accept_ghost();
                Outcome::Editing
            }

            KeyCode::Backspace => {
                if let Some(field) = self.fields.get_mut(self.focused) {
                    field.value.pop();
                }
                Outcome::Editing
            }

            // Every printable character belongs to the field. Including ':' and
            // '/', which an account name needs and a global shortcut would eat.
            KeyCode::Char(c) => {
                if let Some(field) = self.fields.get_mut(self.focused) {
                    field.value.push(c);
                }
                Outcome::Editing
            }

            _ => Outcome::Editing,
        }
    }

    fn focus_next(&mut self) {
        if self.fields.is_empty() {
            return;
        }
        self.focused = (self.focused + 1) % self.fields.len();
    }

    fn focus_prev(&mut self) {
        if self.fields.is_empty() {
            return;
        }
        self.focused = if self.focused == 0 {
            self.fields.len() - 1
        } else {
            self.focused - 1
        };
    }

    /// Accept the completion, taking the candidate's own casing.
    ///
    /// Replaces the value rather than appending to it. Typing `assets:cash` and
    /// appending the suffix would leave `assets:Cash` in the field, and a write
    /// would then be rejected for an undeclared account whose only fault is a
    /// lower-case A.
    fn accept_ghost(&mut self) {
        let Some(field) = self.fields.get(self.focused) else {
            return;
        };
        if field.candidates.is_empty() {
            return;
        }
        let Some(completed) = self.completer.completion(&field.value, &field.candidates) else {
            return;
        };
        if let Some(field) = self.fields.get_mut(self.focused) {
            field.value = completed;
        }
    }

    /// Refuse an incomplete form, naming the field and moving focus to it.
    fn submit(&mut self) -> Outcome {
        if let Some(index) = self
            .fields
            .iter()
            .position(|f| f.required && f.value.trim().is_empty())
        {
            self.focused = index;
            return Outcome::Incomplete(format!("{} is required", self.fields[index].label));
        }
        Outcome::Submit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn form() -> Form {
        Form::new(vec![
            Field::new("Payee").required(),
            Field::new("Amount").required(),
            Field::new("Account").with_candidates(vec![
                "Expenses:Food".to_string(),
                "Expenses:Fuel".to_string(),
                "Assets:Cash".to_string(),
            ]),
        ])
    }

    fn type_into(form: &mut Form, text: &str) {
        for c in text.chars() {
            form.on_key(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn a_colon_types_into_the_field_rather_than_opening_anything() {
        // An account named Assets:Cash cannot be entered if ':' is a global.
        let mut form = form();
        form.on_key(key(KeyCode::Tab));
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "Assets:Cash");
        assert_eq!(form.value("Account"), Some("Assets:Cash"));
    }

    #[test]
    fn a_slash_types_into_the_field_too() {
        let mut form = form();
        type_into(&mut form, "and/or");
        assert_eq!(form.value("Payee"), Some("and/or"));
    }

    #[test]
    fn arrows_always_move_between_fields() {
        let mut form = form();
        assert_eq!(form.focused(), 0);
        form.on_key(key(KeyCode::Down));
        assert_eq!(form.focused(), 1);
        form.on_key(key(KeyCode::Up));
        assert_eq!(form.focused(), 0);
    }

    #[test]
    fn arrows_move_even_with_a_completion_pending() {
        // This is the trap: in z9s a dropdown owned the arrows and the cursor
        // could not leave the field.
        let mut form = form();
        form.on_key(key(KeyCode::Tab));
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "Exp");
        assert!(form.ghost().is_some(), "a completion should be offered");

        form.on_key(key(KeyCode::Down));
        assert_eq!(form.focused(), 0, "the arrow must still navigate");
    }

    #[test]
    fn navigation_wraps_at_both_ends() {
        let mut form = form();
        form.on_key(key(KeyCode::BackTab));
        assert_eq!(form.focused(), 2);
        form.on_key(key(KeyCode::Tab));
        assert_eq!(form.focused(), 0);
    }

    #[test]
    fn enter_advances_then_submits_from_the_last_field() {
        let mut form = form();
        type_into(&mut form, "Kroger");
        assert_eq!(form.on_key(key(KeyCode::Enter)), Outcome::Editing);
        type_into(&mut form, "12.34");
        assert_eq!(form.on_key(key(KeyCode::Enter)), Outcome::Editing);
        assert_eq!(form.focused(), 2);
        assert_eq!(form.on_key(key(KeyCode::Enter)), Outcome::Submit);
    }

    #[test]
    fn ctrl_s_submits_from_anywhere() {
        let mut form = form();
        type_into(&mut form, "Kroger");
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "12.34");
        // Still on field 1, not the last.
        assert_eq!(form.on_key(ctrl('s')), Outcome::Submit);
    }

    #[test]
    fn an_incomplete_form_names_the_field_and_focuses_it() {
        let mut form = form();
        type_into(&mut form, "Kroger");
        // Amount left empty.
        let outcome = form.on_key(ctrl('s'));
        assert_eq!(outcome, Outcome::Incomplete("Amount is required".into()));
        assert_eq!(
            form.focused(),
            1,
            "focus should land on the offending field"
        );
    }

    #[test]
    fn whitespace_does_not_satisfy_a_required_field() {
        let mut form = form();
        type_into(&mut form, "   ");
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "1");
        assert!(matches!(form.on_key(ctrl('s')), Outcome::Incomplete(_)));
    }

    #[test]
    fn esc_cancels() {
        let mut form = form();
        assert_eq!(form.on_key(key(KeyCode::Esc)), Outcome::Cancel);
    }

    #[test]
    fn right_accepts_the_ghost_and_only_extends() {
        let mut form = form();
        form.on_key(key(KeyCode::Tab));
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "Expenses:Foo");
        form.on_key(key(KeyCode::Right));
        assert_eq!(form.value("Account"), Some("Expenses:Food"));
    }

    #[test]
    fn accepting_a_completion_corrects_the_case_of_what_was_typed() {
        let mut form = form();
        form.on_key(key(KeyCode::Tab));
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "expenses:foo");
        form.on_key(key(KeyCode::Right));
        assert_eq!(
            form.value("Account"),
            Some("Expenses:Food"),
            "the field must hold the account as the ledger spells it"
        );
    }

    #[test]
    fn a_field_without_candidates_offers_no_ghost() {
        let mut form = form();
        type_into(&mut form, "Kro");
        assert!(form.ghost().is_none(), "Payee is free text here");
    }

    #[test]
    fn backspace_walks_back_and_is_safe_when_empty() {
        let mut form = form();
        type_into(&mut form, "ab");
        form.on_key(key(KeyCode::Backspace));
        assert_eq!(form.value("Payee"), Some("a"));
        form.on_key(key(KeyCode::Backspace));
        form.on_key(key(KeyCode::Backspace));
        assert_eq!(form.value("Payee"), Some(""));
    }

    #[test]
    fn completed_fields_are_remembered_for_next_time() {
        let mut form = form();
        form.on_key(key(KeyCode::Tab));
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "Assets:Cash");
        form.record_choices();

        let completer = form.into_completer();
        assert_eq!(completer.uses("Assets:Cash"), 1);
    }

    #[test]
    fn an_empty_form_does_not_panic_on_navigation() {
        let mut form = Form::new(Vec::new());
        form.on_key(key(KeyCode::Tab));
        form.on_key(key(KeyCode::Up));
        assert_eq!(form.on_key(key(KeyCode::Enter)), Outcome::Submit);
    }
}
