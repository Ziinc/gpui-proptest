---
title: Reading a report
---

# Reading a report

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

| Part | Meaning |
| --- | --- |
| `[reset zeroes the count]` | the oracle or invariant that failed |
| `counter.count == 0 failed: 1 != 0` | its message |
| `after step 3` | the step after which it failed; no step means it failed as the window opened |
| `seed 0x0` | the scheduler seed. Seed 0 means the failure doesn't depend on task order |
| `shrunk from 27 to 3 steps` | the length of the case first found, and of the shrunk case |
| trace | each step as it resolved: the target's label and path, the key pressed, or "skipped" with the reason |
| regression test | Rust source that replays exactly this case |

## Skipped steps

A step can be recorded but not run:

- **no eligible target, skipped:** nothing was eligible, for example every target was hidden or covered.
- **did not apply, skipped:** a domain action returned `false`, or a state-machine precondition rejected it.
- **window closed, not run:** an earlier step closed the window, and the harness allows that.

Shrinking usually removes skipped steps. When one survives, the case depends on it, often through the scheduler seed.

## Non-deterministic reports

```text
gpui-proptest: NON-DETERMINISTIC failure; the same seed and steps gave different outcomes, so shrinking was skipped.
first run:  [panic] leaked state hit 6 (after step 20)
second run: [panic] leaked state hit 13 (after step 20)
```

The same input gave different results. See [the determinism check](../concepts/determinism-and-replay.md#the-determinism-check) for the usual causes.
