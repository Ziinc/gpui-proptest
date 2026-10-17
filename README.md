# gpui-proptest

Property-based testing for [GPUI](https://www.gpui.rs). gpui-proptest clicks, types and presses keys at random in a real GPUI window. It checks oracles and your invariants after every step. When something fails, it shrinks the sequence to the shortest one that still fails and prints a regression test you can paste.

```text
gpui-proptest: [reset zeroes the count] counter.count == 0 failed: 1 != 0 (after step 3) (seed 0x0, shrunk from 27 to 3 steps)

trace:
seed 0x0
   1. click (increment)
   2. click (increment)
   3. click (reset)

regression test:

#[test]
fn regression_0() {
    use gpui_proptest::{ModifierKeys, Step};
    harness().replay(
        0x0,
        &[
            Step::Click { target: 0, modifiers: ModifierKeys::NONE }, // click (increment)
            Step::Click { target: 0, modifiers: ModifierKeys::NONE }, // click (increment)
            Step::Click { target: 2, modifiers: ModifierKeys::NONE }, // click (reset)
        ],
    );
    // failed: [reset zeroes the count] counter.count == 0 failed: 1 != 0 (after step 3)
}
```

## Install

```toml
[dev-dependencies]
gpui-proptest = { git = "https://github.com/Ziinc/gpui-proptest" }
# With gpui-kit:
# gpui-proptest = { git = "https://github.com/Ziinc/gpui-proptest", features = ["kit"] }
```

gpui-proptest uses `gpui-pre =0.3.9`, the snapshot that gpui-kit 0.7.3 is built against.

| Feature | Default | Adds |
| --- | --- | --- |
| `proptest` | yes | `Harness::check` and `Harness::search`: generation, shrinking, `proptest-regressions/` |
| `kit` | no | gpui-kit elements observed with `.test_support()` become targets; `kit::harness` opens windows through `gpui_kit::open_window` |

## Smoke test any view

Mark the elements generated actions may hit with `.proptest_target()`. The element needs an id. Then add invariants and search.

```rust
use gpui_proptest::{Config, Harness, ProptestTargetExt, ensure_eq};

// in render():
div().id("reset").on_click(...).proptest_target()

fn harness() -> Harness<Counter> {
    Harness::new(|_, cx| cx.new(|_| Counter::default())).invariant(
        "reset zeroes the count",
        |counter, _targets, _window, cx| {
            let counter = counter.read(cx);
            if counter.reset_clicked {
                ensure_eq!(counter.count, 0);
            }
            Ok(())
        },
    )
}

#[test]
fn counter_survives_random_use() {
    harness().check(Config::default());
}
```

Steps never store a concrete element. A step says "click the Nth eligible target", and the runner resolves N against the targets painted in the last frame, top to bottom and then left to right. That way, deleting earlier steps while shrinking never invalidates later ones. A step with no eligible target is recorded as skipped.

| Step | Eligible targets |
| --- | --- |
| `Click` (with modifiers) | visible, disabled included |
| `DoubleClick`, `RightClick`, `Hover`, `Scroll` | visible and enabled |
| `Press` | one of the harness keys (`with_keys`, default `tab`, `shift-tab`, `enter`, `escape`, `space`, `backspace`, arrows) |
| `Type` | whatever has focus |
| `AdvanceClock` | GPUI's fake clock |
| `Domain` | your domain handler (`with_domain`) |

## Built-in oracles

| Oracle | Fails when |
| --- | --- |
| `panic` | anything panics: a listener, render, a task or an invariant |
| `scheduler` | GPUI's test scheduler reports non-determinism at the end of a case |
| `disabled-is-inert` | clicking a disabled target changes any target's checked, selected, expanded or value state |
| `focus-is-visible` | a target that reports focus is not visible (with `kit`, which reports focus) |

Turn them off with `with_disabled_is_inert(false)` and `with_focus_is_visible(false)`.

## Model-based tests

Implement `StateMachine` with a simple reference model. Domain actions derive `arbitrary::Arbitrary` (re-exported as `gpui_proptest::arbitrary`):

```rust
#[derive(Clone, Debug, Arbitrary)]
enum TodoAction { Add(u8), Remove(u8), Clear }

impl StateMachine for TodoSpec {
    type View = TodoList;
    type Model = Vec<u8>;
    type Action = TodoAction;
    fn open(cx: &mut App) -> (AnyWindowHandle, Entity<TodoList>) { ... }
    fn init_model() -> Vec<u8> { Vec::new() }
    fn precondition(model: &Vec<u8>, action: &TodoAction) -> bool { ... }
    fn perform(action: &TodoAction, view: &Entity<TodoList>, window: &mut Window, cx: &mut App) { ... }
    fn apply(model: &mut Vec<u8>, action: &TodoAction) { ... }
    fn check(model: &Vec<u8>, view: &Entity<TodoList>, window: &mut Window, cx: &mut App) -> Result<(), Failure> { ... }
}

Harness::from_state_machine::<TodoSpec>().check(Config::default());
```

State-machine harnesses generate only domain steps by default, because the model cannot see the effects of random clicks. Turn UI steps on with `with_ui_steps(true)` when your check tolerates them.

## gpui-kit

```rust
let harness = gpui_proptest::kit::harness(|_, cx| cx.new(|_| Form::default()))
    .invariant("agree matches the model", |form, _, window, cx| {
        ensure_eq!(window.find("agree").checked(), Some(form.read(cx).agreed));
        Ok(())
    });
```

Every element gpui-kit observes with `.test_support()` (checkboxes, switches, tabs, sliders, dialogs, menus and more) is a target, with its role, label, focus, disabled and checked state.

## Determinism, replay and shrinking

- Each case runs on a fresh `TestAppContext` built from `TestDispatcher::new(seed)`, the same machinery as `#[gpui::test(iterations, seed)]`. The step sequence and GPUI's task interleaving both replay exactly.
- Before shrinking, the first failing case is replayed. A different outcome is reported as non-deterministic with both traces, and shrinking is skipped.
- proptest shrinks the steps and their arguments. A second pass then removes runs and pairs of steps while the same oracle still fails, and tries scheduler seed 0.
- Failures are saved to `proptest-regressions/` and replayed first on later runs (`Config::with_persist_failures(false)` turns this off).
- `GPUI_PROPTEST_CASES` overrides the case count, `GPUI_PROPTEST_SEED` fixes the generator seed and `SEED` fixes the scheduler seed.

## Engines

The core never imports proptest. `Harness::run_case(seed, &steps)` replays one input and returns an `Outcome`, and `Step` derives `arbitrary::Arbitrary`, so a [bolero](https://github.com/camshaft/bolero) or cargo-fuzz driver can call the same entry point:

```rust
bolero::check!()
    .with_arbitrary::<(u64, Vec<Step>)>()
    .for_each(|(seed, steps)| assert!(harness().run_case(*seed, steps).is_ok()));
```

## Speed

A case costs about 1 ms to set up and about 1 ms per click in an optimized build. Unoptimized GPUI is roughly ten times slower. Add this to the crate that runs the tests:

```toml
[profile.dev.package."*"]
opt-level = 2
```

## Linux

Building with the `kit` feature links GPUI's Linux platform, which needs `libxkbcommon-dev` and `libxkbcommon-x11-dev`.
