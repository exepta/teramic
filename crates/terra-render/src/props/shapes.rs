use crate::{
    blocks::PropMaterials,
    meshing::{DetailLevel, builder::MeshBatch},
};
use terra_world::props::{
    PropKind,
    visual::{PropDetail, PropPalette},
};

pub(crate) fn template(
    kind: PropKind,
    variant: u8,
    level: DetailLevel,
    palette: &PropMaterials,
) -> Result<Vec<MeshBatch>, String> {
    let level = match level {
        DetailLevel::Near => PropDetail::Near,
        DetailLevel::Middle => PropDetail::Middle,
        DetailLevel::Far => PropDetail::Far,
    };
    let palette = PropPalette {
        crop: palette.crop,
        foliage: palette.foliage,
        stem: palette.stem,
        dry_grass: palette.dry_grass,
        twig: palette.twig,
        flowers: palette.flowers,
        mushroom: palette.mushroom,
        stone: palette.stone,
    };
    Ok(
        terra_world::props::visual::template(kind, variant, level, &palette)?
            .into_iter()
            .map(|m| MeshBatch {
                material: m.material,
                positions: m.positions,
                normals: m.normals,
                uvs: m.uvs,
                colors: m.colors,
                indices: m.indices,
                ..Default::default()
            })
            .collect(),
    )
}
