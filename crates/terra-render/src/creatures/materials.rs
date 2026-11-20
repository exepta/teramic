//! Shared instance and portrait materials. Mesh vertex alpha stays at one.
use super::{Kind, Models, model::Part};
use bevy::prelude::*;

pub(super) fn slime_body() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.70),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.26,
        reflectance: 0.38,
        // Keep back-face culling: the closed jelly sculpture must not blend its
        // far side a second time. The face is an independent opaque mesh.
        ..default()
    }
}

pub(super) fn skeleton_eyes() -> StandardMaterial {
    StandardMaterial {
        emissive: LinearRgba::rgb(0.12, 0.003, 0.004),
        perceptual_roughness: 0.65,
        ..default()
    }
}

impl Models {
    pub(super) fn material_for(
        &self,
        kind: Kind,
        variant: u8,
        part: Part,
    ) -> Handle<StandardMaterial> {
        if kind == Kind::SpiritGloom {
            let index = usize::from(variant.min(terra_core::creatures::gloom::COUNT - 1));
            let select = |handle: &Handle<StandardMaterial>,
                          fallback: &Handle<StandardMaterial>| {
                if *handle == Handle::default() {
                    fallback.clone()
                } else {
                    handle.clone()
                }
            };
            return match part {
                Part::SpiritArm(_) => select(&self.gloom_wings[index], &self.bell_material),
                Part::SpiritGlow | Part::SpiritOrbit(_) => {
                    select(&self.gloom_glow[index], &self.glow_material)
                }
                Part::Body => select(&self.gloom_shell[index], &self.material),
                _ => self.material.clone(),
            };
        }
        if matches!(part, Part::ThornGlow { head: true }) {
            return self.skeleton_eye_material.clone();
        }
        if matches!(part, Part::ThornGlow { head: false }) {
            return self.blood_glow_material.clone();
        }
        if matches!(part, Part::SpiritGlow | Part::SpiritOrbit(_))
            || kind == Kind::SpiritFox && matches!(part, Part::Tail)
        {
            if kind == Kind::BloodWisp {
                self.blood_glow_material.clone()
            } else if kind == Kind::SpiritGloom && variant == 1 {
                self.violet_glow_material.clone()
            } else {
                self.glow_material.clone()
            }
        } else if matches!(kind, Kind::SpiritGloom | Kind::SpiritFox | Kind::BloodWisp)
            && !matches!(part, Part::Head)
            || kind == Kind::Butterfly && variant == 3
        {
            if kind == Kind::BloodWisp {
                self.blood_spirit_material.clone()
            } else if kind == Kind::SpiritGloom && variant == 1 {
                self.violet_spirit_material.clone()
            } else {
                self.spirit_material.clone()
            }
        } else if kind == Kind::BloodSlime && matches!(part, Part::Body) {
            self.slime_material.clone()
        } else {
            match part {
                Part::Bell => self.bell_material.clone(),
                Part::SpiderEyes => self.eye_material.clone(),
                Part::StalkerEyes => self.stalker_eye_material.clone(),
                Part::SkeletonEyes => self.skeleton_eye_material.clone(),
                _ => self.material.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thorn_heart_glows_brightly_but_eyes_stay_soft_and_bark_is_not_emissive() {
        let mut assets = Assets::<StandardMaterial>::default();
        let models = Models {
            material: assets.add(StandardMaterial::default()),
            blood_glow_material: assets.add(StandardMaterial {
                emissive: LinearRgba::rgb(2.1, 0.025, 0.07),
                ..default()
            }),
            skeleton_eye_material: assets.add(skeleton_eyes()),
            ..default()
        };
        let eyes = models.material_for(Kind::ThornBrute, 0, Part::ThornGlow { head: true });
        let heart = models.material_for(Kind::ThornBrute, 0, Part::ThornGlow { head: false });
        assert_eq!(eyes, models.skeleton_eye_material);
        assert_eq!(heart, models.blood_glow_material);
        assert!(
            assets.get(&eyes).unwrap().emissive.red
                < assets.get(&heart).unwrap().emissive.red * 0.1
        );
        for part in [
            Part::Body,
            Part::Head,
            Part::VillagerArm(-1.0),
            Part::Leg(1.0),
        ] {
            let handle = models.material_for(Kind::ThornBrute, 0, part);
            assert_eq!(handle, models.material);
            assert_eq!(assets.get(&handle).unwrap().emissive, LinearRgba::BLACK);
        }
    }

    #[test]
    fn blood_slime_blends_only_the_jelly_body_and_keeps_its_face_opaque() {
        let mut assets = Assets::<StandardMaterial>::default();
        let models = Models {
            material: assets.add(StandardMaterial::default()),
            slime_material: assets.add(slime_body()),
            skeleton_eye_material: assets.add(skeleton_eyes()),
            ..default()
        };
        let body = models.material_for(Kind::BloodSlime, 0, Part::Body);
        let face = models.material_for(Kind::BloodSlime, 0, Part::Head);
        assert_eq!(body, models.slime_material);
        assert_ne!(body, face);
        let body = assets.get(&body).unwrap();
        let face = assets.get(&face).unwrap();
        assert_eq!(body.alpha_mode, AlphaMode::Blend);
        assert!((body.base_color.alpha() - 0.70).abs() < 1e-6);
        assert!(
            body.cull_mode.is_some(),
            "do not double-blend the back shell"
        );
        assert_eq!(face.alpha_mode, AlphaMode::Opaque);
        assert_eq!(face.base_color.alpha(), 1.0);
        for variant in 0..3 {
            let pupils = models.material_for(Kind::Skeleton, variant, Part::SkeletonEyes);
            assert_eq!(pupils, models.skeleton_eye_material);
            let pupils = assets.get(&pupils).unwrap();
            assert_eq!(pupils.alpha_mode, AlphaMode::Opaque);
            assert!(pupils.emissive.red > pupils.emissive.green * 20.0);
            assert!(pupils.emissive.red > pupils.emissive.blue * 20.0);
        }
    }

    #[test]
    fn gloom_has_a_dark_opaque_face_translucent_wings_and_small_luminous_eyes() {
        let mut assets = Assets::<StandardMaterial>::default();
        let models = Models {
            material: assets.add(StandardMaterial::default()),
            bell_material: assets.add(StandardMaterial {
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            glow_material: assets.add(StandardMaterial {
                emissive: LinearRgba::rgb(0.7, 1.8, 2.6),
                ..default()
            }),
            ..default()
        };
        assert_eq!(
            models.material_for(Kind::SpiritGloom, 0, Part::Head),
            models.material
        );
        assert_eq!(
            models.material_for(Kind::SpiritGloom, 0, Part::Body),
            models.material
        );
        assert_eq!(
            models.material_for(Kind::SpiritGloom, 0, Part::SpiritArm(1.0)),
            models.bell_material
        );
        assert_eq!(
            models.material_for(Kind::SpiritGloom, 0, Part::SpiritOrbit(1.0)),
            models.glow_material
        );
    }
}
