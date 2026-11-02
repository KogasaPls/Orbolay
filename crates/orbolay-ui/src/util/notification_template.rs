use std::fmt::Write;

use orbolay_core::{
  dirs::NOTIFICATION_TEMPLATES,
  payloads::{Notification, NotificationKind},
  util::text::strip,
};

use crate::util::html_template::{Template, TemplateSpec, TemplateStore, escape_html};
use crate::util::theme::Theme;

pub const ACTION_ATTR: &str = "data-action";

const TITLE_TOKEN: &str = "{{title}}";
const BODY_TOKEN: &str = "{{body}}";
const ICON_TOKEN: &str = "{{icon}}";
const ACTIONS_TOKEN: &str = "{{actions}}";

static STORE: TemplateStore = TemplateStore::new(TemplateSpec {
  kind: "notification",
  dir: NOTIFICATION_TEMPLATES,
  root_id: "notification",
  image_token: ICON_TOKEN,
  required_tokens: &[TITLE_TOKEN, BODY_TOKEN],
  builtin: include_str!("../../../../templates/notification/index.html"),
});

pub fn list_notification_templates() -> Vec<String> {
  STORE.list()
}

pub fn load_notification_template(name: Option<&str>) -> Template {
  STORE.load(name)
}

pub fn render_notification(
  template: &Template,
  notification: &Notification,
  theme: &Theme,
) -> String {
  let icon = icon_url(&notification.icon);
  let mut classes = Vec::new();
  if icon.is_empty() {
    classes.push("no-icon");
  }
  if notification
    .actions
    .as_deref()
    .is_some_and(|actions| !actions.is_empty())
  {
    classes.push("has-actions");
  }

  template
    .render_root(&classes, Vec::new(), theme)
    .replace(TITLE_TOKEN, &escape_html(&notification.title))
    .replace(BODY_TOKEN, &escape_html(&strip(&notification.body)))
    .replace(ICON_TOKEN, &escape_html(&icon))
    .replace(ACTIONS_TOKEN, &actions_html(notification))
}

/// Discord gives us both full URLs and relative URLs, this handles those
fn icon_url(icon: &str) -> String {
  if icon.trim().is_empty() {
    return String::new();
  }

  url::Url::parse("https://discord.com")
    .and_then(|base| base.join(icon))
    .ok()
    .filter(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
    .map(String::from)
    .unwrap_or_default()
}

fn actions_html(notification: &Notification) -> String {
  let mut html = String::new();
  for (index, action) in notification.actions.iter().flatten().enumerate() {
    let kind_class = if action.kind == NotificationKind::Secondary {
      "action secondary"
    } else {
      "action"
    };
    let _ = write!(
      html,
      "<button class=\"{kind_class}\" {ACTION_ATTR}=\"{index}\">{}</button>",
      escape_html(&action.label)
    );
  }
  html
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use freya::prelude::Color;
  use orbolay_core::payloads::{Notification, NotificationAction, NotificationKind};

  use super::{ACTION_ATTR, load_notification_template, render_notification};
  use crate::util::theme::Theme;

  fn render(notification: &Notification) -> String {
    let theme = Theme::from_values(Color::BLACK, Color::WHITE, 10.);
    render_notification(&load_notification_template(None), notification, &theme)
  }

  #[test]
  fn relative_icons_resolve_against_discord() {
    let html = render(&Notification {
      title: "Title".into(),
      body: "line\nbreak".into(),
      icon: "/assets/icon.png".into(),
      ..Default::default()
    });
    assert!(html.contains("https://discord.com/assets/icon.png"));
    assert!(html.contains("line break"));
    assert!(html.contains(r#"<div id="notification">"#));
  }

  #[test]
  fn actions_render_as_indexed_buttons() {
    let action = |label: &str, kind| NotificationAction {
      label: label.into(),
      action: Arc::new(|| {}),
      kind,
    };
    let html = render(&Notification {
      title: "Call".into(),
      actions: Some(vec![
        action("Join", NotificationKind::Primary),
        action("<Ignore>", NotificationKind::Secondary),
      ]),
      ..Default::default()
    });
    assert!(html.contains(r#"<div id="notification" class="no-icon has-actions">"#));
    assert!(html.contains(&format!(r#"class="action" {ACTION_ATTR}="0">Join<"#)));
    assert!(html.contains(&format!(
      r#"class="action secondary" {ACTION_ATTR}="1">&lt;Ignore&gt;<"#
    )));
  }
}
