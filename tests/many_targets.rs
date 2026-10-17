use gpui::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px,
};
use gpui_proptest::{Harness, ModifierKeys, ProptestTargetExt, Step};
use std::time::Instant;

struct Grid(usize);
impl Render for Grid {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_wrap()
            .children((0..self.0).map(|i| div().id(("cell", i)).size(px(8.)).proptest_target()))
    }
}

#[test]
#[ignore = "timing; run with --ignored --nocapture"]
fn many_targets_timing() {
    for n in [10, 500, 2000] {
        let steps: Vec<Step> = (0..40u16)
            .map(|i| Step::Click {
                target: i * 37,
                modifiers: ModifierKeys::NONE,
            })
            .collect();
        let harness = Harness::new(move |_, cx| cx.new(|_| Grid(n)));
        let _ = harness.run_case(0, &steps);
        let start = Instant::now();
        let outcome = harness.run_case(0, &steps);
        eprintln!("{n} targets, 40 clicks: {:?}", start.elapsed());
        assert!(outcome.is_ok());
    }
}

struct Bare(usize, bool);
impl Render for Bare {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let wrap = self.1;
        div()
            .size_full()
            .flex()
            .flex_wrap()
            .children((0..self.0).map(move |i| {
                let cell = div().id(("cell", i)).size(px(8.));
                if wrap {
                    cell.proptest_target().into_any_element()
                } else {
                    cell.into_any_element()
                }
            }))
    }
}

#[test]
#[ignore = "timing; run with --ignored --nocapture"]
fn frame_overhead_timing() {
    let steps: Vec<Step> = (0..40).map(|_| Step::AdvanceClock { millis: 1 }).collect();
    for wrap in [false, true] {
        let harness = Harness::new(move |_, cx| cx.new(|_| Bare(2000, wrap)));
        let _ = harness.run_case(0, &steps);
        let start = Instant::now();
        let _ = harness.run_case(0, &steps);
        eprintln!(
            "2000 cells, proptest_target={wrap}, 40 frames: {:?}",
            start.elapsed()
        );
    }
}
