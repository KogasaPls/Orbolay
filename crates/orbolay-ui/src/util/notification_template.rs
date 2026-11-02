use orbolay_core::{config::NOTIFICATION_TEMPLATES, protocol::Notification};

use crate::util::{
  html_template::{Template, TemplateSpec, TemplateStore, escape_html, fill},
  text::strip,
  theme::Theme,
};

const TITLE_TOKEN: &str = "{{title}}";
const BODY_TOKEN: &str = "{{body}}";
const ICON_TOKEN: &str = "{{icon}}";
const ACTIONS_TOKEN: &str = "{{actions}}";

/// Width and height a notification template is laid out at, before UI scaling.
pub const SIZE: (u32, u32) = (400, 66);

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
  let classes: &[&str] = if icon.is_none() { &["no-icon"] } else { &[] };

  fill(
    &template.render_root(classes, Vec::new(), theme),
    &[
      (TITLE_TOKEN, &escape_html(&notification.title)),
      (BODY_TOKEN, &escape_html(&strip(&notification.body))),
      (
        ICON_TOKEN,
        &escape_html(icon.as_deref().unwrap_or_default()),
      ),
      (ACTIONS_TOKEN, ""),
    ],
  )
}

/// Discord gives us both full URLs and relative URLs, this handles those
fn icon_url(icon: &str) -> Option<String> {
  if icon.trim().is_empty() {
    return None;
  }

  url::Url::parse("https://discord.com")
    .and_then(|base| base.join(icon))
    .ok()
    .filter(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
    .map(String::from)
}

#[cfg(test)]
mod tests {
  use freya::prelude::Color;
  use orbolay_core::protocol::Notification;

  use super::{load_notification_template, render_notification};
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
  fn the_legacy_actions_token_renders_nothing() {
    let html = render(&Notification {
      title: "Call".into(),
      ..Default::default()
    });
    assert!(html.contains(r#"<div id="notification" class="no-icon">"#));
    assert!(!html.contains("{{actions}}"));
  }
}
