---
slug: /
sidebar_position: 1
title: Introduction
---

# gpui-proptest

gpui-proptest tests [GPUI](https://www.gpui.rs) views the way a careless user would. It clicks, hovers, scrolls, types and presses keys at random in a real GPUI window, and after every step it checks your invariants and a set of built-in oracles. When something fails, it shrinks the sequence to the shortest one that still fails, then prints the trace and a regression test you can paste.

```text
gpui-proptest: [reset zeroes the count] counter.count == 0 failed: 1 != 0 (after step 3) (seed 0x0, shrunk from 27 to 3 steps)

trace:
seed 0x0
   1. click (increment)
   2. click (increment)
   3. click (reset)
```

## What it finds

- **Broken invariants:** your rules about the view's state stop holding, such as "reset zeroes the count" or "the checkbox matches the model".
- **Panics** anywhere: in a listener, in render, in a spawned task, or in your own checks.
- **Livelocks:** a task that never stops rescheduling itself, or a hover or layout change that undoes itself and redraws forever.
- **Accessibility and wiring bugs:** a disabled control that still changes state, focus on an element that isn't visible, two elements sharing an id, or a window that closes when it shouldn't.
- **Non-determinism:** the same input producing a different result.

## How it works

1. A **harness** opens your view in a fresh `TestAppContext`. The context is seeded, so the order of GPUI's tasks replays exactly.
2. An **engine** generates inputs. An input is a scheduler seed plus a list of abstract **steps**, such as "click the 3rd eligible target" or "press a key".
3. The runner resolves each step against the **targets** painted in the last frame, performs it, waits for the app to settle, renders, and checks every oracle.
4. When a case fails, the engine shrinks the input. gpui-proptest then deletes runs and pairs of steps until nothing more can go, and prints the report.

Two engines ship with the crate:

| Engine | Feature | Best for |
| --- | --- | --- |
| [proptest](./engines/proptest.md) | `proptest` (default) | everyday `cargo test` runs with strong shrinking and saved regressions |
| [bolero](./engines/bolero.md) | `bolero` | random-byte runs under `cargo test`, and coverage-guided fuzzing with libFuzzer, AFL or honggfuzz |

Start with [Getting started](./getting-started.md).
