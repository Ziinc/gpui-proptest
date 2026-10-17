---
title: gpui-kit
---

# gpui-kit

With the `kit` feature, gpui-proptest understands [GPUI Kit](https://gpui-kit.com) apps:

```toml title="Cargo.toml"
[dev-dependencies]
gpui-proptest = { git = "https://github.com/Ziinc/gpui-proptest", features = ["kit"] }
gpui-kit = { version = "=0.7.3", features = ["test-support"] }
```

## Open the view like an app does

`kit::harness` opens the window through `gpui_kit::open_window` at 800×600, after `gpui_kit::init`. Themes, dialogs, notifications and the Root overlay layers all work as they do in the app.

```rust
use gpui_kit::test::TestWindowExt as _;
use gpui_proptest::{ensure_eq, kit};

fn harness() -> gpui_proptest::Harness<SettingsForm> {
    kit::harness(|_, cx| cx.new(|_| SettingsForm::default()))
        .invariant("agree matches the model", |form, _targets, window, cx| {
            ensure_eq!(window.find("agree").checked(), Some(form.read(cx).agreed));
            Ok(())
        })
}
```

## Targets for free

Every element gpui-kit observes with `.test_support()` is a target, with no `proptest_target()` calls. That includes checkboxes, switches, radios, tabs, sliders, selects, dialogs and menus. Each target carries:

- its role and label;
- whether it is disabled and whether it has focus;
- its checked, indeterminate, selected, expanded and value state.

That state feeds two oracles:

- **disabled-is-inert:** clicking a disabled control must not change any control's state.
- **focus-is-visible:** the focused control must be visible.

## Reading state in invariants

Use gpui-kit's `TestWindowExt` inside invariants to read what is rendered:

```rust
.invariant("save is disabled while invalid", |form, _, window, cx| {
    let valid = form.read(cx).is_valid();
    ensure_eq!(window.find("save").disabled(), Some(!valid));
    Ok(())
})
```

## Notes

- The window content is wrapped in a `FrameGuard`, so render loops in kit components fail with `livelock` instead of hanging.
- gpui-kit targets are aimed at their centre and are not hit-tested for overlays. With a modal open, clicks on controls behind it are recorded as clicks on those controls even though the modal takes them. If that matters, add an invariant that checks the modal's own state.
