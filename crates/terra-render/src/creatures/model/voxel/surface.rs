//! One exterior surface per occupied cell, shared by animals and resident sculptures.
use bevy::prelude::*;
use std::collections::BTreeMap;

pub(in crate::creatures::model) type Cell = [i16; 3];
pub(in crate::creatures::model) type Tint = [u32; 3];

pub(in crate::creatures::model) fn mesh(
    cells: &BTreeMap<Cell, Tint>,
    origin: Vec3,
    step: f32,
    maximum: Vec3,
) -> Result<Mesh, String> {
    if cells.len() > 262_144 {
        return Err("Creature sculpture exceeds its occupied-cell budget".into());
    }
    // Ceil(extent / step) may produce one extra boundary cell after f32
    // roundoff. Such a zero-width cell must not hide a real neighbor face or
    // emit two coincident caps when its coordinates are clipped to `maximum`.
    let has_volume = |cell: Cell| {
        let low = origin + Vec3::from_array(cell.map(f32::from)) * step;
        low.cmplt(maximum - Vec3::splat(step * 1e-5)).all()
    };
    let mut output = crate::players::model::Builder::default();
    let mut rectangles = 0;
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        for sign in [-1_i16, 1] {
            let mut faces = BTreeMap::new();
            for (&cell, &color) in cells {
                if !has_volume(cell) {
                    continue;
                }
                let mut neighbor = cell;
                neighbor[axis] += sign;
                if !has_volume(neighbor) || !cells.contains_key(&neighbor) {
                    faces.insert([cell[axis], cell[v], cell[u]], color);
                }
            }
            while let Some((cell, color)) = faces.pop_first() {
                let mut end = cell[2] + 1;
                while faces.get(&[cell[0], cell[1], end]) == Some(&color) {
                    faces.remove(&[cell[0], cell[1], end]);
                    end += 1;
                }
                let mut top = cell[1] + 1;
                while (cell[2]..end).all(|x| faces.get(&[cell[0], top, x]) == Some(&color)) {
                    for x in cell[2]..end {
                        faces.remove(&[cell[0], top, x]);
                    }
                    top += 1;
                }
                let mut p = origin;
                p[axis] += f32::from(cell[0] + i16::from(sign > 0)) * step;
                p[u] += f32::from(cell[2]) * step;
                p[v] += f32::from(cell[1]) * step;
                let mut du = Vec3::ZERO;
                du[u] = f32::from(end - cell[2]) * step;
                let mut dv = Vec3::ZERO;
                dv[v] = f32::from(top - cell[1]) * step;
                let mut corners = [p, p + du, p + du + dv, p + dv].map(|p| p.min(maximum));
                if sign < 0 {
                    corners.reverse();
                }
                if (corners[1] - corners[0])
                    .cross(corners[2] - corners[0])
                    .length_squared()
                    > 1e-18
                {
                    rectangles += 1;
                    if rectangles > 16_000 {
                        return Err("Creature sculpture exceeds its exterior surface budget".into());
                    }
                    output.quad(corners, color.map(f32::from_bits))?;
                }
            }
        }
    }
    output.finish()
}
