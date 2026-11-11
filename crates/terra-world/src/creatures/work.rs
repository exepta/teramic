use super::{Creature, Habitat, Kind, navigation::Navigation, support, villagers};

/// Idle work still advances route expiry and gravity; ordinary nighttime residents do not call this.
pub fn work_rest(npc: &mut Creature, habitat: &Habitat, dt: f32) {
    if npc.kind != Kind::Villager
        || npc.health <= 0.0
        || !dt.is_finite()
        || !(0.001..=0.25).contains(&dt)
    {
        return;
    }
    npc.age += f64::from(dt);
    support::settle(npc, habitat, dt);
}

/// Productive workers reuse ordinary physical walking, cached A* and gravity.
pub fn work_step(
    npc: &mut Creature,
    habitat: &Habitat,
    destination: [f64; 3],
    dt: f32,
    navigation: &mut Navigation,
) -> bool {
    if npc.kind != Kind::Villager
        || npc.health <= 0.0
        || !dt.is_finite()
        || !(0.001..=0.25).contains(&dt)
    {
        return false;
    }
    npc.age += f64::from(dt);
    npc.needs.search_in = (npc.needs.search_in - dt).max(0.0);
    let search = npc.needs.search_in == 0.0;
    if search {
        npc.needs.search_in = 0.8;
    }
    let before = npc.position;
    villagers::walk(npc, habitat, destination, dt, search, false, navigation);
    support::settle(npc, habitat, dt);
    (npc.position[0] - before[0]).hypot(npc.position[2] - before[2]) > 0.00001
}
