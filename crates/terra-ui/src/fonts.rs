use bevy::{
    prelude::*,
    text::{ComputedTextBlock, Font, FontCx},
};
use std::collections::{HashSet, VecDeque};

const MAX_PINNED_FONTS: usize = 32;
const MAX_PINNED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiFontCacheStatus {
    pub pinned_fonts: usize,
    pub pinned_bytes: u64,
    pub evicted_fonts: u64,
    pub oversized_fonts: u64,
}

#[derive(Resource)]
struct FontPins {
    fonts: VecDeque<Font>,
    bytes: usize,
    font_limit: usize,
    byte_limit: usize,
    evicted: u64,
    oversized: u64,
}

impl Default for FontPins {
    fn default() -> Self {
        Self::new(MAX_PINNED_FONTS, MAX_PINNED_BYTES)
    }
}

impl FontPins {
    fn new(font_limit: usize, byte_limit: usize) -> Self {
        Self {
            fonts: VecDeque::new(),
            bytes: 0,
            font_limit,
            byte_limit,
            evicted: 0,
            oversized: 0,
        }
    }

    fn pin(&mut self, font: Font) {
        if let Some(index) = self
            .fonts
            .iter()
            .position(|old| old.data.id() == font.data.id())
        {
            if let Some(existing) = self.fonts.remove(index) {
                self.fonts.push_back(existing);
            }
            return;
        }
        let bytes = font.data.len();
        if self.font_limit == 0 || bytes > self.byte_limit {
            self.oversized = self.oversized.saturating_add(1);
            return;
        }
        while self.fonts.len() >= self.font_limit
            || bytes > self.byte_limit.saturating_sub(self.bytes)
        {
            let Some(oldest) = self.fonts.pop_front() else {
                break;
            };
            self.bytes -= oldest.data.len();
            self.evicted = self.evicted.saturating_add(1);
        }
        self.bytes += bytes;
        self.fonts.push_back(font);
    }

    fn status(&self) -> UiFontCacheStatus {
        UiFontCacheStatus {
            pinned_fonts: self.fonts.len(),
            pinned_bytes: self.bytes as u64,
            evicted_fonts: self.evicted,
            oversized_fonts: self.oversized,
        }
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<FontPins>()
        .init_resource::<UiFontCacheStatus>()
        .add_systems(PreStartup, share_font_sources)
        .add_systems(Last, pin_layout_fonts);
}

fn share_font_sources(fonts: Option<ResMut<FontCx>>) {
    if let Some(mut fonts) = fonts {
        // The shared store can recover an existing Blob ID after Bevy prunes its local path cache.
        fonts.context.source_cache.make_shared();
    }
}

fn pin_layout_fonts(
    changed: Query<&ComputedTextBlock, Changed<ComputedTextBlock>>,
    assets: Option<Res<Assets<Font>>>,
    mut pins: ResMut<FontPins>,
    mut status: ResMut<UiFontCacheStatus>,
) {
    if changed.is_empty() {
        return;
    }
    let asset_ids: HashSet<_> = assets
        .as_ref()
        .into_iter()
        .flat_map(|assets| assets.iter().map(|(_, font)| font.data.id()))
        .collect();
    for text in &changed {
        for line in text.buffer().lines() {
            for run in line.runs() {
                let data = &run.font().data;
                if !asset_ids.contains(&data.id()) {
                    // Shared sources retain only weak references; bounded strong pins bridge relayouts.
                    pins.pin(Font {
                        data: data.clone(),
                        alias: String::new(),
                    });
                }
            }
        }
    }
    *status = pins.status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_pins_deduplicate_and_release_the_least_recently_used_blob() {
        let mut pins = FontPins::new(2, 100);
        let first = Font::from_bytes(vec![1; 4]);
        let first_weak = first.data.downgrade();
        let second = Font::from_bytes(vec![2; 4]);
        let second_weak = second.data.downgrade();
        pins.pin(first.clone());
        pins.pin(second);
        pins.pin(first);
        assert_eq!(pins.status().pinned_fonts, 2);
        assert_eq!(pins.status().pinned_bytes, 8);
        pins.pin(Font::from_bytes(vec![3; 4]));
        assert!(first_weak.upgrade().is_some());
        assert!(second_weak.upgrade().is_none());
        assert_eq!(pins.status().pinned_fonts, 2);
        assert_eq!(pins.status().pinned_bytes, 8);
        assert_eq!(pins.status().evicted_fonts, 1);
    }

    #[test]
    fn font_pins_enforce_the_byte_budget_and_skip_oversized_fonts() {
        let mut pins = FontPins::new(32, 8);
        let first = Font::from_bytes(vec![1; 4]);
        let first_weak = first.data.downgrade();
        let second = Font::from_bytes(vec![2; 4]);
        let second_weak = second.data.downgrade();
        pins.pin(first);
        pins.pin(second);
        pins.pin(Font::from_bytes(vec![3; 5]));
        assert!(first_weak.upgrade().is_none());
        assert!(second_weak.upgrade().is_none());
        assert_eq!(pins.status().pinned_fonts, 1);
        assert_eq!(pins.status().pinned_bytes, 5);
        assert_eq!(pins.status().evicted_fonts, 2);
        let oversized = Font::from_bytes(vec![4; 9]);
        let oversized_weak = oversized.data.downgrade();
        pins.pin(oversized);
        assert!(oversized_weak.upgrade().is_none());
        assert_eq!(pins.status().pinned_fonts, 1);
        assert_eq!(pins.status().pinned_bytes, 5);
        assert_eq!(pins.status().oversized_fonts, 1);
    }

    #[test]
    fn shared_system_font_sources_with_pins_preserve_blob_ids_across_cache_churn() {
        let mut fonts = FontCx::default();
        let family = ["DejaVu Sans", "Liberation Sans", "Noto Sans", "Arial"]
            .into_iter()
            .find_map(|name| fonts.collection.family_by_name(name));
        let Some(font) = family.and_then(|family| family.default_font().cloned()) else {
            eprintln!("SKIP: no native system font available for the path-cache regression");
            return;
        };
        let original = font
            .load(Some(&mut fonts.context.source_cache))
            .expect("the discovered native system font must be readable");
        let original_id = original.id();
        fonts.context.source_cache.prune(0, false);
        let reloaded = font
            .load(Some(&mut fonts.context.source_cache))
            .expect("the same native font must remain readable after pruning");
        assert_ne!(
            original_id,
            reloaded.id(),
            "unshared path reload must reproduce ID churn"
        );

        fonts.context.source_cache.make_shared();
        let stable_id = reloaded.id();
        let mut pins = FontPins::default();
        pins.pin(Font {
            data: reloaded,
            alias: String::new(),
        });
        for _ in 0..256 {
            fonts.context.source_cache.prune(0, false);
            let data = font
                .load(Some(&mut fonts.context.source_cache))
                .expect("the pinned native system font must remain readable");
            assert_eq!(data.id(), stable_id);
            pins.pin(Font {
                data,
                alias: String::new(),
            });
        }
        assert_eq!(pins.status().pinned_fonts, 1);
        assert!(pins.status().pinned_bytes > 0);
    }

    #[test]
    fn font_cache_installation_accepts_a_fixture_without_text_resources() {
        let mut app = App::new();
        install(&mut app);
        app.update();
        assert_eq!(
            *app.world().resource::<UiFontCacheStatus>(),
            UiFontCacheStatus::default()
        );
    }
}
