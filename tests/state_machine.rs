use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, IntoElement,
    ParentElement as _, Render, Window, WindowBounds, WindowOptions, div, point, px, size,
};
use gpui_proptest::{Config, Failure, Harness, StateMachine, arbitrary::Arbitrary, ensure_eq};

/// A list whose removal is off by one once it holds three items.
struct TodoList {
    items: Vec<u8>,
}

impl TodoList {
    fn remove(&mut self, index: usize) {
        let index = if self.items.len() == 3 {
            index + 1
        } else {
            index
        };
        if index < self.items.len() {
            self.items.remove(index);
        }
    }
}

impl Render for TodoList {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().children(self.items.iter().map(|item| div().child(item.to_string())))
    }
}

#[derive(Clone, Debug, Arbitrary)]
enum TodoAction {
    Add(u8),
    Remove(u8),
    Clear,
}

struct TodoSpec;

impl StateMachine for TodoSpec {
    type View = TodoList;
    type Model = Vec<u8>;
    type Action = TodoAction;

    fn open(cx: &mut App) -> (AnyWindowHandle, Entity<TodoList>) {
        let mut root = None;
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.), px(0.)),
                size(px(400.), px(300.)),
            ))),
            ..Default::default()
        };
        let handle = cx
            .open_window(options, |_, cx| {
                let view = cx.new(|_| TodoList { items: Vec::new() });
                root = Some(view.clone());
                view
            })
            .unwrap();
        (handle.into(), root.unwrap())
    }

    fn init_model() -> Vec<u8> {
        Vec::new()
    }

    fn precondition(model: &Vec<u8>, action: &TodoAction) -> bool {
        match action {
            TodoAction::Remove(index) => (*index as usize) < model.len(),
            _ => true,
        }
    }

    fn perform(action: &TodoAction, view: &Entity<TodoList>, _: &mut Window, cx: &mut App) {
        view.update(cx, |list, cx| {
            match action {
                TodoAction::Add(item) => list.items.push(*item),
                TodoAction::Remove(index) => list.remove(*index as usize),
                TodoAction::Clear => list.items.clear(),
            }
            cx.notify();
        });
    }

    fn apply(model: &mut Vec<u8>, action: &TodoAction) {
        match action {
            TodoAction::Add(item) => model.push(*item),
            TodoAction::Remove(index) => {
                model.remove(*index as usize);
            }
            TodoAction::Clear => model.clear(),
        }
    }

    fn check(
        model: &Vec<u8>,
        view: &Entity<TodoList>,
        _: &mut Window,
        cx: &mut App,
    ) -> Result<(), Failure> {
        ensure_eq!(&view.read(cx).items, model);
        Ok(())
    }
}

#[test]
fn finds_the_off_by_one_removal() {
    let report = Harness::from_state_machine::<TodoSpec>()
        .search(
            Config::default()
                .with_cases(512)
                .with_persist_failures(false),
        )
        .expect_err("the off-by-one is found");
    assert_eq!(report.failure().oracle(), "model");
    // Three adds and one remove are the least that reach the bug.
    assert_eq!(report.steps().len(), 4, "{report}");
    assert!(
        report.steps().iter().any(|step| step.contains("Remove")),
        "{report}"
    );
}
