# Sound packs

Cosmetic start/stop pairs for TTP (see `docs/ttp-pro-design.md`, "1. A voice --
sound packs"). Every file here is synthesised from scratch -- no samples, no
downloads -- by `scripts/synth_sounds.py`.

| Pack | Character |
| --- | --- |
| `radio` | 1970s walkie-talkie. Start: band-limited squelch hiss resolving into a relay click. Stop: the click first, squelch tail dying away behind it, pitched duller. |
| `arcade` | 8-bit. Start: rising two-tone square blip on the coin interval (B5 to E6), 5-bit crushed. Stop: the same interval falling, and shorter. |
| `submarine` | Sonar. Start: a 660 Hz sine ping with a long exponential decay and a downward pitch bend that lands inside the first 50 ms. Stop: the same ping a perfect fourth lower. |
| `typewriter` | Start: a mechanical key click over a small inharmonic bell. Stop: the carriage-return ding alone, pitched down, left to ring. |
| `bubble` | Water. Start: a sine with a fast upward pitch sweep and a soft attack. Stop: the sweep downward. |

## Format

44100 Hz, 16-bit signed, mono -- identical to the built-in `../start.wav` and
`../stop.wav`. Every sound is under 400 ms, begins and ends at exactly zero
amplitude behind a raised-cosine fade, and is loudness-matched to the built-in
beeps at -14.8 dBFS RMS under a -3 dBFS peak ceiling, so switching packs is
never a volume jump.

## Regenerating

```sh
python3 scripts/synth_sounds.py            # regenerate all ten files, then verify
python3 scripts/synth_sounds.py --verify   # verify the existing files only
```

Standard library only, and deterministic: the noise generators are explicitly
seeded, so a re-run produces byte-identical files. Do not hand-edit the WAVs --
change the synthesis and re-run.
