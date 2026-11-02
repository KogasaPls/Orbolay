use orbolay_core::{
  config::USER_TEMPLATES,
  user::{User, UserVoiceState},
};

use crate::util::{
  html_template::{Template, TemplateSpec, TemplateStore, escape_html, fill},
  theme::Theme,
};

const NAME_TOKEN: &str = "{{name}}";
const AVATAR_TOKEN: &str = "{{avatar}}";
const VOLUME_PERCENT_TOKEN: &str = "{{volume-percent}}";
const STREAMING_TOKEN: &str = "{{streaming}}";

const MUTED_ICON_TOKEN: &str = "{{muted-icon}}";
const DEAFENED_ICON_TOKEN: &str = "{{deafened-icon}}";
const CAMERA_ICON_TOKEN: &str = "{{camera-icon}}";

const MUTED_ICON: &str = include_str!("../../../../assets/muted.svg");
const DEAFENED_ICON: &str = include_str!("../../../../assets/deafened.svg");
const CAMERA_ICON: &str = include_str!("../../../../assets/camera.svg");

/// Width and height a user template is laid out at, before UI scaling.
pub const SIZE: (u32, u32) = (320, 62);

static STORE: TemplateStore = TemplateStore::new(TemplateSpec {
  kind: "user",
  dir: USER_TEMPLATES,
  root_id: "user",
  image_token: AVATAR_TOKEN,
  required_tokens: &[NAME_TOKEN],
  builtin: include_str!("../../../../templates/user/index.html"),
});

pub fn list_user_templates() -> Vec<String> {
  STORE.list()
}

pub fn load_user_template(name: Option<&str>) -> Template {
  STORE.load(name)
}

pub fn render_user(
  template: &Template,
  user: &User,
  is_self: bool,
  theme: &Theme,
  is_right_aligned: bool,
) -> String {
  let mut classes = user_template_classes(user, is_self);
  if is_right_aligned {
    classes.push("right");
  }

  let html = template.render_root(
    &classes,
    vec![("data-user-id".to_string(), user.id.clone())],
    theme,
  );
  let state = user.voice_state();
  fill(
    &html,
    &[
      (AVATAR_TOKEN, &escape_html(&avatar_url(user))),
      (NAME_TOKEN, &escape_html(&user.name)),
      (
        VOLUME_PERCENT_TOKEN,
        &format!("{:.0}", user.volume.clamp(0., 200.)),
      ),
      (
        MUTED_ICON_TOKEN,
        &status_icon(state == UserVoiceState::Muted, "muted", MUTED_ICON),
      ),
      (
        DEAFENED_ICON_TOKEN,
        &status_icon(state == UserVoiceState::Deafened, "deafened", DEAFENED_ICON),
      ),
      (
        CAMERA_ICON_TOKEN,
        &status_icon(user.camera, "camera", CAMERA_ICON),
      ),
      (STREAMING_TOKEN, &live_indicator(user.streaming)),
    ],
  )
}

fn status_icon(active: bool, state: &str, svg: &str) -> String {
  if active {
    format!("<span class=\"status-icon {state}\">{svg}</span>")
  } else {
    String::new()
  }
}

fn live_indicator(active: bool) -> String {
  if active {
    "<span class=\"live-indicator\">LIVE</span>".to_string()
  } else {
    String::new()
  }
}

fn user_template_classes(user: &User, is_self: bool) -> Vec<&'static str> {
  let mut classes = Vec::new();

  match user.voice_state() {
    UserVoiceState::Speaking => classes.push("speaking"),
    UserVoiceState::Muted => classes.push("muted"),
    UserVoiceState::Deafened => classes.push("deafened"),
    UserVoiceState::NotSpeaking => {}
  }

  if user.streaming {
    classes.push("streaming");
  }

  if user.camera {
    classes.push("camera");
  }

  if user.avatar.is_none() {
    classes.push("no-avatar");
  }

  if is_self {
    classes.push("self");
  }

  classes
}

fn avatar_url(user: &User) -> String {
  user.avatar.as_ref().map_or_else(
    || {
      let index = user.id.parse::<u64>().map_or(0, |id| (id >> 22) % 6);
      format!("https://cdn.discordapp.com/embed/avatars/{index}.png")
    },
    |hash| {
      format!(
        "https://cdn.discordapp.com/avatars/{}/{hash}.png?size=160",
        user.id
      )
    },
  )
}

#[cfg(test)]
mod tests {
  use freya::prelude::Color;
  use orbolay_core::user::{Speech, User};

  use super::{load_user_template, render_user};
  use crate::util::theme::Theme;

  fn user(avatar: Option<&str>, muted: bool) -> User {
    User {
      name: "<b>Name</b>".into(),
      id: "175928847299117063".into(),
      avatar: avatar.map(Into::into),
      muted,
      deafened: false,
      speech: Speech::Silent,
      streaming: true,
      camera: false,
      volume: 100.,
    }
  }

  fn render(user: &User, is_self: bool, right: bool) -> String {
    let theme = Theme::from_values(Color::BLACK, Color::WHITE, 10.);
    render_user(&load_user_template(None), user, is_self, &theme, right)
  }

  #[test]
  fn builtin_template_renders_state_classes_and_tokens() {
    let html = render(&user(Some("abc"), true), true, true);
    assert!(
      html.contains(r#"class="muted streaming self right""#),
      "{html}"
    );
    assert!(html.contains(r#"data-user-id="175928847299117063""#));
    assert!(html.contains("&lt;b&gt;Name&lt;/b&gt;"));
    assert!(
      html.contains("https://cdn.discordapp.com/avatars/175928847299117063/abc.png?size=160")
    );
    assert!(html.contains("live-indicator"));
    assert!(html.contains("--border-radius: 10px;"));
    assert!(!html.contains("{{"));
  }

  #[test]
  fn names_cannot_inject_tokens() {
    let mut member = user(Some("abc"), false);
    member.name = "{{muted-icon}}".into();
    let html = render(&member, false, false);
    assert!(html.contains("{{muted-icon}}"));
    assert!(!html.contains("status-icon muted"));
  }

  #[test]
  fn missing_avatar_uses_the_default_avatar_for_the_user_id() {
    let html = render(&user(None, false), false, false);
    assert!(html.contains(r#"class="streaming no-avatar""#));
    assert!(html.contains("https://cdn.discordapp.com/embed/avatars/"));
  }
}
