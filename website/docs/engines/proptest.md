---
title: proptest engine
---

# proptest engine

The default engine. It is enabled by the `proptest` feature, which is on by default.

```rust
use gpui_proptest::Config;

#[test]
fn settings_survive_random_use() {
    harness().check(Config::default());
}
```

- `check(config)` panics with the report when a case fails.
- `search(config)` returns `Result<(), Box<Report>>`, so you can assert on the failure. This is useful for testing your own oracles.

```rust
let report = harness().search(Config::default()).expect_err("the bug is found");
assert_eq!(report.failure().oracle(), "reset zeroes the count");
assert!(report.steps().len() <= 3);
```

## Configuration

`Config::default()` is a good starting point. Each setting has a `with_*` builder:

| Setting | Default | Builder |
| --- | --- | --- |
| cases | 256 (`GPUI_PROPTEST_CASES` overrides) | `with_cases(n)` |
| steps per case | up to 40 | `with_max_steps(n)` |
| shrink attempts | 2,000 | `with_max_shrink_iters(n)` |
| shrink time | 60 s | `with_max_shrink_time(duration)` |
| typed text | `[a-zA-Z0-9 ]{0,8}` | `with_text(regex)` |
| determinism check | on | `with_check_determinism(bool)` |
| save failures | on | `with_persist_failures(bool)` |

## How cases are generated

Each step kind has a weight:

| Step | Weight |
| --- | --- |
| Click | 30 (1 in 7 with random modifiers) |
| Press | 20 |
| Type | 10 |
| Hover | 8 |
| DoubleClick, Scroll, AdvanceClock | 5 each |
| RightClick | 3 |
| Domain (when the harness has one) | 25 |

Domain actions are decoded from up to 64 random bytes with `arbitrary`. The same `#[derive(Arbitrary)]` serves both engines.

## How failures shrink

1. proptest shrinks the step list and every step's arguments: it removes steps, lowers target numbers and shortens text.
2. Before shrinking, the failing case is replayed once. A different outcome is reported as non-deterministic instead.
3. proptest removes steps one at a time in a single pass. That stalls when two steps must go together, such as an increment and the decrement that cancels it. gpui-proptest then deletes contiguous runs of every length, and every pair of steps for traces of 16 steps or fewer, until nothing more can go.
4. It finally tries scheduler seed 0, which shows whether the failure depends on task order at all.

Only candidates that fail with the **same oracle** are kept, so a shrink never swaps your bug for a different one.

## Saved regressions

Failures are saved to `proptest-regressions/` next to the test file and run first on the next `cargo test`. Commit the directory. Delete an entry once the regression test from the report is in your suite.
