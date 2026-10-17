---
title: Targets
---

# Targets

A target is an element that generated clicks, hovers and scrolls may hit. After every frame, the runner collects the targets from each **target source**:

- **`proptest_target()`**: any element with an id. It records bounds, visibility, the accessibility role and label, the disabled state, and a fingerprint of checked, selected, expanded and value state.
- **gpui-kit** (`kit` feature): every element gpui-kit observes with `.test_support()`, including checkboxes, switches, tabs, sliders, dialogs and menus. These targets also report focus.
- **Your own sources**: implement [`TargetSource`](../guides/custom-windows-and-targets.md#custom-target-sources).

When two sources report the same element path, the first source wins.

## Order and choice

Targets are sorted top to bottom, then left to right, then by path. A step stores a number, not an element: "click the Nth eligible target". N wraps around the eligible list, so any number picks something. This is what makes shrinking safe. Deleting an earlier step never leaves a later step pointing at an element that no longer exists.

## Eligibility

| Step | Eligible targets |
| --- | --- |
| `Click` | visible, disabled included (so the disabled-is-inert oracle can check them) |
| `DoubleClick`, `RightClick`, `Hover`, `Scroll` | visible and enabled |

A target is visible when some part of it was painted inside the window and its clip. A step with no eligible target is recorded as skipped.

## Covered targets

GPUI sends a click to every element under the pointer, down to the first one that blocks the mouse with `occlude()`. Before a pointer step, the chosen `proptest_target` is hit-tested:

1. Its centre is checked first, then a 3×3 grid of points across it.
2. The step aims at the first point that nothing covers.
3. A target covered everywhere passes the pick to the next eligible target.

This keeps the trace honest: a click is never credited to a target whose click went to a modal scrim.

gpui-kit targets are aimed at their centre without this check, because gpui-kit's test snapshots don't expose hitboxes.

## Ids must be unique

Two `proptest_target`s with the same id under one parent share GPUI element state: hover, focus, scroll and the target registration. Only one of them could ever be chosen. The `unique-ids` oracle fails the case when this happens. See [Oracles](./oracles-and-invariants.md).
