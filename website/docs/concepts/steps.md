---
title: Steps
---

# Steps

An input is a scheduler seed plus a list of steps. `Step<A>` is a plain enum that derives `Clone`, `Debug`, `PartialEq` and `arbitrary::Arbitrary`, so any engine can build one.

| Step | What happens |
| --- | --- |
| `Click { target, modifiers }` | move the pointer, render, then press and release the left button |
| `DoubleClick { target }` | two clicks, the second with a click count of 2 |
| `RightClick { target }` | press and release the right button |
| `Hover { target }` | move the pointer over the target |
| `Scroll { target, lines }` | move over the target, render, then scroll by `lines` |
| `Press { key }` | press the `key`th harness key (wraps) |
| `Type { text }` | type `text` one character at a time into whatever has focus |
| `AdvanceClock { millis }` | advance GPUI's fake clock, firing due timers |
| `Domain(A)` | run your domain action (see [Model-based testing](../guides/model-based-testing.md)) |

After each step the runner:

1. runs tasks until nothing is runnable (failing with `livelock` after 100,000 scheduler ticks);
2. renders a frame;
3. checks every oracle and invariant.

## Keys

`Press` picks from the harness keys. The default list is `tab`, `shift-tab`, `enter`, `escape`, `space`, `backspace` and the four arrows. Replace it with GPUI keystroke strings:

```rust
harness.with_keys(["cmd-z", "cmd-shift-z", "enter", "escape", "a"])
```

A space separates keystrokes in a chord sequence: `"ctrl-k ctrl-c"`. `with_keys` panics on a key GPUI wouldn't recognise, such as `"ctrl-banana"`. GPUI would otherwise accept any word as a key and press nothing.

## Text

The proptest engine draws typed text from the regex `[a-zA-Z0-9 ]{0,8}`. Change it with `Config::with_text`, for example `with_text("[a-z@.]{0,20}")` for email fields. The bolero engine types any Unicode that `arbitrary` produces.

## Domain steps

`with_domain` adds actions that drive your view directly, such as "load fixture 3" or "server pushes an update". The handler returns `false` when the action doesn't apply, and the step is recorded as skipped:

```rust
#[derive(Clone, Debug, Arbitrary)]
enum Server { Push(u8), Disconnect }

harness.with_domain(|action: &Server, view, window, cx| {
    view.update(cx, |list, cx| list.apply_server(action, window, cx));
    true
})
```

Turn generic UI steps off with `with_ui_steps(false)` when only domain steps make sense.
