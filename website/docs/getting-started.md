---
sidebar_position: 2
title: Getting started
---

# Getting started

## Install

```toml title="Cargo.toml"
[dev-dependencies]
gpui-proptest = { git = "https://github.com/Ziinc/gpui-proptest" }

# GPUI is far slower unoptimized; this makes test runs about ten times faster.
[profile.dev.package."*"]
opt-level = 2
```

gpui-proptest depends on `gpui-pre =0.3.9`, the GPUI snapshot that gpui-kit 0.7.3 is built against. Your view must use the same GPUI.

| Feature | Default | Adds |
| --- | --- | --- |
| `proptest` | yes | `Harness::check` and `Harness::search` |
| `bolero` | no | the `fuzz!` macro |
| `kit` | no | gpui-kit elements become targets, and `kit::harness` opens windows the way gpui-kit apps do |

On Linux, the `kit` feature links GPUI's platform layer, which needs `libxkbcommon-dev` and `libxkbcommon-x11-dev`.

## Mark targets

Generated clicks, hovers and scrolls only hit elements you mark. Call `.proptest_target()` on any element that has an id:

```rust
use gpui_proptest::ProptestTargetExt;

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap_2()
            .child(
                div()
                    .id("increment")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count += 1;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
            .child(
                div()
                    .id("reset")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count = 0;
                        this.reset_clicked = true;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
    }
}
```

`proptest_target` only records the element's bounds and accessibility facts. It doesn't change layout or behaviour, so leaving it in production code is harmless. You can also put it behind `#[cfg(test)]`.

## Write a harness and an invariant

```rust
use gpui_proptest::{Config, Harness, ensure_eq};

fn harness() -> Harness<Counter> {
    Harness::new(|_window, cx| cx.new(|_| Counter::default())).invariant(
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

`check` runs 256 cases of up to 40 steps each. If a case fails, it panics with the shrunk report. An invariant gets:

- the view entity;
- the targets painted in the last frame;
- the window and the app context.

It returns `Ok(())`, or a `Failure` built with `ensure!`, `ensure_eq!` or `fail!`.

## Run it

```bash
cargo test counter_survives_random_use
```

When a case fails, paste the printed regression test into your test file:

```rust
#[test]
fn regression_0() {
    use gpui_proptest::{ModifierKeys, Step};
    harness().replay(
        0x0,
        &[
            Step::Click { target: 0, modifiers: ModifierKeys::NONE }, // click (increment)
            Step::Click { target: 0, modifiers: ModifierKeys::NONE }, // click (increment)
            Step::Click { target: 1, modifiers: ModifierKeys::NONE }, // click (reset)
        ],
    );
}
```

`replay` panics while the bug exists and passes once it is fixed.

## Next

- [Targets](./concepts/targets.md) and [Steps](./concepts/steps.md) explain what gets clicked and how.
- [Oracles and invariants](./concepts/oracles-and-invariants.md) lists what is checked for free.
- [Engines](./engines/overview.md) compares proptest and bolero.
- [Best practices](./best-practices.md) covers writing invariants that find real bugs.
