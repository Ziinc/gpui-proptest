//! Byte-driven cases for coverage-guided and random-byte engines such as bolero,
//! cargo-fuzz and AFL.

use crate::{Harness, Report, Step, step::rust};
use arbitrary::{Arbitrary, Unstructured};
use std::{fmt::Debug, time::Duration};

/// The most steps one input decodes to.
pub const MAX_FUZZ_STEPS: usize = 64;

/// Cases the final minimizing pass may run.
const MINIMIZE_ITERS: u32 = 2_000;

impl<V: 'static, A> Harness<V, A>
where
    A: Debug + for<'a> Arbitrary<'a> + 'static,
{
    /// Decodes `bytes` into a scheduler seed and up to [`MAX_FUZZ_STEPS`] steps, runs
    /// the case, and panics with its [`Report`] if it fails. Engines that shrink
    /// bytes, such as bolero, shrink the steps with them.
    ///
    /// Harnesses without UI steps decode domain actions only.
    #[track_caller]
    pub fn fuzz(&self, bytes: &[u8])
    where
        A: Clone,
    {
        let (seed, mut steps) = self.decode(bytes);
        let original = steps.len();
        let outcome = self.run_case(seed, &steps);
        if let Some(failure) = outcome.failure() {
            // Steps after the failing one never ran.
            steps.truncate(failure.step().map_or(0, |step| step + 1));
            *self.last_fuzz_failure.borrow_mut() = Some((seed, steps.clone()));
            panic!("{}", Report::new(outcome, rust(&steps), original));
        }
    }

    /// The last failure [`Harness::fuzz`] saw, with steps minimized, or `None`.
    /// [`fuzz!`](crate::fuzz!) reports it once the fuzzer has shrunk the input.
    pub fn take_fuzz_report(&self) -> Option<Report>
    where
        A: Clone,
    {
        let (seed, steps) = self.last_fuzz_failure.borrow_mut().take()?;
        let original = steps.len();
        let (_, steps, outcome) =
            self.minimize(seed, steps, MINIMIZE_ITERS, Duration::from_secs(60));
        outcome.failure()?;
        Some(Report::new(outcome, rust(&steps), original))
    }

    /// The `(seed, steps)` input `bytes` decode to. Decoding never fails: missing
    /// bytes read as zeros and end the steps.
    pub fn decode(&self, bytes: &[u8]) -> (u64, Vec<Step<A>>) {
        let mut input = Unstructured::new(bytes);
        let seed = u64::arbitrary(&mut input).unwrap_or(0);
        let mut steps = Vec::new();
        while steps.len() < MAX_FUZZ_STEPS && !input.is_empty() {
            let step = if self.has_ui_steps() {
                Step::<A>::arbitrary(&mut input)
            } else {
                A::arbitrary(&mut input).map(Step::Domain)
            };
            match step {
                Ok(step) => steps.push(step),
                Err(_) => break,
            }
        }
        (seed, steps)
    }
}
