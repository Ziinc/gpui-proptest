---
title: bolero engine
---

# bolero engine

[bolero](https://camshaft.github.io/bolero/) runs the same test with different backends: random bytes under `cargo test`, and libFuzzer, AFL or honggfuzz under `cargo bolero test`. Enable it with the `bolero` feature:

```toml title="Cargo.toml"
[dev-dependencies]
gpui-proptest = { git = "https://github.com/Ziinc/gpui-proptest", features = ["bolero"] }
```

## Write a fuzz test

```rust
#[test]
fn settings_fuzz() {
    gpui_proptest::fuzz!(harness());
}
```

Pass bolero builder calls after the harness:

```rust
#[test]
fn settings_fuzz_longer() {
    gpui_proptest::fuzz!(harness(), |check| check
        .with_iterations(10_000)
        .with_max_len(512));
}
```

## Run it

```bash
# Random bytes, as part of the normal test suite
cargo test settings_fuzz

# Coverage-guided, until you stop it
cargo install cargo-bolero
cargo bolero test settings_fuzz --engine libfuzzer
```

`cargo bolero test` keeps a corpus of interesting inputs and its crash files in a `__fuzz__` directory next to the test file. `cargo test` replays both before generating new inputs, so commit them.

## How bytes become a case

`Harness::fuzz(bytes)` decodes an input with `arbitrary`:

1. The first 8 bytes are the scheduler seed. Missing bytes read as zeros.
2. The remaining bytes decode into steps until they run out or reach `MAX_FUZZ_STEPS` (64).
3. A harness without UI steps, such as `from_state_machine`, decodes domain actions only.

Decoding never fails, so every byte string is a valid case. `Harness::decode(bytes)` shows what a byte string decodes to.

## How failures shrink

1. bolero shrinks the failing bytes.
2. When bolero stops, `fuzz!` takes the last failing case and runs gpui-proptest's run and pair deletion over its steps. This is the same pass the proptest engine uses.
3. The panic carries the usual report: the oracle, the trace and a regression test.

Under `cargo bolero test`, the fuzzer aborts on the first crash instead. The crash message holds that input's report, truncated to the failing step. Replay the crash file with `cargo test` to get the minimized report.

## Tips

- **Keep the in-suite test short.** `with_iterations(200)` gives smoke coverage in `cargo test`, and long campaigns run separately.
- **Share the harness.** The same `fn harness()` should back both the proptest and the bolero test.
- **Fuzz state machines.** `Harness::from_state_machine::<Spec>()` works with `fuzz!` unchanged, and coverage guidance is good at reaching deep model states.
