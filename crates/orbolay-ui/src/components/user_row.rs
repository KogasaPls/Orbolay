use freya::prelude::*;
use orbolay_core::{app_state::AppHandle, user::User};

use crate::{
  components::{TemplateView, user_context_menu_item::UserContextMenuItem},
  util::{
    scale::{GapsScaleExt, UiScale},
    theme::Theme,
    user_template::{self, load_user_template, render_user},
  },
};

#[derive(PartialEq, Clone)]
pub struct UserRow {
  pub app: AppHandle,
  pub user: User,
  pub is_self: bool,
  /// Drawn at half opacity because the member is quiet and the list is semi-transparent.
  pub faded: bool,
  pub theme: Theme,
  pub user_template: Option<String>,
  pub is_right_aligned: bool,
  pub ui_scale: UiScale,
}

impl Component for UserRow {
  // Stable diff key based on user ID
  fn render_key(&self) -> DiffKey {
    DiffKey::from(&self.user.id)
  }

  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let (width, height) = TemplateView::scaled_size(user_template::SIZE, scale);
    let opacity = if self.faded { 0.5_f32 } else { 1.0_f32 };
    let template = load_user_template(self.user_template.as_deref());
    let html = render_user(
      &template,
      &self.user,
      self.is_self,
      &self.theme,
      self.is_right_aligned,
    );

    rect()
      .direction(Direction::Horizontal)
      .main_align(Alignment::Center)
      .cross_align(Alignment::Center)
      .width(width)
      .height(height)
      .opacity(opacity)
      .child(TemplateView {
        html,
        size: user_template::SIZE,
        ui_scale: scale,
      })
      .maybe(!self.is_self, |el| {
        el.on_secondary_down({
          let user = self.user.clone();
          let theme = self.theme;
          let app = self.app.clone();
          move |_| {
            ContextMenu::open_from_down(
              Menu::new()
                .theme(MenuContainerThemePartial {
                  background: Some(Preference::Specific(theme.darkish_gray)),
                  padding: Some(Preference::Specific(Gaps::new_all(6.).scaled(scale))),
                  shadow: Some(Preference::Specific(theme.transparent_gray)),
                  border_fill: Some(Preference::Specific(theme.muted_gray)),
                  corner_radius: Some(Preference::Specific(CornerRadius::new_all(
                    theme.border_radius,
                  ))),
                })
                .child(UserContextMenuItem {
                  user: user.clone(),
                  theme,
                  app: app.clone(),
                  ui_scale: scale,
                }),
            );
          }
        })
      })
  }
}
