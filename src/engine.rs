//! The proptest engine: generates `(seed, steps)` inputs, runs them through
//! [`Harness::run_case`], and shrinks failures.

use crate::{Harness, ModifierKeys, Report, Step, step::rust};
use proptest::{
    collection::vec,
    prelude::*,
    strategy::{BoxedStrategy, Union},
    string::string_regex,
    test_runner::{
        Config as ProptestConfig, FileFailurePersistence, RngSeed, TestCaseError, TestError,
        TestRunner,
    },
};
use std::{
    cell::{Cell, RefCell},
    fmt::Debug,
    panic::Location,
    time::Duration,
};

/// How a search runs. Built with `with_*` methods from [`Config::default`].
///
/// `GPUI_PROPTEST_CASES` overrides the case count and `GPUI_PROPTEST_SEED` fixes the
/// generator seed, so a CI failure reproduces locally.
#[derive(Clone, Debug)]
pub struct Config {
    cases: u32,
    max_steps: usize,
    max_shrink_iters: u32,
    max_shrink_time: Duration,
    text: String,
    check_determinism: bool,
    persist_failures: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cases: env_var("GPUI_PROPTEST_CASES").unwrap_or(256),
            max_steps: 40,
            max_shrink_iters: 2_000,
            max_shrink_time: Duration::from_secs(60),
            text: "[a-zA-Z0-9 ]{0,8}".into(),
            check_determinism: true,
            persist_failures: true,
        }
    }
}

impl Config {
    /// Sets the number of cases to generate. `GPUI_PROPTEST_CASES` still wins.
    pub fn with_cases(mut self, cases: u32) -> Self {
        self.cases = env_var("GPUI_PROPTEST_CASES").unwrap_or(cases);
        self
    }

    /// Sets the longest step sequence a case may have.
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Caps the number of shrink attempts.
    pub fn with_max_shrink_iters(mut self, max_shrink_iters: u32) -> Self {
        self.max_shrink_iters = max_shrink_iters;
        self
    }

    /// Caps the time spent shrinking.
    pub fn with_max_shrink_time(mut self, max_shrink_time: Duration) -> Self {
        self.max_shrink_time = max_shrink_time;
        self
    }

    /// Sets the regular expression that typed text is drawn from.
    pub fn with_text(mut self, regex: impl Into<String>) -> Self {
        self.text = regex.into();
        self
    }

    /// Turns the replay-before-shrinking determinism check on or off. On by default.
    pub fn with_check_determinism(mut self, check_determinism: bool) -> Self {
        self.check_determinism = check_determinism;
        self
    }

    /// Turns saving failures to `proptest-regressions/` on or off. On by default;
    /// saved failures are replayed first on later runs.
    pub fn with_persist_failures(mut self, persist_failures: bool) -> Self {
        self.persist_failures = persist_failures;
        self
    }

    /// The number of cases to generate.
    pub fn cases(&self) -> u32 {
        self.cases
    }

    /// The longest step sequence a case may have.
    pub fn max_steps(&self) -> usize {
        self.max_steps
    }
}

fn env_var<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
}

impl<V: 'static, A> Harness<V, A>
where
    A: Clone + Debug + for<'a> arbitrary::Arbitrary<'a> + 'static,
{
    /// Searches for a failing case and panics with the shrunk [`Report`] if one is found.
    #[track_caller]
    pub fn check(&self, config: Config) {
        if let Err(report) = self.search_from(config, Location::caller().file()) {
            panic!("{report}");
        }
    }

    /// Searches for a failing case and returns the shrunk [`Report`] instead of panicking.
    #[track_caller]
    pub fn search(&self, config: Config) -> Result<(), Box<Report>> {
        self.search_from(config, Location::caller().file())
    }

    fn search_from(&self, config: Config, source_file: &'static str) -> Result<(), Box<Report>> {
        let strategy = (
            seed_strategy(),
            vec(self.step_strategy(&config), 0..=config.max_steps),
        );
        let mut runner = TestRunner::new(ProptestConfig {
            cases: config.cases,
            max_shrink_iters: config.max_shrink_iters,
            max_shrink_time: config.max_shrink_time.as_millis().min(u32::MAX as u128) as u32,
            source_file: Some(source_file),
            failure_persistence: config.persist_failures.then(|| {
                Box::new(FileFailurePersistence::SourceParallel(
                    "proptest-regressions",
                )) as Box<dyn proptest::test_runner::FailurePersistence>
            }),
            rng_seed: env_var("GPUI_PROPTEST_SEED")
                .map(RngSeed::Fixed)
                .unwrap_or(RngSeed::Random),
            ..ProptestConfig::default()
        });

        let original_steps = Cell::new(None);
        let nondeterministic = RefCell::new(None);
        // proptest reports the last input that failed, so this is its outcome.
        let last_failure = RefCell::new(None);
        let result = runner.run(&strategy, |(seed, steps)| {
            if nondeterministic.borrow().is_some() {
                // Shrinking noise finds nothing; end it quickly.
                return Ok(());
            }
            let outcome = self.run_case(seed, &steps);
            let Some(failure) = outcome.failure() else {
                return Ok(());
            };
            let message = failure.to_string();
            if original_steps.get().is_none() {
                original_steps.set(Some(steps.len()));
                if config.check_determinism {
                    let again = self.run_case(seed, &steps);
                    if again != outcome {
                        *nondeterministic.borrow_mut() = Some(Report::nondeterministic(
                            outcome.clone(),
                            again,
                            rust(&steps),
                        ));
                        return Err(TestCaseError::fail("non-deterministic"));
                    }
                }
            }
            *last_failure.borrow_mut() = Some(outcome);
            Err(TestCaseError::fail(message))
        });

        match result {
            Ok(()) => Ok(()),
            Err(_) if nondeterministic.borrow().is_some() => {
                Err(Box::new(nondeterministic.into_inner().unwrap()))
            }
            Err(TestError::Fail(_, (seed, steps))) => {
                let original = original_steps.get().unwrap_or(steps.len());
                let (_, minimal, outcome) = self.minimize(
                    seed,
                    steps.clone(),
                    config.max_shrink_iters,
                    config.max_shrink_time,
                );
                if outcome.is_ok() {
                    // The failure did not replay: the determinism check is off and
                    // the case is flaky.
                    let first = last_failure
                        .into_inner()
                        .expect("a failing case recorded its outcome");
                    return Err(Box::new(Report::nondeterministic(
                        first,
                        outcome,
                        rust(&steps),
                    )));
                }
                Err(Box::new(Report::new(outcome, rust(&minimal), original)))
            }
            Err(TestError::Abort(reason)) if reason.message().contains("local rejects") => panic!(
                "gpui-proptest aborted: the domain action type's `Arbitrary` impl rejects \
                 almost every input, so no steps could be generated ({reason})"
            ),
            Err(TestError::Abort(reason)) => panic!("gpui-proptest aborted: {reason}"),
        }
    }

    fn step_strategy(&self, config: &Config) -> BoxedStrategy<Step<A>> {
        let target = any::<u16>;
        let mut steps: Vec<(u32, BoxedStrategy<Step<A>>)> = Vec::new();
        if self.has_ui_steps() {
            let text = string_regex(&config.text).expect("Config::with_text is a valid regex");
            steps.extend([
                (
                    30,
                    (target(), modifier_strategy())
                        .prop_map(|(target, modifiers)| Step::Click { target, modifiers })
                        .boxed(),
                ),
                (
                    5,
                    target()
                        .prop_map(|target| Step::DoubleClick { target })
                        .boxed(),
                ),
                (
                    3,
                    target()
                        .prop_map(|target| Step::RightClick { target })
                        .boxed(),
                ),
                (
                    8,
                    target().prop_map(|target| Step::Hover { target }).boxed(),
                ),
                (
                    5,
                    (target(), -5i8..=5)
                        .prop_map(|(target, lines)| Step::Scroll { target, lines })
                        .boxed(),
                ),
                (20, any::<u8>().prop_map(|key| Step::Press { key }).boxed()),
                (10, text.prop_map(|text| Step::Type { text }).boxed()),
                (
                    5,
                    (0u16..2_000)
                        .prop_map(|millis| Step::AdvanceClock { millis })
                        .boxed(),
                ),
            ]);
        }
        if self.has_domain() {
            steps.push((25, domain_strategy::<A>().prop_map(Step::Domain).boxed()));
        }
        assert!(
            !steps.is_empty(),
            "the harness generates no steps: turn UI steps on or add a domain"
        );
        Union::new_weighted(steps).boxed()
    }
}

/// The scheduler seed. Shrinks toward 0, which shows whether a failure depends
/// on task interleaving at all.
fn seed_strategy() -> BoxedStrategy<u64> {
    match env_var::<u64>("SEED") {
        Some(seed) => Just(seed).boxed(),
        None => any::<u64>().boxed(),
    }
}

fn modifier_strategy() -> BoxedStrategy<ModifierKeys> {
    prop_oneof![
        6 => Just(ModifierKeys::NONE),
        1 => (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()).prop_map(
            |(shift, control, alt, platform)| ModifierKeys {
                shift,
                control,
                alt,
                platform,
            }
        ),
    ]
    .boxed()
}

/// Domain actions come from `arbitrary` bytes, so the same derive serves every
/// engine. Shrinking the bytes toward empty shrinks toward the simplest action.
fn domain_strategy<A>() -> BoxedStrategy<A>
where
    A: Debug + for<'a> arbitrary::Arbitrary<'a> + 'static,
{
    vec(any::<u8>(), 0..64)
        .prop_filter_map("action bytes did not decode", |bytes| {
            A::arbitrary(&mut arbitrary::Unstructured::new(&bytes)).ok()
        })
        .boxed()
}
