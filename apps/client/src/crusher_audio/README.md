# Crusher Material Audio

The machine keeps one stereo stream, up to twelve fixed sample voices and a
single immutable eight-clip bank. Startup decoding is asynchronous; no disk
access, allocations, sound entities or unbounded queues occur on the audio
thread. Distance, facing-aware panning, mute and ambience/master gain remain
controlled by acknowledged nearby Crusher states.

The new sound layers pitch/envelope/filter existing recordings with irregular
intervals. Empty cranking uses wood/bearing sounds and short synthesized damped
metal contacts. Active crushing additionally layers stone impacts and separate
gravel fragments. These are contextual Foley, not a recording of a real crusher.
The old steady pitched noise/ratchet bed is no longer the main sound.

Existing CC0 recordings and original attribution are documented in
`assets/audio/CREDITS.md`:

- `footsteps/detail/wood-{3,4,5}.wav`: Kenney, *Impact Sounds*.
- `footsteps/detail/stone-{3,5}.wav`: Fantozzi, *Fantozzi's Footsteps*.
- `footsteps/detail/gravel-{2,4,7}.wav`: Ali_6868, *Gravel Footsteps*.

No new external recordings or dependencies were added.

Generate the reproducible six-second stereo listening fixture explicitly:

```sh
cargo test --locked -p terra-client crusher_audio::source::tests::write_crusher_listening_preview -- --ignored --nocapture
```

Output: `target/phase108/crusher-mix.wav`, PCM16/48000Hz/stereo. Set
`TERRA_CRUSHER_AUDIO_PREVIEW` to override the destination. The first two seconds
are empty cranking, seconds two through five are mineral crushing, and the last
second verifies the finite mute release. A generated preview does not establish
human listening approval or native spatial playback by itself.
