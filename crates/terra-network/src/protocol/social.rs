use serde::{Deserialize, Serialize};
use terra_core::{ItemStack, PlayerId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChestCursorInput {
    pub anchor: [i64; 3],
    /// X packs a physical slot index (high 6 bits) and its local fraction (10 bits).
    /// Y is the local fraction. This survives different chest/workbench column counts.
    pub position: [u16; 2],
    pub color: [u8; 3],
}
impl ChestCursorInput {
    pub fn valid(&self) -> bool {
        self.anchor.iter().all(|p| p.unsigned_abs() <= 990_000)
            && self.slot() < 36
            && self
                .color
                .iter()
                .max()
                .copied()
                .unwrap_or(0)
                .saturating_sub(self.color.iter().min().copied().unwrap_or(0))
                >= 40
    }
    pub fn slot(&self) -> usize {
        usize::from(self.position[0] >> 10)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedCursor {
    pub player: PlayerId,
    pub name: String,
    pub cursor: ChestCursorInput,
    /// Authoritative cursor stack, never supplied by the sharing client.
    pub item: Option<ItemStack>,
}
