use crate::step::StepRecord;
use std::fmt;

/// Why a case failed: which oracle or invariant, its message, and the step after
/// which it fired (`None` when it fired while opening the window).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    oracle: String,
    message: String,
    step: Option<usize>,
}

impl Failure {
    /// A failure reported by an invariant or a model check.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            oracle: "check".into(),
            message: message.into(),
            step: None,
        }
    }

    pub(crate) fn from_oracle(oracle: &str, message: impl Into<String>) -> Self {
        Self {
            oracle: oracle.into(),
            message: message.into(),
            step: None,
        }
    }

    pub(crate) fn at_step(mut self, step: Option<usize>) -> Self {
        self.step = step;
        self
    }

    pub(crate) fn named(mut self, oracle: &str) -> Self {
        self.oracle = oracle.into();
        self
    }

    /// The oracle or invariant name, such as `panic` or `"reset zeroes the count"`.
    pub fn oracle(&self) -> &str {
        &self.oracle
    }

    /// What went wrong.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Zero-based index of the step after which the failure fired.
    pub fn step(&self) -> Option<usize> {
        self.step
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.step {
            Some(step) => write!(
                f,
                "[{}] {} (after step {})",
                self.oracle,
                self.message,
                step + 1
            ),
            None => write!(f, "[{}] {} (before any step)", self.oracle, self.message),
        }
    }
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::new(message)
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Failure::new(message)
    }
}

/// Builds a [`Failure`] for an invariant or model check from a format string.
#[macro_export]
macro_rules! fail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::Failure::new(format!($($arg)*)))
    };
}

/// Fails the check unless both sides are equal.
#[macro_export]
macro_rules! ensure_eq {
    ($left:expr, $right:expr $(,)?) => {{
        let (left, right) = (&$left, &$right);
        if left != right {
            $crate::fail!(
                "{} == {} failed: {:?} != {:?}",
                stringify!($left),
                stringify!($right),
                left,
                right
            );
        }
    }};
}

/// Fails the check unless the condition holds.
#[macro_export]
macro_rules! ensure {
    ($condition:expr $(,)?) => {
        if !$condition {
            $crate::fail!("{} failed", stringify!($condition));
        }
    };
    ($condition:expr, $($arg:tt)+) => {
        if !$condition {
            $crate::fail!($($arg)+);
        }
    };
}

/// The steps of one case as they ran.
#[derive(Clone, PartialEq, Eq)]
pub struct Trace {
    seed: u64,
    records: Vec<StepRecord>,
}

impl Trace {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            seed,
            records: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, record: StepRecord) {
        self.records.push(record);
    }

    pub(crate) fn mark_last_skipped(&mut self) {
        if let Some(record) = self.records.last_mut() {
            record.mark_skipped();
        }
    }

    /// The scheduler seed the case ran with.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The steps that ran, in order. Stops at the step that failed.
    pub fn records(&self) -> &[StepRecord] {
        &self.records
    }
}

impl fmt::Debug for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "seed {:#x}", self.seed)?;
        for (index, record) in self.records.iter().enumerate() {
            writeln!(f, "  {:>2}. {}", index + 1, record.description())?;
        }
        Ok(())
    }
}

/// The result of running one case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    trace: Trace,
    failure: Option<Failure>,
    reached: usize,
}

impl Outcome {
    pub(crate) fn new(trace: Trace, failure: Option<Failure>, reached: usize) -> Self {
        Self {
            trace,
            failure,
            reached,
        }
    }

    /// How many steps the case started, including one whose targets failed to
    /// read before it ran. Replaying that many steps reproduces the outcome.
    pub(crate) fn reached(&self) -> usize {
        self.reached
    }

    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }

    pub fn is_ok(&self) -> bool {
        self.failure.is_none()
    }
}

/// A failure found by a search, after shrinking.
#[derive(Clone)]
pub struct Report {
    outcome: Outcome,
    steps: Vec<String>,
    original_steps: usize,
    nondeterministic: Option<Outcome>,
}

impl Report {
    pub(crate) fn new(outcome: Outcome, steps: Vec<String>, original_steps: usize) -> Self {
        Self {
            outcome,
            steps,
            original_steps,
            nondeterministic: None,
        }
    }

    pub(crate) fn nondeterministic(first: Outcome, second: Outcome, steps: Vec<String>) -> Self {
        let original_steps = steps.len();
        Self {
            outcome: first,
            steps,
            original_steps,
            nondeterministic: Some(second),
        }
    }

    /// The failure of the minimal case.
    pub fn failure(&self) -> &Failure {
        self.outcome
            .failure()
            .expect("a report always holds a failing outcome")
    }

    /// The minimal case as it ran.
    pub fn trace(&self) -> &Trace {
        self.outcome.trace()
    }

    /// The scheduler seed of the minimal case.
    pub fn seed(&self) -> u64 {
        self.outcome.trace().seed()
    }

    /// Rust source of each step of the minimal case.
    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    /// The step count of the case first found, before shrinking.
    pub fn original_steps(&self) -> usize {
        self.original_steps
    }

    /// Whether replaying the first failing case gave a different outcome. Shrinking
    /// is skipped for such cases because it would shrink noise.
    pub fn is_nondeterministic(&self) -> bool {
        self.nondeterministic.is_some()
    }

    /// A test that replays the minimal case. `harness` is an expression that
    /// builds the same harness, such as `harness()`.
    pub fn regression_test(&self, harness: &str) -> String {
        let mut out = format!(
            "#[test]\nfn regression_{:x}() {{\n    use gpui_proptest::{{ModifierKeys, Step}};\n    {harness}.replay(\n        {:#x},\n        &[\n",
            self.seed(),
            self.seed()
        );
        for (step, record) in self.steps.iter().zip(self.trace().records()) {
            out.push_str(&format!(
                "            {step}, // {}\n",
                record.description()
            ));
        }
        for step in self.steps.iter().skip(self.trace().records().len()) {
            out.push_str(&format!("            {step},\n"));
        }
        out.push_str(&format!(
            "        ],\n    );\n    // failed: {}\n}}\n",
            self.failure()
        ));
        out
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(second) = &self.nondeterministic {
            writeln!(
                f,
                "gpui-proptest: NON-DETERMINISTIC failure; the same seed and steps gave different outcomes, so shrinking was skipped."
            )?;
            writeln!(f, "first run:  {}", self.failure())?;
            match second.failure() {
                Some(failure) => writeln!(f, "second run: {failure}")?,
                None => writeln!(f, "second run: passed")?,
            }
            writeln!(f, "\nfirst trace:\n{:?}", self.trace())?;
            writeln!(f, "second trace:\n{:?}", second.trace())?;
            writeln!(
                f,
                "Look for wall-clock time, real threads or global state outside GPUI's executor."
            )?;
            return Ok(());
        }
        writeln!(
            f,
            "gpui-proptest: {} (seed {:#x}, shrunk from {} to {} steps)\n",
            self.failure(),
            self.seed(),
            self.original_steps,
            self.steps.len()
        )?;
        writeln!(f, "trace:\n{:?}", self.trace())?;
        writeln!(
            f,
            "regression test:\n\n{}",
            self.regression_test("harness()")
        )
    }
}

impl fmt::Debug for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
