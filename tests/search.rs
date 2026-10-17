use gpui::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_proptest::{Config, Harness, ModifierKeys, ProptestTargetExt, Step, ensure_eq};

/// A counter whose reset is wrong when the count is exactly 2.
#[derive(Default)]
struct Counter {
    count: i32,
    reset_clicked: bool,
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button = |id: &'static str| div().id(id).w(px(80.)).h(px(24.));
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                button("increment")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count += 1;
                        this.reset_clicked = false;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
            .child(
                button("decrement")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count -= 1;
                        this.reset_clicked = false;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
            .child(
                button("reset")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count = if this.count == 2 { 1 } else { 0 };
                        this.reset_clicked = true;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
    }
}

fn harness() -> Harness<Counter> {
    Harness::new(|_, cx| cx.new(|_| Counter::default())).invariant(
        "reset zeroes the count",
        |counter, _, _, cx| {
            let counter = counter.read(cx);
            if counter.reset_clicked {
                ensure_eq!(counter.count, 0);
            }
            Ok(())
        },
    )
}

fn config() -> Config {
    Config::default().with_persist_failures(false)
}

fn click(target: u16) -> Step {
    Step::Click {
        target,
        modifiers: ModifierKeys::NONE,
    }
}

#[test]
fn finds_and_shrinks_to_the_minimal_sequence() {
    let report = harness()
        .search(config().with_cases(512).with_max_steps(30))
        .expect_err("the reset bug is found");

    assert_eq!(report.failure().oracle(), "reset zeroes the count");
    // The shortest failures are two increments then reset, or a double-click on
    // increment (two clicks) then reset.
    assert!(report.steps().len() <= 3, "{report}");
    let records = report.trace().records();
    assert_eq!(
        records.last().unwrap().description(),
        "click (reset)",
        "{report}"
    );
    assert!(report.original_steps() >= report.steps().len());
    // The emitted test replays the same failure.
    assert!(report.regression_test("harness()").contains(".replay("));
    let replayed = harness().run_case(report.seed(), &replay_steps(&report));
    assert_eq!(replayed.failure(), Some(report.failure()));
}

fn replay_steps(report: &gpui_proptest::Report) -> Vec<Step> {
    report
        .trace()
        .records()
        .iter()
        .map(|record| match record.description() {
            "click (increment)" => click(0),
            "double-click (increment)" => Step::DoubleClick { target: 0 },
            "click (reset)" => click(2),
            other => panic!("unexpected step {other}"),
        })
        .collect()
}

#[test]
fn targets_resolve_top_to_bottom_and_wrap() {
    let outcome = harness().run_case(7, &[click(0), click(4), click(2)]);
    let descriptions: Vec<_> = outcome
        .trace()
        .records()
        .iter()
        .map(|record| record.description().to_string())
        .collect();
    // Choice 4 wraps to the second of three targets.
    assert_eq!(
        descriptions,
        ["click (increment)", "click (decrement)", "click (reset)"]
    );
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn the_same_input_gives_the_same_outcome() {
    let steps = [
        click(0),
        Step::Press { key: 3 },
        Step::Type { text: "abc".into() },
        Step::AdvanceClock { millis: 250 },
        click(0),
        click(2),
    ];
    let first = harness().run_case(42, &steps);
    let second = harness().run_case(42, &steps);
    assert_eq!(first, second);
    assert_eq!(first.failure().map(|failure| failure.step()), Some(Some(5)));
}

#[test]
#[should_panic(expected = "reset zeroes the count")]
fn replay_panics_while_the_bug_exists() {
    harness().replay(0, &[click(0), click(0), click(2)]);
}

/// Panics on its third click.
#[derive(Default)]
struct Fragile {
    clicks: u32,
}

impl Render for Fragile {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("fragile")
                .size(px(40.))
                .on_click(cx.listener(|this, _, _, _| {
                    this.clicks += 1;
                    assert!(this.clicks < 3, "clicked {} times", this.clicks);
                }))
                .proptest_target(),
        )
    }
}

#[test]
fn panics_in_listeners_are_failures() {
    let report = Harness::new(|_, cx| cx.new(|_| Fragile::default()))
        .search(config().with_cases(256))
        .expect_err("the panic is found");
    assert_eq!(report.failure().oracle(), "panic");
    assert!(report.failure().message().contains("clicked 3 times"));
    // Three clicks, or a click and a double-click.
    assert!(report.steps().len() <= 3, "{report}");
}

/// A checkbox that is marked disabled but still toggles when clicked.
#[derive(Default)]
struct LeakyDisabled {
    checked: bool,
}

impl Render for LeakyDisabled {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toggled = if self.checked {
            gpui::accesskit::Toggled::True
        } else {
            gpui::accesskit::Toggled::False
        };
        div().size_full().child(
            div()
                .id("locked")
                .size(px(40.))
                .role(gpui::Role::CheckBox)
                .aria_toggled(toggled)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.checked = !this.checked;
                    cx.notify();
                }))
                .proptest_target()
                .disabled(true),
        )
    }
}

#[test]
fn disabled_targets_must_be_inert() {
    let outcome =
        Harness::new(|_, cx| cx.new(|_| LeakyDisabled::default())).run_case(0, &[click(0)]);
    let failure = outcome.failure().expect("the disabled click changed state");
    assert_eq!(failure.oracle(), "disabled-is-inert", "{failure}");
}

#[test]
fn steps_without_targets_are_skipped() {
    struct Empty;
    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    let outcome = Harness::new(|_, cx| cx.new(|_| Empty)).run_case(0, &[click(3)]);
    assert!(outcome.is_ok());
    assert!(outcome.trace().records()[0].is_skipped());
}

#[test]
fn steps_derive_arbitrary() {
    use gpui_proptest::arbitrary::{Arbitrary, Unstructured};
    let bytes = [7u8; 64];
    let steps = Vec::<Step>::arbitrary(&mut Unstructured::new(&bytes)).unwrap();
    let _ = harness().run_case(1, &steps);
}
