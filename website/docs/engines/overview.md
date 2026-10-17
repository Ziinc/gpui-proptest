---
title: Choosing an engine
---

# Choosing an engine

The core of gpui-proptest never imports an engine. Two entry points drive a harness:

- `Harness::run_case(seed, &steps)` runs one structured input.
- `Harness::fuzz(bytes)` decodes one input from raw bytes.

An engine only has to produce those inputs. Two ship with the crate.

| | proptest | bolero |
| --- | --- | --- |
| Feature | `proptest` (default) | `bolero` |
| Entry point | `harness.check(config)` / `harness.search(config)` | `fuzz!(harness)` |
| Generation | weighted strategies per step kind | bytes decoded with `arbitrary` |
| Guidance | none (random) | none under `cargo test`; coverage with libFuzzer, AFL or honggfuzz |
| Shrinking | proptest, then run and pair deletion | bolero byte shrinking, then run and pair deletion |
| Saved failures | `proptest-regressions/` | bolero's corpus and crash files |
| Runs in plain `cargo test` | yes | yes (random bytes) |
| Long fuzzing campaigns | no | yes, with `cargo bolero test` |

## Which to use

- **Start with proptest.** Its strategies weight each kind of step sensibly, for example more clicks than right-clicks, and short readable text. Shrinking gets to minimal cases fast, and saved regressions make failures sticky. It's the right default for every view's test suite.
- **Add bolero** when you want to run a view for minutes or hours. Coverage-guided engines find paths that random sampling rarely reaches, such as a state that needs a specific long sequence. Keep a short `fuzz!` test in `cargo test` and run the long campaign separately.
- **Use both on the same harness.** A harness is engine-neutral, so one `fn harness()` can serve a proptest test and a bolero test.

```rust
fn harness() -> Harness<Editor> { /* ... */ }

#[test]
fn editor_proptest() {
    harness().check(Config::default());
}

#[test]
fn editor_fuzz() {
    gpui_proptest::fuzz!(harness());
}
```

## Writing your own driver

Anything that can produce bytes or a `(u64, Vec<Step<A>>)` can drive a harness. For example, a `cargo-fuzz` target:

```rust title="fuzz/fuzz_targets/editor.rs"
#![no_main]
libfuzzer_sys::fuzz_target!(|bytes: &[u8]| {
    my_crate::tests::harness().fuzz(bytes);
});
```
