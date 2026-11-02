use freya::prelude::*;

/// Editable text for a saved setting. Shows `saved`, follows it when it changes elsewhere (for
/// example after a reset), and calls `commit` with the edited text when focus leaves the field
/// or the field goes away, if the text differs from the saved value.
pub fn use_setting_text(
  id: AccessibilityId,
  saved: &str,
  commit: impl Fn(String) + Clone + 'static,
) -> State<String> {
  let focus = use_focus(id);
  let saved = use_reactive(&saved.to_owned());
  let mut text = use_state(|| saved.peek().clone());

  use_side_effect(move || {
    let saved = saved.read().clone();
    text.set_if_modified(saved);
  });

  let flush = move || {
    let (Some(current), Some(saved)) = (text.try_peek(), saved.try_peek()) else {
      return;
    };
    if *current != *saved {
      commit(current.clone());
    }
  };

  use_side_effect({
    let flush = flush.clone();
    move || {
      if !focus.read().is_focused() {
        flush();
      }
    }
  });
  use_drop(flush);

  text
}

#[cfg(test)]
mod tests {
  use std::cell::RefCell;

  use freya::prelude::*;
  use freya_testing::launch_test;

  use super::use_setting_text;

  thread_local! {
    static COMMITS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static SHOWN: RefCell<String> = const { RefCell::new(String::new()) };
  }

  fn app() -> impl IntoElement {
    let mut saved = use_state(|| "before".to_string());
    let id = use_a11y();
    let text = use_setting_text(id, &saved.read(), |text| {
      COMMITS.with(|commits| commits.borrow_mut().push(text));
    });
    use_hook(move || spawn(async move { saved.set("after".into()) }));
    SHOWN.with(|shown| shown.borrow_mut().clone_from(&text.read()));
    rect()
  }

  #[test]
  fn external_changes_show_up_without_being_committed() {
    let mut test = launch_test(app);
    test.sync_and_update();
    test.poll(
      std::time::Duration::from_millis(1),
      std::time::Duration::from_millis(50),
    );
    test.sync_and_update();

    assert_eq!(SHOWN.with(|shown| shown.borrow().clone()), "after");
    assert_eq!(
      COMMITS.with(|commits| commits.borrow().clone()),
      Vec::<String>::new()
    );
  }
}
