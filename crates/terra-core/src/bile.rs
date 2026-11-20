//! Shared identities and optical/contact bounds of the sandworm's acid.
pub const FLUID: u16 = 439;
pub const BLOCK: u16 = 440;
pub const RANGE: u64 = 4;
pub const FLOW_SECONDS: f64 = 0.2;
pub const DAMAGE: f32 = 6.0;
pub const DAMAGE_INTERVAL: f64 = 0.75;

/// Fine-volume marker in the existing age byte; lower bits hold a temporary
/// source's remaining 0.2-second steps. Zero means a bucket-owned permanent source.
pub const FINE: u8 = 0x80;
pub const LIFETIME_MASK: u8 = 0x7f;
pub const CAPACITY: u8 = 64;
pub const SPIT_STEPS: u8 = 25;
pub const fn fine_volume(volume: u8, age: u8) -> u8 {
    if age & FINE != 0 {
        volume
    } else {
        volume.saturating_mul(8)
    }
}
pub const fn facing(volume: u8, age: u8) -> u8 {
    fine_volume(volume, age).saturating_sub(1) / 8
}
pub fn state_height(volume: u8, age: u8) -> f32 {
    (f32::from(fine_volume(volume, age).clamp(1, CAPACITY)) / f32::from(CAPACITY) - 0.002)
        .max(0.002)
}

pub const fn height(amount: u8) -> f32 {
    let amount = if amount < 1 {
        1
    } else if amount > 8 {
        8
    } else {
        amount
    };
    amount as f32 / 8.0 - 0.015625
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collected_bucket_appends_without_changing_older_tool_ids() {
        use crate::{ItemKind, ToolKind};
        assert_eq!(ToolKind::TarBucket as u8, 41);
        assert_eq!(ToolKind::BileBucket as u8, 42);
        let item = ItemKind::Tool(ToolKind::BileBucket);
        assert_eq!(item.max_stack(), 1);
        assert_eq!(
            serde_json::to_string(&item).unwrap(),
            r#"{"tool":"bile_bucket"}"#
        );
        assert_eq!(RANGE, 4);
        assert!(height(1) > 0. && height(1) < 0.125);
        assert!(height(8) < 1.);
    }
}
