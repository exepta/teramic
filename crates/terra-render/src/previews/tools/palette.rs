#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolMaterial {
    Wood,
    Stone,
    Iron,
    Gold,
    Diamond,
    Ruby,
}

pub(super) struct Palette {
    pub kind: ToolMaterial,
    pub body: [f32; 3],
    pub light: [f32; 3],
    pub dark: [f32; 3],
    pub edge: [f32; 3],
    pub fitting: [f32; 3],
    pub fitting_dark: [f32; 3],
    pub inlay: [f32; 3],
}

pub(super) const WOOD: [f32; 3] = [0.40, 0.20, 0.075];
pub(super) const WOOD_LIGHT: [f32; 3] = [0.56, 0.31, 0.125];
pub(super) const WOOD_DARK: [f32; 3] = [0.25, 0.115, 0.045];
pub(super) const LEATHER: [f32; 3] = [0.34, 0.235, 0.15];
pub(super) const LEATHER_LIGHT: [f32; 3] = [0.54, 0.40, 0.255];

impl Palette {
    pub fn for_material(kind: ToolMaterial) -> Self {
        use ToolMaterial::*;
        let (body, light, dark, edge, fitting, fitting_dark, inlay) = match kind {
            Wood => (
                [0.55, 0.315, 0.145],
                [0.70, 0.455, 0.23],
                [0.33, 0.18, 0.08],
                [0.80, 0.575, 0.32],
                [0.50, 0.35, 0.21],
                [0.29, 0.20, 0.13],
                [0.68, 0.53, 0.32],
            ),
            Stone => (
                [0.45, 0.46, 0.45],
                [0.62, 0.63, 0.60],
                [0.27, 0.29, 0.28],
                [0.77, 0.77, 0.71],
                [0.43, 0.37, 0.28],
                [0.25, 0.22, 0.18],
                [0.65, 0.56, 0.40],
            ),
            Iron => (
                [0.57, 0.64, 0.67],
                [0.78, 0.84, 0.84],
                [0.32, 0.39, 0.43],
                [0.91, 0.94, 0.92],
                [0.55, 0.61, 0.62],
                [0.29, 0.34, 0.35],
                [0.79, 0.83, 0.80],
            ),
            Gold => (
                [0.84, 0.53, 0.105],
                [0.99, 0.77, 0.27],
                [0.49, 0.28, 0.045],
                [1.0, 0.91, 0.53],
                [0.88, 0.61, 0.17],
                [0.50, 0.33, 0.07],
                [1.0, 0.84, 0.36],
            ),
            Diamond => (
                [0.13, 0.70, 0.74],
                [0.36, 0.90, 0.88],
                [0.045, 0.39, 0.48],
                [0.73, 1.0, 0.97],
                [0.66, 0.76, 0.76],
                [0.33, 0.45, 0.47],
                [0.12, 0.78, 0.81],
            ),
            Ruby => (
                [0.73, 0.075, 0.14],
                [0.96, 0.22, 0.26],
                [0.36, 0.035, 0.09],
                [1.0, 0.57, 0.55],
                [0.89, 0.63, 0.20],
                [0.47, 0.31, 0.075],
                [0.88, 0.11, 0.19],
            ),
        };
        Self {
            kind,
            body,
            light,
            dark,
            edge,
            fitting,
            fitting_dark,
            inlay,
        }
    }

    pub fn primitive(&self) -> bool {
        matches!(self.kind, ToolMaterial::Wood | ToolMaterial::Stone)
    }
    pub fn crystal(&self) -> bool {
        matches!(self.kind, ToolMaterial::Diamond | ToolMaterial::Ruby)
    }
}
