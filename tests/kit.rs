use gpui_kit::component::{Disableable as _, checkbox::Checkbox, switch::Switch};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _, Window, div,
};
use gpui_proptest::{Config, Harness, ModifierKeys, Step, ensure_eq, kit};

/// A settings form. With `drop_every_second_change`, every second change reaches
/// the checkbox but not the model, so the two drift apart.
#[derive(Default)]
struct Form {
    agreed: bool,
    shown: bool,
    notifications: bool,
    changes: u32,
    drop_every_second_change: bool,
}

impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                Checkbox::new("agree")
                    .label("Agree")
                    .checked(self.shown)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        this.changes += 1;
                        this.shown = *checked;
                        if !(this.drop_every_second_change && this.changes % 2 == 0) {
                            this.agreed = *checked;
                        }
                        cx.notify();
                    })),
            )
            .child(
                Switch::new("notifications")
                    .checked(self.notifications)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        this.notifications = *checked;
                        cx.notify();
                    })),
            )
            .child(
                Checkbox::new("locked")
                    .label("Locked")
                    .checked(true)
                    .disabled(true),
            )
    }
}

fn harness(drop_every_second_change: bool) -> Harness<Form> {
    kit::harness(move |_, cx| {
        cx.new(|_| Form {
            drop_every_second_change,
            ..Form::default()
        })
    })
    .invariant("agree checkbox matches the model", |form, _, window, cx| {
        let agreed = form.read(cx).agreed;
        ensure_eq!(window.find("agree").checked(), Some(agreed));
        Ok(())
    })
}

#[test]
fn kit_elements_are_targets() {
    let click = |target| Step::Click {
        target,
        modifiers: ModifierKeys::NONE,
    };
    let outcome = harness(false).run_case(0, &[click(0), click(1), click(2)]);
    assert!(outcome.is_ok(), "{outcome:?}");
    let descriptions: Vec<_> = outcome
        .trace()
        .records()
        .iter()
        .map(|record| record.description().to_string())
        .collect();
    assert!(descriptions[0].contains("agree"), "{descriptions:?}");
    assert!(
        descriptions[1].contains("notifications"),
        "{descriptions:?}"
    );
    assert!(descriptions[2].contains("locked"), "{descriptions:?}");
}

#[test]
fn a_correct_form_survives_random_use() {
    harness(false).check(
        Config::default()
            .with_cases(48)
            .with_persist_failures(false),
    );
}

#[test]
fn finds_a_checkbox_that_drifts_from_its_model() {
    let report = harness(true)
        .search(
            Config::default()
                .with_cases(256)
                .with_persist_failures(false),
        )
        .expect_err("the dropped change is found");
    assert_eq!(
        report.failure().oracle(),
        "agree checkbox matches the model"
    );
    assert!(report.steps().len() <= 2, "{report}");
}
