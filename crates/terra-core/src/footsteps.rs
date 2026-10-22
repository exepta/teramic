//! Client presentation messages, independent of audio devices and terrain edits.
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walker {
    Player,
    Sheep,
    Cow,
    Pig,
    Bear,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct Footstep {
    pub position: Vec3,
    pub forward: Vec2,
    pub walker: Walker,
    pub material: u16,
    pub alternate: bool,
}

impl Footstep {
    pub fn lifetime(self) -> Option<f32> {
        match (self.walker, self.material) {
            (Walker::Player, 4) => Some(10.0),
            (Walker::Player, 5 | 98) => Some(22.0),
            (_, 4) => Some(3.0),
            (_, 5 | 98) => Some(5.0),
            _ => None,
        }
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FootstepSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterMotionKind {
    Splash,
    Wake,
    Bubble,
}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WaterMotionSet;
#[derive(Message, Debug, Clone, Copy)]
pub struct WaterMotion {
    pub position: Vec3,
    pub velocity: Vec3,
    pub strength: f32,
    pub kind: WaterMotionKind,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracks_have_requested_lifetimes_only_on_soft_ground() {
        let mut step = Footstep {
            position: Vec3::ZERO,
            forward: Vec2::Y,
            walker: Walker::Player,
            material: 4,
            alternate: false,
        };
        assert_eq!(step.lifetime(), Some(10.0));
        step.material = 5;
        assert_eq!(step.lifetime(), Some(22.0));
        step.walker = Walker::Cow;
        assert_eq!(step.lifetime(), Some(5.0));
        step.material = 4;
        assert_eq!(step.lifetime(), Some(3.0));
        step.material = 3;
        assert_eq!(step.lifetime(), None);
    }
}
#[derive(bevy::prelude::Message, Clone, Copy, Debug)]
pub enum UiSound {
    Click,
    Mention,
}
