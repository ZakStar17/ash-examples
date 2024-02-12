use crate::player_sprite::SpritePushConstants;

#[derive(PartialEq)]
enum HorMovement {
  LEFT,
  NO,
  RIGHT,
}

#[derive(PartialEq)]
enum VertMovement {
  UP,
  NO,
  DOWN,
}

pub struct Player {
  pub position: [f32; 2],
  hor_movement: HorMovement,
  vert_movement: VertMovement,
}

impl Player {
  pub const SPEED_VERT: f32 = 1.0;
  pub const SPEED_HOR: f32 = 1.0;

  pub fn new(position: [f32; 2]) -> Self {
    Self {
      position,
      hor_movement: HorMovement::NO,
      vert_movement: VertMovement::NO,
    }
  }

  pub fn update(&mut self, delta: f32) {
    if self.hor_movement == HorMovement::LEFT {
      self.position[0] -= Self::SPEED_HOR * delta;
    } else if self.hor_movement == HorMovement::RIGHT {
      self.position[0] += Self::SPEED_HOR * delta;
    }

    if self.vert_movement == VertMovement::UP {
      self.position[1] -= Self::SPEED_VERT * delta;
    } else if self.vert_movement == VertMovement::DOWN {
      self.position[1] += Self::SPEED_VERT * delta;
    }
  }

  fn texture_index(&self) -> usize {
    match self.hor_movement {
      HorMovement::NO => 0,
      HorMovement::RIGHT => 1,
      HorMovement::LEFT => 2,
    }
  }

  pub fn sprite_data(&self) -> SpritePushConstants {
    SpritePushConstants::new(self.position, self.texture_index())
  }

  pub fn up_press(&mut self) {
    self.vert_movement = VertMovement::UP;
  }

  pub fn down_press(&mut self) {
    self.vert_movement = VertMovement::DOWN;
  }

  pub fn left_press(&mut self) {
    self.hor_movement = HorMovement::LEFT;
  }

  pub fn right_press(&mut self) {
    self.hor_movement = HorMovement::RIGHT;
  }

  pub fn up_release(&mut self) {
    if self.vert_movement == VertMovement::UP {
      self.vert_movement = VertMovement::NO;
    }
  }

  pub fn down_release(&mut self) {
    if self.vert_movement == VertMovement::DOWN {
      self.vert_movement = VertMovement::NO;
    }
  }

  pub fn left_release(&mut self) {
    if self.hor_movement == HorMovement::LEFT {
      self.hor_movement = HorMovement::NO;
    }
  }

  pub fn right_release(&mut self) {
    if self.hor_movement == HorMovement::RIGHT {
      self.hor_movement = HorMovement::NO;
    }
  }
}
