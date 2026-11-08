use super::*;
use terra_core::{
    Inventory, ItemKind, ItemStack,
    trains::{TrainPart, Vehicle, VehicleKind},
};

#[test]
fn public_vehicle_updates_keep_private_contents_and_token_until_the_container_becomes_invalid() {
    let owner = PlayerUuid::from_u128(91);
    let parts = terra_core::train_blueprint::template([0, 2, 2], 0, VehicleKind::Locomotive)
        .into_iter()
        .map(|part| TrainPart {
            offset: part.cell.map(|v| v as i16),
            material: part.material,
            facing: part.facing,
            wood: Default::default(),
        })
        .collect::<Vec<_>>();
    let engine = parts.iter().position(|part| part.material == 218).unwrap() as u16;
    let fuel = Inventory {
        slots: vec![ItemStack {
            item: ItemKind::Mineral(terra_core::MineralItem::Coal),
            count: 3,
        }],
        revision: 2,
        ..Default::default()
    };
    let car = Vehicle {
        id: 1,
        owner,
        kind: VehicleKind::Locomotive,
        anchor: [0; 3],
        parts,
        cargo: std::collections::BTreeMap::from([(engine, fuel.clone())]),
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None; 2],
        revision: 7,
        motion: Default::default(),
        burn_seconds: 0.0,
    };
    assert!(car.valid());
    let cached = crate::protocol::trains::TrainCargoSnapshot {
        vehicle: 1,
        part: engine,
        revision: 7,
        inventory: fuel,
    };
    let mut session = ClientSession {
        uuid: Some(owner),
        train_cargo: Some(cached.clone()),
        ..Default::default()
    };
    let mut public = car.clone();
    public.revision = 11;
    public.motion.throttle = 2;
    clear_invalid_train_cargo(&mut session, &[public.view()]);
    assert_eq!(
        session.train_cargo,
        Some(cached.clone()),
        "public metadata must never stamp a private inventory token"
    );
    for mutation in 0..4 {
        session.train_cargo = Some(cached.clone());
        let mut invalid = car.clone();
        let views = match mutation {
            0 => Vec::new(),
            1 => {
                invalid.owner = PlayerUuid::from_u128(92);
                vec![invalid.view()]
            }
            2 => {
                invalid.parts.remove(usize::from(engine));
                invalid.cargo.remove(&engine);
                assert!(invalid.valid());
                vec![invalid.view()]
            }
            _ => {
                invalid.parts[usize::from(engine)].material = 212;
                invalid.cargo.remove(&engine);
                assert!(invalid.valid());
                vec![invalid.view()]
            }
        };
        assert!(views.iter().all(|view| view.valid()));
        clear_invalid_train_cargo(&mut session, &views);
        assert!(
            session.train_cargo.is_none(),
            "container invalidation case {mutation} must clear its private cache"
        );
    }
}
