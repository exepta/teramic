//! Optional exploration sidecar. Old TRMAP tiles and waypoint books are untouched.
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use terra_core::kingdom::SettlementDomain;
const MAGIC: &[u8; 8] = b"TRDOM001";
pub const MAX_DOMAINS: usize = 16_384;
const MAX_BYTES: u64 = 4 * 1024 * 1024;
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
pub fn load(root: &Path) -> Result<Vec<SettlementDomain>, String> {
    let file = match fs::File::open(root.join("settlements.dom")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() < 16 || bytes.len() as u64 > MAX_BYTES || &bytes[..8] != MAGIC {
        return Err("Invalid settlement exploration header/size".into());
    }
    let end = bytes.len() - 8;
    if u64::from_le_bytes(bytes[end..].try_into().unwrap()) != checksum(&bytes[..end]) {
        return Err("Invalid settlement exploration checksum".into());
    }
    let domains: Vec<SettlementDomain> =
        postcard::from_bytes(&bytes[8..end]).map_err(|e| e.to_string())?;
    validate(&domains)?;
    Ok(domains)
}
fn validate(domains: &[SettlementDomain]) -> Result<(), String> {
    if domains.len() > MAX_DOMAINS || domains.iter().any(|d| !d.valid()) {
        return Err("Invalid settlement exploration domain".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    if domains.iter().any(|d| !ids.insert(d.id)) {
        return Err("Duplicate settlement exploration identity".into());
    }
    Ok(())
}
pub fn save(root: &Path, domains: &[SettlementDomain]) -> Result<(), String> {
    validate(domains)?;
    let mut sorted = domains.to_vec();
    sorted.sort_unstable_by_key(|d| d.id);
    let payload = postcard::to_allocvec(&sorted).map_err(|e| e.to_string())?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend(payload);
    bytes.extend(checksum(&bytes).to_le_bytes());
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Settlement exploration exceeds size limit".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let temp = root.join("settlements.dom.tmp");
    {
        let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
    }
    fs::rename(temp, root.join("settlements.dom")).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::kingdom::{BotKingdom, DomainKind};
    #[test]
    fn explored_settlement_sidecar_roundtrips_without_touching_tiles_or_waypoints() {
        let root = std::env::temp_dir().join(format!("terra-domains-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("-1_0.map"), b"legacy exploration").unwrap();
        fs::write(root.join("waypoints.toml"), b"waypoints remain").unwrap();
        assert!(load(&root).unwrap().is_empty());
        let domain = SettlementDomain {
            id: 94,
            center: [-16, 50, 32],
            bounds: [[-32, 20, 16], [15, 80, 63]],
            kind: DomainKind::Village,
            native: Some(BotKingdom::Lutera),
        };
        save(&root, &[domain]).unwrap();
        assert_eq!(load(&root).unwrap(), vec![domain]);
        assert_eq!(
            fs::read(root.join("-1_0.map")).unwrap(),
            b"legacy exploration"
        );
        assert_eq!(
            fs::read(root.join("waypoints.toml")).unwrap(),
            b"waypoints remain"
        );
        assert!(save(&root, &[domain, domain]).is_err());
        let file = root.join("settlements.dom");
        let mut bytes = fs::read(&file).unwrap();
        bytes[10] ^= 1;
        fs::write(file, bytes).unwrap();
        assert!(load(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
