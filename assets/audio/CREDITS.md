# TERAMIC Recorded Audio

## Sandworm voice and movement (20 November 2026)

Eight original runtime-prepared clips add a low, breath-driven breach roar,
granular digging, sand entry impact, chitin bite, viscous spit, hurt grunt,
collapsing death call and quiet exposed breathing. The voice layers are original
deterministic, filtered noise and irregular glottal synthesis by Project TERAMIC.
The sand, stone, wood and water Foley layers reuse the existing CC0 recordings
credited below; no animal voice or third-party monster recording is copied.

`creature_audio/worm/synthesis.rs` prepares these once on the existing audio
loading worker. The shared bank now contains 132 clips (the previous 124 indices
are unchanged). World audio uses the shared body listener, stereo pan, distance
falloff and voice limit. Breach calls require a newly visible mouth; late-join
baselines and repeated snapshots do not replay them.

`bash tools/benchmarks/run-worm-audio.sh` exports the eight mono 24000 Hz WAVs
and observes actual replicated phase fixtures through the shared audio device.
The clips can also be exported to `TERRA_CAPTURE_DIR` when both
`--smoke-test-steps` and `TERRA_WORM_AUDIO_INSPECT=1` are set.

## Nature-led biome ambience (18 November 2026)

Outdoor biomes use the existing bird, cricket, foliage and coast recordings as
recognizable natural surroundings. Spirit Blossom and Blood Forest retain fewer
birds and occasional distant crows; their background no longer uses the dungeon
recording, synthetic drones, voice-like breaths or bell phrases. The existing
`environment/leaves.wav` moving-leaf recording by Augmentality / Brandon Morris
(CC0, credited below) now supplies unhurried foliage movements at its original
pitch. Quiet gaps, source distance and modest filtering distinguish regions.
Mushroom Park keeps its nighttime insects; Cherry Forest can add a rare, very
quiet glass/seed glint using the existing Kenney CC0 glass contacts credited below.
That contextual accent is not a new recording of a magical plant.

The previous low-wind mix, eleven shared beds, equal-power biome transitions and
long loop overlaps remain. Outdoor reverb is restrained; the existing dungeon
sample stays in caves. All preparation runs once on the current audio loading
worker; no new external downloads or licenses are introduced. The extra foliage
input does not add a bank index: the shared bank still contains 124 clips.

### Environmental source preparation (16 November 2026)

New inputs are CC0 1.0 and are adapted by Project TERAMIC as mono 24000 Hz
PCM16 with filtering, loudness headroom and short edge fades:

- IgnasD, [Crows singing](https://opengameart.org/content/crows-singing),
  `Crows.ogg` → `environment/crows.wav`.
- jasinski, excerpts prepared by qubodup,
  [Beach Ocean Waves](https://opengameart.org/content/beach-ocean-waves),
  four `wave_0{1,2,3,4}_cc0-18363__jasinski__alkaibeach.flac` recordings
  → `environment/ocean-surf.wav`.
- isaiah658, [Ambient Bird Sounds](https://opengameart.org/content/ambient-bird-sounds),
  `birds-isaiah658.ogg` → `environment/woodland-birds.wav`.
- Luke.RUSTLTD, [wind1](https://opengameart.org/content/wind1),
  `wind1.wav` → `environment/wind-ambience.wav`. This source is synthesized
  wind made with PureData, not a field recording.

Reproduction: `python3 tools/assets/prepare-biome-ambience.py /path/to/source-dir`.
`environment/biome-sources.json` records original file hashes and download URLs.
The runtime prepares quieter natural foliage beds and the existing restrained
desert/highland wind on the same loading worker. Nature recordings keep their
original pitch; wind retains its previous low-level filtered variations. Sparse
bird/crow/cave details use the shared positional-category mixer and cave reverb.
The sandworm warning adapts TERAMIC's original procedural thunder into a short
filtered ground rumble. Those adaptations are sound design, not new recordings.
The existing cave sample retains its CC0 credit below. No Minecraft or mod
sound files are included.

## Dry Stalker Breathing

`animals/stalker-airway.wav` is the quiet 7.48..8.33-second airway/tail excerpt
of Scorcher24's **animal_eating_breathing.wav**, **CC0 1.0**, from **Scorchers
Foley Sounds**, already credited below. No new external recording was downloaded.
The excerpt is mono 24000 Hz PCM16, filtered at 220..5600 Hz, normalized with
headroom and edge fades. Reproduce with
`bash tools/assets/prepare-stalker-breath.sh /path/to/animal_eating_breathing.wav`.
The runtime derives separate asymmetric inhale/exhale phases on the existing
loading worker. It does not add a tonal oscillator, bubbling, liquid recordings
or periodic tremolo to this breath. This is adapted recorded Foley, not a new
recording of a human actor performing Stalker breathing; final in-game listening
remains a subjective audio check.

## Chest Hinges And Latches (Phase 113)

Chest opening/closing Foley derives once on the existing loading worker from
the Ryding CC0 wooden-door hinge/latch takes and the Kenney CC0 wood contacts
credited below. The closing take adds a soft timber contact at 0.34 seconds;
both use short edge fades and bounded filtering in `creature_audio/chests.rs`.
These are contextual chest Foley, not newly recorded chest samples. Playback
shares the existing positional, volume-controlled 24-voice mixer. No new external
assets or licenses are introduced.

## Spider And Swimming Foley (Phase 111)

Spider contacts layer shortened Fantozzi stone contacts and TinyWorlds leaf
recordings credited below (CC0). Quiet idle rubbing also reuses those leaf
recordings. The large-spider hiss is deterministic, band-limited, envelope-shaped
noise generated in `creature_audio/spiders.rs`, not a sampled animal vocalization.
Four revised swimming strokes filter the existing CC0 swim-stroke takes and mix
a quiet late water tail from jcpmcdonald's recording. Short edge fades and bounded
levels avoid sharp digital clicks. All derivations run once on the existing audio
loading worker, with shared clips and the unchanged bounded positional mixer.
No new external recordings or licenses are introduced.

## Crusher Material Foley (Phase 108)

The manual crusher reuses `footsteps/detail/wood-3.wav`, `wood-4.wav`,
`wood-5.wav`, `stone-3.wav`, `stone-5.wav`, `gravel-2.wav`, `gravel-4.wav`
and `gravel-7.wav`. These are the existing CC0 Kenney, Fantozzi and Ali_6868
recordings credited below, not newly sourced machine recordings. The live
mix applies bounded pitch/envelope variations and brief synthesized metal
contacts. No additional external sound assets or licenses are introduced.

## Furniture Audio Polish (Phase 90)

`player/cough.wav`, `cough-2.wav` and `cough-3.wav` use three distinct natural
takes from the same CC0 Fabrizio84 recording credited below (starts 0, 1.08,
2.20 seconds; 0.94 seconds each). Mono 24000 Hz PCM16, 160..6500 Hz filtering,
-25 LUFS / -9 dBTP limits and short fades. Runtime avoids consecutive repeats,
uses reduced gain and spaces repeated exposure cues 8.5..12.5 seconds apart.
`objects/furnace-fire.wav` now normalizes the existing CC0 fireplace source to
-24 LUFS / -9 dBTP after filtering, rather than amplifying an already quiet
torch mix. Reproduce: `bash tools/assets/prepare90-furnace-polish-audio.sh`.
No Minecraft, Hytale or mod samples are used.

## Furniture And Smoke (Phase 89)

Retrieved 2026-10-20, **CC0 1.0**, adapted by TERAMIC:

- Ryding, **Wooden door open-close**:
  https://freesound.org/people/Ryding/sounds/125958/
  Public high-quality preview: https://cdn.freesound.org/previews/125/125958_981397-hq.mp3
  `objects/door-open.wav` and `objects/door-close.wav` use separate 0.82-second
  opening/closing windows. Windows reuse these hinge/latch recordings at lower
  gain and slightly higher pitch; these are contextual window Foley.
- Fabrizio84, **Cough**:
  https://freesound.org/people/Fabrizio84/sounds/457972/
  Public high-quality preview: https://cdn.freesound.org/previews/457/457972_2841496-hq.mp3
  `player/cough.wav` uses the first 0.95-second cough.

`objects/furnace-fire.wav` filters the existing credited CC0 torch-fire recording
below for a softer enclosed-fire bed. Mono 24000 Hz PCM16, high/low-pass,
bounded gain and edge fades; no game audio. Reproduce with
`bash tools/assets/prepare89-furniture-audio.sh`.

## Aquatic Creatures (Phase 78)

Squid jets, jellyfish bell pulses, shark fin strokes and squid ink releases use
four bounded adaptations of the existing `animals/swim.wav` recording by
**jcpmcdonald / Skippy Fish Water Sound Collection**, **CC0 1.0**, credited below.
The sample bank derives them once on its existing asynchronous worker; no new
decoder, per-creature audio entity or additional recording download is needed.
Exact windows, 90 Hz high-pass, gentle 900..2200 Hz low-pass and edge envelopes
are reproducible in `creature_audio/clips.rs::aquatic_foley`.

These are quiet contextual water-movement Foley, not biological squid,
jellyfish or shark vocalizations. Playback is body-listened and underwater-only,
with positional attenuation, bounded cadence and the existing 24-voice budget.
Surface splashes still use the shared physical water-contact system.

## Material Contacts (Phase 71)

The runtime now uses 51 recorded contact variants in nine material families.
Original Phase-57 files outside `footsteps/detail/` remain archived. The active
detail files are rebuilt directly from their sources, without the previous
FFT denoising/dynamic-normalization pass. Grass combines its recorded footfall
with a quiet, separately recorded leaf contact. Dirt and gravel have separate
banks; clay/mushrooms use earth, logs/planks/furniture use wood, foliage uses
leaves and glass uses quiet recorded glass taps rather than stone.

New sources retrieved 2026-10-19, all **CC0 1.0**:

- Kenney, **Impact Sounds (1.0)**, recorded Foley:
  https://kenney.nl/assets/impact-sounds
  Archive: https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip
  `footsteps/detail/grass{,-2,-3,-4,-5}.wav`: `Audio/footstep_grass_000..004.ogg`.
  `footsteps/detail/wood{,-2,-3,-4,-5}.wav`: `Audio/footstep_wood_000..004.ogg`.
  `footsteps/detail/glass{,-2,-3,-4,-5}.wav`: `Audio/impactGlass_light_000..004.ogg`.
  Glass contacts are contextual recorded glass Foley, not recordings of walking
  across a glass floor. The archive's `License.txt` confirms CC0.
- monte32, **Footsteps_6_Dirt_shoe.wav**, six recorded dirt/gravel shoe contacts,
  RME UCX / NT55:
  https://freesound.org/people/monte32/sounds/353799/
  Public high-quality preview used: https://cdn.freesound.org/previews/353/353799_5337613-hq.mp3
  `footsteps/detail/dirt{,-2,-3,-4,-5,-6}.wav`: 0.57-second source windows
  beginning at 0.30, 1.29, 2.25, 3.18, 4.15 and 5.12 seconds, then edge-trimmed.
  This is derived from the public MP3 preview, not claimed to be lossless audio.
- Ali_6868, **Gravel Footsteps**, ten left/right contacts on a gravel path:
  https://freesound.org/people/Ali_6868/packs/21608/
  Distributed as trimmed contacts by congusbongus in **Footsteps on different surfaces**:
  https://opengameart.org/content/footsteps-on-different-surfaces
  Archive: https://opengameart.org/sites/default/files/footsteps_0.zip
  `footsteps/detail/gravel{,-2,...,-10}.wav`: `footsteps/gravel/0..9.ogg`.
  Only the gravel subset is used. Its own `gravel/license.txt` identifies
  Ali_6868 and CC0; other subsets of this archive carry different licenses.
  Individual source confirmation:
  https://freesound.org/people/Ali_6868/sounds/384873/
  https://freesound.org/people/Ali_6868/sounds/384880/

Expanded existing CC0 sources credited under Phase 57 below:

- `footsteps/detail/sand{,-2,...,-6}.wav`: Fantozzi-SandL1/R1/L2/R2/L3/R3.flac.
- `footsteps/detail/stone{,-2,...,-6}.wav`: Fantozzi-StoneL1/R1/L2/R2/L3/R3.flac.
- `footsteps/detail/snow{,-2,...,-6}.wav`: Corsica_S Walking_on_snow_covered_gravel_01..06.flac.
- `footsteps/detail/leaves{,-2}.wav`: TinyWorlds/pdsounds.org leaves01/02.ogg.

Adaptation by Project TERAMIC: mono 24000 Hz PCM16, 55 Hz high-pass and 10500 Hz
low-pass, envelope-based removal of silence/noise tails, retained attack lead,
2/22-ms edge fades, bounded RMS normalization with a 0.72 peak ceiling.
No synthesized layers or generated noise are added to material contacts.
Runtime take selection avoids consecutive repeats and adds small bounded gain
and pitch variation; distance cadence, positional attenuation and cave reverb
remain active.

Reproduce with `bash tools/prepare-footstep-audio.sh /path/to/recordings`, using
the Phase-57 extracted directory layout, plus these new inputs:
`kenney-impact/Audio/` from the Kenney archive, `monte32-dirt-hq.mp3` from the
public preview URL, and `cdogs-footsteps/footsteps/gravel/` from the C-Dogs
archive. The script modifies material contacts only; environmental recordings
are unaffected. `tools/assets/prepare71-steps.mjs` contains exact source mappings
and processing. `prepare60-steps.sh` is historical and must not be run over the
Phase-71 bank.

## Phase 60 Adaptations

The former Phase-60 `footsteps/detail/` bank derived only from the footstep recordings
below. Conservative noise-floor reduction, sub-bass removal, bounded dynamic
normalization and short edge fades reduce hiss and harsh transients. Originals
are retained. Reproduce with `tools/assets/prepare60-steps.sh`; no added sample
rate or claims of newly recorded source detail.

`ui/click.wav` and `ui/mention.wav` are original TERAMIC synthesized UI cues,
generated by `tools/assets/build60-ui-audio.mjs` with smooth attack/release.
They are not field recordings or sounds extracted from another game.

These recordings are third-party assets with the licenses below. The repository's
software license does not replace their asset licenses. Sources were retrieved
2026-10-18. Included license texts accompany this directory.

## Farm Recordings: CC BY-SA 3.0

Secretlondon, recorded at Mudchute City Farm, London; distributed by qubodup:
https://opengameart.org/content/farm-animals
https://commons.wikimedia.org/wiki/User:Secretlondon
Source archive: https://opengameart.org/sites/default/files/mudchuteanimals.7z
License: https://creativecommons.org/licenses/by-sa/3.0/

- `animals/sheep-bleat.wav`: `Mudchute_sheep_1.ogg`
- `animals/cow-moo.wav`: `Mudchute_cow_1.ogg`
- `animals/pig-grunt.wav`: `Mudchute_pig_1.ogg`
- `animals/pig-grunt-2.wav`: `Mudchute_pig_2.ogg`

These four adapted recordings remain CC BY-SA 3.0. Changes: mono conversion,
24000 Hz PCM, trimming, 45 Hz high-pass, loudness adjustment and short fades.

## Additional Voices: CC0 1.0

Technopeasant's Baby Animals Sounds Pack, adapted from CC0 Freesound recordings:
https://opengameart.org/content/baby-animals-sounds-pack
Source archive: https://opengameart.org/sites/default/files/baby-animals_0.zip

- `animals/sheep-bleat-2.wav`: `Baa.ogg`
- `animals/cow-moo-2.wav`: `Moo.ogg`

The source page lists the original contributors and individual Freesound links:
AGFX, felix.blume, foxen10, Foleyhaven, lolamadeus, AntumDeluge, swiftoid,
jsbarrett. Converted/trimmed as above; CC0 1.0.

## Movement, Foliage And Feeding: CC0 1.0

Augmentality / Brandon Morris (submitted by HaelDB), Random Sounds Samples:
https://opengameart.org/content/random-sounds-samples

- `animals/step.wav`: `foot left stereo.ogg`, natural ground-footstep Foley.
- `environment/leaves.wav`: `moving leaves stereo.ogg`.

Iochi Glaucus, Fabric Rustling (CC0 1.0):
https://opengameart.org/content/fabric-rustling
- `animals/sheep-shake.wav`: `rustling.ogg`, fabric/wool-like Foley; leading
  silence removed and a short rustle selected.

Scorcher24, Scorchers Foley Sounds:
https://opengameart.org/content/scorchers-foley-sounds
- `animals/eat.wav`: excerpt of `animal_eating_breathing.wav`.

Converted to mono 24000 Hz PCM, trimmed, high-pass/loudness adjustment and fades.
The fabric rustling and footsteps are contextual Foley, not recordings of sheep hooves
or an actual sheep shaking.

## Water Movement: CC0 1.0

jcpmcdonald, Skippy Fish Water Sound Collection:
https://opengameart.org/content/skippy-fish-water-sound-collection

- `animals/swim.wav`, `animals/drink.wav`: edited excerpts of `swim.wav`, recorded
  water Foley. Fish use quiet water movement, not an invented fish vocalization.

Phase 78 derives four short aquatic movement cues from the same credited
`animals/swim.wav` recording on the existing audio-bank worker: squid jet,
jelly pulse, shark stroke and squid ink release. Excerpts are 0.50-0.72 seconds,
mono 24000 Hz, with a 90 Hz high-pass, 900-2200 Hz low-pass, 60 ms attack and
140 ms release. Processing is in `apps/client/src/creature_audio/clips.rs`.
These are contextual underwater Foley, not recordings of animal voices.

## Outdoor Wind: CC BY 4.0

Lanea Zimmerman / Sharm, Breeze:
https://opengameart.org/content/breeze
Original: https://opengameart.org/sites/default/files/wind_0.ogg
License: https://creativecommons.org/licenses/by/4.0/

- `environment/wind.wav`: mono 24000 Hz PCM adaptation, trimmed, high-pass,
  loudness adjustment and fades. Adapted by Project TERAMIC.

## Cave Ambience: CC0 1.0

JaggedStone, Loopable Dungeon Ambience:
https://opengameart.org/content/loopable-dungeon-ambience
Original: https://opengameart.org/sites/default/files/dungeon_ambient_1_0.ogg

- `environment/cave.wav`: 45-second mono 24000 Hz PCM adaptation with loudness
  adjustment, high-pass and fades. Original cave/wind/dripping sound design.

No Minecraft or mod audio is included. The runtime crossfades environmental loop
boundaries and applies body-position attenuation and underwater filtering.

## Biome Soundscape And Material Steps: CC0 1.0 (Phase 57)

The previous wind recording remains archived. The moving-leaf recording was
reused for the natural biome mix on 18 November 2026. The following recordings are not
extracted from Minecraft, Hytale or any ambience mod.

- Thimras, Park Ambiences:
  https://opengameart.org/content/park-ambiences
  Original: https://opengameart.org/sites/default/files/park_ambience_birds.wav
  `environment/birds.wav`: seconds 5..35 of the bird-focused park field recording.
- syncopika, Cricket Sounds:
  https://opengameart.org/content/cricket-sounds
  Original: https://opengameart.org/sites/default/files/cricketsounds090613_0.mp3
  `environment/crickets.wav`: seconds 2..32 of the backyard field recording.
- Fantozzi, sliced by qubodup, Footsteps (Grass/Sand & Stone):
  https://opengameart.org/content/fantozzis-footsteps-grasssand-stone
  Original: https://opengameart.org/sites/default/files/Fantozzi-footsteps.7z
  `footsteps/sand{,-2}.wav`: Fantozzi-SandL1/R1.flac;
  `footsteps/stone{,-2}.wav`: Fantozzi-StoneL1/R1.flac.
- Corsica_S, sliced by Iwan Gabovitch/qubodup, 42 Snow And Gravel Footsteps:
  https://opengameart.org/content/42-snow-and-gravel-footsteps
  Original: https://opengameart.org/sites/default/files/corsica_s-walking_in_snow.7z
  `footsteps/snow{,-2}.wav`: Walking_on_snow_covered_gravel_01/02.flac.
  The source page records permission for the extracted sounds to be CC0.
- TinyWorlds, adapted from pdsounds.org, Different Steps:
  https://opengameart.org/content/different-steps-on-wood-stone-leaves-gravel-and-mud
  Original: https://opengameart.org/sites/default/files/%5Bkdd%5DDifferentSteps_0.zip
  `footsteps/grass{,-2}.wav`: leaves01/02.ogg, grass/leaf ground Foley;
  `footsteps/dirt{,-2}.wav`: mud02.ogg / gravel.ogg, mixed earth/grit variants;
  `footsteps/wood{,-2}.wav`: wood01/02.ogg.

All fourteen additions are CC0 1.0. Processing: trimmed excerpts, mono 24000 Hz
PCM16, high-pass filtering, loudness normalization and short fades. The exact
reprocessing script is `tools/prepare-footstep-audio.sh`. Grass and dirt sounds
are contextual foliage/earth Foley, not new biological animal recordings.
Runtime cave reverb is a bounded damped-delay approximation, not a sampled mod
impulse response or a physically simulated room.

## Weather Rain (Phase 72)

`environment/rain.wav` uses `1.ogg` from **Rain (loopable)** by **Ylmir**, CC0 1.0:
https://opengameart.org/content/rain-loopable
Original archive: https://opengameart.org/sites/default/files/Rain%20OGG.zip
Retrieved 2026-10-19. Mono 24000 Hz PCM16, 100 Hz high-pass and 9000 Hz low-pass;
runtime crossfade looping, smoothed intensity, roof filtering, underground
attenuation and the existing underwater low-pass. Playback uses the player's
body, not the third-person camera. Reproduce with
`bash tools/assets/prepare72-rain.sh /path/to/Rain-OGG.zip`.
The recording plays through the existing environmental mixer, not a sound
entity per raindrop. Snowfall itself is intentionally quiet.

## Rain Impacts, Wet Contacts And Swimming (Phase 76)

The same **Ylmir / Rain (loopable), CC0 1.0** collection above supplies
`environment/rain.wav` (`1.ogg`, 27 seconds) and the new
`environment/rain-detail.wav` (`3.ogg`, 45 seconds). Reprocessing uses gentle
compression, high/low-pass filtering and peak headroom instead of synthesized
noise. The existing mixer combines a shared rain bed with independently offset
left/right detail, smoothly filtered by shelter and submersion. Loaded surface
probes at the **player body** place the sound at nearby rain impacts; flying
far above surfaces is silent. Snow itself does not make rain noise.

`animals/swim-stroke{,-2,-3,-4}.wav` are four overlapping 0.78-second windows
at 0.03, 0.34, 0.69 and 1.02 seconds from **jcpmcdonald / Skippy Fish Water
Sound Collection**, `swim.wav`, CC0 1.0, credited above. These are adapted takes
from the existing recording, not newly captured swimmer recordings.
`footsteps/wet-film-{1,2}.wav` use 0.34-second windows at 0.31 and 1.06 seconds
from the same source. They are quiet contextual water-film Foley layered over
the proper material contact only while rain falls on an already wet block.
Material identity and the existing cave/underwater filters are retained.
No game/mod audio or added synthesized noise is used for these additions.

Reproduce with
`bash tools/assets/prepare76-water-audio.sh /path/to/Rain-OGG.zip /path/to/original/swim.wav`.
Exact filters and fades are in that script. Runtime source/take variation is
bounded and the existing 24-voice limit is unchanged.

## TERAMIC Bee Foley (Phase 58)

`animals/bee-flight.wav` and `animals/bee-land.wav` are original procedural
wing/landing Foley generated by `tools/assets/build_bee_audio.mjs` (mono 24000 Hz
PCM16). They are synthesized, not claimed to be field recordings. No Minecraft,
Hytale or mod samples are used. These two generated files use the project's
MIT OR Apache-2.0 license. Landing, breaking and placing blocks reuse the
credited material contact recordings above with different gain/pitch envelopes.

## Water Contacts And Nest Foley (Phase 59)

`animals/splash.wav` and `animals/swim-stroke.wav` are further edited excerpts
of jcpmcdonald's credited CC0 swim recording above, not new field recordings.
`environment/birds-soft.wav` filters the existing credited CC0 park recording
(350 Hz high-pass, 6500 Hz low-pass, peak limiting); the original is retained.
Reproduction: `bash tools/assets/prepare59-audio.sh`.
`animals/bee-enter.wav` and `animals/bee-leave.wav` are original synthesized
pitch/envelope transitions from the existing bee generator, MIT OR Apache-2.0.
Runtime nest humming reuses the wing cue at reduced gain, only while occupied.

## Torch Fire And Sleeping Animals (Phase 62)

`environment/torch-fire.wav` replaces the Phase-61 synthesized clip with a
10-second excerpt of **Fireplace Sound loop**, PagDev, **CC0**:
https://opengameart.org/content/fireplace-sound-loop
Original: https://opengameart.org/sites/default/files/fire.wav
Retrieved 2026-10-18. Mono 24000 Hz PCM16; high/low-pass filtering, gentle
compression, -27 LUFS normalization, -9 dBTP ceiling and 200-ms fades.
Reproduce with `node tools/assets/build62-audio.mjs /path/to/original/fire.wav`.
The original 10.3 MB recording is not required at runtime.

`animals/sleep-breath.wav` is original non-vocal airflow Foley synthesized
by the same script (MIT OR Apache-2.0), not an animal field recording.

## Thunder (Phase 74)

The eight-second thunder clip is original procedural crack/rolling-noise Foley
generated once by `apps/client/src/creature_audio/thunder.rs` on the existing
sample-bank worker (mono 24000 Hz). It is synthesized, not a field recording,
and contains no Minecraft, Hytale or mod samples. MIT OR Apache-2.0.
Playback uses the listener body, distance delay, existing ambience gain and
underwater filtering; no audio entity per lightning segment.
