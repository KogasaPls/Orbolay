use freya::prelude::{Alignment, Gaps, Position};
use orbolay_core::config::{Axis, Corner};

pub const fn to_alignment(axis: Axis) -> Alignment {
  match axis {
    Axis::Start => Alignment::Start,
    Axis::Center => Alignment::Center,
    Axis::End => Alignment::End,
  }
}

/// Padding that offsets content away from the edges it is anchored to.
pub fn corner_gaps(corner: Corner, offset_x: f32, offset_y: f32) -> Gaps {
  let (x, y) = corner.axes();
  let (top, bottom) = match y {
    Axis::Start => (offset_y, 0.),
    Axis::End => (0., offset_y),
    Axis::Center => (0., 0.),
  };
  let (left, right) = match x {
    Axis::Start => (offset_x, 0.),
    Axis::End => (0., offset_x),
    Axis::Center => (0., 0.),
  };
  Gaps::new(top, right, bottom, left)
}

pub fn corner_position(corner: Corner) -> Position {
  let (x, y) = corner.axes();
  let mut position = Position::new_absolute();

  match x {
    Axis::Start => position = position.left(0.),
    Axis::Center => {}
    Axis::End => position = position.right(0.),
  }

  match y {
    Axis::Start => position = position.top(0.),
    Axis::Center => {}
    Axis::End => position = position.bottom(0.),
  }

  position
}
