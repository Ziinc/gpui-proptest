---
title: Oracles and invariants
---

# Oracles and invariants

An oracle is a check that holds for every GPUI view. An invariant is a check you write. Both run once the window opens and again after every step. The first one that fails ends the case.

## Built-in oracles

| Oracle | Fails when | Turn off |
| --- | --- | --- |
| `panic` | anything panics: a listener, render, a task or an invariant | always on |
| `livelock` | a step never settles: a task reschedules itself for 100,000 ticks, or the window draws 1,000 frames in one step | always on |
| `teardown` | closing the app panics, or GPUI's scheduler reports non-determinism | always on |
| `disabled-is-inert` | clicking a disabled target changes any target's checked, selected, expanded or value state | `with_disabled_is_inert(false)` |
| `focus-is-visible` | a target that reports focus is not visible (gpui-kit targets report focus) | `with_focus_is_visible(false)` |
| `unique-ids` | two `proptest_target`s share an id path | `with_unique_ids(false)` |
| `window-closed` | a step closes the window | `with_window_may_close(true)` |

A panicking invariant is reported under the invariant's name, not as `panic`.

## Writing invariants

```rust
use gpui_proptest::{ensure, ensure_eq, fail};

harness
    .invariant("total matches the line items", |cart, _targets, _window, cx| {
        let cart = cart.read(cx);
        ensure_eq!(cart.total(), cart.items.iter().map(|item| item.price).sum::<u32>());
        Ok(())
    })
    .invariant("checkout is enabled only with items", |cart, targets, _window, cx| {
        let checkout = targets.iter().find(|target| target.label() == Some("Checkout"));
        let Some(checkout) = checkout else { fail!("the checkout button is missing") };
        ensure!(checkout.is_disabled() == cart.read(cx).items.is_empty());
        Ok(())
    })
```

- `ensure!(condition)` and `ensure!(condition, "format {}", args)` fail when the condition is false.
- `ensure_eq!(left, right)` fails with both values.
- `fail!("...")` returns a failure.

The name you pass to `invariant` becomes the failure's oracle. That name is how a report says what broke, and how shrinking knows a smaller case still fails the same way.

## What makes a good invariant

See [Best practices](../best-practices.md#write-invariants-not-examples). In short:

- **State relations, not outcomes.** "The badge shows the unread count" holds after any sequence. "After clicking X the badge shows 3" only holds after one.
- **Compare against a model.** When the view's logic is non-trivial, a [state machine](../guides/model-based-testing.md) is the strongest invariant there is.
- **Check what the user sees.** Read rendered state from `targets` or gpui-kit's `window.find(..)` rather than only the view's fields. That catches a view whose state is right but whose render is stale.
