---
title: Custom windows and targets
---

# Custom windows and targets

## Opening the window yourself

`Harness::new(build)` opens an 800×600 window with your view inside a `FrameGuard`. Use `Harness::with_window` for anything else, such as a different size, window options, or a root built by your app's own code:

```rust
use gpui_proptest::{FrameGuard, Harness};

Harness::with_window(|cx| {
    let mut view = None;
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(1280.), px(800.)),
        ))),
        ..Default::default()
    };
    let window = cx
        .open_window(options, |window, cx| {
            let editor = cx.new(|cx| Editor::new(window, cx));
            view = Some(editor.clone());
            cx.new(|_| AppRoot { editor })
        })
        .unwrap();
    (window.into(), view.unwrap())
})
.with_setup(|cx| my_app::init(cx))
```

- `with_setup` runs on every fresh app before the window opens. Use it for globals, themes, keymaps and settings.
- The returned entity is the view your invariants receive. It doesn't have to be the window's root.
- Render the content inside `FrameGuard::new(..)` so a render loop fails with `livelock` instead of hanging:

```rust
impl Render for AppRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        FrameGuard::new(self.editor.clone())
    }
}
```

## Custom target sources

A `TargetSource` returns targets for elements gpui-proptest can't see on its own, for example cells of a canvas-drawn grid:

```rust
use gpui_proptest::{TargetInfo, TargetSource};

struct GridCells;

impl TargetSource for GridCells {
    fn targets(&self, window: &Window, cx: &App) -> Vec<TargetInfo> {
        let grid = window.root::<Grid>().flatten().unwrap();
        grid.read(cx)
            .cells()
            .map(|cell| {
                TargetInfo::new(vec![ElementId::from(("cell", cell.index))], cell.bounds)
                    .with_label(Some(cell.label.clone()))
                    .with_disabled(cell.locked)
                    .with_state(format!("value={:?}", cell.value))
            })
            .collect()
    }
}

harness.with_target_source(GridCells)
```

The builders on `TargetInfo` are:

- `with_visible`, `with_disabled`, `with_focused`, `with_role`, `with_label`;
- `with_state`: a fingerprint the disabled-is-inert oracle compares before and after a click on a disabled target;
- `with_click_point`: where pointer steps aim. `None` marks the target as covered.

## Targets without accessibility info

`proptest_target()` reads the role, label and disabled state from the element's accessibility info. An element that declares none can still be marked disabled for eligibility and the disabled-is-inert oracle:

```rust
div().id("upload").on_click(...).proptest_target().disabled(self.uploading)
```
