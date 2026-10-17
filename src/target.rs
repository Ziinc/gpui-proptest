use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window, WindowId, px,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    rc::{Rc, Weak},
};

/// An element the runner can act on, as painted in the last frame.
#[derive(Clone, PartialEq)]
pub struct TargetInfo {
    path: Vec<ElementId>,
    bounds: Bounds<Pixels>,
    visible: bool,
    disabled: bool,
    focused: Option<bool>,
    role: Option<gpui::Role>,
    label: Option<String>,
    state: String,
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
                        .map(|entry| entry.facts.borrow().clone())
                        .collect()
                })
                .unwrap_or_default()
        })
    }
}

/// Orders targets top to bottom, then left to right, then by path, so a target
/// choice resolves to the same element on every run.
pub(crate) fn sort(targets: &mut [TargetInfo]) {
    targets.sort_by(|a, b| {
        (a.bounds.origin.y, a.bounds.origin.x)
            .partial_cmp(&(b.bounds.origin.y, b.bounds.origin.x))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| format!("{:?}", a.path).cmp(&format!("{:?}", b.path)))
    });
}

type Registry = HashMap<WindowId, HashMap<GlobalElementId, Weak<Registration>>>;
thread_local! { static REGISTRY: RefCell<Registry> = RefCell::new(HashMap::new()); }

thread_local! { static PREPAINTS: RefCell<HashMap<GlobalElementId, u32>> = RefCell::new(HashMap::new()); }

/// Prefixes the panic a render loop raises, so the harness reports it as `livelock`.
pub(crate) const LIVELOCK: &str = "livelock";

/// How often one target may lay out within a single step before the step counts
/// as a render loop. A step draws a handful of frames; a feedback loop draws forever.
pub(crate) const FRAME_LIMIT: u32 = 1_000;

/// Starts counting frames for a new step.
pub(crate) fn begin_step() {
    PREPAINTS.with(|counts| counts.borrow_mut().clear());
}

/// Forgets every registration. Each case builds a new app whose window ids may repeat.
pub(crate) fn clear_registry() {
    REGISTRY.with(|registry| registry.borrow_mut().clear());
    begin_step();
}

/// A target's registration, owned by GPUI element state so it lives exactly as long
/// as the element keeps being painted.
#[doc(hidden)]
pub struct Registration {
    window: WindowId,
    global_id: GlobalElementId,
    facts: RefCell<TargetInfo>,
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
        let frames = PREPAINTS.with(|counts| {
            let mut counts = counts.borrow_mut();
            let count = counts.entry(global_id.clone()).or_default();
            *count += 1;
            *count
        });
        if frames > FRAME_LIMIT {
            begin_step();
            panic!(
                "{LIVELOCK}: {global_id:?} was laid out {FRAME_LIMIT} times in one step; \
                 rendering never settles, such as a hover or layout change that undoes itself"
            );
        }
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
        (
            self.inner
                .prepaint(id, inspector, bounds, layout, window, cx),
            registration,
        )
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
