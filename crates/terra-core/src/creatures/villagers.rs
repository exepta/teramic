//! Settlement residents share creature persistence without claiming natural spawn slots.
/// Existing professions retain their stable 0..9 values.
pub const CIVILIAN_ROLE_COUNT: u8 = 10;
pub const ROLE_COUNT: u8 = 13;
/// Identity capacity for later recruitment/growth, not a generated population cap.
pub const MAX_RESIDENTS: u16 = 4096;
const NAMESPACE: u64 = 1 << 58;

pub fn id(home: [i64; 2], slot: u16) -> Option<u64> {
    if slot >= MAX_RESIDENTS || home.iter().any(|v| v.unsigned_abs() > 31_250) {
        return None;
    }
    Some(
        NAMESPACE
            | ((home[0] + 32768) as u64) << 23
            | ((home[1] + 32768) as u64) << 7
            | (u64::from(slot / 128) << 39)
            | u64::from(slot % 128),
    )
}

pub fn slot(value: u64, home: [i64; 2]) -> Option<u16> {
    let slot = ((value & 127) | (((value >> 39) & 31) << 7)) as u16;
    (id(home, slot) == Some(value)).then_some(slot)
}

pub fn name(id: u64) -> &'static str {
    const NAMES: [&str; 32] = [
        "Alva", "Bram", "Cara", "Darin", "Elin", "Fenn", "Greta", "Hanno", "Ida", "Jorin", "Kira",
        "Lars", "Mara", "Nils", "Oda", "Pavel", "Runa", "Sven", "Tara", "Ulrik", "Vera", "Willem",
        "Yara", "Zora", "Ansel", "Bente", "Cora", "Edda", "Finn", "Lina", "Milo", "Nora",
    ];
    let mut h = id ^ 0x761c_bde4_a931_820f;
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    NAMES[((h ^ (h >> 27)) % NAMES.len() as u64) as usize]
}

pub fn sleeping(id: u64, phase: f64) -> bool {
    // Residents turn in individually between 20:00 and 21:00, waking around 06:00.
    let bedtime = (20.0 + (id % 60) as f64 / 60.0) / 24.0;
    phase >= bedtime || phase < 0.25
}

pub fn greeting(id: u64, role: u8, asleep: bool) -> String {
    let text = if asleep {
        "Ich schlafe gerade. Komm morgen wieder."
    } else {
        match role {
            10 => "Schwert und Schild bewachen unser Koenigreich.",
            11 => "Mit dem Speer halte ich die Tore frei.",
            12 => "Vom Wachturm habe ich die Wege im Blick.",
            1 => "Die Felder brauchen jeden Tag Pflege.",
            2..=4 => "Ich halte nach dem Rechten Ausschau.",
            5 => "Moegest du sicher durch die Nacht kommen.",
            6 => "Ein guter Pfeil braucht gerade Federn.",
            7 => "Ruh dich aus, wenn du verletzt bist.",
            8 => "Hier im Dorf gibt es immer etwas zu reparieren.",
            9 => "Im Wald wartet noch Arbeit auf mich.",
            _ => "Willkommen in unserem Dorf.",
        }
    };
    format!("{}: {text}", name(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identities_are_injective_negative_safe_and_separate_from_all_mob_namespaces() {
        let mut ids = std::collections::BTreeSet::new();
        for x in -2..=2 {
            for z in -2..=2 {
                let home = [x, z];
                for s in 0..MAX_RESIDENTS {
                    let value = id(home, s).unwrap();
                    assert!(ids.insert(value));
                    assert_eq!(slot(value, home), Some(s));
                    assert!(!super::super::spawning::is_command_id(value));
                    assert!(!super::super::breeding::is_birth_id(value));
                    assert_ne!(Some(value), super::super::creature_id(home, 0));
                    assert_ne!(Some(value), super::super::stalker_id(home));
                    assert_eq!(name(value), name(value));
                }
            }
        }
        assert!(id([31_251, 0], 0).is_none());
        assert!(id([0, 0], MAX_RESIDENTS).is_none());
    }
    #[test]
    fn extended_residents_keep_every_legacy_identity_and_survive_more_than_initial_army_counts() {
        for home in [[0, 0], [-31250, 31250], [18, -23]] {
            for slot in 0..128 {
                let old = NAMESPACE
                    | ((home[0] + 32768) as u64) << 23
                    | ((home[1] + 32768) as u64) << 7
                    | u64::from(slot);
                assert_eq!(id(home, slot), Some(old));
            }
            for index in [127, 128, 143, 255, 511, 4095] {
                let value = id(home, index).unwrap();
                assert_eq!(slot(value, home), Some(index));
                assert_ne!(Some(value), super::super::stalker_id(home));
            }
        }
    }
    #[test]
    fn night_schedule_is_staggered_and_ends_at_six() {
        assert!(sleeping(1, 0.9));
        assert!(sleeping(1, 0.1));
        assert!(!sleeping(1, 0.5));
        assert!(!sleeping(1, 0.25));
        assert_ne!(sleeping(0, 20.5 / 24.0), sleeping(59, 20.5 / 24.0));
    }
}
