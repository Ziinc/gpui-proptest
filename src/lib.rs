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
//! `(seed, steps)` input, and [`Step`] derives `arbitrary::Arbitrary` so engines
//! other than proptest can drive it.

#[cfg(feature = "proptest")]
mod engine;
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
pub use guard::FrameGuard;
pub use harness::{DEFAULT_KEYS, Harness};
pub use outcome::{Failure, Outcome, Report, Trace};
pub use state_machine::StateMachine;
pub use step::{ModifierKeys, NoDomain, Step, StepRecord};
pub use target::{ObservedTargets, ProptestTargetExt, Target, TargetInfo, TargetSource};

/// Re-exported so domain actions can derive `Arbitrary` without a direct dependency.
pub use arbitrary;
