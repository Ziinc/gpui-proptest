use crate::{
    guard::Guarded,
    outcome::{Failure, Outcome, Trace},
    step::{NoDomain, Step, StepRecord, Verb},
    target::{self, ObservedTargets, TargetInfo, TargetSource},
};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, InputEvent, Keystroke, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Render, ScrollDelta, ScrollWheelEvent,
    TestAppContext, TestDispatcher, TouchPhase, Window, WindowBounds, WindowOptions, point, px,
    size,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt::Debug,
    panic::{self, AssertUnwindSafe},
    rc::Rc,
    sync::Once,
    time::Duration,
};

/// Keys a [`Step::Press`] chooses from when the harness sets none.
pub const DEFAULT_KEYS: &[&str] = &[
    "tab",
    "shift-tab",
    "enter",
    "escape",
    "space",
    "backspace",
    "left",
    "right",
    "up",
    "down",
];

type Open<V> = Rc<dyn Fn(&mut App) -> (AnyWindowHandle, Entity<V>)>;
type Setup = Rc<dyn Fn(&mut App)>;
type Invariant<V> =
    Rc<dyn Fn(&Entity<V>, &[TargetInfo], &mut Window, &mut App) -> Result<(), Failure>>;
type Perform<V, A> = Rc<dyn Fn(&A, &Entity<V>, &mut Window, &mut App) -> bool>;

/// What one run drives: how to open the view, which keys to press, which domain
/// actions exist, and what must hold after every step.
///
/// The harness is engine-neutral. [`Harness::run_case`] replays one `(seed, steps)`
/// input; the `proptest` feature adds [`Harness::check`], which generates inputs
/// and shrinks failures.
pub struct Harness<V: 'static, A: 'static = NoDomain> {
    open: Open<V>,
    setup: Vec<Setup>,
    keys: Vec<String>,
    invariants: Vec<(String, Invariant<V>)>,
    domain: Option<Perform<V, A>>,
    sources: Vec<Rc<dyn TargetSource>>,
    disabled_is_inert: bool,
    focus_is_visible: bool,
    unique_ids: bool,
    window_may_close: bool,
    ui_steps: bool,
    /// The last input `fuzz` saw fail, minimized once the fuzzer stops.
    pub(crate) last_fuzz_failure: RefCell<Option<(u64, Vec<Step<A>>)>>,
}

impl<V: Render + 'static> Harness<V> {
    /// Opens `build`'s view in an 800×600 test window, inside a [`FrameGuard`].
    /// The window's root view is the guard, so `window.root::<V>()` is `None`.
    pub fn new(build: impl Fn(&mut Window, &mut App) -> Entity<V> + 'static) -> Self {
        Self::with_window(move |cx| {
            let mut root = None;
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(800.), px(600.)),
                ))),
                ..Default::default()
            };
            let handle = cx
                .open_window(options, |window, cx| {
                    let view = build(window, cx);
                    root = Some(view.clone());
                    cx.new(|_| Guarded(view.into()))
                })
                .expect("open test window");
            (handle.into(), root.expect("window built its root view"))
        })
    }
}

impl<V: 'static> Harness<V> {
    /// Opens the window with `open`, for roots the harness does not build itself,
    /// such as `gpui_kit::open_window`.
    pub fn with_window(open: impl Fn(&mut App) -> (AnyWindowHandle, Entity<V>) + 'static) -> Self {
        Self {
            open: Rc::new(open),
            setup: Vec::new(),
            keys: DEFAULT_KEYS.iter().map(|key| key.to_string()).collect(),
            invariants: Vec::new(),
            domain: None,
            sources: default_sources(),
            disabled_is_inert: true,
            focus_is_visible: true,
            unique_ids: true,
            window_may_close: false,
            ui_steps: true,
            last_fuzz_failure: RefCell::new(None),
        }
    }
}

impl<V: 'static, A: 'static> Harness<V, A> {
    /// Runs `setup` on each new app before the window opens, such as `gpui_kit::init`.
    pub fn with_setup(mut self, setup: impl Fn(&mut App) + 'static) -> Self {
        self.setup.push(Rc::new(setup));
        self
    }

    /// Replaces the keys [`Step::Press`] chooses from, as GPUI keystroke strings
    /// such as `"cmd-z"`. The first key is the one shrinking prefers.
    ///
    /// Panics on a key GPUI would not recognise, such as `"ctrl-banana"`: GPUI
    /// accepts any word as a key name, so a typo would otherwise press nothing.
    pub fn with_keys<S: Into<String>>(mut self, keys: impl IntoIterator<Item = S>) -> Self {
        self.keys = keys.into_iter().map(Into::into).collect();
        for key in &self.keys {
            for keystroke in key.split(' ') {
                assert!(
                    is_known_key(keystroke),
                    "with_keys: {keystroke:?} in {key:?} is not a key GPUI recognises"
                );
            }
        }
        self
    }

    /// Adds a named invariant, checked once the window opens and after every step.
    pub fn invariant(
        mut self,
        name: impl Into<String>,
        check: impl Fn(&Entity<V>, &[TargetInfo], &mut Window, &mut App) -> Result<(), Failure>
        + 'static,
    ) -> Self {
        self.invariants.push((name.into(), Rc::new(check)));
        self
    }

    /// Sets how domain actions are performed. Return `false` when the action does
    /// not apply in the current state; the step is then recorded as skipped.
    pub fn with_domain<B: 'static>(
        self,
        perform: impl Fn(&B, &Entity<V>, &mut Window, &mut App) -> bool + 'static,
    ) -> Harness<V, B> {
        Harness {
            open: self.open,
            setup: self.setup,
            keys: self.keys,
            invariants: self.invariants,
            domain: Some(Rc::new(perform)),
            sources: self.sources,
            disabled_is_inert: self.disabled_is_inert,
            focus_is_visible: self.focus_is_visible,
            unique_ids: self.unique_ids,
            window_may_close: self.window_may_close,
            ui_steps: self.ui_steps,
            last_fuzz_failure: RefCell::new(None),
        }
    }

    /// Adds a source of targets besides the defaults.
    pub fn with_target_source(mut self, source: impl TargetSource) -> Self {
        self.sources.push(Rc::new(source));
        self
    }

    /// Turns the disabled-is-inert oracle on or off. On by default: clicking a
    /// disabled target must not change any target's state fingerprint.
    pub fn with_disabled_is_inert(mut self, enabled: bool) -> Self {
        self.disabled_is_inert = enabled;
        self
    }

    /// Turns the focus-is-visible oracle on or off. On by default: no target that
    /// reports focus may be invisible. Only sources that report focus feed it.
    pub fn with_focus_is_visible(mut self, enabled: bool) -> Self {
        self.focus_is_visible = enabled;
        self
    }

    /// Turns the unique-ids oracle on or off. On by default: no two targets may
    /// share an id path, since GPUI gives them one element state and only one of
    /// them can be chosen.
    pub fn with_unique_ids(mut self, enabled: bool) -> Self {
        self.unique_ids = enabled;
        self
    }

    /// Whether a step may close the window. Off by default: closing it fails the
    /// case with the `window-closed` oracle. When on, the case ends there and the
    /// remaining steps are recorded as not run.
    pub fn with_window_may_close(mut self, enabled: bool) -> Self {
        self.window_may_close = enabled;
        self
    }

    /// Whether engines generate generic UI steps (clicks, keys, text, clock). On by
    /// default; [`Harness::from_state_machine`] turns them off.
    pub fn with_ui_steps(mut self, enabled: bool) -> Self {
        self.ui_steps = enabled;
        self
    }

    pub(crate) fn has_ui_steps(&self) -> bool {
        self.ui_steps
    }

    pub(crate) fn has_domain(&self) -> bool {
        self.domain.is_some()
    }

    /// Runs one case: a fresh app on a `TestDispatcher` seeded with `seed`, then
    /// `steps` in order, with every oracle and invariant checked after each step.
    /// The same input always gives the same outcome.
    pub fn run_case(&self, seed: u64, steps: &[Step<A>]) -> Outcome
    where
        A: Debug,
    {
        install_quiet_panic_hook();
        target::clear_registry();
        let trace = RefCell::new(Trace::new(seed));
        let dispatcher = TestDispatcher::new(seed);
        let scheduler = dispatcher.scheduler().clone();
        let mut cx = TestAppContext::build(dispatcher, None);

        let result = quietly(|| {
            panic::catch_unwind(AssertUnwindSafe(|| self.drive(&mut cx, steps, &trace)))
        });
        let last_step = trace.borrow().records().len().checked_sub(1);
        let mut failure = match result {
            Ok(Ok(())) => None,
            Ok(Err(failure)) => Some(failure),
            Err(payload) => Some(panic_failure(payload.as_ref()).at_step(last_step)),
        };

        let teardown = quietly(|| {
            panic::catch_unwind(AssertUnwindSafe(move || {
                cx.quit();
                drop(cx);
                scheduler.end_test();
            }))
        });
        if let (Err(payload), None) = (teardown, &failure) {
            failure = Some(
                Failure::from_oracle("teardown", panic_message(payload.as_ref()))
                    .at_step(last_step),
            );
        }
        target::clear_registry();
        Outcome::new(trace.into_inner(), failure)
    }

    /// Runs one case and panics with its trace if it fails. Emitted regression
    /// tests call this.
    #[track_caller]
    pub fn replay(&self, seed: u64, steps: &[Step<A>])
    where
        A: Debug,
    {
        let outcome = self.run_case(seed, steps);
        if let Some(failure) = outcome.failure() {
            panic!("{failure}\n\n{:?}", outcome.trace());
        }
    }

    fn drive(
        &self,
        cx: &mut TestAppContext,
        steps: &[Step<A>],
        trace: &RefCell<Trace>,
    ) -> Result<(), Failure>
    where
        A: Debug,
    {
        for setup in &self.setup {
            cx.update(|cx| setup(cx));
        }
        target::begin_step();
        let (window, view) = cx.update(|cx| (self.open)(cx));
        settle(cx).map_err(|failure| failure.at_step(None))?;
        if !render(cx, window) {
            return self.window_closed(None, steps, trace);
        }
        self.check_oracles(cx, window, &view, None, None)?;

        for (index, step) in steps.iter().enumerate() {
            let Some(targets) = self.targets(cx, window) else {
                return self.window_closed(index.checked_sub(1), &steps[index..], trace);
            };
            let verb = step.verb();
            let target = step
                .target_choice()
                .and_then(|choice| self.choose(cx, window, &targets, verb, choice));
            let key = match step {
                Step::Press { key } if !self.keys.is_empty() => {
                    Some(self.keys[*key as usize % self.keys.len()].clone())
                }
                _ => None,
            };
            let skipped = match step {
                Step::Press { .. } => key.is_none(),
                Step::Domain(_) => self.domain.is_none(),
                _ => verb != Verb::Untargeted && target.is_none(),
            };
            trace.borrow_mut().push(StepRecord::new(
                step.to_rust(),
                step.describe(
                    target.as_ref().map(|target| target.name()).as_deref(),
                    key.as_deref(),
                ),
                skipped,
            ));
            if skipped {
                continue;
            }

            let before = (self.disabled_is_inert
                && verb == Verb::Click
                && target.as_ref().is_some_and(TargetInfo::is_disabled))
            .then(|| (target.clone().unwrap(), targets.clone()));

            target::begin_step();
            let performed = self
                .perform(cx, window, &view, step, target.as_ref(), key.as_deref())
                .map_err(|failure| failure.at_step(Some(index)))?;
            if !performed {
                trace.borrow_mut().mark_last_skipped();
                continue;
            }
            settle(cx).map_err(|failure| failure.at_step(Some(index)))?;
            if !render(cx, window) {
                return self.window_closed(Some(index), &steps[index + 1..], trace);
            }
            self.check_oracles(cx, window, &view, Some(index), before)?;
        }
        Ok(())
    }

    /// Deletes runs and pairs of steps while the same oracle still fails, then
    /// tries scheduler seed 0, within `max_iters` cases and `max_time`. Engines
    /// call it on their shrunk failure: proptest deletes each step once, in one
    /// pass, and byte shrinkers do not see steps at all.
    pub(crate) fn minimize(
        &self,
        seed: u64,
        mut steps: Vec<Step<A>>,
        max_iters: u32,
        max_time: Duration,
    ) -> (u64, Vec<Step<A>>, Outcome)
    where
        A: Clone + Debug,
    {
        let mut outcome = self.run_case(seed, &steps);
        let Some(oracle) = outcome.failure().map(|failure| failure.oracle().to_owned()) else {
            return (seed, steps, outcome);
        };
        let fails_the_same = |outcome: &Outcome| {
            outcome
                .failure()
                .is_some_and(|failure| failure.oracle() == oracle)
        };
        let start = std::time::Instant::now();
        let mut budget = max_iters;
        let mut spend = || {
            budget = budget.saturating_sub(1);
            budget > 0 && start.elapsed() < max_time
        };

        // Contiguous runs of every length, longest first, then (for short traces)
        // every pair: a decrement that cancels an increment is rarely adjacent.
        let mut changed = true;
        while changed {
            changed = false;
            let mut candidates: Vec<Vec<usize>> = (1..=steps.len())
                .rev()
                .flat_map(|len| {
                    (0..=steps.len() - len).map(move |start| (start..start + len).collect())
                })
                .collect();
            if steps.len() <= 16 {
                for first in 0..steps.len() {
                    for second in first + 2..steps.len() {
                        candidates.push(vec![first, second]);
                    }
                }
            }
            for removed in candidates {
                if !spend() {
                    return (seed, steps, outcome);
                }
                let candidate: Vec<_> = steps
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !removed.contains(index))
                    .map(|(_, step)| step.clone())
                    .collect();
                let candidate_outcome = self.run_case(seed, &candidate);
                if fails_the_same(&candidate_outcome) {
                    steps = candidate;
                    outcome = candidate_outcome;
                    changed = true;
                    break;
                }
            }
        }

        if seed != 0 && spend() {
            let zero = self.run_case(0, &steps);
            if fails_the_same(&zero) {
                return (0, steps, zero);
            }
        }
        (seed, steps, outcome)
    }

    /// The eligible target `choice` picks, aimed at a point nothing covers. A
    /// covered target passes the pick to the next eligible one, so only the
    /// targets walked past are hit-tested.
    fn choose(
        &self,
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        targets: &[TargetInfo],
        verb: Verb,
        choice: u16,
    ) -> Option<TargetInfo> {
        let eligible: Vec<_> = targets
            .iter()
            .filter(|target| is_eligible(verb, target))
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let start = choice as usize % eligible.len();
        cx.update_window(window, |_, window, _| {
            (0..eligible.len()).find_map(|offset| {
                let target = eligible[(start + offset) % eligible.len()];
                target::aim(target, window)
                    .map(|point| target.clone().with_click_point(Some(point)))
            })
        })
        .ok()
        .flatten()
    }

    /// Ends the case once the window has closed after `step`: a failure unless the
    /// harness allows it, else `rest` is recorded as not run.
    fn window_closed(
        &self,
        step: Option<usize>,
        rest: &[Step<A>],
        trace: &RefCell<Trace>,
    ) -> Result<(), Failure>
    where
        A: Debug,
    {
        if !self.window_may_close {
            return Err(Failure::from_oracle(
                "window-closed",
                "the window closed; allow it with Harness::with_window_may_close(true)",
            )
            .at_step(step));
        }
        let mut trace = trace.borrow_mut();
        for step in rest {
            trace.push(StepRecord::new(
                step.to_rust(),
                format!("{} (window closed, not run)", step.to_rust()),
                true,
            ));
        }
        Ok(())
    }

    fn perform(
        &self,
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        view: &Entity<V>,
        step: &Step<A>,
        target: Option<&TargetInfo>,
        key: Option<&str>,
    ) -> Result<bool, Failure> {
        let center = target.and_then(TargetInfo::click_point);
        match step {
            Step::Click { modifiers, .. } => click(
                cx,
                window,
                center.unwrap(),
                MouseButton::Left,
                1,
                modifiers.to_gpui(),
            )?,
            Step::DoubleClick { .. } => {
                let position = center.unwrap();
                click(
                    cx,
                    window,
                    position,
                    MouseButton::Left,
                    1,
                    Default::default(),
                )?;
                render(cx, window);
                click(
                    cx,
                    window,
                    position,
                    MouseButton::Left,
                    2,
                    Default::default(),
                )?;
            }
            Step::RightClick { .. } => click(
                cx,
                window,
                center.unwrap(),
                MouseButton::Right,
                1,
                Default::default(),
            )?,
            Step::Hover { .. } => dispatch(
                cx,
                window,
                MouseMoveEvent {
                    position: center.unwrap(),
                    pressed_button: None,
                    modifiers: Default::default(),
                },
            )?,
            Step::Scroll { lines, .. } => {
                let position = center.unwrap();
                dispatch(
                    cx,
                    window,
                    MouseMoveEvent {
                        position,
                        pressed_button: None,
                        modifiers: Default::default(),
                    },
                )?;
                render(cx, window);
                dispatch(
                    cx,
                    window,
                    ScrollWheelEvent {
                        position,
                        delta: ScrollDelta::Lines(point(0., *lines as f32)),
                        modifiers: Default::default(),
                        touch_phase: TouchPhase::Moved,
                    },
                )?;
            }
            // Dispatched one keystroke at a time, rather than through
            // `simulate_keystrokes`, so a task that never parks is caught by `settle`.
            Step::Press { .. } => {
                for keystroke in key.unwrap().split(' ') {
                    let keystroke = Keystroke::parse(keystroke).expect("with_keys validated keys");
                    if !press(cx, window, keystroke)? {
                        break;
                    }
                }
            }
            Step::Type { text } => {
                for keystroke in text.split("").filter_map(|ch| Keystroke::parse(ch).ok()) {
                    if !press(cx, window, keystroke)? {
                        break;
                    }
                }
            }
            Step::AdvanceClock { millis } => cx
                .executor()
                .advance_clock(Duration::from_millis(*millis as u64)),
            Step::Domain(action) => {
                let perform = self.domain.as_ref().unwrap();
                return Ok(cx
                    .update_window(window, |_, window, cx| perform(action, view, window, cx))
                    .unwrap_or(false));
            }
        }
        Ok(true)
    }

    fn check_oracles(
        &self,
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        view: &Entity<V>,
        step: Option<usize>,
        disabled_click: Option<(TargetInfo, Vec<TargetInfo>)>,
    ) -> Result<(), Failure> {
        let duplicates = target::take_duplicates();
        let Some(targets) = self.targets(cx, window) else {
            return Ok(());
        };

        if self.unique_ids
            && let Some(path) = duplicates.first()
        {
            let name = TargetInfo::new(path.clone(), Bounds::default()).name();
            return Err(Failure::from_oracle(
                "unique-ids",
                format!("{name} is the id of more than one target; GPUI gives them one shared element state"),
            )
            .at_step(step));
        }

        if self.focus_is_visible
            && let Some(target) = targets
                .iter()
                .find(|target| target.focused() == Some(true) && !target.is_visible())
        {
            return Err(Failure::from_oracle(
                "focus-is-visible",
                format!("{} holds focus but is not visible", target.name()),
            )
            .at_step(step));
        }

        if let Some((clicked, before)) = disabled_click {
            let after: HashMap<_, _> = targets
                .iter()
                .map(|target| (target.path().to_vec(), target.state()))
                .collect();
            for target in &before {
                if let Some(state) = after.get(target.path())
                    && *state != target.state()
                {
                    return Err(Failure::from_oracle(
                        "disabled-is-inert",
                        format!(
                            "clicking disabled {} changed {}: {} -> {}",
                            clicked.name(),
                            target.name(),
                            target.state(),
                            state
                        ),
                    )
                    .at_step(step));
                }
            }
        }

        for (name, invariant) in &self.invariants {
            // A panicking invariant is still that invariant's failure.
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                cx.update_window(window, |_, window, cx| {
                    invariant(view, &targets, window, cx)
                })
                .unwrap_or(Ok(()))
            }))
            .unwrap_or_else(|payload| {
                Err(Failure::new(format!(
                    "panicked: {}",
                    panic_message(payload.as_ref())
                )))
            });
            result.map_err(|failure| failure.named(name).at_step(step))?;
        }
        Ok(())
    }

    /// Every target from every source, deduplicated by path and in a stable order.
    /// `None` once the window has closed.
    fn targets(&self, cx: &mut TestAppContext, window: AnyWindowHandle) -> Option<Vec<TargetInfo>> {
        cx.update_window(window, |_, window, cx| {
            let mut targets: Vec<_> = self
                .sources
                .iter()
                .flat_map(|source| source.targets(window, cx))
                .collect();
            // A stable sort keeps the first source's copy first among equal paths.
            target::sort(&mut targets);
            targets.dedup_by(|later, earlier| later.path() == earlier.path());
            targets
        })
        .ok()
    }
}

fn default_sources() -> Vec<Rc<dyn TargetSource>> {
    let sources: Vec<Rc<dyn TargetSource>> = vec![Rc::new(ObservedTargets)];
    #[cfg(feature = "kit")]
    let sources = {
        let mut sources = sources;
        sources.push(Rc::new(crate::kit::KitTargets));
        sources
    };
    sources
}

fn is_eligible(verb: Verb, target: &TargetInfo) -> bool {
    target.is_visible()
        && target.click_point().is_some()
        && (verb == Verb::Click || !target.is_disabled())
}

/// Completes a frame. Returns `false` once the window has closed.
fn render(cx: &mut TestAppContext, window: AnyWindowHandle) -> bool {
    cx.update_window(window, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .is_ok()
}

fn dispatch(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    event: impl InputEvent,
) -> Result<(), Failure> {
    let input = event.to_platform_input();
    target::begin_step();
    let _ = cx.update_window(window, |_, window, cx| {
        window.dispatch_event(input, cx);
    });
    settle(cx)
}

/// Dispatches one keystroke and settles. Returns `false` once the window has
/// closed, so the remaining keys are not pressed; the caller's render then ends
/// the case. `TestAppContext::dispatch_keystroke` would panic instead.
fn press(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    keystroke: Keystroke,
) -> Result<bool, Failure> {
    target::begin_step();
    let open = cx
        .update_window(window, |_, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .is_ok();
    settle(cx)?;
    Ok(open)
}

/// Scheduler ticks one step may take before it counts as a task that never parks.
const TICK_LIMIT: usize = 100_000;

/// Runs tasks until none is runnable, like `run_until_parked`, but fails instead
/// of hanging when a task keeps rescheduling itself.
fn settle(cx: &mut TestAppContext) -> Result<(), Failure> {
    let executor = cx.executor();
    for _ in 0..TICK_LIMIT {
        if !executor.tick() {
            return Ok(());
        }
    }
    Err(Failure::from_oracle(
        "livelock",
        format!(
            "tasks were still runnable after {TICK_LIMIT} scheduler ticks; a task reschedules itself forever"
        ),
    ))
}

/// Keys GPUI names; any other multi-character key is a typo GPUI would accept.
const NAMED_KEYS: &[&str] = &[
    "enter",
    "escape",
    "tab",
    "space",
    "backspace",
    "delete",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "left",
    "right",
    "up",
    "down",
    "shift",
    "control",
    "alt",
    "platform",
    "function",
];

fn is_known_key(keystroke: &str) -> bool {
    let Ok(parsed) = Keystroke::parse(keystroke) else {
        return false;
    };
    let key = parsed.key.as_str();
    key.chars().count() == 1
        || NAMED_KEYS.contains(&key)
        || key
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n))
}

/// Moves the pointer, renders so hover state and hitboxes reflect it, then presses
/// and releases. The caller renders once after the step.
fn click(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    position: gpui::Point<Pixels>,
    button: MouseButton,
    click_count: usize,
    modifiers: gpui::Modifiers,
) -> Result<(), Failure> {
    dispatch(
        cx,
        window,
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        },
    )?;
    render(cx, window);
    dispatch(
        cx,
        window,
        MouseDownEvent {
            button,
            position,
            modifiers,
            click_count,
            first_mouse: false,
        },
    )?;
    dispatch(
        cx,
        window,
        MouseUpEvent {
            button,
            position,
            modifiers,
            click_count,
        },
    )
}

/// A panic is the `panic` oracle's failure, unless the render-loop guard raised it.
fn panic_failure(payload: &(dyn Any + Send)) -> Failure {
    let message = panic_message(payload);
    match message
        .strip_prefix(target::LIVELOCK)
        .and_then(|rest| rest.strip_prefix(": "))
    {
        Some(rest) => Failure::from_oracle("livelock", rest),
        None => Failure::from_oracle("panic", message),
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message.to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic with a non-string payload".into()
    }
}

thread_local! { static QUIET: Cell<bool> = const { Cell::new(false) }; }

/// Panics inside a case are expected while searching and shrinking; the report
/// carries their message. Silences the default hook on this thread only.
fn install_quiet_panic_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !QUIET.with(Cell::get) {
                previous(info);
            }
        }));
    });
}

fn quietly<R>(f: impl FnOnce() -> R) -> R {
    let was = QUIET.with(|quiet| quiet.replace(true));
    let result = f();
    QUIET.with(|quiet| quiet.set(was));
    result
}
