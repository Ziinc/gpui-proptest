//! Hostile views and misconfigurations: the harness must report them, not hang
//! or blame the wrong thing.

use gpui::{
    AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_proptest::{Config, Harness, ModifierKeys, ProptestTargetExt, Step, ensure};
use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};

fn click(target: u16) -> Step {
    Step::Click {
        target,
        modifiers: ModifierKeys::NONE,
    }
}

fn config() -> Config {
    Config::default().with_persist_failures(false)
}

/// Runs `f` on its own thread and fails if it does not finish in time.
fn within<R: Send + 'static>(secs: u64, f: impl FnOnce() -> R + Send + 'static) -> R {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .expect("hung: did not finish in time")
}

// A. A click that closes the window ends the case silently.
struct Closer;
impl Render for Closer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("close")
                .size(px(40.))
                .on_click(|_, window, _| window.remove_window())
                .proptest_target(),
        )
    }
}

#[test]
fn a_closing_window() {
    let outcome = Harness::new(|_, cx| cx.new(|_| Closer)).run_case(0, &[click(0), click(0)]);
    let failure = outcome
        .failure()
        .expect("closing the window fails the case");
    assert_eq!(failure.oracle(), "window-closed");
    assert_eq!(failure.step(), Some(0));

    // When closing is allowed, the rest of the steps are recorded as not run.
    let outcome = Harness::new(|_, cx| cx.new(|_| Closer))
        .with_window_may_close(true)
        .run_case(0, &[click(0), click(0)]);
    assert!(outcome.is_ok(), "{outcome:?}");
    let records = outcome.trace().records();
    assert_eq!(records.len(), 2);
    assert!(records[1].is_skipped());
    assert!(
        records[1].description().contains("window closed"),
        "{outcome:?}"
    );
}

// A2. Typing a key that closes the window ends the case like a closing click;
// the keys after it are not pressed.
struct QuitOnQ(FocusHandle);
impl Render for QuitOnQ {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.0)
            .on_key_down(|event, window, _| {
                if event.keystroke.key == "q" {
                    window.remove_window();
                }
            })
    }
}

fn quit_on_q() -> Harness<QuitOnQ> {
    Harness::new(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.new(|_| QuitOnQ(focus))
    })
}

#[test]
fn a_typing_that_closes_the_window() {
    let steps = [Step::Type { text: "qx".into() }];
    let outcome = quit_on_q().run_case(0, &steps);
    let failure = outcome
        .failure()
        .expect("closing the window fails the case");
    assert_eq!(failure.oracle(), "window-closed");

    let outcome = quit_on_q().with_window_may_close(true).run_case(0, &steps);
    assert!(outcome.is_ok(), "{outcome:?}");

    // A multi-key press stops at the key that closed the window.
    let outcome = quit_on_q()
        .with_keys(["q x"])
        .with_window_may_close(true)
        .run_case(0, &[Step::Press { key: 0 }]);
    assert!(outcome.is_ok(), "{outcome:?}");
}

// B. A target under an `occlude()` overlay. Clicks aim at an uncovered point, and
// a target covered everywhere is not chosen at all.
struct Covered {
    cover_width: f32,
    clicked: bool,
}
impl Render for Covered {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("under")
                    .size(px(40.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clicked = true;
                        cx.notify();
                    }))
                    .proptest_target(),
            )
            .child(
                div()
                    .id("overlay")
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(self.cover_width))
                    .h_full()
                    .occlude(),
            )
    }
}

fn covered(cover_width: f32) -> Harness<Covered> {
    Harness::new(move |_, cx| {
        cx.new(|_| Covered {
            cover_width,
            clicked: false,
        })
    })
    .invariant("clicks reach the target", |view, _, _, cx| {
        ensure!(!view.read(cx).clicked, "clicked");
        Ok(())
    })
}

#[test]
fn b_fully_covered_target_is_not_chosen() {
    let outcome = covered(800.).run_case(0, &[click(0)]);
    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(outcome.trace().records()[0].is_skipped());
}

#[test]
fn b_partly_covered_target_is_clicked_where_uncovered() {
    // The overlay covers the centre; the right edge stays clickable.
    let outcome = covered(30.).run_case(0, &[click(0)]);
    let failure = outcome.failure().expect("the click reached the target");
    assert_eq!(failure.message(), "clicked");
}

// C. A task that never parks.
struct Spinner;
impl Render for Spinner {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("spin")
                .size(px(40.))
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.spawn(async move |_, _| {
                        loop {
                            YieldNow(false).await;
                        }
                    })
                    .detach();
                }))
                .proptest_target(),
        )
    }
}

struct YieldNow(bool);
impl std::future::Future for YieldNow {
    type Output = ();
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        if self.0 {
            std::task::Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    }
}

#[test]
fn c_task_that_never_parks() {
    let oracle = within(60, || {
        let outcome = Harness::new(|_, cx| cx.new(|_| Spinner)).run_case(0, &[click(0)]);
        outcome.failure().map(|failure| failure.oracle().to_owned())
    });
    assert_eq!(oracle.as_deref(), Some("livelock"));
}

// D. A periodic timer, like a caret blink, must not hang or fail teardown.
struct Ticker {
    ticks: u32,
}
impl Render for Ticker {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.ticks.to_string())
    }
}

#[test]
fn d_periodic_timer() {
    let outcome = within(30, || {
        let outcome = Harness::new(|_, cx| {
            cx.new(|cx: &mut Context<Ticker>| {
                cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                        if this
                            .update(cx, |t, cx| {
                                t.ticks += 1;
                                cx.notify();
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .detach();
                Ticker { ticks: 0 }
            })
        })
        .run_case(0, &vec![Step::AdvanceClock { millis: 1999 }; 3]);
        format!("{outcome:?}")
    });
    assert!(outcome.contains("failure: None"), "{outcome}");
}

// E. State that leaks across cases through a static: the replay differs.
static LEAK: AtomicU32 = AtomicU32::new(0);
struct Leaky;
impl Render for Leaky {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("leak")
                .size(px(40.))
                .on_click(|_, _, _| {
                    let n = LEAK.fetch_add(1, Ordering::SeqCst);
                    assert!(n % 7 != 6, "leaked state hit {n}");
                })
                .proptest_target(),
        )
    }
}

#[test]
fn e_cross_case_leak() {
    let report = Harness::new(|_, cx| cx.new(|_| Leaky))
        .search(config().with_cases(64))
        .expect_err("fails eventually");
    assert!(report.is_nondeterministic(), "{report}");
}

// E2. The same with the determinism check off: the failure does not replay when
// minimizing, and the report says the case is flaky instead of holding a pass.
static FLAKY: AtomicU32 = AtomicU32::new(0);

#[test]
fn e_flaky_failure_without_determinism_check() {
    let report = Harness::new(|_, cx| cx.new(|_| Plain))
        .invariant("first run only", |_, _, _, _| {
            ensure!(FLAKY.fetch_add(1, Ordering::SeqCst) != 0, "flaky");
            Ok(())
        })
        .search(config().with_cases(4).with_check_determinism(false))
        .expect_err("the first run fails");
    assert!(report.is_nondeterministic(), "{report}");
    assert_eq!(report.failure().message(), "flaky");
    assert!(
        report.to_string().contains("second run: passed"),
        "{report}"
    );
}

// G. A misspelled key in with_keys is reported as a bug in the view.
#[derive(Default)]
struct Plain;
impl Render for Plain {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(div().id("a").size(px(40.)).proptest_target())
    }
}

#[test]
#[should_panic(expected = "not a key GPUI recognises")]
fn g_bad_key() {
    let _ = Harness::new(|_, cx| cx.new(|_| Plain)).with_keys(["ctrl-shift-banana"]);
}

#[test]
fn g_good_keys() {
    let outcome = Harness::new(|_, cx| cx.new(|_| Plain))
        .with_keys([
            "cmd-z",
            "ctrl-shift-a",
            "f12",
            "pagedown",
            "-",
            "shift",
            "escape enter",
        ])
        .run_case(
            0,
            &(0..7).map(|key| Step::Press { key }).collect::<Vec<_>>(),
        );
    assert!(outcome.is_ok(), "{outcome:?}");
}

// H. An invariant that panics instead of returning Err.
#[test]
fn h_panicking_invariant() {
    let outcome = Harness::new(|_, cx| cx.new(|_| Plain))
        .invariant("never panics", |_, _, _, _| panic!("invariant blew up"))
        .run_case(0, &[]);
    let failure = outcome.failure().unwrap();
    assert_eq!(failure.oracle(), "never panics");
    assert!(failure.message().contains("invariant blew up"), "{failure}");
}

// I. A panic in the view's Drop at teardown.
struct BadDrop;
impl Render for BadDrop {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
impl Drop for BadDrop {
    fn drop(&mut self) {
        panic!("drop blew up");
    }
}

#[test]
fn i_panic_in_drop() {
    let outcome = Harness::new(|_, cx| cx.new(|_| BadDrop)).run_case(0, &[]);
    assert_eq!(outcome.failure().unwrap().oracle(), "teardown");
}

// J. Domain actions that never decode.
#[derive(Clone, Debug)]
struct Undecodable;
impl<'a> gpui_proptest::arbitrary::Arbitrary<'a> for Undecodable {
    fn arbitrary(
        _: &mut gpui_proptest::arbitrary::Unstructured<'a>,
    ) -> gpui_proptest::arbitrary::Result<Self> {
        Err(gpui_proptest::arbitrary::Error::IncorrectFormat)
    }
}

#[test]
#[should_panic(expected = "rejects almost every input")]
fn j_undecodable_domain() {
    let _ = Harness::new(|_, cx| cx.new(|_| Plain))
        .with_domain(|_: &Undecodable, _, _, _| true)
        .with_ui_steps(false)
        .search(config().with_cases(8));
}

// S. Typed text with quotes and newlines must give compilable regression source.
#[test]
fn s_escaping() {
    let step: Step = Step::Type {
        text: "a\"b\n\\c".into(),
    };
    let rust = step.to_rust();
    assert!(rust.contains(r#""a\"b\n\\c""#), "{rust}");
}

// T. A target whose layout shifts on hover: the click lands where it was.
struct Shifty {
    hovered: bool,
    clicks: u32,
}
impl Render for Shifty {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .when(self.hovered, |d| d.pt(px(100.)))
            .child(
                div()
                    .id("shifty")
                    .size(px(40.))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.hovered = *hovered;
                        cx.notify();
                    }))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clicks += 1;

                        cx.notify();
                    }))
                    .proptest_target(),
            )
    }
}
use gpui::prelude::FluentBuilder as _;

#[test]
fn t_layout_shift_on_hover() {
    let outcome = within(60, || {
        Harness::new(|_, cx| {
            cx.new(|_| Shifty {
                hovered: false,
                clicks: 0,
            })
        })
        .run_case(0, &[click(0)])
        .failure()
        .map(|failure| failure.oracle().to_owned())
    });
    assert_eq!(outcome.as_deref(), Some("livelock"));
}

// U. Typed text with characters the keystroke parser treats specially.
#[test]
fn u_awkward_text() {
    for text in ["", " ", "-", "a-b", "A", "ä", "->", "日本", "\n", "\t"] {
        let outcome = Harness::new(|_, cx| cx.new(|_| Plain))
            .run_case(0, &[Step::Type { text: text.into() }]);
        assert!(outcome.is_ok(), "{text:?}: {outcome:?}");
    }
}

// N. Two targets with the same id under one parent share GPUI element state.
struct Twins;
impl Render for Twins {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(div().id("twin").size(px(40.)).proptest_target())
            .child(div().id("twin").size(px(40.)).proptest_target())
    }
}

#[test]
fn n_duplicate_ids() {
    let outcome = Harness::new(|_, cx| cx.new(|_| Twins)).run_case(0, &[]);
    let failure = outcome.failure().expect("the shared id is reported");
    assert_eq!(failure.oracle(), "unique-ids");
    assert!(failure.message().contains("twin"), "{failure}");
    let outcome = Harness::new(|_, cx| cx.new(|_| Twins))
        .with_unique_ids(false)
        .run_case(0, &[click(0)]);
    assert!(outcome.is_ok(), "{outcome:?}");
}

// N2. A target inside an invisible parent is prepainted every frame but never
// painted; that is not a second target with its id.
struct Hidden;
impl Render for Hidden {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .invisible()
                .child(div().id("only").size(px(40.)).proptest_target()),
        )
    }
}

#[test]
fn n_invisible_target_is_not_a_duplicate() {
    let outcome =
        Harness::new(|_, cx| cx.new(|_| Hidden)).run_case(0, &[Step::AdvanceClock { millis: 1 }]);
    assert!(outcome.is_ok(), "{outcome:?}");
}

// T2. The same loop in a view with no targets is caught by the frame guard.
struct UntargetedShifty {
    hovered: bool,
}
impl Render for UntargetedShifty {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .when(self.hovered, |d| d.pt(px(100.)))
            .child(div().id("shifty").size(px(40.)).on_hover(cx.listener(
                |this, hovered: &bool, _, cx| {
                    this.hovered = *hovered;
                    cx.notify();
                },
            )))
    }
}

#[test]
fn t_untargeted_layout_shift_on_hover() {
    let failure = within(60, || {
        Harness::new(|_, cx| cx.new(|_| UntargetedShifty { hovered: false }))
            .run_case(0, &[])
            .failure()
            .map(|failure| (failure.oracle().to_owned(), failure.message().to_owned()))
    });
    let (oracle, message) = failure.expect("the render loop is reported");
    assert_eq!(oracle, "livelock");
    assert!(message.contains("frames in one step"), "{message}");
}
