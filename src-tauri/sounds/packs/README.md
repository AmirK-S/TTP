# Sound packs

Cosmetic start/stop pairs for TTP (see `docs/ttp-pro-design.md`, "1. A voice --
sound packs"). Every file here is synthesised from scratch -- no samples, no
downloads -- by `scripts/synth_sounds.py`.

| Pack | Character |
| --- | --- |
| `bowl` | A small brass bowl, struck with something soft. Start: a slow bloom that rings out, breathing once as its split fundamental beats against itself. Stop: the same bowl, same pitch, with a hand laid on it at 84 ms. |
| `marimba` | Rosewood and a yarn mallet. Start: two notes rising a major third, the second struck softer so the figure settles. Stop: the same two, falling. |
| `submarine` | Sonar. Start: a 660 Hz sine ping with a long exponential decay and a downward pitch bend that lands inside the first 50 ms. Stop: the same ping a perfect fourth lower. |
| `felt` | A piano with cloth over the strings. Start: a root, its fifth and its octave struck together, soft enough that the hammer knock is the only hard thing in it. Stop: the same voicing a whole tone down and shorter. |
| `bubble` | Water. Start: a sine with a fast upward pitch sweep and a soft attack. Stop: the sweep downward. |

## Format

44100 Hz, 16-bit signed, mono -- identical to the built-in `../start.wav` and
`../stop.wav`. Every sound is under 400 ms, begins and ends at exactly zero
amplitude behind a raised-cosine fade, and is loudness-matched to the built-in
beeps at -14.8 dBFS RMS under a -3 dBFS peak ceiling, so switching packs is
never a volume jump.

## Warm, not bright

These fire every time someone starts and stops talking, dozens of times a day,
out of a laptop speaker a foot from their face. The test a pack has to pass is
not "is this characterful" but "would I still like this on the fiftieth hearing
today", and the two are not the same test: a noise burst is charming the first
time and abrasive the fiftieth.

So the packs are pitched material with soft attacks and natural decays, and
nothing here has a noise source in it at all. Every one of the five is either a
set of decaying sinusoids at fixed ratios to a fundamental or a swept sine.
Timbre carries the identity of a pack; the transient is only how the timbre
arrives.

That is enforced, not just intended. Measured over the first 50 ms, every sound
has to come in under a 2 kHz spectral centroid and a 0.05 spectral flatness --
the first says warm rather than bright, the second says pitched rather than
noisy. For scale, the three packs these replaced measured 2740-4023 Hz and up to
0.269; the five here measure 324-1335 Hz and at most 0.002.

## Telling them apart

A pack also has to be recognisable inside its first 50 ms, which is a real
constraint rather than a nicety: warm pitched material all wants to live in the
same 300-900 Hz window, and two packs that land on the same centroid there sound
like the same sound. `--verify` checks that no two packs sit within 2 semitones
of each other on that measurement. The closest pair in the current set is 2.5
semitones apart, and the pitches were chosen to make that true -- `bowl` is
tuned to 360 Hz, which is not a note, because that is the middle of the only gap
left between submarine's two ends.

## Regenerating

```sh
python3 scripts/synth_sounds.py            # regenerate all ten files, then verify
python3 scripts/synth_sounds.py --verify   # verify the existing files only
```

Standard library only, and deterministic: there is no random number generator
involved anywhere, so a re-run produces byte-identical files. Do not hand-edit
the WAVs -- change the synthesis and re-run.
