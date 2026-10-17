use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId,
    IntoElement, LayoutId, Pixels, Point, Window, WindowId, point, px,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    rc::{Rc, Weak},
};

/// An element the runner can act on, as painted in the last frame.
#[derive(Clone)]
pub struct TargetInfo {
    path: Vec<ElementId>,
    bounds: Bounds<Pixels>,
    visible: bool,
    disabled: bool,
    focused: Option<bool>,
    role: Option<gpui::Role>,
    label: Option<String>,
    state: String,
    click_point: Option<Point<Pixels>>,
    /// The probe hitbox of a `proptest_target`, hit-tested only when the target is
    /// chosen: testing every target each step is quadratic in the hitbox count.
    probe: Option<Rc<Hitbox>>,
}

impl PartialEq for TargetInfo {
    fn eq(&self, other: &Self) -> bool {
        (
            &self.path,
            self.bounds,
            self.visible,
            self.disabled,
            self.focused,
            self.role,
            &self.label,
            &self.state,
            self.click_point,
        ) == (
            &other.path,
            other.bounds,
            other.visible,
            other.disabled,
            other.focused,
            other.role,
            &other.label,
            &other.state,
            other.click_point,
        )
    }
}

impl TargetInfo {
    /// Creates target facts for a custom [`TargetSource`].
    pub fn new(path: Vec<ElementId>, bounds: Bounds<Pixels>) -> Self {
        Self {
            path,
            bounds,
            visible: true,
            disabled: false,
            focused: None,
            role: None,
            label: None,
            state: String::new(),
            click_point: Some(bounds.center()),
            probe: None,
        }
    }

    /// Sets whether any part of the target was painted inside the window.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Sets whether the target is disabled.
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets whether the target holds focus, when the source knows.
    pub fn with_focused(mut self, focused: Option<bool>) -> Self {
        self.focused = focused;
        self
    }

    /// Sets the accessibility role.
    pub fn with_role(mut self, role: Option<gpui::Role>) -> Self {
        self.role = role;
        self
    }

    /// Sets the accessible label.
    pub fn with_label(mut self, label: Option<String>) -> Self {
        self.label = label;
        self
    }

    /// Sets a fingerprint of user-visible state (checked, selected, value).
    /// The disabled-is-inert oracle compares fingerprints before and after a click.
    pub fn with_state(mut self, state: String) -> Self {
        self.state = state;
        self
    }

    /// Sets where pointer steps aim, or `None` when another element covers the
    /// whole target. Defaults to the centre of the bounds. A `proptest_target` is
    /// also hit-tested when chosen, and aimed at a point no overlay covers.
    pub fn with_click_point(mut self, click_point: Option<Point<Pixels>>) -> Self {
        self.click_point = click_point;
        self
    }

    /// Where pointer steps aim, or `None` when the target is covered everywhere.
    pub fn click_point(&self) -> Option<Point<Pixels>> {
        self.click_point
    }

    /// The GPUI element path, root first.
    pub fn path(&self) -> &[ElementId] {
        &self.path
    }

    /// Window-relative bounds from the last frame.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// Whether any part of the target was painted inside the window.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether the target is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Whether the target holds focus, or `None` when the source cannot tell.
    pub fn focused(&self) -> Option<bool> {
        self.focused
    }

    /// The accessibility role, if the element declares one.
    pub fn role(&self) -> Option<gpui::Role> {
        self.role
    }

    /// The accessible label, if any.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The user-visible state fingerprint.
    pub fn state(&self) -> &str {
        &self.state
    }

    /// A short name for reports: the label when there is one, else the last path segment,
    /// followed by the full path.
    pub fn name(&self) -> String {
        let path = self
            .path
            .iter()
            .filter(|id| !matches!(id, ElementId::View(_)))
            .map(|id| format!("{id}"))
            .collect::<Vec<_>>()
            .join("/");
        match &self.label {
            Some(label) => format!("{label:?} ({path})"),
            None => format!("({path})"),
        }
    }
}

impl fmt::Debug for TargetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// Supplies the elements a run may act on, read after every settled frame.
pub trait TargetSource: 'static {
    fn targets(&self, window: &Window, cx: &App) -> Vec<TargetInfo>;
}

/// Elements wrapped with [`ProptestTargetExt::proptest_target`]. The default source.
pub struct ObservedTargets;

impl TargetSource for ObservedTargets {
    fn targets(&self, window: &Window, _: &App) -> Vec<TargetInfo> {
        let key = window.window_handle().window_id();
        REGISTRY.with(|registry| {
            registry
                .borrow()
                .get(&key)
                .map(|entries| {
                    entries
                        .values()
                        .filter_map(Weak::upgrade)
                        .map(|entry| {
                            let mut facts = entry.facts.borrow().clone();
                            facts.probe = entry.hitbox.borrow().clone();
                            facts
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
    }
}

/// Where to aim at `target` in the current frame: its click point, refined by
/// a hit test when it has a probe. `None` when it is covered everywhere.
pub(crate) fn aim(target: &TargetInfo, window: &Window) -> Option<Point<Pixels>> {
    match &target.probe {
        Some(hitbox) => uncovered_point(hitbox, window),
        None => target.click_point,
    }
}

/// The centre of the hitbox if nothing covers it, else the first uncovered point
/// on a 3×3 grid. GPUI routes a click to every hitbox under the pointer down to
/// the first one that blocks the mouse (`occlude`), so a covered point would send
/// the click to whatever covers it.
fn uncovered_point(hitbox: &Hitbox, window: &Window) -> Option<Point<Pixels>> {
    let bounds = hitbox.bounds.intersect(&hitbox.content_mask.bounds);
    if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
        return None;
    }
    let at = |x: f32, y: f32| {
        point(
            bounds.origin.x + bounds.size.width * x,
            bounds.origin.y + bounds.size.height * y,
        )
    };
    std::iter::once(at(0.5, 0.5))
        .chain(
            [1. / 6., 0.5, 5. / 6.]
                .into_iter()
                .flat_map(|y| [1. / 6., 0.5, 5. / 6.].map(|x| at(x, y))),
        )
        .find(|position| hitbox.is_hovered_at(*position, window))
}

/// Orders targets top to bottom, then left to right, then by path, so a target
/// choice resolves to the same element on every run.
/// Paths are formatted only to break ties, which are rare.
pub(crate) fn sort(targets: &mut [TargetInfo]) {
    targets.sort_by(|a, b| {
        (a.bounds.origin.y, a.bounds.origin.x)
            .partial_cmp(&(b.bounds.origin.y, b.bounds.origin.x))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                if a.path == b.path {
                    std::cmp::Ordering::Equal
                } else {
                    format!("{:?}", a.path).cmp(&format!("{:?}", b.path))
                }
            })
    });
}

type Registry = HashMap<WindowId, HashMap<GlobalElementId, Weak<Registration>>>;
thread_local! { static REGISTRY: RefCell<Registry> = RefCell::new(HashMap::new()); }

thread_local! { static STEP: Cell<u64> = const { Cell::new(0) }; }

/// Prefixes the panic a render loop raises, so the harness reports it as `livelock`.
pub(crate) const LIVELOCK: &str = "livelock";

/// How often one target may lay out within a single step before the step counts
/// as a render loop. A step draws a handful of frames; a feedback loop draws forever.
pub(crate) const FRAME_LIMIT: u32 = 1_000;

/// Starts counting frames for a new step.
pub(crate) fn begin_step() {
    STEP.with(|step| step.set(step.get() + 1));
}

thread_local! { static DUPLICATES: RefCell<Vec<Vec<ElementId>>> = const { RefCell::new(Vec::new()) }; }

/// Paths that more than one target used in the same frame, since the last call.
pub(crate) fn take_duplicates() -> Vec<Vec<ElementId>> {
    DUPLICATES.with(|duplicates| std::mem::take(&mut *duplicates.borrow_mut()))
}

thread_local! { static DUPLICATES: RefCell<Vec<Vec<ElementId>>> = const { RefCell::new(Vec::new()) }; }

/// Paths that more than one target used in the same frame, since the last call.
pub(crate) fn take_duplicates() -> Vec<Vec<ElementId>> {
    DUPLICATES.with(|duplicates| std::mem::take(&mut *duplicates.borrow_mut()))
}

/// Forgets every registration. Each case builds a new app whose window ids may repeat.
pub(crate) fn clear_registry() {
    REGISTRY.with(|registry| registry.borrow_mut().clear());
    take_duplicates();
    begin_step();
}

/// A target's registration, owned by GPUI element state so it lives exactly as long
/// as the element keeps being painted.
#[doc(hidden)]
pub struct Registration {
    window: WindowId,
    global_id: GlobalElementId,
    facts: RefCell<TargetInfo>,
    hitbox: RefCell<Option<Rc<Hitbox>>>,
    /// Prepaints since the last paint. Elements with the same id share element
    /// state, so a second prepaint before the first paint means a duplicate id.
    unpainted: Cell<u32>,
    /// The step this target was last laid out in, and how often within it.
    frames: Cell<(u64, u32)>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = REGISTRY.try_with(|registry| {
            if let Ok(mut registry) = registry.try_borrow_mut()
                && let Some(entries) = registry.get_mut(&self.window)
            {
                entries.remove(&self.global_id);
            }
        });
    }
}

/// Adds [`proptest_target`](ProptestTargetExt::proptest_target) to every element.
pub trait ProptestTargetExt: IntoElement + Sized {
    /// Makes this element a target for generated clicks, hovers and scrolls.
    ///
    /// The element must have an id. Its role, label and disabled state are read from
    /// the accessibility information the element writes; [`Target::disabled`] marks
    /// an element disabled when it declares no accessibility state.
    fn proptest_target(self) -> Target<Self::Element> {
        let inner = self.into_element();
        assert!(
            Element::id(&inner).is_some(),
            "proptest_target requires an element with an id; call .id(..) first"
        );
        Target {
            inner,
            disabled: false,
        }
    }
}

impl<E: IntoElement> ProptestTargetExt for E {}

/// A transparent wrapper that records its element's facts each frame.
pub struct Target<E> {
    inner: E,
    disabled: bool,
}

impl<E> Target<E> {
    /// Marks the target disabled for target eligibility and the disabled-is-inert oracle.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<E: Element> IntoElement for Target<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: Element> Element for Target<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = (E::PrepaintState, Rc<Registration>);

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.inner)
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let global_id = id.expect("targets have an id");
        let role = self.inner.a11y_role();
        let mut node = gpui::accesskit::Node::new(role.unwrap_or(gpui::Role::Unknown));
        self.inner.write_a11y_info(&mut node);
        let state = format!(
            "toggled={:?} selected={:?} expanded={:?} value={:?}",
            node.toggled(),
            node.is_selected(),
            node.is_expanded(),
            node.value()
        );
        let facts = TargetInfo::new(global_id.to_vec(), bounds)
            .with_visible(false)
            .with_disabled(self.disabled || node.is_disabled())
            .with_role(role)
            .with_label(node.label().map(str::to_owned))
            .with_state(state);
        let registration =
            window.with_element_state(global_id, |state: Option<Rc<Registration>>, window| {
                let registration = state.unwrap_or_else(|| {
                    Rc::new(Registration {
                        window: window.window_handle().window_id(),
                        global_id: global_id.clone(),
                        facts: RefCell::new(facts.clone()),
                        hitbox: RefCell::new(None),
                        unpainted: Cell::new(0),
                        frames: Cell::new((0, 0)),
                    })
                });
                *registration.facts.borrow_mut() = facts;
                REGISTRY.with(|registry| {
                    registry
                        .borrow_mut()
                        .entry(registration.window)
                        .or_default()
                        .insert(global_id.clone(), Rc::downgrade(&registration));
                });
                (registration.clone(), registration)
            });
        let step = STEP.with(Cell::get);
        let frames = match registration.frames.get() {
            (seen, frames) if seen == step => frames + 1,
            _ => 1,
        };
        registration.frames.set((step, frames));
        if frames > FRAME_LIMIT {
            // Later layouts while the panic unwinds count afresh, so they cannot
            // panic again.
            begin_step();
            panic!(
                "{LIVELOCK}: {global_id:?} was laid out {FRAME_LIMIT} times in one step; \
                 rendering never settles, such as a hover or layout change that undoes itself"
            );
        }
        let unpainted = registration.unpainted.get() + 1;
        registration.unpainted.set(unpainted);
        if unpainted == 2 {
            DUPLICATES.with(|duplicates| duplicates.borrow_mut().push(global_id.to_vec()));
        }
        let inner = self
            .inner
            .prepaint(id, inspector, bounds, layout, window, cx);
        // Inserted after the element's own content, so only elements painted later
        // (siblings, overlays, deferred popovers) can cover it.
        *registration.hitbox.borrow_mut() = Some(Rc::new(
            window.insert_hitbox(bounds, HitboxBehavior::Normal),
        ));
        (inner, registration)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let clipped = bounds
            .intersect(&window.content_mask().bounds)
            .intersect(&Bounds::new(Default::default(), window.viewport_size()));
        prepaint.1.unpainted.set(0);
        prepaint.1.facts.borrow_mut().visible =
            clipped.size.width > px(0.) && clipped.size.height > px(0.);
        self.inner
            .paint(id, inspector, bounds, layout, &mut prepaint.0, window, cx);
    }

    fn a11y_role(&self) -> Option<gpui::Role> {
        self.inner.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        self.inner.write_a11y_info(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut gpui::A11ySubtreeBuilder,
    ) {
        self.inner.a11y_synthetic_children(&mut prepaint.0, builder);
    }
}
