//! Client presentation messages, independent of audio devices and terrain edits.
use bevy::prelude::*;
use terra_voxel::MaterialId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSurface {
    Sand,
    Snow,
}

impl TrackSurface {
    pub fn from_block(block: u16) -> Option<Self> {
        use MaterialId::*;
        match MaterialId::from_raw(block)? {
            Sand => Some(Self::Sand),
            Snow | SnowLayer | SnowDustStone | CastleSnowWall | Firn | IcySnow | SnowCoveredSod => {
                Some(Self::Snow)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walker {
    Player,
    Sheep,
    Cow,
    Pig,
    Bear,
    Cat,
    Stalker,
    Spider,
    Dragon,
    Slime,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct Footstep {
    pub creature: Option<u64>,
    pub position: Vec3,
    pub forward: Vec2,
    pub walker: Walker,
    pub material: u16,
    pub alternate: bool,
}

impl Footstep {
    pub fn lifetime(self) -> Option<f32> {
        Some(
            match (self.walker, TrackSurface::from_block(self.material)?) {
                (Walker::Player, TrackSurface::Sand) => 10.0,
                (Walker::Player, TrackSurface::Snow) => 22.0,
                (_, TrackSurface::Sand) => 3.0,
                (_, TrackSurface::Snow) => 5.0,
            },
        )
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
            creature: None,
            position: Vec3::ZERO,
            forward: Vec2::Y,
            walker: Walker::Player,
            material: 4,
            alternate: false,
        };
        assert_eq!(step.lifetime(), Some(10.0));
        for snow in [
            MaterialId::Snow,
            MaterialId::SnowLayer,
            MaterialId::Firn,
            MaterialId::IcySnow,
            MaterialId::SnowCoveredSod,
            MaterialId::SnowDustStone,
            MaterialId::CastleSnowWall,
        ] {
            step.material = snow as u16;
            step.walker = Walker::Player;
            assert_eq!(step.lifetime(), Some(22.0));
            step.walker = Walker::Cow;
            assert_eq!(step.lifetime(), Some(5.0));
        }
        step.material = MaterialId::Ice as u16;
        assert_eq!(step.lifetime(), None);
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
