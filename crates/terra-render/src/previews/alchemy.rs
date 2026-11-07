//! Faceted, corked bottles share the existing opaque and glass item materials.
use super::*;
use crate::players::model::Builder;

fn ring_point(y: f32, radius: f32, side: usize) -> Vec3 {
    let angle = side as f32 * std::f32::consts::TAU / 8.0;
    Vec3::new(0.5 + angle.cos() * radius, y, 0.5 + angle.sin() * radius)
}

fn vessel(builder: &mut Builder, rows: &[(f32, f32)], color: [f32; 3]) -> Result<(), String> {
    for pair in rows.windows(2) {
        for side in 0..8 {
            builder.quad(
                [
                    ring_point(pair[0].0, pair[0].1, side),
                    ring_point(pair[1].0, pair[1].1, side),
                    ring_point(pair[1].0, pair[1].1, side + 1),
                    ring_point(pair[0].0, pair[0].1, side + 1),
                ],
                color,
            )?;
        }
    }
    for (&(y, radius), top) in [(rows.first().unwrap(), false), (rows.last().unwrap(), true)] {
        for side in 0..8 {
            let points = [
                Vec3::new(0.5, y, 0.5),
                ring_point(y, radius, side),
                ring_point(y, radius, side + 1),
            ];
            builder.triangle(
                if top {
                    [points[0], points[2], points[1]]
                } else {
                    points
                },
                color,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn batches(item: ItemKind, templates: &TemplateSet) -> Result<Vec<MeshBatch>, String> {
    let liquid = match item {
        ItemKind::GlassBottle => None,
        ItemKind::WaterBottle => Some([0.22, 0.60, 0.88]),
        ItemKind::Potion(id) => Some(
            terra_core::brewing::recipe(id)
                .ok_or("Unknown potion bottle")?
                .color
                .map(|value| f32::from(value) / 255.0),
        ),
        _ => return Err("Not an alchemy bottle".into()),
    };
    let mut glass = Builder::default();
    vessel(
        &mut glass,
        &[
            (0.12, 0.12),
            (0.18, 0.20),
            (0.49, 0.21),
            (0.61, 0.10),
            (0.67, 0.065),
            (0.83, 0.065),
        ],
        [0.68, 0.89, 0.96],
    )?;
    let mut shell = glass.finish_batch()?;
    shell.material = templates.item_materials.glass;
    let mut opaque = Builder::default();
    if let Some(color) = liquid {
        vessel(
            &mut opaque,
            &[(0.15, 0.105), (0.20, 0.18), (0.49, 0.19), (0.55, 0.14)],
            color,
        )?;
        vessel(
            &mut opaque,
            &[(0.81, 0.068), (0.87, 0.078), (0.92, 0.075)],
            [0.56, 0.32, 0.14],
        )?;
    }
    // Copper neck ring and a bright glass edge keep even an empty bottle legible.
    vessel(
        &mut opaque,
        &[(0.80, 0.075), (0.825, 0.075)],
        [0.73, 0.49, 0.24],
    )?;
    opaque.bevel_box(
        [0.45, 0.38, 0.69],
        [0.014, 0.17, 0.012],
        [0.82, 0.96, 1.0],
        Quat::IDENTITY,
    )?;
    let mut contents = opaque.finish_batch()?;
    contents.material = templates.item_materials.opaque;
    Ok(vec![contents, shell])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_bottles_have_bounded_real_contents_and_a_shared_transparent_shell() {
        let mut templates = TemplateSet::default();
        templates.item_materials.opaque = 7;
        templates.item_materials.glass = 11;
        let empty = batches(ItemKind::GlassBottle, &templates).unwrap();
        for item in [ItemKind::GlassBottle, ItemKind::WaterBottle]
            .into_iter()
            .chain((1..=terra_core::brewing::POTION_COUNT as u16).map(ItemKind::Potion))
        {
            let parts = batches(item, &templates).unwrap();
            assert_eq!(parts.len(), 2);
            assert_eq!([parts[0].material, parts[1].material], [7, 11]);
            assert_eq!(parts[1].positions, empty[1].positions);
            assert!(parts.iter().map(|part| part.positions.len()).sum::<usize>() < 1024);
            for part in &parts {
                assert_eq!(part.positions.len(), part.colors.len());
                assert!(
                    part.positions
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
                );
                assert!(
                    part.indices
                        .iter()
                        .all(|&index| (index as usize) < part.positions.len())
                );
            }
            if item != ItemKind::GlassBottle {
                assert!(parts[0].positions.len() > empty[0].positions.len());
            }
        }
        assert!(batches(ItemKind::Potion(u16::MAX), &templates).is_err());
    }
}
