use super::EditJournal;
use terra_core::logistics::{DistributorConfig, FilterConfig};

impl EditJournal {
    pub fn tunnel_random_at(&self, position: [i64; 3]) -> bool {
        self.random_tunnels.contains(&position)
    }

    pub fn tunnel_random(&self) -> impl Iterator<Item = [i64; 3]> + '_ {
        self.random_tunnels.iter().copied()
    }

    pub fn set_tunnel_random(&mut self, position: [i64; 3], random: bool) -> Result<bool, String> {
        let material = self
            .block(position)
            .filter(|m| terra_core::logistics::is_tunnel(*m));
        if material.is_none()
            || self
                .object_state(position)
                .is_none_or(|state| !state.valid_for(position, material.unwrap_or(0)))
        {
            return Err("Invalid tunnel extraction configuration".into());
        }
        if self.tunnel_random_at(position) == random {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Tunnel configuration revision overflow")?;
        if random {
            self.random_tunnels.insert(position);
        } else {
            self.random_tunnels.remove(&position);
        }
        self.gameplay_mut().revision = revision;
        Ok(true)
    }

    pub fn distributor_at(&self, position: [i64; 3]) -> Option<&DistributorConfig> {
        self.distributors.get(&position)
    }

    pub fn distributors(&self) -> impl Iterator<Item = ([i64; 3], &DistributorConfig)> + '_ {
        self.distributors
            .iter()
            .map(|(&position, config)| (position, config))
    }

    pub fn set_distributor(
        &mut self,
        position: [i64; 3],
        config: DistributorConfig,
    ) -> Result<bool, String> {
        if self.block(position) != Some(terra_core::logistics::BELT_DISTRIBUTOR)
            || self.object_state(position).is_none_or(|state| {
                !state.valid_for(position, terra_core::logistics::BELT_DISTRIBUTOR)
            })
            || !config.valid()
        {
            return Err("Invalid distributor configuration".into());
        }
        if self.distributors.get(&position) == Some(&config) {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Filter revision overflow")?;
        self.distributors.insert(position, config);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }

    pub fn logistics_filter_at(&self, position: [i64; 3]) -> Option<&FilterConfig> {
        self.block(position)
            .filter(|material| terra_core::logistics::supports_filter(*material))?;
        self.logistics_filters.get(&position)
    }

    pub fn logistics_filters(&self) -> impl Iterator<Item = ([i64; 3], &FilterConfig)> + '_ {
        self.logistics_filters
            .iter()
            .filter(|(position, _)| {
                self.block(**position)
                    .is_some_and(terra_core::logistics::supports_filter)
            })
            .map(|(&position, filter)| (position, filter))
    }

    pub fn set_logistics_filter(
        &mut self,
        position: [i64; 3],
        filter: FilterConfig,
    ) -> Result<bool, String> {
        let material = self
            .block(position)
            .filter(|material| terra_core::logistics::supports_filter(*material));
        if material.is_none()
            || self
                .object_state(position)
                .is_none_or(|state| !state.valid_for(position, material.unwrap_or(0)))
            || !filter.valid()
        {
            return Err("Invalid filtered funnel configuration".into());
        }
        if self.logistics_filters.get(&position) == Some(&filter) {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Filter revision overflow")?;
        self.logistics_filters.insert(position, filter);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;
    use terra_core::ItemKind;
    use terra_core::logistics::FILTERED_FUNNEL;

    #[test]
    fn ordinary_ports_reject_filters_and_ignore_legacy_private_ghosts() {
        for material in [terra_core::logistics::TUNNEL, terra_core::logistics::FUNNEL] {
            let mut journal = EditJournal::default();
            let p = [-17, 4, -1];
            journal.record(p, material).unwrap();
            journal
                .set_object_state(p, Some(BlockObjectState::default()))
                .unwrap();
            let filter = FilterConfig {
                item: Some(ItemKind::Block(3)),
                allowed: false,
            };
            let revision = journal.gameplay().revision;
            assert!(journal.set_logistics_filter(p, filter).is_err());
            assert_eq!(journal.gameplay().revision, revision);
            // A legacy ghost must never restrict real transport or be saved again.
            journal.logistics_filters.insert(p, filter);
            assert!(journal.logistics_filter_at(p).is_none());
            assert_eq!(journal.logistics_filters().count(), 0);
            if terra_core::logistics::is_tunnel(material) {
                assert!(journal.set_tunnel_random(p, true).unwrap());
                assert!(journal.tunnel_random_at(p));
            }
        }
    }

    #[test]
    fn random_extraction_is_opt_in_for_both_tunnels_and_private_until_removed() {
        for material in [
            terra_core::logistics::TUNNEL,
            terra_core::logistics::FILTERED_TUNNEL,
        ] {
            let mut journal = EditJournal::default();
            let position = [-17, 4, -1];
            journal.record(position, material).unwrap();
            journal
                .set_object_state(position, Some(BlockObjectState::default()))
                .unwrap();
            let epoch = journal.mesh_epoch(terra_voxel::global_to_section(position).0);
            assert!(!journal.tunnel_random_at(position));
            assert!(journal.set_tunnel_random(position, true).unwrap());
            assert!(!journal.set_tunnel_random(position, true).unwrap());
            assert_eq!(journal.tunnel_random().collect::<Vec<_>>(), vec![position]);
            assert_eq!(
                journal.mesh_epoch(terra_voxel::global_to_section(position).0),
                epoch
            );
            journal.set_object_state(position, None).unwrap();
            assert!(!journal.tunnel_random_at(position));
            journal.record(position, 0).unwrap();
            assert!(journal.set_tunnel_random(position, true).is_err());
        }
        let mut journal = EditJournal::default();
        journal.record([0; 3], FILTERED_FUNNEL).unwrap();
        journal
            .set_object_state([0; 3], Some(BlockObjectState::default()))
            .unwrap();
        assert!(journal.set_tunnel_random([0; 3], true).is_err());
    }

    #[test]
    fn failed_random_config_revision_is_atomic() {
        let mut journal = EditJournal::default();
        journal
            .record([0; 3], terra_core::logistics::TUNNEL)
            .unwrap();
        journal
            .set_object_state([0; 3], Some(BlockObjectState::default()))
            .unwrap();
        journal.gameplay_mut().revision = u64::MAX;
        assert!(journal.set_tunnel_random([0; 3], true).is_err());
        assert!(!journal.tunnel_random_at([0; 3]));
    }

    #[test]
    fn filter_edits_are_private_metadata_and_removed_with_the_port() {
        let mut journal = EditJournal::default();
        let position = [-17, 4, -1];
        journal.record(position, FILTERED_FUNNEL).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let section = terra_voxel::global_to_section(position).0;
        let epoch = journal.mesh_epoch(section);
        let filter = FilterConfig {
            item: Some(ItemKind::Block(1)),
            allowed: false,
        };
        assert!(journal.set_logistics_filter(position, filter).unwrap());
        assert!(!journal.set_logistics_filter(position, filter).unwrap());
        assert_eq!(journal.mesh_epoch(section), epoch);
        assert_eq!(journal.logistics_filter_at(position), Some(&filter));
        assert_eq!(journal.logistics_filters().count(), 1);
        journal.record(position, 0).unwrap();
        assert!(journal.logistics_filter_at(position).is_none());
        assert!(journal.set_logistics_filter(position, filter).is_err());
    }

    #[test]
    fn invalid_and_overflowing_filter_changes_are_atomic() {
        let mut journal = EditJournal::default();
        let position = [1, 1, 1];
        journal.record(position, FILTERED_FUNNEL).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let filter = FilterConfig {
            item: Some(ItemKind::Block(1)),
            allowed: true,
        };
        journal.set_logistics_filter(position, filter).unwrap();
        assert!(
            journal
                .set_logistics_filter(
                    position,
                    FilterConfig {
                        item: Some(ItemKind::Block(u16::MAX)),
                        ..filter
                    }
                )
                .is_err()
        );
        assert_eq!(journal.logistics_filter_at(position), Some(&filter));
        journal.gameplay_mut().revision = u64::MAX;
        assert!(
            journal
                .set_logistics_filter(
                    position,
                    FilterConfig {
                        allowed: false,
                        ..filter
                    }
                )
                .is_err()
        );
        assert_eq!(journal.logistics_filter_at(position), Some(&filter));
        assert!(journal.container_at(position).is_none());
    }

    #[test]
    fn distributor_three_filters_are_atomic_private_metadata_with_bounded_identity() {
        let mut journal = EditJournal::default();
        let position = [-17, 15, -16];
        journal
            .record(position, terra_core::logistics::BELT_DISTRIBUTOR)
            .unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let section = terra_voxel::global_to_section(position).0;
        let epoch = journal.mesh_epoch(section);
        let config = DistributorConfig {
            filters: [FilterConfig {
                item: Some(ItemKind::Block(1)),
                allowed: false,
            }; 3],
        };
        assert!(journal.set_distributor(position, config).unwrap());
        assert!(!journal.set_distributor(position, config).unwrap());
        assert_eq!(journal.mesh_epoch(section), epoch);
        assert!(journal.container_at(position).is_none());
        let mut invalid = config;
        invalid.filters[2].item = Some(ItemKind::Block(u16::MAX));
        assert!(journal.set_distributor(position, invalid).is_err());
        journal.gameplay_mut().revision = u64::MAX;
        assert!(
            journal
                .set_distributor(position, Default::default())
                .is_err()
        );
        assert_eq!(journal.distributor_at(position), Some(&config));
        journal.gameplay_mut().revision = 0;
        journal.set_object_state(position, None).unwrap();
        assert!(journal.distributor_at(position).is_none());
        assert!(journal.set_distributor(position, config).is_err());
    }
}
