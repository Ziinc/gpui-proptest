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
fn domain_only_harnesses_decode_domain_steps() {
    let harness = harness(false)
        .with_domain(|_: &u8, _, _, _| true)
        .with_ui_steps(false);
    let (_, steps) = harness.decode(&[9; 64]);
    assert!(!steps.is_empty());
    assert!(steps.iter().all(|step| matches!(step, Step::Domain(_))));
}
