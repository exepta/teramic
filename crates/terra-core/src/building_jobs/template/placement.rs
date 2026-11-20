//! Exact authored inputs and rotation of the actual saved construction states.
use super::*;

pub fn cell_inputs(cells: &[TemplateCell]) -> Result<Vec<ItemStack>, String> {
    let first = cells.first().ok_or("A building step needs cells")?;
    if cells.len() > 2
        || cells.iter().any(|cell| crate::roads::is_tar(cell.material))
        || first.material == 0
        || terra_voxel::MaterialId::from_raw(first.material).is_none()
    {
        return Err("Invalid building material input".into());
    }
    let mut counts = BTreeMap::<ItemKind, u16>::new();
    if let Some(halves) = crate::construction::slab_halves(first.material, first.facing) {
        for (material, _) in halves {
            *counts.entry(ItemKind::Block(material)).or_default() += 1;
        }
    } else {
        counts.insert(
            ItemKind::Block(first.material),
            if first.material == 29 && cells.len() == 2 {
                2
            } else {
                1
            },
        );
    }
    if first.material == crate::flower_pot::MATERIAL
        && let Some(item) = crate::flower_pot::contents(first.facing)
    {
        *counts.entry(item).or_default() += 1;
    }
    Ok(counts
        .into_iter()
        .map(|(item, count)| ItemStack { item, count })
        .collect())
}
fn turn([x, y, z]: [i8; 3], quarter_turns: u8) -> [i8; 3] {
    match quarter_turns % 4 {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}
fn half_facing(facing: u8, quarter_turns: u8) -> u8 {
    (0..quarter_turns).fold(facing, |f, _| match f {
        5 => 8,
        8 => 6,
        6 => 7,
        7 => 5,
        _ => f,
    })
}
pub fn rotated_facing(cell: &TemplateCell, quarter_turns: u8) -> Result<u8, String> {
    if quarter_turns > 3 || !valid_pose(cell) {
        return Err("Invalid authored construction rotation".into());
    }
    if !cell.rotate_facing || quarter_turns == 0 {
        return Ok(cell.facing);
    }
    if crate::construction::is_stair(cell.material) {
        let basis =
            crate::construction::stair_basis(cell.facing).map(|axis| turn(axis, quarter_turns));
        return (0..24)
            .find(|f| crate::construction::stair_basis(*f) == basis)
            .ok_or_else(|| "Unsupported stair rotation".into());
    }
    if crate::construction::is_slab(cell.material) {
        if cell.facing == crate::construction::DOUBLE_SLAB {
            return Ok(cell.facing);
        }
        if let Some([(first, facing), (second, _)]) =
            crate::construction::slab_halves(cell.material, cell.facing)
        {
            return crate::construction::merged_slab(
                first,
                half_facing(facing, quarter_turns),
                second,
            )
            .ok_or_else(|| "Unsupported mixed slab rotation".into());
        }
        return Ok(half_facing(cell.facing, quarter_turns));
    }
    Ok((cell.facing + quarter_turns) % 4)
}
