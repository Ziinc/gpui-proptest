---
title: Configuration
---

# Configuration

## Harness

| Method | Default | Effect |
| --- | --- | --- |
| `Harness::new(build)` | — | opens `build`'s view in an 800×600 window inside a `FrameGuard` |
| `Harness::with_window(open)` | — | opens the window with your own code; returns `(AnyWindowHandle, Entity<V>)` |
| `Harness::from_state_machine::<S>()` | — | a [model-based](../guides/model-based-testing.md) harness with domain steps only |
| `kit::harness(build)` | — | opens through `gpui_kit::open_window` after `gpui_kit::init` (`kit` feature) |
| `with_setup(f)` | none | runs `f` on each fresh app before the window opens |
| `invariant(name, f)` | none | adds a check run after the window opens and after every step |
| `with_keys(keys)` | tab, shift-tab, enter, escape, space, backspace, arrows | the keys `Press` chooses from |
| `with_domain(f)` | none | adds domain steps; `f` returns `false` when the action doesn't apply |
| `with_target_source(s)` | `proptest_target`s, plus gpui-kit with `kit` | adds a target source |
| `with_ui_steps(bool)` | `true` (`false` for state machines) | generate clicks, keys, text and clock steps |
| `with_disabled_is_inert(bool)` | `true` | the disabled-is-inert oracle |
| `with_focus_is_visible(bool)` | `true` | the focus-is-visible oracle |
| `with_unique_ids(bool)` | `true` | the unique-ids oracle |
| `with_window_may_close(bool)` | `false` | allow steps to close the window |

## Running cases

| Method | Engine | Returns |
| --- | --- | --- |
| `run_case(seed, &steps)` | none | `Outcome` |
| `replay(seed, &steps)` | none | panics if the case fails |
| `decode(bytes)` | none | `(u64, Vec<Step<A>>)` |
| `fuzz(bytes)` | none | panics with a `Report` if the case fails |
| `check(config)` | proptest | panics with the shrunk `Report` |
| `search(config)` | proptest | `Result<(), Box<Report>>` |
| `fuzz!(harness)` | bolero | panics with the minimized `Report` |

## `Config` (proptest)

| Builder | Default |
| --- | --- |
| `with_cases(n)` | 256 |
| `with_max_steps(n)` | 40 |
| `with_max_shrink_iters(n)` | 2,000 |
| `with_max_shrink_time(d)` | 60 s |
| `with_text(regex)` | `[a-zA-Z0-9 ]{0,8}` |
| `with_check_determinism(bool)` | `true` |
| `with_persist_failures(bool)` | `true` |

## Environment

| Variable | Effect |
| --- | --- |
| `GPUI_PROPTEST_CASES` | overrides the case count of every `Config` |
| `GPUI_PROPTEST_SEED` | fixes proptest's generator seed |
| `SEED` | fixes the scheduler seed of every case |

## Limits

| Limit | Value | Oracle |
| --- | --- | --- |
| scheduler ticks per step | 100,000 | `livelock` |
| frames per step | 1,000 | `livelock` |
| steps per fuzz input | 64 (`MAX_FUZZ_STEPS`) | — |
