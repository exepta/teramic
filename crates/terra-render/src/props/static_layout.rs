//! Remove section-inherited growth data only from independently static pieces.
use crate::meshing::builder::MeshBatch;
use bevy::mesh::VertexAttributeValues;

fn enabled_for(smoke: bool, setting: Option<&str>) -> bool {
    !(smoke && setting == Some("0"))
}

pub(super) fn strip_identity_growth(batch: &mut MeshBatch) {
    static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        enabled_for(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_STATIC_PROP_LAYOUT").ok().as_deref(),
        )
    });
    let count = batch.positions.len();
    if !*ENABLED
        || batch.staged.is_some()
        || !batch.growth_ranges.is_empty()
        || count == 0
        || batch.rooted_wind.len() != count
        || batch.growth_anchors.len() != count
        || batch.rooted_wind.iter().any(|&timing| timing != [0.0, 1.0])
        || batch
            .positions
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        || batch
            .growth_anchors
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
    {
        return;
    }
    // Flowers carry alpha ranks, while crops and regrowing grass carry ranges.
    // Require one unambiguous, completely opaque color stream as a second guard.
    let opaque = match (
        batch.colors.as_slice(),
        batch.packed_colors.as_slice(),
        &batch.half_colors,
    ) {
        (colors, [], None) => colors.len() == count && colors.iter().all(|color| color[3] == 1.0),
        ([], colors, None) => colors.len() == count && colors.iter().all(|color| color[3] == 255),
        ([], [], Some(VertexAttributeValues::Float16x4(colors))) => {
            colors.len() == count && colors.iter().all(|color| color[3].to_f32() == 1.0)
        }
        _ => false,
    };
    if opaque {
        batch.rooted_wind = Vec::new();
        batch.growth_anchors = Vec::new();
    }
}

#[cfg(test)]
mod tests {
    use super::enabled_for;

    #[test]
    fn only_explicit_smoke_control_can_restore_the_original_static_prop_layout() {
        for setting in [None, Some("0"), Some("1"), Some("false")] {
            assert!(enabled_for(false, setting));
            assert_eq!(enabled_for(true, setting), setting != Some("0"));
        }
    }
}
