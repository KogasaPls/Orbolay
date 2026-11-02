use std::time::Instant;

use freya::prelude::*;
use orbolay_core::{
  app_state::AppHandle,
  config::{Axis, Corner, DisplayVoiceMembers, SpeakingGraceStyle},
  user::{Speech, User, UserVoiceState},
};

use crate::{
  components::UserRow,
  layout::{corner_gaps, to_alignment},
  util::{scale::UiScale, text::censor, theme::Theme},
};

#[derive(PartialEq)]
pub struct VoiceSection {
  pub app: AppHandle,
  pub voice_users: Vec<User>,
  pub self_id: String,
  pub is_open: bool,
  pub is_censor: bool,
  pub user_alignment: Corner,
  pub user_offset_x: i32,
  pub user_offset_y: i32,
  pub display_voice_members: DisplayVoiceMembers,
  pub speaking_grace_style: SpeakingGraceStyle,
  pub theme: Theme,
  pub ui_scale: UiScale,
  pub user_template_size: (u32, u32),
  pub user_template: Option<String>,
}

impl Component for VoiceSection {
  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let (x, y) = self.user_alignment.axes();
    let gaps = corner_gaps(
      self.user_alignment,
      scale.int(self.user_offset_x) as f32,
      scale.int(self.user_offset_y) as f32,
    );
    let is_right_aligned = x == Axis::End;

    let mut sorted_users = self.voice_users.clone();
    sorted_users.sort_by(|a, b| a.id.cmp(&b.id));

    // Filter users based on display_voice_members
    let filtered_users: Vec<_> = sorted_users
      .into_iter()
      .filter(|user| match self.display_voice_members {
        DisplayVoiceMembers::Always | DisplayVoiceMembers::AlwaysSemiTransparent => true,
        DisplayVoiceMembers::WhenSpeaking => user.speech != Speech::Silent || self.is_open,
      })
      .collect();

    let now = Instant::now();

    rect().child(ContextMenuViewer::new()).child(
      filtered_users.iter().fold(
        rect()
          .direction(Direction::Vertical)
          .cross_align(to_alignment(x))
          .main_align(to_alignment(y))
          .position(Position::new_absolute().top(0.).left(0.))
          .background(Color::TRANSPARENT)
          .height(Size::fill())
          .width(Size::fill())
          .padding(gaps),
        |el, user| {
          let mut u = user.clone();
          let dimmed = self.display_voice_members == DisplayVoiceMembers::WhenSpeaking
            && self.speaking_grace_style == SpeakingGraceStyle::Dimmed
            && u.voice_state() == UserVoiceState::Speaking
            && matches!(u.speech, Speech::Grace { until } if until > now);
          if dimmed {
            u.speech = Speech::Silent;
          }

          if self.is_censor {
            u.name = censor(&u.name);
          }

          let is_self = self.self_id == u.id;
          let can_context_menu = !is_self;
          let is_voice_semitransparent = dimmed
            || matches!(
              self.display_voice_members,
              DisplayVoiceMembers::AlwaysSemiTransparent
            );

          let row: Element = UserRow {
            app: self.app.clone(),
            user: u,
            is_self,
            is_open: self.is_open,
            is_voice_semitransparent,
            can_context_menu,
            theme: self.theme,
            box_size: self.user_template_size,
            user_template: self.user_template.clone(),
            is_right_aligned,
            ui_scale: scale,
          }
          .into();

          el.child(row)
        },
      ),
    )
  }
}
