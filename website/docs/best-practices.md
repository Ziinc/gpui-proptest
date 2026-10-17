---
sidebar_position: 50
title: Best practices
---

# Best practices

## Write invariants, not examples

An example test says "after these three clicks, the badge shows 3". An invariant says "the badge always shows the number of unread messages". gpui-proptest can only find bugs your invariants can see, so most of the value is in what you assert.

Good invariants for UI code:

- **Consistency:** two views of the same data agree. The badge count equals the list's unread items, and the header total equals the sum of the rows.
- **Round trips:** undo after any action restores the previous state, and save then load gives the same document.
- **Bounds:** the scroll offset stays in range, the selection is inside the text, and no number is negative.
- **Rendering matches state:** read what is drawn, from `targets` or gpui-kit's `window.find(..)`, and compare it with the view's fields.
- **A model:** for anything with real logic, a [state machine](./guides/model-based-testing.md) checks everything at once.

Avoid invariants that restate the implementation. If the check calls the same function the view calls, it shares the same bugs.

## Mark the controls a user would use

Mark every interactive element with `proptest_target()`, or use the `kit` feature. Leave purely decorative elements unmarked: each target dilutes how often the others get picked. If a few controls matter much more, such as the editor surface in an editor, give them their own domain actions so they're exercised more often.

## Keep ids unique and stable

The `unique-ids` oracle fails when siblings share an id. Ids also appear in every trace, so `"delete-row-3"` reads far better than `("item", 17)`. Prefer ids derived from the data over list positions, so a trace still means something after the data changes.

## Make your view deterministic

- Use GPUI's executor for time (`cx.background_executor().timer(..)`), never `std::thread::sleep` or `Instant::now`.
- Use `cx.spawn` and `cx.background_spawn`, not `std::thread::spawn`.
- Keep state in entities and globals that live in the app, not in `static`s, because each case builds a new app.
- Inject I/O such as files, the network and the clipboard behind a trait, and fake it in tests.

The determinism check catches violations, but a view designed this way never needs it.

## Keep cases fast

- Set `opt-level = 2` for dependencies. Unoptimized GPUI is about ten times slower.
- Prefer a small focused view per harness over the whole app window.
- Use `with_max_steps` to match how deep bugs live. Most UI bugs need fewer than 10 steps, and a lower maximum makes every case cheaper.
- Run fewer cases in CI on every push (`GPUI_PROPTEST_CASES=64`) and more on a schedule.

## Keep the regression tests

Every report ends with a regression test. Paste it into the suite before you fix the bug: it fails now, it passes after the fix, and it guards against the bug coming back. Commit `proptest-regressions/` and bolero's `__fuzz__` corpus too.

## Use both engines

Back one `fn harness()` with a proptest test for everyday runs and a short `fuzz!` test, and run long `cargo bolero test` campaigns on a schedule. See [Choosing an engine](./engines/overview.md).

## Turn oracles off deliberately

Each built-in oracle encodes a rule most apps want. When a view breaks one on purpose, turn off just that oracle with a comment saying why:

```rust
// The quit button closes the window by design.
harness.with_window_may_close(true)
```

## Test your invariants

Before trusting a green run, break the view on purpose (flip a condition, drop an update) and check that `search` finds it:

```rust
#[test]
fn the_invariant_catches_a_dropped_update() {
    let report = buggy_harness().search(Config::default()).expect_err("found");
    assert_eq!(report.failure().oracle(), "badge matches unread");
}
```

An invariant that never fails can't protect you.
