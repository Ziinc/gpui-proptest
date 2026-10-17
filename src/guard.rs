use crate::target::{self, FRAME_LIMIT, LIVELOCK, STEP};
use gpui::{
    AnyElement, AnyView, App, Bounds, Context, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Render, Window,
};
use std::{cell::Cell, panic::Location};

thread_local! { static FRAMES: Cell<(u64, u32)> = const { Cell::new((0, 0)) }; }

/// Wraps a window's content and fails the case with `livelock` when the window
/// draws [`FRAME_LIMIT`] frames within one step, such as a hover or layout change
/// that undoes itself. [`Harness::new`](crate::Harness::new) and
/// `kit::harness` add it; wrap the content yourself when opening the window with
/// [`Harness::with_window`](crate::Harness::with_window).
///
/// Layout passes straight through to the child.
pub struct FrameGuard {
    child: AnyElement,
}

impl FrameGuard {
    /// Guards `child`, usually the window's root view.
    pub fn new(child: impl IntoElement) -> Self {
        Self {
            child: child.into_any_element(),
        }
    }
}

impl IntoElement for FrameGuard {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for FrameGuard {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let step = STEP.with(Cell::get);
        let frames = match FRAMES.with(Cell::get) {
            (seen, frames) if seen == step => frames + 1,
            _ => 1,
        };
        FRAMES.with(|cell| cell.set((step, frames)));
        if frames > FRAME_LIMIT {
            // Later frames while the panic unwinds count afresh.
            target::begin_step();
            panic!(
                "{LIVELOCK}: the window drew {FRAME_LIMIT} frames in one step; rendering \
                 never settles, such as a hover or layout change that undoes itself"
            );
        }
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

/// A root view that renders its view inside a [`FrameGuard`].
pub(crate) struct Guarded(pub(crate) AnyView);

impl Render for Guarded {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        FrameGuard::new(self.0.clone())
    }
}
