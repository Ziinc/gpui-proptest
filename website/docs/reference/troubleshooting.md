---
title: Troubleshooting
---

# Troubleshooting

## `livelock: tasks were still runnable after 100000 scheduler ticks`

A task reschedules itself forever without waiting on a timer, for example a loop that yields and spawns again. In a real app that is a CPU spin. Wait on `cx.background_executor().timer(..)` between iterations, or stop the loop when there is nothing to do.

## `livelock: the window drew 1000 frames for one input event or timer`

Rendering never settles. Usually a hover, size or layout change triggers a state change that undoes it, so the next frame flips it back. In a real app that shows as flicker. Animations that draw one frame per timer are fine: frames are counted per input event and per clock instant. Look for `on_hover`, `on_resize` or measured-layout callbacks that call `cx.notify()` unconditionally.

## `unique-ids: … is the id of more than one target`

Two siblings share an id. Make ids unique, for example by deriving them from the item's key. If the duplicate is intended, turn the oracle off with `with_unique_ids(false)`.

## `window-closed: the window closed`

A step closed the window. If that's intended, such as a close button, use `with_window_may_close(true)`. Otherwise you've found a bug.

## `NON-DETERMINISTIC failure`

The same input gave different outcomes. See [the determinism check](../concepts/determinism-and-replay.md#the-determinism-check).

## `with_keys: "…" is not a key GPUI recognises`

Use GPUI keystroke syntax: modifiers `ctrl`, `alt`, `shift`, `cmd` (or `super`, `win`), `fn`, or `secondary`; then a single character or a named key (`enter`, `escape`, `tab`, `space`, `backspace`, `delete`, `home`, `end`, `pageup`, `pagedown`, arrows, `f1`–`f24`).

## `gpui-proptest aborted: the domain action type's Arbitrary impl rejects almost every input`

A hand-written `Arbitrary` impl returns errors for most byte strings. Derive `Arbitrary`, or make the impl fall back to a default value.

## Every step is skipped

No target is eligible. Check that:

- the elements have ids and `.proptest_target()`, or that the `kit` feature is on for gpui-kit elements;
- they are inside the window bounds;
- nothing covers them.

## Linking fails with `-lxkbcommon`

The `kit` feature links GPUI's Linux platform. Install `libxkbcommon-dev` and `libxkbcommon-x11-dev`.

## Tests are slow

Add `[profile.dev.package."*"] opt-level = 2`, lower `with_max_steps`, and test focused views. See [Best practices](../best-practices.md#keep-cases-fast).
