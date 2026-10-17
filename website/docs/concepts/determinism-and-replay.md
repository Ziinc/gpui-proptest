---
title: Determinism and replay
---

# Determinism and replay

Every case runs on a fresh `TestAppContext` built from `TestDispatcher::new(seed)`. This is the same machinery as `#[gpui::test(iterations, seed)]`. GPUI's scheduler picks the order of ready tasks from the seed, so a `(seed, steps)` input replays exactly:

```rust
let first = harness().run_case(42, &steps);
let second = harness().run_case(42, &steps);
assert_eq!(first, second);
```

## The determinism check

Before shrinking, the proptest engine runs the first failing case a second time. If the outcome differs, it reports a **non-deterministic** failure with both traces and skips shrinking, because shrinking a flaky failure finds noise. The usual causes are:

- state shared between cases through a `static`, a `thread_local!` or a global cache;
- reading the real clock (`Instant::now`, `SystemTime`) instead of GPUI's executor clock;
- spawning real OS threads, or doing blocking I/O that races with the test scheduler;
- iterating a `HashMap` whose iteration order affects behaviour.

Turn the check off with `Config::with_check_determinism(false)` only if you understand where the variation comes from.

## Replay

`Harness::replay(seed, &steps)` runs one case and panics with the failure and trace. Every report ends with a regression test that calls it. Keep those tests: they pin the exact sequence that once broke the view.

`Harness::run_case(seed, &steps)` returns the `Outcome` instead of panicking. Use it to assert on the trace in your own tests.

## Environment variables

| Variable | Effect |
| --- | --- |
| `GPUI_PROPTEST_CASES` | overrides the number of cases for every `Config` |
| `GPUI_PROPTEST_SEED` | fixes the proptest generator seed, so a CI run reproduces locally |
| `SEED` | fixes the scheduler seed of every case (the same variable `#[gpui::test]` reads) |

proptest saves failures to `proptest-regressions/<test file>.txt` next to the test and replays them first on later runs. Commit that directory.
