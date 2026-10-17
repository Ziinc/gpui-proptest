use crate::{Failure, Harness};
use gpui::{AnyWindowHandle, App, Entity, Window};
use std::{cell::RefCell, fmt::Debug, rc::Rc};

/// A model-based test: a simple reference model that the real view must agree with.
///
/// Each generated domain action is performed on the view (through the UI where
/// possible) and applied to the model; [`check`](StateMachine::check) then compares
/// them. Build the harness with [`Harness::from_state_machine`].
///
/// `Action` derives `arbitrary::Arbitrary`, so every engine can generate it.
pub trait StateMachine: 'static {
    type View: 'static;
    type Model: Debug + 'static;
    type Action: Clone + Debug + for<'a> arbitrary::Arbitrary<'a> + 'static;

    /// Opens the window under test.
    fn open(cx: &mut App) -> (AnyWindowHandle, Entity<Self::View>);

    /// The model's state when the window opens.
    fn init_model() -> Self::Model;

    /// Whether `action` applies in `model`'s state. Actions that do not apply are
    /// recorded as skipped and neither performed nor applied.
    fn precondition(_model: &Self::Model, _action: &Self::Action) -> bool {
        true
    }

    /// Performs `action` on the real view.
    fn perform(action: &Self::Action, view: &Entity<Self::View>, window: &mut Window, cx: &mut App);

    /// Applies `action` to the model.
    fn apply(model: &mut Self::Model, action: &Self::Action);

    /// Fails when the view disagrees with the model.
    fn check(
        model: &Self::Model,
        view: &Entity<Self::View>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), Failure>;
}

impl<V: 'static, A: 'static> Harness<V, A> {
    /// A harness that generates `S`'s domain actions and checks `S`'s model after
    /// every step. Generic UI steps are off, because the model cannot see their
    /// effects; turn them on with [`Harness::with_ui_steps`] when the check tolerates them.
    pub fn from_state_machine<S: StateMachine<View = V, Action = A>>() -> Self {
        let model = Rc::new(RefCell::new(S::init_model()));
        let reset = model.clone();
        let performed = model.clone();
        let checked = model;
        Harness::with_window(S::open)
            .with_setup(move |_| *reset.borrow_mut() = S::init_model())
            .with_domain(move |action: &A, view, window, cx| {
                if !S::precondition(&performed.borrow(), action) {
                    return false;
                }
                S::perform(action, view, window, cx);
                S::apply(&mut performed.borrow_mut(), action);
                true
            })
            .invariant("model", move |view, _, window, cx| {
                S::check(&checked.borrow(), view, window, cx)
            })
            .with_ui_steps(false)
    }
}
