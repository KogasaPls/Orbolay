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
