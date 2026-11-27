use super::{GOLD, LEATHER, Sculpt, cloth, pouch, tint};
use terra_core::creatures::villagers::{SWAMP_HERMIT, SWAMP_WARLOCK, SWAMP_WITCH};

const HERBS: [f32; 3] = [0.44, 0.55, 0.22];
const WISP: [f32; 3] = [0.39, 0.85, 0.73];

pub(super) fn head(b: &mut Sculpt, role: u8) {
    let color = cloth(role);
    match role {
        SWAMP_WITCH => {
            b.oval([0., 0.18, 0.01], [0.77, 0.075, 0.64], tint(color, 0.72));
            // An asymmetric stepped crown turns forward at its tip.
            for (x, y, z, w) in [
                (0., 0.25, 0., 0.43),
                (-0.02, 0.36, 0.01, 0.33),
                (-0.065, 0.46, 0.025, 0.23),
                (-0.115, 0.53, 0.005, 0.15),
                (-0.16, 0.55, -0.04, 0.10),
            ] {
                b.oval([x, y, z], [w, 0.12, w * 0.89], color);
            }
            b.oval([0., 0.23, 0.], [0.445, 0.046, 0.40], LEATHER);
            b.block([0., 0.228, -0.204], [0.082, 0.062, 0.03], GOLD);
            b.block([0.235, 0.237, -0.080], [0.043, 0.098, 0.038], HERBS);
        }
        SWAMP_WARLOCK => {
            b.rounded_box([0., 0.12, 0.06], [0.52, 0.36, 0.45], 0.09, color);
            b.oval([0., 0.20, -0.02], [0.525, 0.075, 0.51], tint(color, 1.20));
            for side in [-1., 1.] {
                b.block([side * 0.214, 0., -0.05], [0.08, 0.21, 0.30], color);
                b.block([side * 0.235, 0.20, 0.03], [0.045, 0.21, 0.065], LEATHER);
                b.block([side * 0.274, 0.314, 0.03], [0.075, 0.08, 0.06], GOLD);
            }
        }
        SWAMP_HERMIT => {
            b.oval([0., 0.17, 0.], [0.64, 0.05, 0.53], tint(color, 1.25));
            b.oval([0., 0.235, 0.], [0.43, 0.125, 0.38], color);
            b.block([0.20, 0.29, -0.04], [0.045, 0.15, 0.03], HERBS);
        }
        _ => {}
    }
}

pub(super) fn costume(b: &mut Sculpt, role: u8) {
    if !matches!(role, SWAMP_WITCH | SWAMP_WARLOCK | SWAMP_HERMIT) {
        return;
    }
    b.block([0., 0.9, -0.16], [0.43, 0.09, 0.085], LEATHER);
    for (i, color) in [HERBS, WISP, [0.75, 0.43, 0.32]].into_iter().enumerate() {
        let x = -0.12 + i as f32 * 0.11;
        b.block([x, 0.85, -0.224], [0.067, 0.09, 0.067], color);
        b.block([x, 0.909, -0.224], [0.037, 0.034, 0.037], LEATHER);
    }
    pouch(b, 0.214, 0.725, 0.05, LEATHER);
}

pub(super) fn tool(b: &mut Sculpt, role: u8) {
    match role {
        SWAMP_WITCH => {
            // A capped medicine flask secured in a wicker carrier.
            b.block([0., -0.415, -0.10], [0.105, 0.16, 0.105], WISP);
            b.block([0., -0.32, -0.10], [0.055, 0.06, 0.055], LEATHER);
            b.block([0., -0.451, -0.10], [0.123, 0.038, 0.122], GOLD);
        }
        SWAMP_WARLOCK | SWAMP_HERMIT => {
            b.block([0.016, -0.30, -0.12], [0.045, 1.48, 0.053], LEATHER);
            if role == SWAMP_WARLOCK {
                b.block([0.019, 0.448, -0.12], [0.109, 0.10, 0.115], GOLD);
                b.block([0.019, 0.54, -0.12], [0.084, 0.16, 0.09], WISP);
            } else {
                b.block(
                    [0.06, 0.43, -0.10],
                    [0.13, 0.06, 0.058],
                    tint(LEATHER, 1.45),
                );
            }
        }
        _ => {}
    }
}
