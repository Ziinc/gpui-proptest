//! Property-based testing for GPUI.
//!
//! gpui-proptest generates random sequences of user actions against a real GPUI
//! window, checks oracles and invariants after every step, and shrinks any failure
//! to the shortest sequence that still fails, printed with a replayable regression
//! test.
//!
//! ```ignore
//! use gpui_proptest::{Config, Harness, ProptestTargetExt, ensure};
//!
//! #[test]
//! fn counter_never_goes_negative() {
//!     Harness::new(|_, cx| cx.new(|_| Counter::default()))
//!         .invariant("count is not negative", |counter, _, _, cx| {
//!             ensure!(counter.read(cx).count >= 0);
//!             Ok(())
//!         })
//!         .check(Config::default());
//! }
//! ```
//!
//! Elements become targets for clicks, hovers and scrolls with
//! [`proptest_target`](ProptestTargetExt::proptest_target); with the `kit` feature,
//! every gpui-kit element observed with `.test_support()` is a target too.
//!
//! Each case runs on a fresh `TestAppContext` built from a `TestDispatcher` seeded
//! with the case's seed, so the step sequence and GPUI's task interleaving replay
//! exactly. The core never imports an engine: [`Harness::run_case`] replays one
//! `(seed, steps)` input, and [`Harness::fuzz`] decodes one from bytes. The
//! `proptest` feature (default) adds [`Harness::check`]; the `bolero` feature adds
//! [`fuzz!`].

#[cfg(feature = "proptest")]
mod engine;
mod fuzz;
mod guard;
mod harness;
#[cfg(feature = "kit")]
pub mod kit;
mod outcome;
mod state_machine;
mod step;
mod target;

#[cfg(feature = "proptest")]
pub use engine::Config;
pub use fuzz::MAX_FUZZ_STEPS;
pub use guard::FrameGuard;
pub use harness::{DEFAULT_KEYS, Harness};
pub use outcome::{Failure, Outcome, Report, Trace};
pub use state_machine::StateMachine;
pub use step::{ModifierKeys, NoDomain, Step, StepRecord};
pub use target::{ObservedTargets, ProptestTargetExt, Target, TargetInfo, TargetSource};

/// Re-exported so domain actions can derive `Arbitrary` without a direct dependency.
pub use arbitrary;

/// Re-exported for [`fuzz!`].
#[cfg(feature = "bolero")]
pub use bolero;

/// Runs a harness under bolero: random bytes with `cargo test`, or libFuzzer, AFL
/// and honggfuzz with `cargo bolero test`. Each input is decoded by
/// [`Harness::fuzz`]; bolero shrinks a failing input, the steps it decodes to are
/// minimized like the proptest engine's, and the panic carries the [`Report`].
///
/// ```ignore
/// #[test]
/// fn counter_fuzz() {
///     gpui_proptest::fuzz!(harness());
/// }
/// ```
///
/// Configure the run with bolero's builder methods after the harness:
///
/// ```ignore
/// gpui_proptest::fuzz!(harness(), |check| check.with_iterations(500));
/// ```
#[cfg(feature = "bolero")]
#[macro_export]
macro_rules! fuzz {
    ($harness:expr) => {
        $crate::fuzz!($harness, |check| check)
    };
    ($harness:expr, |$check:ident| $configure:expr) => {{
        // Each case builds a fresh app, so a panic leaves nothing in the harness
        // half-updated.
        let harness = ::std::panic::AssertUnwindSafe($harness);
        let harness = &harness;
        let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
            let $check = $crate::bolero::check!();
            $configure.for_each(move |bytes: &[u8]| harness.0.fuzz(bytes));
        }));
        if let Err(payload) = result {
            // bolero has shrunk the bytes; minimize the steps they decode to.
            match harness.0.take_fuzz_report() {
                Some(report) => panic!("{report}"),
                None => ::std::panic::resume_unwind(payload),
            }
        }
    }};
}
