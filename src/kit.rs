//! gpui-kit integration (`kit` feature).
//!
//! Every gpui-kit element observed with `.test_support()` becomes a target, with
//! the role, label, disabled, focus and checked state that gpui-kit records. That
//! feeds the focus-is-visible and disabled-is-inert oracles without any
//! `proptest_target` calls.

use crate::{Harness, TargetInfo, TargetSource, guard::Guarded};
use gpui::{
    App, AppContext as _, Bounds, Entity, Render, Window, WindowBounds, WindowOptions, point, px,
    size,
};
use gpui_base::test_support::{self, ElementSnapshot};

/// Targets from gpui-kit's test-support observation. Added to every harness when
/// the `kit` feature is on.
pub struct KitTargets;

impl TargetSource for KitTargets {
    fn targets(&self, window: &Window, _: &App) -> Vec<TargetInfo> {
        test_support::snapshots(window)
            .iter()
            .map(target_info)
            .collect()
    }
}

fn target_info(snapshot: &ElementSnapshot) -> TargetInfo {
    TargetInfo::new(snapshot.path().to_vec(), snapshot.bounds())
        .with_visible(snapshot.visible())
        .with_disabled(snapshot.disabled() == Some(true))
        .with_focused(snapshot.focused())
        .with_role(snapshot.role())
        .with_label(snapshot.label().map(str::to_owned))
        .with_state(format!(
            "checked={:?} indeterminate={:?} selected={:?} expanded={:?} value={:?}",
            snapshot.checked(),
            snapshot.indeterminate(),
            snapshot.selected(),
            snapshot.expanded(),
            snapshot.value()
        ))
}

/// A harness whose window opens through `gpui_kit::open_window` after
/// `gpui_kit::init`, at 800×600, as gpui-kit applications and tests do. The
/// content is wrapped in a [`FrameGuard`](crate::FrameGuard).
pub fn harness<V: Render + 'static>(
    build: impl Fn(&mut Window, &mut App) -> Entity<V> + 'static,
) -> Harness<V> {
    Harness::with_window(move |cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.), px(0.)),
                size(px(800.), px(600.)),
            ))),
            ..Default::default()
        };
        let mut content = None;
        let (window, _) = gpui_kit::open_window(options, cx, |window, cx| {
            let view = build(window, cx);
            content = Some(view.clone());
            cx.new(|_| Guarded(view.into()))
        })
        .expect("open gpui-kit window");
        (window, content.expect("open_window ran its build closure"))
    })
    .with_setup(gpui_kit::init)
}
