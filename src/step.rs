use arbitrary::Arbitrary;
use std::fmt::{self, Debug};

/// One abstract user action.
///
/// A step never stores a concrete element. `target` is a choice that the runner
/// resolves against the targets painted in the last frame, so deleting earlier
/// steps while shrinking never makes later steps invalid: they re-resolve. A step
/// with no eligible target is recorded as skipped, not as a failure.
///
/// `A` is the domain action type of a [`StateMachine`](crate::StateMachine) or of
/// [`Harness::with_domain`](crate::Harness::with_domain). It defaults to
/// [`NoDomain`], which does nothing.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Arbitrary)]
pub enum Step<A = NoDomain> {
    /// A left click at the center of a visible target, disabled targets included.
    Click {
        target: u16,
        modifiers: ModifierKeys,
    },
    /// A left double click at the center of a visible, enabled target.
    DoubleClick { target: u16 },
    /// A right click at the center of a visible, enabled target.
    RightClick { target: u16 },
    /// Moves the pointer to the center of a visible, enabled target.
    Hover { target: u16 },
    /// A wheel event over a visible, enabled target, in lines (positive scrolls down).
    Scroll { target: u16, lines: i8 },
    /// Presses one key from the harness's key set, chosen by index.
    Press { key: u8 },
    /// Types text into whatever has focus.
    Type { text: String },
    /// Advances GPUI's fake clock and runs every timer that becomes due.
    AdvanceClock { millis: u16 },
    /// A domain action, performed by the harness's domain handler.
    Domain(A),
}

/// Generated [`Step::AdvanceClock`] steps advance the clock by less than this.
pub(crate) const MAX_ADVANCE_MILLIS: u16 = 2_000;

/// Modifier keys held during a click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Arbitrary)]
pub struct ModifierKeys {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub platform: bool,
}

impl ModifierKeys {
    /// No modifier keys.
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alt: false,
        platform: false,
    };

    pub(crate) fn to_gpui(self) -> gpui::Modifiers {
        gpui::Modifiers {
            shift: self.shift,
            control: self.control,
            alt: self.alt,
            platform: self.platform,
            function: false,
        }
    }

    fn to_rust(self) -> String {
        if self == Self::NONE {
            return "ModifierKeys::NONE".into();
        }
        format!(
            "ModifierKeys {{ shift: {}, control: {}, alt: {}, platform: {} }}",
            self.shift, self.control, self.alt, self.platform
        )
    }

    fn label(self) -> String {
        [
            (self.control, "ctrl-"),
            (self.alt, "alt-"),
            (self.shift, "shift-"),
            (self.platform, "cmd-"),
        ]
        .iter()
        .filter(|(held, _)| *held)
        .map(|(_, name)| *name)
        .collect()
    }
}

/// The domain action of a harness without one. Performing it does nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Arbitrary)]
pub struct NoDomain;

impl<A: Debug> Step<A> {
    /// Rust source for this step, as used in emitted regression tests.
    pub fn to_rust(&self) -> String {
        match self {
            Step::Click { target, modifiers } => format!(
                "Step::Click {{ target: {target}, modifiers: {} }}",
                modifiers.to_rust()
            ),
            Step::DoubleClick { target } => format!("Step::DoubleClick {{ target: {target} }}"),
            Step::RightClick { target } => format!("Step::RightClick {{ target: {target} }}"),
            Step::Hover { target } => format!("Step::Hover {{ target: {target} }}"),
            Step::Scroll { target, lines } => {
                format!("Step::Scroll {{ target: {target}, lines: {lines} }}")
            }
            Step::Press { key } => format!("Step::Press {{ key: {key} }}"),
            Step::Type { text } => format!("Step::Type {{ text: {text:?}.into() }}"),
            Step::AdvanceClock { millis } => format!("Step::AdvanceClock {{ millis: {millis} }}"),
            Step::Domain(action) => format!("Step::Domain({action:?})"),
        }
    }

    pub(crate) fn verb(&self) -> Verb {
        match self {
            Step::Click { .. } => Verb::Click,
            Step::DoubleClick { .. } => Verb::DoubleClick,
            Step::RightClick { .. } => Verb::RightClick,
            Step::Hover { .. } => Verb::Hover,
            Step::Scroll { .. } => Verb::Scroll,
            Step::Press { .. }
            | Step::Type { .. }
            | Step::AdvanceClock { .. }
            | Step::Domain(_) => Verb::Untargeted,
        }
    }

    pub(crate) fn target_choice(&self) -> Option<u16> {
        match self {
            Step::Click { target, .. }
            | Step::DoubleClick { target }
            | Step::RightClick { target }
            | Step::Hover { target }
            | Step::Scroll { target, .. } => Some(*target),
            _ => None,
        }
    }

    pub(crate) fn describe(&self, target: Option<&str>, key: Option<&str>) -> String {
        let at = |verb: &str| match target {
            Some(target) => format!("{verb} {target}"),
            None => format!("{verb} (no eligible target, skipped)"),
        };
        match self {
            Step::Click { modifiers, .. } => at(&format!("{}click", modifiers.label())),
            Step::DoubleClick { .. } => at("double-click"),
            Step::RightClick { .. } => at("right-click"),
            Step::Hover { .. } => at("hover"),
            Step::Scroll { lines, .. } => at(&format!("scroll {lines} lines over")),
            Step::Press { .. } => format!("press {}", key.unwrap_or("<no keys>")),
            Step::Type { text } => format!("type {text:?}"),
            Step::AdvanceClock { millis } => format!("advance clock {millis} ms"),
            Step::Domain(action) => format!("domain {action:?}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verb {
    Click,
    DoubleClick,
    RightClick,
    Hover,
    Scroll,
    Untargeted,
}

/// A step as it ran: what was asked and what it resolved to.
#[derive(Clone, PartialEq, Eq)]
pub struct StepRecord {
    rust: String,
    description: String,
    skipped: bool,
}

impl StepRecord {
    pub(crate) fn new(rust: String, description: String, skipped: bool) -> Self {
        Self {
            rust,
            description,
            skipped,
        }
    }

    pub(crate) fn mark_skipped(&mut self) {
        self.skipped = true;
        self.description.push_str(" (did not apply, skipped)");
    }

    /// Rust source for the step.
    pub fn rust(&self) -> &str {
        &self.rust
    }

    /// The step in words, with its resolved target, such as `click "save"`.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Whether the step had no eligible target or no handler and did nothing.
    pub fn is_skipped(&self) -> bool {
        self.skipped
    }
}

impl Debug for StepRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description)
    }
}

/// Rust source for each step, as reports print them.
pub(crate) fn rust<A: Debug>(steps: &[Step<A>]) -> Vec<String> {
    steps.iter().map(Step::to_rust).collect()
}
