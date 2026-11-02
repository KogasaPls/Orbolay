use std::{fmt::Display, str::FromStr};

use freya::prelude::{Alignment, Gaps, Position};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub enum AxisAlignment {
  Start,
  Center,
  End,
}

impl Display for AxisAlignment {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Start => write!(f, "start"),
      Self::Center => write!(f, "center"),
      Self::End => write!(f, "end"),
    }
  }
}

impl AxisAlignment {
  pub fn to_freya(&self) -> Alignment {
    match self {
      Self::Start => Alignment::Start,
      Self::Center => Alignment::Center,
      Self::End => Alignment::End,
    }
  }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CornerAlignment {
  pub x: AxisAlignment,
  pub y: AxisAlignment,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CornerAlignmentParseError;

impl FromStr for CornerAlignment {
  type Err = CornerAlignmentParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(match s.to_ascii_lowercase().as_str() {
      "topright" => Self {
        x: AxisAlignment::End,
        y: AxisAlignment::Start,
      },
      "bottomleft" => Self {
        x: AxisAlignment::Start,
        y: AxisAlignment::End,
      },
      "bottomright" => Self {
        x: AxisAlignment::End,
        y: AxisAlignment::End,
      },
      "topcenter" => Self {
        x: AxisAlignment::Center,
        y: AxisAlignment::Start,
      },
      "bottomcenter" => Self {
        x: AxisAlignment::Center,
        y: AxisAlignment::End,
      },
      "centerleft" => Self {
        x: AxisAlignment::Start,
        y: AxisAlignment::Center,
      },
      "centerright" => Self {
        x: AxisAlignment::End,
        y: AxisAlignment::Center,
      },
      _ => Self {
        x: AxisAlignment::Start,
        y: AxisAlignment::Start,
      },
    })
  }
}

impl CornerAlignment {
  pub fn to_gaps(&self, offset_x: i32, offset_y: i32) -> Gaps {
    let (top, bottom) = match self.y {
      AxisAlignment::Start => (offset_y as f32, 0.),
      AxisAlignment::End => (0., offset_y as f32),
      AxisAlignment::Center => (0., 0.),
    };
    let (left, right) = match self.x {
      AxisAlignment::Start => (offset_x as f32, 0.),
      AxisAlignment::End => (0., offset_x as f32),
      AxisAlignment::Center => (0., 0.),
    };
    Gaps::new(top, right, bottom, left)
  }

  pub fn unused_corner(occupied_corners: &[String]) -> Option<Self> {
    const CORNERS: [&str; 4] = ["bottomright", "bottomleft", "topright", "topleft"];

    CORNERS
      .iter()
      .copied()
      .find(|corner| !occupied_corners.iter().any(|occupied| occupied == corner))
      .map(|corner| corner.parse().unwrap())
  }

  pub fn to_position(&self) -> Position {
    let mut position = Position::new_absolute();

    match self.x {
      AxisAlignment::Start => position = position.left(0.),
      AxisAlignment::Center => {}
      AxisAlignment::End => position = position.right(0.),
    }

    match self.y {
      AxisAlignment::Start => position = position.top(0.),
      AxisAlignment::Center => {}
      AxisAlignment::End => position = position.bottom(0.),
    }

    position
  }
}
