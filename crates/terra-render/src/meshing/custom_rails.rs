//! Shared Core rail beams, partitioned into disjoint editable-cell surfaces.
use super::{MeshBatch, ModelCell};
use crate::blocks::{BlockTemplate, TemplateSet};
use bevy::math::{Vec2, Vec3, Vec4};
use std::collections::BTreeMap;
use terra_core::rails::{PartMaterial, Piece};

#[derive(Clone, Copy)]
struct Vertex {
    position: Vec3,
    normal: Vec3,
    uv: Vec2,
    tangent: Vec4,
}

#[derive(Default)]
struct Profile {
    full: BTreeMap<usize, MeshBatch>,
    cells: BTreeMap<[i32; 3], BTreeMap<usize, MeshBatch>>,
}

#[derive(Default)]
pub(super) struct Profiles(BTreeMap<(u16, u8), Profile>);

impl Profiles {
    pub(super) fn append(
        &mut self,
        output: &mut BTreeMap<usize, MeshBatch>,
        model: &ModelCell,
        templates: &TemplateSet,
        origin: [i64; 3],
        standalone: bool,
    ) -> Result<(), String> {
        let piece = Piece::from_facing([0; 3], model.material, model.state.facing)
            .ok_or("Invalid rail model pose")?;
        let key = (piece.material, piece.encoded_facing());
        if let std::collections::btree_map::Entry::Vacant(entry) = self.0.entry(key) {
            let template = templates
                .blocks
                .get(&model.material)
                .ok_or("Missing rail template")?;
            entry.insert(profile(piece, template)?);
        }
        let profile = &self.0[&key];
        let cell = std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
        let anchor = model.state.peer.unwrap_or(cell);
        let offset: [i32; 3] = std::array::from_fn(|axis| {
            cell[axis]
                .saturating_sub(anchor[axis])
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
        });
        let source = if standalone {
            &profile.full
        } else {
            let Some(source) = profile.cells.get(&offset) else {
                return Ok(());
            };
            source
        };
        let translation = Vec3::from_array(std::array::from_fn(|axis| {
            model.cell[axis] as f32 - if standalone { 0.0 } else { offset[axis] as f32 }
        }));
        for (&material, source) in source {
            let target = output.entry(material).or_insert_with(|| MeshBatch {
                material,
                ..Default::default()
            });
            let base = u32::try_from(target.positions.len()).map_err(|_| "Rail batch overflow")?;
            target.positions.extend(
                source
                    .positions
                    .iter()
                    .map(|p| (Vec3::from_array(*p) + translation).to_array()),
            );
            target.normals.extend_from_slice(&source.normals);
            target.uvs.extend_from_slice(&source.uvs);
            target.append_tangents(base as usize, source.tangents.iter().copied());
            target
                .indices
                .extend(source.indices.iter().map(|index| index + base));
        }
        Ok(())
    }
}

fn profile(piece: Piece, template: &BlockTemplate) -> Result<Profile, String> {
    let mut result = Profile::default();
    for surface in piece.local_surfaces() {
        let beam = &piece.local_geometry()[surface.beam];
        // Native GLTF primitives use this fixed semantic material-slot order.
        let slot = match surface.material {
            PartMaterial::Wood => 0,
            PartMaterial::Steel => 1,
            PartMaterial::SteelHead => 2,
            PartMaterial::Bolt => 3,
            PartMaterial::Red => 4,
        };
        let material = template
            .model
            .get(slot)
            .ok_or("Missing rail material slot")?
            .material;
        let bounds = beam.bounds();
        let low = bounds.minimum.map(|value| (value + 1.0e-8).floor() as i32);
        let high = bounds.maximum.map(|value| (value - 1.0e-8).floor() as i32);
        for triangle in triangles(surface) {
            append_triangle(&mut result.full, material, triangle)?;
            for x in low[0]..=high[0] {
                for y in low[1]..=high[1] {
                    for z in low[2]..=high[2] {
                        let cell = [x, y, z];
                        let polygon = clip_cell(triangle, cell);
                        for corner in 1..polygon.len().saturating_sub(1) {
                            append_triangle(
                                result.cells.entry(cell).or_default(),
                                material,
                                [polygon[0], polygon[corner], polygon[corner + 1]],
                            )?;
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}

fn triangles(surface: &terra_core::rails::Surface) -> Vec<[Vertex; 3]> {
    let normal = Vec3::from_array(surface.normal.map(|value| value as f32));
    let source_tangent = Vec3::from_array(surface.tangent.map(|value| value as f32));
    let tangent = (source_tangent - normal * source_tangent.dot(normal)).normalize();
    let bitangent = normal.cross(tangent);
    let vertices = surface
        .points
        .iter()
        .map(|point| {
            let position = Vec3::from_array(point.map(|value| {
                // Occupancy applies the same port/grid tolerance. A mathematically
                // zero ramp cap must not be discarded as a tiny negative position.
                let integer = value.round();
                if (value - integer).abs() < 1.0e-8 {
                    integer as f32
                } else {
                    value as f32
                }
            }));
            Vertex {
                position,
                normal,
                uv: Vec2::new(position.dot(tangent), position.dot(bitangent)),
                tangent: tangent.extend(1.0),
            }
        })
        .collect::<Vec<_>>();
    (1..vertices.len().saturating_sub(1))
        .map(|index| [vertices[0], vertices[index], vertices[index + 1]])
        .collect()
}

fn clip_cell(triangle: [Vertex; 3], cell: [i32; 3]) -> Vec<Vertex> {
    for (axis, &coordinate) in cell.iter().enumerate() {
        let low = coordinate as f32;
        let high = low + 1.0;
        if triangle.iter().all(|vertex| vertex.position[axis] < low)
            || triangle.iter().all(|vertex| vertex.position[axis] > high)
        {
            return Vec::new();
        }
    }
    if triangle.iter().all(|vertex| {
        cell.iter().enumerate().all(|(axis, coordinate)| {
            (0.0..=1.0).contains(&(vertex.position[axis] - *coordinate as f32))
        })
    }) {
        return triangle.to_vec();
    }
    let mut polygon = triangle.to_vec();
    for (axis, coordinate) in cell.into_iter().enumerate() {
        for upper in [false, true] {
            if polygon.is_empty() {
                return polygon;
            }
            let plane = coordinate as f32 + if upper { 1.0 } else { 0.0 };
            let inside = |v: Vertex| {
                if upper {
                    v.position[axis] <= plane
                } else {
                    v.position[axis] >= plane
                }
            };
            let input = std::mem::take(&mut polygon);
            let mut previous = *input.last().unwrap();
            for current in input {
                if inside(current) != inside(previous) {
                    let fraction = (plane - previous.position[axis])
                        / (current.position[axis] - previous.position[axis]);
                    let mut intersection = Vertex {
                        position: previous.position.lerp(current.position, fraction),
                        normal: previous.normal.lerp(current.normal, fraction),
                        uv: previous.uv.lerp(current.uv, fraction),
                        tangent: previous.tangent.lerp(current.tangent, fraction),
                    };
                    intersection.position[axis] = plane;
                    polygon.push(intersection);
                }
                if inside(current) {
                    polygon.push(current);
                }
                previous = current;
            }
        }
    }
    polygon
}

fn append_triangle(
    output: &mut BTreeMap<usize, MeshBatch>,
    material: usize,
    points: [Vertex; 3],
) -> Result<(), String> {
    if (points[1].position - points[0].position)
        .cross(points[2].position - points[0].position)
        .length_squared()
        <= 1.0e-14
    {
        return Ok(());
    }
    let batch = output.entry(material).or_insert_with(|| MeshBatch {
        material,
        ..Default::default()
    });
    let base = u32::try_from(batch.positions.len()).map_err(|_| "Rail triangle overflow")?;
    for vertex in points {
        batch.positions.push(vertex.position.to_array());
        batch.normals.push(vertex.normal.to_array());
        batch.uvs.push(vertex.uv.to_array());
        batch.tangents.push(vertex.tangent.to_array());
    }
    batch.indices.extend([base, base + 1, base + 2]);
    Ok(())
}

#[cfg(test)]
#[path = "custom_rails_tests.rs"]
mod tests;
