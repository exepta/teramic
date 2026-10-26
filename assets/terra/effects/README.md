# Status Effects

Each JSON defines one existing `EffectKind`; IDs and kinds are stable. Both client
and authority use the validated definitions. Invalid overrides fall back to the
embedded defaults. Restart after editing; live reload and arbitrary new effect
kinds are not part of this registry.

`max_duration_seconds` is bounded to 3600 seconds and `max_level` to 10. The shipped
effects allow levels 1-3. Reapplication follows `stacking`:

- `refresh_strongest`: keep the longer remaining duration and stronger level.
- `extend`: add duration up to the configured maximum; keep the stronger level.
- `replace`: replace both remaining duration and level.

Rates (`damage_per_second`, `health_per_second`, `hunger_per_second`) scale with
level. Multipliers compound by level and resulting movement/attack multipliers
are bounded to 0.1-4. Shield capacity scales with level up to 100 points. Shield
Power absorbs physical, fire and freezing damage, not poison, bleeding, smoke,
starvation, drowning or void damage. Matching resistances cancel their damage;
Freeze Resistance also cancels the freezing movement penalty.

Jellyfish apply timed poison for 10 seconds. Furnace smoke uses a separate
exposure flag: it displays Poison while inside smoke, but retains the existing
authoritative smoke damage instead of adding another poison tick. Leaving smoke
does not erase timed jellyfish poison. Exposure is replicated but not persisted;
it is resampled after loading. Sleep applies Regeneration for 5 seconds.

Existing food JSON regeneration keeps its own fractional healing rate. Food and
the Regeneration effect share one HUD card and use the greater active healing
rate instead of summing. Food regeneration still respects hunger and the
existing damage recovery delay; sleep/status regeneration does not.

Timers, levels and the remaining shield pool persist in player record `PLY7`.
Explicit readers preserve `PLYR`, `PLY2`, `PLY3`, `PLY4` and `PLY5` inventories and stats.
The world/region format and block IDs are unchanged.

On loading a player after intentional JSON edits, effect duration and level are
clamped to the current definition and shield charge is capped to its current
capacity. Loading never increases remaining duration or charge. Non-finite,
duplicate and out-of-hard-bounds records remain invalid. Client/server catalogs
must match; the connection handshake checks a stable checksum of the canonical
validated definitions, including modifiers and stacking rules.
