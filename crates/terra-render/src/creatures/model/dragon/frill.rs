//! Paired facial crests, mounted at the gill cover behind each jaw hinge.
use super::*;

pub(super) fn build(m: &mut Builder, c: Coat, side: i8) -> Result<(), String> {
    let s = f32::from(side);
    let root = Joint::Frill(side).pivot();
    let local = |width: f32, height: f32, aft: f32| root + v(s * width, height, aft);
    let form = f32::from(c.variant);
    match c.kind {
        Kind::WaterDragon => {
            // A continuous fan joins the gill rays. Pearly raised bands follow
            // their length and the trailing margin has lobes between the ribs.
            let extent = [1.00, 0.87, 1.21, 1.14, 1.26, 1.02][usize::from(c.variant.min(5))];
            let mut previous = local(0.27, -0.76, 0.42);
            for i in 0..5 {
                let t = i as f32 / 4.;
                let start = local(0.03, -0.56 + 0.98 * t, 0.08 * t);
                let tip = local(
                    extent * (1.32 + 0.55 * (t * 3.0).sin()),
                    -0.96 + 2.82 * t,
                    1.72 + extent * 0.98 * (t * 2.8).sin(),
                );
                let notch = previous.lerp(tip, 0.47).lerp(root, 0.19);
                forms::blade(m, start, previous, notch, 0.10, tint(c.wing, 0.91))?;
                forms::blade(m, start, notch, tip, 0.10, tint(c.wing, 1.08))?;
                beam(m, start, tip, 0.15, 0.036, c.growth)?;
                previous = tip;
            }
        }
        Kind::EarthDragon => {
            // Layered cheek shields overlap toward the lower jaw. A pale seam
            // divides the face of each slab from its broken, mineral-rich edge.
            let stone = [c.growth, c.horn, c.scale, c.light][usize::from(c.variant.min(3))];
            for i in 0..3 {
                let t = i as f32 / 2.;
                let a = local(0.03, 0.32 - t * 1.06, t * 0.35);
                let b = local(0.31, 0.89 - t * 1.12, 0.49 + t * 0.30);
                let tip = local(1.10 + 0.35 * t, 0.83 - t * 1.37, 1.83 + t * 0.28);
                forms::blade(m, a, b, tip, 0.39, tint(c.ridge, 0.89 + t * 0.18))?;
                beam(m, b, tip, 0.16, 0.07, stone)?;
            }
            let tusk = local(0.26, -0.49, 0.12);
            horn(
                m,
                tusk,
                local(0.89, -1.02, -0.06),
                local(1.20, -0.79, -0.69 - form * 0.08),
                0.32,
                c.horn,
            )?;
        }
        Kind::FireDragon => {
            // A throat fan with a charcoal margin and forked tips unfolds for
            // the first intake. Broader obsidian and taller sulfur blades read in silhouette.
            let mut previous = local(0.22, -0.72, 0.38);
            for i in 0..4 {
                let t = i as f32 / 3.;
                let start = local(0.02, -0.65 + t * 0.98, 0.05);
                let tip = local(
                    1.05 + t * 0.45 + if c.variant == 2 { 0.45 } else { 0. },
                    -0.76 + t * (1.95 + form * 0.11),
                    1.54 + 0.87 * t,
                );
                let edge = previous.lerp(tip, 0.54).lerp(root, 0.24);
                forms::blade(m, start, previous, edge, 0.09, c.wing)?;
                forms::blade(m, start, edge, tip, 0.10, c.wing)?;
                beam(m, start, tip, 0.16, 0.09, c.ridge)?;
                horn(
                    m,
                    tip.lerp(start, 0.20),
                    tip,
                    tip + v(s * 0.20, 0.13, 0.36),
                    0.15,
                    c.horn,
                )?;
                previous = tip;
            }
        }
        Kind::DarkDragon => {
            // Two swept ear lances overlap like a raven's flight feathers.
            // The coloured inner facet is recessed within a dark outer blade.
            let reach = 1.0 + form * 0.075;
            for i in 0..2 {
                let t = i as f32;
                let a = local(0.02, -0.47 + 0.59 * t, 0.);
                let b = local(0.32, 0.31 + 0.55 * t, 0.46);
                let tip = local(
                    (1.36 + t * 0.28) * reach,
                    0.17 + t * 1.43,
                    (2.43 + t * 0.60) * reach,
                );
                forms::blade(m, a, b, tip, 0.27, c.ridge)?;
                let middle = a.lerp(b, 0.48) + v(s * 0.12, 0., 0.);
                forms::blade(
                    m,
                    middle,
                    b,
                    tip.lerp(b, 0.22) + v(s * 0.12, 0., 0.),
                    0.13,
                    c.wing,
                )?;
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}
