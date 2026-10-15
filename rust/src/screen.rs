//! Screens and the page stack — CONTRACT.md §2.
//!
//! The required surface is three methods. Everything else is an optional
//! capability a screen opts into, discovered at runtime rather than stubbed out
//! by every screen that does not need it.
//!
//! In Go this is a type assertion against a small interface. Rust has no
//! equivalent for trait objects, so each capability is exposed as an
//! `as_filterable()`-style downcast hook with a default `None`. The cost is one
//! line per capability a screen actually implements; the benefit is that a
//! screen needing none of them stays three methods long.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Hint;

/// A view on the page stack.
pub trait Screen {
    /// Draw into `area`.
    fn render(&mut self, frame: &mut Frame, area: Rect);

    /// Breadcrumb label.
    fn name(&self) -> String;

    /// Bindings valid right now. Build these with [`crate::Authority::hints`]
    /// so what is advertised and what is permitted cannot drift apart.
    fn hints(&self) -> Vec<Hint>;

    // ── Optional capabilities ───────────────────────────────────────────────
    // Default to None. Implement the hook only if the screen implements the
    // trait.

    /// Opt in to `/` filtering.
    fn as_filterable(&mut self) -> Option<&mut dyn Filterable> {
        None
    }
    /// Contribute to the window title.
    fn as_titled(&self) -> Option<&dyn Titled> {
        None
    }
    /// Provide screen-specific help for `?`.
    fn as_helper(&self) -> Option<&dyn Helper> {
        None
    }
    /// Release resources when popped.
    fn as_closable(&mut self) -> Option<&mut dyn Closable> {
        None
    }
    /// Opt in to list navigation, so the shell can own j/k/G/enter.
    fn as_navigable(&mut self) -> Option<&mut dyn Navigable> {
        None
    }
}

/// Screens presenting a selectable list.
///
/// This exists so the shell can own the motion keys without knowing what a
/// screen contains. The alternative — downcasting the boxed screen back to its
/// concrete type in the key handler — would put a second place that decides what
/// a keystroke means, which is precisely what CONTRACT.md §4.1 forbids.
pub trait Navigable {
    /// Move the cursor. Implementations clamp rather than wrap: wrapping from
    /// the top of a long list to its bottom loses the reader's place.
    fn move_by(&mut self, delta: isize);
    fn select_first(&mut self);
    fn select_last(&mut self);
    /// Identifier of the current row, for drilling in. `None` when nothing is
    /// selected or the screen has nothing to drill into.
    fn selected_key(&self) -> Option<String> {
        None
    }
}

/// Screens that support `/` filtering.
pub trait Filterable {
    fn set_filter(&mut self, filter: &str);
    /// The active filter. Esc clears this before popping the screen.
    fn filter(&self) -> String;
}

/// Screens that contribute to the window title, e.g. `packages(/har)[3]`.
pub trait Titled {
    fn title_info(&self) -> String;
}

/// Screens with their own help text.
pub trait Helper {
    fn help_text(&self) -> String;
}

/// Screens holding resources — a tunnel, a watcher, a subscription — that must
/// be released the moment the user navigates away rather than at process exit.
pub trait Closable {
    fn close(&mut self);
}

/// What `Esc` did, so the caller can decide whether to keep going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeOutcome {
    /// An active filter was cleared; the screen stayed.
    ClearedFilter,
    /// A screen was popped.
    Popped,
    /// Nothing to do — this is the root and it has no filter.
    AtRoot,
}

/// The page stack.
///
/// Always holds at least one screen: the root is never popped, so Esc at the
/// bottom is a no-op rather than an exit. Quitting is explicit.
pub struct Stack {
    screens: Vec<Box<dyn Screen>>,
}

impl Stack {
    /// Create a stack rooted at `root`.
    pub fn new(root: Box<dyn Screen>) -> Self {
        Self {
            screens: vec![root],
        }
    }

    pub fn push(&mut self, screen: Box<dyn Screen>) {
        self.screens.push(screen);
    }

    /// Pop the top screen, closing it first if it holds resources.
    ///
    /// Refuses to pop the root and returns `None`.
    pub fn pop(&mut self) -> Option<Box<dyn Screen>> {
        if self.screens.len() <= 1 {
            return None;
        }
        let mut screen = self.screens.pop().expect("len > 1 checked above");
        if let Some(closable) = screen.as_closable() {
            closable.close();
        }
        Some(screen)
    }

    /// Replace the top screen, closing the outgoing one.
    pub fn replace(&mut self, screen: Box<dyn Screen>) {
        if let Some(top) = self.screens.last_mut() {
            if let Some(closable) = top.as_closable() {
                closable.close();
            }
            *top = screen;
        }
    }

    pub fn top(&self) -> &dyn Screen {
        self.screens.last().expect("stack is never empty").as_ref()
    }

    pub fn top_mut(&mut self) -> &mut dyn Screen {
        self.screens
            .last_mut()
            .expect("stack is never empty")
            .as_mut()
    }

    pub fn depth(&self) -> usize {
        self.screens.len()
    }

    /// Breadcrumb trail, root first.
    pub fn breadcrumbs(&self) -> Vec<String> {
        self.screens.iter().map(|s| s.name()).collect()
    }

    /// Handle `Esc`: clear an active filter first, then pop.
    ///
    /// The order is the whole behaviour. Popping while a filter is active loses
    /// two pieces of state to one keypress and leaves the user unsure which one
    /// went.
    pub fn escape(&mut self) -> EscapeOutcome {
        if let Some(filterable) = self.top_mut().as_filterable() {
            if !filterable.filter().is_empty() {
                filterable.set_filter("");
                return EscapeOutcome::ClearedFilter;
            }
        }
        if self.pop().is_some() {
            EscapeOutcome::Popped
        } else {
            EscapeOutcome::AtRoot
        }
    }

    /// Window title: the top screen's name plus whatever it contributes, plus
    /// the stack depth once nested.
    pub fn window_title(&self) -> String {
        let top = self.top();
        let mut title = top.name();
        if let Some(titled) = top.as_titled() {
            let info = titled.title_info();
            if !info.is_empty() {
                title.push_str(&format!("({info})"));
            }
        }
        if self.depth() > 1 {
            title.push_str(&format!("[{}]", self.depth()));
        }
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A screen with every capability, so the stack's behaviour is observable.
    struct Probe {
        label: String,
        filter: String,
        info: String,
        closed: Rc<RefCell<Vec<String>>>,
        capable: bool,
    }

    impl Probe {
        fn new(label: &str, closed: Rc<RefCell<Vec<String>>>) -> Self {
            Self {
                label: label.to_string(),
                filter: String::new(),
                info: String::new(),
                closed,
                capable: true,
            }
        }
        /// A screen implementing nothing optional — the minimum a tool can write.
        fn plain(label: &str) -> Self {
            Self {
                label: label.to_string(),
                filter: String::new(),
                info: String::new(),
                closed: Rc::new(RefCell::new(Vec::new())),
                capable: false,
            }
        }
        fn boxed(self) -> Box<dyn Screen> {
            Box::new(self)
        }
    }

    impl Screen for Probe {
        fn render(&mut self, _frame: &mut Frame, _area: Rect) {}
        fn name(&self) -> String {
            self.label.clone()
        }
        fn hints(&self) -> Vec<Hint> {
            Vec::new()
        }
        fn as_filterable(&mut self) -> Option<&mut dyn Filterable> {
            self.capable.then_some(self as &mut dyn Filterable)
        }
        fn as_titled(&self) -> Option<&dyn Titled> {
            self.capable.then_some(self as &dyn Titled)
        }
        fn as_closable(&mut self) -> Option<&mut dyn Closable> {
            self.capable.then_some(self as &mut dyn Closable)
        }
    }

    impl Filterable for Probe {
        fn set_filter(&mut self, filter: &str) {
            self.filter = filter.to_string();
        }
        fn filter(&self) -> String {
            self.filter.clone()
        }
    }
    impl Titled for Probe {
        fn title_info(&self) -> String {
            self.info.clone()
        }
    }
    impl Closable for Probe {
        fn close(&mut self) {
            self.closed.borrow_mut().push(self.label.clone());
        }
    }

    fn log() -> Rc<RefCell<Vec<String>>> {
        Rc::new(RefCell::new(Vec::new()))
    }

    #[test]
    fn the_root_is_never_popped() {
        let mut stack = Stack::new(Probe::plain("root").boxed());
        assert!(stack.pop().is_none());
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.escape(), EscapeOutcome::AtRoot);
        assert_eq!(stack.depth(), 1, "esc at the root must not empty the stack");
    }

    #[test]
    fn popping_closes_the_screen() {
        let closed = log();
        let mut stack = Stack::new(Probe::plain("root").boxed());
        stack.push(Probe::new("detail", Rc::clone(&closed)).boxed());

        stack.pop();
        assert_eq!(
            &*closed.borrow(),
            &["detail"],
            "a popped screen must release its resources"
        );
    }

    #[test]
    fn replacing_closes_the_outgoing_screen() {
        let closed = log();
        let mut stack = Stack::new(Probe::plain("root").boxed());
        stack.push(Probe::new("first", Rc::clone(&closed)).boxed());
        stack.replace(Probe::plain("second").boxed());

        assert_eq!(&*closed.borrow(), &["first"]);
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.top().name(), "second");
    }

    #[test]
    fn escape_clears_a_filter_before_popping() {
        let mut stack = Stack::new(Probe::plain("root").boxed());
        stack.push(Probe::new("list", log()).boxed());
        stack
            .top_mut()
            .as_filterable()
            .expect("probe filters")
            .set_filter("cash");

        assert_eq!(stack.escape(), EscapeOutcome::ClearedFilter);
        assert_eq!(stack.depth(), 2, "the screen must survive the first esc");
        assert_eq!(stack.escape(), EscapeOutcome::Popped);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn a_screen_without_capabilities_still_works() {
        let mut stack = Stack::new(Probe::plain("root").boxed());
        stack.push(Probe::plain("plain").boxed());
        // No filter to clear, so esc pops straight away, and nothing panics on
        // the missing capabilities.
        assert_eq!(stack.escape(), EscapeOutcome::Popped);
        assert_eq!(stack.window_title(), "root");
    }

    #[test]
    fn breadcrumbs_run_root_first() {
        let mut stack = Stack::new(Probe::plain("clusters").boxed());
        stack.push(Probe::plain("pods").boxed());
        stack.push(Probe::plain("logs").boxed());
        assert_eq!(stack.breadcrumbs(), vec!["clusters", "pods", "logs"]);
    }

    #[test]
    fn window_title_carries_screen_info_and_depth() {
        let mut stack = Stack::new(Probe::plain("packages").boxed());
        assert_eq!(
            stack.window_title(),
            "packages",
            "no depth marker at the root"
        );

        let mut nested = Probe::new("packages", log());
        nested.info = "/har".to_string();
        stack.push(nested.boxed());
        assert_eq!(stack.window_title(), "packages(/har)[2]");
    }
}
