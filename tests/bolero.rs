use gpui::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_proptest::{Harness, MAX_FUZZ_STEPS, ProptestTargetExt, Step, ensure_eq};

/// A counter whose reset is wrong when the count is exactly 2.
#[derive(Default)]
struct Counter {
    count: i32,
    reset_clicked: bool,
    buggy: bool,
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id("increment")
                    .w(px(80.))
                    .h(px(24.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count += 1;
                        this.reset_clicked = false;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
            .child(
                div()
                    .id("reset")
                    .w(px(80.))
                    .h(px(24.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count = if this.buggy && this.count == 2 { 1 } else { 0 };
                        this.reset_clicked = true;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
    }
}

fn harness(buggy: bool) -> Harness<Counter> {
    Harness::new(move |_, cx| {
        cx.new(|_| Counter {
            buggy,
            ..Counter::default()
        })
    })
    .invariant("reset zeroes the count", |counter, _, _, cx| {
        let counter = counter.read(cx);
        if counter.reset_clicked {
            ensure_eq!(counter.count, 0);
        }
        Ok(())
    })
}

#[test]
fn a_correct_counter_survives_fuzzing() {
    gpui_proptest::fuzz!(harness(false), |check| check
        .with_iterations(200)
        .with_max_len(256));
}

#[test]
fn fuzzing_finds_and_minimizes_the_reset_bug() {
    let payload = std::panic::catch_unwind(|| {
        gpui_proptest::fuzz!(harness(true), |check| check.with_iterations(5_000));
    })
    .expect_err("the reset bug is found");
    let report = payload
        .downcast_ref::<String>()
        .expect("the panic carries the report");
    assert!(report.contains("[reset zeroes the count]"), "{report}");
    // Two increments then reset, or a double-click then reset.
    let steps = report.matches("Step::").count();
    assert!((2..=3).contains(&steps), "{report}");
    assert!(report.contains(".replay("), "{report}");
}

#[test]
fn decoding_is_total_and_bounded() {
    let harness = harness(false);
    assert_eq!(harness.decode(&[]), (0, Vec::new()));
    let (_, steps) = harness.decode(&[0xab; 4096]);
    assert!(steps.len() <= MAX_FUZZ_STEPS);
    // The same bytes always decode to the same case.
    assert_eq!(
        harness.decode(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
        harness.decode(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])
    );
}

#[test]
fn decoded_clock_steps_are_capped_like_proptest() {
    let harness = harness(false);
    let millis: Vec<u16> = (0..=u8::MAX)
        .flat_map(|byte| harness.decode(&[byte; 256]).1)
        .filter_map(|step| match step {
            Step::AdvanceClock { millis } => Some(millis),
            _ => None,
        })
        .collect();
    assert!(!millis.is_empty());
    assert!(millis.iter().all(|millis| *millis < 2_000), "{millis:?}");
}

#[test]
fn domain_only_harnesses_decode_domain_steps() {
    let harness = harness(false)
        .with_domain(|_: &u8, _, _, _| true)
        .with_ui_steps(false);
    let (_, steps) = harness.decode(&[9; 64]);
    assert!(!steps.is_empty());
    assert!(steps.iter().all(|step| matches!(step, Step::Domain(_))));
}

/// Domain actions decoded one byte each: even bytes poke, odd bytes check.
#[derive(Clone, Debug)]
enum Probe {
    Poke,
    Check,
}

impl<'a> gpui_proptest::arbitrary::Arbitrary<'a> for Probe {
    fn arbitrary(
        input: &mut gpui_proptest::arbitrary::Unstructured<'a>,
    ) -> gpui_proptest::arbitrary::Result<Self> {
        Ok(match u8::arbitrary(input)? % 2 {
            0 => Probe::Poke,
            _ => Probe::Check,
        })
    }
}

/// A seed of 0, then one byte per action.
fn probes(actions: &[u8]) -> Vec<u8> {
    [[0; 8].as_slice(), actions].concat()
}

thread_local! { static POKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

/// Panics once poked, as a buggy custom source might.
struct PokedSource;
impl gpui_proptest::TargetSource for PokedSource {
    fn targets(&self, _: &Window, _: &gpui::App) -> Vec<gpui_proptest::TargetInfo> {
        assert!(!POKED.get(), "the source broke");
        Vec::new()
    }
}

fn probe_harness(perform: impl Fn(&Probe) -> bool + 'static) -> Harness<Counter, Probe> {
    Harness::new(|_, cx| {
        POKED.set(false);
        cx.new(|_| Counter::default())
    })
    .with_ui_steps(false)
    .with_domain(move |probe: &Probe, _, _, _| perform(probe))
}

#[test]
fn fuzz_keeps_the_step_whose_targets_failed() {
    // The poke does not apply, so no oracle runs after it; the source fails
    // while the next step reads targets.
    let harness = probe_harness(|_| {
        POKED.set(true);
        false
    })
    .with_target_source(PokedSource);
    let bytes = probes(&[0, 0]);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| harness.fuzz(&bytes)))
        .expect_err("the source fails");
    let report = harness.take_fuzz_report().expect("the failure replays");
    assert!(
        report.failure().message().contains("the source broke"),
        "{report}"
    );
}

#[test]
fn minimizing_keeps_the_failure_message() {
    // Checking alone panics too, but with another message: a different bug.
    let harness = probe_harness(|probe| {
        match probe {
            Probe::Poke => POKED.set(true),
            Probe::Check if POKED.get() => panic!("checked after a poke"),
            Probe::Check => panic!("checked alone"),
        }
        true
    });
    let bytes = probes(&[0, 1]);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| harness.fuzz(&bytes)))
        .expect_err("the check panics");
    let report = harness.take_fuzz_report().expect("the failure replays");
    assert_eq!(
        report.failure().message(),
        "checked after a poke",
        "{report}"
    );
    assert_eq!(report.steps().len(), 2, "{report}");
}
