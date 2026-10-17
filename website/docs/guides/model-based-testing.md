---
title: Model-based testing
---

# Model-based testing

A model-based test runs every action against your view and against a small reference model at the same time, then checks that they agree. It's the strongest invariant you can write, because it checks every observable fact after every step, not just the few you thought of.

## Define the state machine

```rust
use gpui_proptest::{Failure, StateMachine, arbitrary::Arbitrary, ensure_eq};

#[derive(Clone, Debug, Arbitrary)]
enum TodoAction {
    Add(u8),
    Remove(u8),
    Clear,
}

struct TodoSpec;

impl StateMachine for TodoSpec {
    type View = TodoList;
    type Model = Vec<u8>;
    type Action = TodoAction;

    fn open(cx: &mut App) -> (AnyWindowHandle, Entity<TodoList>) {
        // Open a window with the view, as Harness::new does.
    }

    fn init_model() -> Vec<u8> {
        Vec::new()
    }

    // Only generate actions that make sense in the model's current state.
    fn precondition(model: &Vec<u8>, action: &TodoAction) -> bool {
        match action {
            TodoAction::Remove(index) => (*index as usize) < model.len(),
            _ => true,
        }
    }

    // Drive the real view: through its API, or by dispatching actions.
    fn perform(action: &TodoAction, view: &Entity<TodoList>, window: &mut Window, cx: &mut App) {
        view.update(cx, |list, cx| match action {
            TodoAction::Add(item) => list.add(*item, window, cx),
            TodoAction::Remove(index) => list.remove(*index as usize, cx),
            TodoAction::Clear => list.clear(cx),
        });
    }

    // Apply the same action to the model, in the simplest correct way.
    fn apply(model: &mut Vec<u8>, action: &TodoAction) {
        match action {
            TodoAction::Add(item) => model.push(*item),
            TodoAction::Remove(index) => { model.remove(*index as usize); }
            TodoAction::Clear => model.clear(),
        }
    }

    fn check(model: &Vec<u8>, view: &Entity<TodoList>, _: &mut Window, cx: &mut App) -> Result<(), Failure> {
        ensure_eq!(&view.read(cx).items, model);
        Ok(())
    }
}
```

## Run it

```rust
#[test]
fn todo_list_matches_its_model() {
    Harness::from_state_machine::<TodoSpec>().check(Config::default());
}

#[test]
fn todo_list_fuzz() {
    gpui_proptest::fuzz!(Harness::from_state_machine::<TodoSpec>());
}
```

Each step:

1. picks a domain action;
2. skips it if `precondition` rejects it;
3. otherwise runs `perform` on the view and `apply` on the model;
4. renders, and calls `check` as the invariant named `model`.

A failure shrinks to the shortest action sequence, such as `Add, Add, Add, Remove(1)`.

## UI steps alongside the model

State-machine harnesses generate only domain steps by default, because the model can't see the effect of a random click. You can turn UI steps back on when:

- your check tolerates them, for example when it only compares facts that clicks can't change; or
- the clicks themselves are modelled. A domain action can be "click the delete button of row N", performed with `window.dispatch_event` or through gpui-kit's test helpers.

```rust
Harness::from_state_machine::<TodoSpec>().with_ui_steps(true)
```

## Tips

- **Keep the model dumb.** A `Vec`, a `BTreeMap` or a counter. If the model needs the same logic as the view, it will share the same bugs.
- **Make actions small and orthogonal.** `Add`, `Remove`, `Move` beat `ApplyBatch(Vec<Op>)`: shrinking removes whole actions, so small ones give small reports.
- **Use preconditions, not filtering in `perform`.** A rejected action is recorded as skipped and shrinks away. An action that silently does nothing looks like a real step in every report.
- **Check rendered output too.** Compare the model against what the user sees, such as labels from `targets` or gpui-kit's `window.find(..)`, not only the view's fields.
