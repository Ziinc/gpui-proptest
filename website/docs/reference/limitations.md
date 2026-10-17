---
title: Limitations
---

# Limitations

- **gpui-kit targets aren't hit-tested.** They are aimed at their centre, because gpui-kit's test snapshots don't expose hitboxes. Behind an open modal, a click is credited to the control even though the modal takes it. `proptest_target`s are hit-tested.
- **Custom windows need a `FrameGuard`.** A window opened with `Harness::with_window` only catches render loops if its content is wrapped in `FrameGuard::new(..)`. `Harness::new` and `kit::harness` add it.
- **`window.root::<V>()` is the guard.** Under `Harness::new` the window's root view is the frame guard, so `window.root::<V>()` returns `None`. Use the entity the harness passes to invariants.
- **One window per case.** Targets are read from the harness's window. Popups opened as separate windows aren't targets.
- **No drags or resizes yet.** Steps cover clicks, hovers, scrolls, keys, text and the clock. Drag-and-drop and window resizes need domain actions for now.
