#!/usr/bin/env python3
"""
Synthesise TTP's cosmetic sound packs from scratch.

Everything here is generated with the Python standard library only (`wave`,
`math`, `cmath`, `struct`, `array`). No samples, no downloads, no numpy
dependency -- if numpy happens to be installed it is not used, so the output
bytes are identical on every machine. There is no random number generator
involved anywhere any more either (see "why no noise" below), so determinism
is now structural rather than a matter of remembering to seed things.

Writes, for each pack:
    src-tauri/sounds/packs/<pack>/start.wav
    src-tauri/sounds/packs/<pack>/stop.wav

and then verifies every file it wrote (format, duration, peak, clipping,
boundary samples, first-50ms timbre) and prints a loudness and brightness
table.

    python3 scripts/synth_sounds.py            # generate + verify
    python3 scripts/synth_sounds.py --verify   # verify existing files only

Design constraints (see docs/ttp-pro-design.md, "1. A voice -- sound packs"):

  * 44100 Hz, 16-bit signed, mono -- same as the built-in start.wav/stop.wav.
  * Under 400 ms. These fire on every dictation; length is the enemy.
  * First and last sample are exactly zero, with a raised-cosine fade at both
    edges, so no pack ever ticks on playback.
  * Loudness-matched to the built-in beeps at -14.8 dBFS RMS, under a -3.0
    dBFS peak ceiling.
  * Warm and tonal: a first-50ms spectral centroid under 2 kHz and a spectral
    flatness under 0.05, and no two packs within 2 semitones of each other on
    that centroid.

Why loudness and not peak is the normalisation target: the built-in beeps peak
at -10.5 dBFS, not -3. Peak-normalising every file to -3 dBFS would spread the
RMS across packs by more than 10 dB, because a sound's peak-to-average ratio
depends entirely on how percussive it is -- which is exactly the volume jump on
pack switch that we are trying to avoid. So the chain peak-normalises to the
-3 dBFS ceiling first (that is the never-clip guarantee) and then trims down to
the reference RMS. Attenuation only: nothing is ever pushed back up through the
ceiling.

For packs that come out quieter than the reference at the ceiling, gain is not
available -- it would breach -3 dBFS -- so the crest factor is reduced instead,
by searching for the mild tanh drive that lands the sound on target loudness
while still peaking at -3. That is a synthesis change, not a gain change.

Why no noise, anywhere: the first version of this file leaned on transients and
band-limited noise for character -- a squelch, a bit-crushed square, a type-bar
click. Measured, those three packs sat at 2.7-4.0 kHz centroid and up to 0.27
flatness, and they were the three that got rejected on listening. The failure is
structural, not a matter of taste: a noise burst is charming on first hearing and
abrasive on the fiftieth, and these fire dozens of times a day from a laptop
speaker a foot from someone's face. Everything here is now a small set of
decaying sinusoids at fixed ratios to a fundamental. Timbre carries the identity
of a pack; the transient is only how the timbre arrives.

Why the pitches are what they are: the packs have to be told apart inside the
first 50 ms, and warm pitched material all wants to live in the same 300-900 Hz
window, so the fundamentals are chosen against a measured map of the whole set
(including the two built-in beeps) rather than for musical reasons alone. The
`--verify` pass enforces the resulting spacing. The other constraint on pitch is
the speaker: nothing carries its identity much below 200 Hz on a laptop, so no
fundamental here goes under F3, and the pack that gets closest to that floor
(felt) is voiced in octaves so that the notes above the root carry the loudness.
"""

from __future__ import annotations

import argparse
import array
import cmath
import math
import os
import struct
import sys
import wave

SR = 44100                 # sample rate, matched to the built-in beeps
MAX_DURATION_S = 0.400     # hard cap, per pack sound
PEAK_CEILING_DBFS = -3.0   # no sound may ever peak above this
PEAK_FLOOR_DBFS = -16.0    # ...nor be so quiet that it is inaudible
FULL_SCALE = 32768.0
BOUNDARY_MAX = 2           # |first| and |last| sample must be <= this (of 32768)

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACKS_DIR = os.path.join(REPO_ROOT, "src-tauri", "sounds", "packs")

# The built-in beeps, measured (100 ms sines, 880/440 Hz, peak -10.46 dBFS).
# Their RMS is the loudness every pack is matched to.
REFERENCE_RMS_DBFS = -14.8
RMS_TOLERANCE_DB = 0.75    # how far a pack may sit from the reference

# Timbre budget, measured over the first 50 ms -- the window in which the user
# has to know which pack is playing, and the window that decides whether the
# sound is warm or bright. See "why no noise" in the module docstring; the three
# packs that were replaced measured 2740-4023 Hz and 0.128-0.269.
TIMBRE_WINDOW = 2048             # 46.4 ms at 44.1 kHz, the largest power of two
CENTROID_CEILING_HZ = 2000.0     # above this a sound reads as bright, not warm
FLATNESS_CEILING = 0.05          # above this it reads as noise, not as a note
MIN_PACK_SEPARATION_ST = 2.0     # semitones between any two packs' centroids


# --------------------------------------------------------------------------
# tiny DSP kit
# --------------------------------------------------------------------------

def n_samples(ms: float) -> int:
    """Milliseconds -> whole samples."""
    return int(round(ms * SR / 1000.0))


def silence(n: int) -> list:
    return [0.0] * n


def osc(freqs, n: int, shape: str = "sin", duty: float = 0.5, phase0: float = 0.0) -> list:
    """
    Phase-accumulating oscillator. `freqs` is a constant or a per-sample list,
    so sweeps and drifts cost nothing extra.
    """
    if not isinstance(freqs, (list, tuple)):
        freqs = [float(freqs)] * n
    out, phase = [], phase0
    for i in range(n):
        if shape == "sin":
            out.append(math.sin(2.0 * math.pi * phase))
        elif shape == "square":
            out.append(1.0 if (phase % 1.0) < duty else -1.0)
        elif shape == "tri":
            p = phase % 1.0
            out.append(4.0 * abs(p - 0.5) - 1.0)
        else:
            raise ValueError("unknown shape " + shape)
        phase += freqs[i] / SR
    return out


def exp_sweep(n: int, f0: float, f1: float) -> list:
    """Geometric frequency sweep -- pitch reads as linear to the ear."""
    if n <= 1:
        return [f0]
    r = math.log(f1 / f0)
    return [f0 * math.exp(r * i / (n - 1)) for i in range(n)]


def env_exp(n: int, attack_ms: float, tau_ms: float, tail_ms: float = 8.0) -> list:
    """Raised-cosine attack, exponential decay, forced to zero at the end."""
    a = max(1, n_samples(attack_ms))
    tau = max(1e-6, tau_ms / 1000.0)
    tail = max(1, n_samples(tail_ms))
    out = []
    for i in range(n):
        if i < a:
            e = 0.5 - 0.5 * math.cos(math.pi * i / a)
        else:
            e = math.exp(-(i - a) / SR / tau)
        if i >= n - tail:                      # glide the tail into silence
            k = (n - 1 - i) / float(tail)
            e *= 0.5 - 0.5 * math.cos(math.pi * k)
        out.append(e)
    return out


def env_ar(n: int, attack_ms: float, hold_ms: float, release_ms: float,
           curve: float = 2.0) -> list:
    """Attack / hold / power-curve release. Used where an exponential rings too long."""
    a = max(1, n_samples(attack_ms))
    h = max(0, n_samples(hold_ms))
    r = max(1, n - a - h)
    out = []
    for i in range(n):
        if i < a:
            e = 0.5 - 0.5 * math.cos(math.pi * i / a)
        elif i < a + h:
            e = 1.0
        else:
            k = min(1.0, (i - a - h) / float(r))
            e = (1.0 - k) ** curve
        out.append(e)
    return out


def bell(n: int, partials, start_amp: float = 1.0, attack_ms: float = 1.0) -> list:
    """
    Inharmonic struck-metal tone. `partials` is (freq, amp, tau_ms) triples --
    the non-integer frequency ratios are what make it a bell and not an organ.
    """
    out = silence(n)
    for freq, amp, tau in partials:
        tone = osc(freq, n, "sin")
        env = env_exp(n, attack_ms, tau, tail_ms=min(12.0, n / SR * 1000.0 / 3))
        for i in range(n):
            out[i] += start_amp * amp * tone[i] * env[i]
    return out


def mix(dest: list, src: list, offset_ms: float = 0.0, gain: float = 1.0) -> list:
    """Place an event on a timeline. Anything past the end is dropped."""
    off = n_samples(offset_ms)
    for i, v in enumerate(src):
        j = off + i
        if 0 <= j < len(dest):
            dest[j] += gain * v
    return dest


def apply_env(sig: list, env: list) -> list:
    return [s * e for s, e in zip(sig, env)]


def damp_env(n: int, hold_ms: float, mute_ms: float) -> list:
    """
    Full level, then a raised-cosine mute down to silence -- a hand laid on
    something that is still ringing. Not the same shape as a decay: a decay
    fades because the energy ran out, a mute stops because someone stopped it,
    and the ear hears the difference as intent.
    """
    h = max(1, min(n, n_samples(hold_ms)))
    m = max(1, min(n - h, n_samples(mute_ms))) if n > h else 1
    out = [1.0] * n
    for i in range(m):
        j = h + i
        if j >= n:
            break
        out[j] = 0.5 + 0.5 * math.cos(math.pi * i / m)
    for j in range(min(n, h + m), n):
        out[j] = 0.0
    return out


def peak_of(sig: list) -> float:
    return max((abs(s) for s in sig), default=0.0)


def rms_of(sig: list) -> float:
    if not sig:
        return 0.0
    return math.sqrt(sum(s * s for s in sig) / len(sig))


def db(x: float) -> float:
    return 20.0 * math.log10(x) if x > 0 else -999.0


def normalise(sig: list, target: float = 1.0) -> list:
    p = peak_of(sig)
    if p == 0.0:
        return list(sig)
    g = target / p
    return [s * g for s in sig]


def saturate(sig: list, drive: float) -> list:
    """
    Mild tanh saturation. Rounds off the sharpest transients, which lifts RMS
    relative to peak -- that is how a click-led pack ends up as loud as a
    sustained beep without either of them clipping.
    """
    if drive <= 0.0:
        return list(sig)
    k = math.tanh(drive)
    return [math.tanh(drive * s) / k for s in sig]


def edge_fade(sig: list, fade_in_ms: float = 3.0, fade_out_ms: float = 6.0) -> list:
    """
    Raised-cosine fade at both boundaries, with the first and last sample
    forced to exact zero. This is the anti-click guarantee, applied last so
    nothing downstream can undo it.
    """
    n = len(sig)
    fi = max(2, min(n_samples(fade_in_ms), n // 2))
    fo = max(2, min(n_samples(fade_out_ms), n // 2))
    out = list(sig)
    for i in range(fi):
        out[i] *= 0.5 - 0.5 * math.cos(math.pi * i / (fi - 1))
    for i in range(fo):
        j = n - 1 - i
        out[j] *= 0.5 - 0.5 * math.cos(math.pi * i / (fo - 1))
    out[0] = 0.0
    out[-1] = 0.0
    return out


def master(sig: list, max_drive: float = 2.4, fade_in_ms: float = 3.0,
           fade_out_ms: float = 6.0,
           target_rms_dbfs: float = REFERENCE_RMS_DBFS) -> list:
    """
    saturate -> fade edges -> peak-normalise to the ceiling -> loudness trim.

    The tanh drive is not a taste knob here, it is solved for: saturation
    raises RMS at a fixed peak, so if a sound is quieter than the reference
    when it is sitting on the -3 dBFS ceiling, the only way up is to reduce
    its crest factor. Bisect for the drive that lands on target; if even
    `max_drive` cannot reach it, stop there rather than distorting further and
    let the verifier report the miss. Anything louder than the reference is
    trimmed down linearly, which is why most packs peak below -3.
    """
    base = normalise(sig, 1.0)
    ceiling = 10.0 ** (PEAK_CEILING_DBFS / 20.0)

    def render(drive: float) -> list:
        s = saturate(base, drive)
        s = edge_fade(s, fade_in_ms, fade_out_ms)
        return normalise(s, ceiling)

    lo, hi = 0.0, max_drive
    if db(rms_of(render(hi))) <= target_rms_dbfs:
        out = render(hi)                       # as loud as we are willing to go
    elif db(rms_of(render(lo))) >= target_rms_dbfs:
        out = render(lo)                       # already loud enough clean
    else:
        for _ in range(28):                    # bisect on drive
            mid = 0.5 * (lo + hi)
            if db(rms_of(render(mid))) < target_rms_dbfs:
                lo = mid
            else:
                hi = mid
        out = render(0.5 * (lo + hi))

    trim = min(1.0, 10.0 ** ((target_rms_dbfs - db(rms_of(out))) / 20.0))
    out = [s * trim for s in out]
    out[0] = 0.0
    out[-1] = 0.0
    return out


def to_int16(sig: list) -> array.array:
    out = array.array("h")
    for s in sig:
        v = int(round(s * 32767.0))
        out.append(max(-32767, min(32767, v)))
    return out


def write_wav(path: str, sig: list) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    data = to_int16(sig)
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(struct.pack("<%dh" % len(data), *data))


# --------------------------------------------------------------------------
# the struck-resonator kit
#
# Three of the five packs are built entirely from this one function: a handful
# of decaying sinusoids at fixed ratios to a fundamental. Which ratios, and how
# fast each one dies, is the whole difference between a bowl, a bar and a
# string. Nothing here has a noise source or a hard transient in it.
# --------------------------------------------------------------------------

def struck(n: int, f0: float, modes, attack_ms: float, amp: float = 1.0) -> list:
    """A struck resonator. `modes` is (ratio, amp, tau_ms) relative to `f0`."""
    return bell(n, [(f0 * r, a, tau) for r, a, tau in modes],
                start_amp=amp, attack_ms=attack_ms)


# --------------------------------------------------------------------------
# pack 1 -- bowl: a small brass bowl, struck with something soft
# --------------------------------------------------------------------------

# Modes, not harmonics. The 2.68 ratio is what keeps this from reading as an
# organ, and the reason the fundamental gets more than half the energy is that
# a bowl's identity is its slow warble, not its shimmer -- so the modes above
# 3f are present enough to be heard on the strike and gone within 100 ms.
#
# The fundamental is a doublet. A real bowl is never perfectly circular, so its
# modes split into close pairs that beat against their twins, and that slow
# breathing is the whole character of the pack for the cost of one extra
# sinusoid. The pair is deliberately lopsided, 0.66 against 0.32: at equal
# amplitudes the two cancel almost completely at the trough and the sound
# pulses rather than breathes, which is the sort of thing nobody notices once
# and everybody notices fifty times a day. 0.7 % apart is a 2.5 Hz beat, so
# roughly one slow swell across the length of the sound.
#
# The 520 ms time constant is longer than the sound is, which is not an
# oversight. A bowl that decays away inside its own 380 ms has a high crest
# factor, and `master` answers a high crest factor with tanh drive, and tanh
# drive adds harmonics -- which moved the start's measured centroid 35 Hz above
# the stop's and pushed the pair out of the narrow gap it has to fit into. A
# bowl that barely decays needs no drive at all, so both ends of the pair
# measure identically and the release is done by the fade instead.
BOWL_MODES = [
    (1.000, 0.66, 520.0),
    (1.007, 0.32, 520.0),
    (2.680, 0.30, 210.0),
    (2.694, 0.25, 210.0),
    (4.120, 0.07,  95.0),
    (5.150, 0.05,  60.0),
]


def pack_bowl():
    # 360 Hz, which is not a note -- about a quarter tone under F#4. This pack
    # has the least room of the five: it lives in the 4.95 semitones between
    # submarine's stop (552 Hz) and its start (739 Hz), and after 2 semitones of
    # clearance either side that leaves a window barely a semitone wide. 360 Hz
    # puts the measured centroid at 638, as near the middle of that window as it
    # can be placed: 2.50 semitones above one neighbour, 2.55 below the other.
    # Nobody tunes a bowl to concert pitch anyway.
    f = 360.0

    # START -- struck once, left to bloom, then released over the last 60 ms.
    start = struck(n_samples(380.0), f, BOWL_MODES, attack_ms=9.0)

    # STOP -- the same bowl at the same pitch, with a hand laid on it.
    #
    # The closing gesture is the damping, not a pitch drop. There is nowhere to
    # drop to: every slot below F#4 is taken by submarine's stop, bubble's start
    # or felt, so transposing down would collide with another pack inside the
    # first 50 ms and buy nothing. Damping is the better gesture anyway -- a
    # long ring means the machine is listening and a short one means it has
    # stopped, which is exactly the information the sound is carrying.
    n2 = n_samples(190.0)
    stop = struck(n2, f, BOWL_MODES, attack_ms=9.0)
    stop = apply_env(stop, damp_env(n2, hold_ms=84.0, mute_ms=80.0))

    return master(start, fade_out_ms=60.0), master(stop, fade_out_ms=8.0)


# --------------------------------------------------------------------------
# pack 2 -- marimba: a rosewood bar and a yarn mallet
# --------------------------------------------------------------------------

# A marimba bar is undercut on its underside precisely to pull its second mode
# onto 4f and its third onto 10f. Those two are what say "wood" rather than
# "metal", and both are gone inside 40 ms -- which is why the note is bright on
# the strike and warm for the rest of its life. The 0.63 ratio is not a mode at
# all, it is the mallet's knock against the bar: inharmonic on purpose, so it
# does not fuse with the fundamental, and over in 10 ms.
MARIMBA_BAR = [
    (1.000, 1.000, 185.0),
    (3.930, 0.260,  38.0),
    (9.550, 0.055,  13.0),
    (0.630, 0.160,  10.0),
]


def marimba_note(f0: float, dur_ms: float, amp: float = 1.0) -> list:
    return struck(n_samples(dur_ms), f0, MARIMBA_BAR, attack_ms=2.2, amp=amp)


def pack_marimba():
    # E5 and G#5, a major third. Higher than a marimba's warmest register, and
    # chosen for spacing rather than for tone: everything below this is taken,
    # and the nearest neighbour underneath is bubble's stop at 878 Hz. E5 puts
    # the strike 3.35 semitones clear of it. The measured centroid still lands
    # at 1061 Hz -- less than half of what the 8-bit blip this replaces measured
    # -- because the bar's bright modes are over inside 40 ms.
    e5, gs5 = 659.25, 830.61
    total = n_samples(378.0)
    # 4 ms of lead-in silence, as the old arcade blip had: the master fade-in
    # then shapes silence instead of softening the mallet strike.
    lead, first_ms, second_ms = 4.0, 374.0, 256.0

    # START -- two notes, rising. STOP -- the same two, falling. The second
    # note is struck at 0.68 in both, which is what it takes for the figure to
    # settle rather than arrive twice: by 122 ms the first note has decayed far
    # enough that an evenly-struck second one measures as the loudest moment of
    # the sound, and a two-note figure whose accent is on the second note reads
    # as a question rather than an answer.
    start = silence(total)
    mix(start, marimba_note(e5, first_ms), lead)
    mix(start, marimba_note(gs5, second_ms, amp=0.68), 122.0)

    stop = silence(total)
    mix(stop, marimba_note(gs5, first_ms), lead)
    mix(stop, marimba_note(e5, second_ms, amp=0.68), 122.0)

    return master(start, fade_out_ms=10.0), master(stop, fade_out_ms=10.0)


# --------------------------------------------------------------------------
# pack 3 -- submarine: sonar
# --------------------------------------------------------------------------

def sonar_ping(f0: float, dur_ms: float, tau_ms: float, drift: float = 0.88,
               drift_tau_ms: float = 45.0) -> list:
    """
    Sine ping, long exponential decay, downward pitch drift.

    The drift is front-loaded on its own time constant rather than spread
    evenly across the ping: a linear-in-log sweep puts only a fraction of the
    bend inside the first 50 ms, which is where the sound has to announce
    itself as sonar and not as a plain beep. Here most of the bend has
    happened by ~50 ms and the pitch then settles.
    """
    n = n_samples(dur_ms)
    f_end = f0 * drift
    dtau = max(1e-6, drift_tau_ms / 1000.0)
    freqs = [f_end + (f0 - f_end) * math.exp(-(i / SR) / dtau) for i in range(n)]
    env = env_exp(n, 4.5, tau_ms, tail_ms=45.0)
    fund = apply_env(osc(freqs, n, "sin"), env)
    h2 = apply_env(osc([f * 2 for f in freqs], n, "sin"),
                   env_exp(n, 4.0, tau_ms * 0.55, tail_ms=45.0))
    h3 = apply_env(osc([f * 3 for f in freqs], n, "sin"),
                   env_exp(n, 3.5, tau_ms * 0.35, tail_ms=45.0))
    return [a + 0.11 * b + 0.045 * c for a, b, c in zip(fund, h2, h3)]


def pack_submarine():
    # 660 Hz. The pack has to clear BOTH built-in beeps, and because the stop
    # is a fourth below the start while the defaults are an octave apart
    # (880/440), moving the start can just relocate the collision onto the
    # stop. Measured first-50ms centroid separation, in semitones:
    #
    #     f0    vs 880 start   vs 440 stop   worst
    #     605       5.1            1.8        1.8   <- stop nearly on the default
    #     660       3.6            3.3        3.3   <- balanced, the maximum
    #     762       1.0            5.7        1.0   <- start nearly on the default
    #
    # 660 maximises the worst case. The original 762 measured an 884 Hz
    # centroid against the default start's 887 and read as "the normal beep,
    # but longer" -- the one failure that makes the whole set feel thin.
    f = 660.0
    start = sonar_ping(f, 340.0, 104.0)
    stop = sonar_ping(f * 3.0 / 4.0, 300.0, 98.0)    # a perfect fourth lower
    return master(start, fade_out_ms=10.0), master(stop, fade_out_ms=10.0)


# --------------------------------------------------------------------------
# pack 4 -- felt: a piano with cloth over the strings
# --------------------------------------------------------------------------

# The darkest thing in the set, and the point of it. A felted hammer barely
# excites anything above the third harmonic, so the partial amplitudes fall off
# a cliff -- which is what makes this readable as a piano heard through a wall
# rather than as a sine. The ratios are slightly sharp of the integers because
# real strings are stiff, not ideal: that inharmonicity is small enough to be
# inaudible as pitch and large enough to stop the partials phase-locking into
# something that sounds synthetic.
#
# The 1.41 entry is the hammer's knock. Nine milliseconds, inharmonic so it
# cannot fuse with the note, and without it the whole thing reads as an organ.
FELT_STRINGS = [
    (1.000, 1.000, 300.0),
    (2.002, 0.260, 195.0),
    (3.008, 0.075, 125.0),
    (4.020, 0.030,  85.0),
    (1.410, 0.100,   9.0),
]


def felt_chord(notes, dur_ms: float, attack_ms: float = 15.0) -> list:
    """
    Several felt notes struck together. The 15 ms attack is the pack: a hard
    attack on this spectrum sounds like a broken sine, and a soft one sounds
    like something with hammers in it.
    """
    n = n_samples(dur_ms)
    out = silence(n)
    for f0, amp in notes:
        mix(out, struck(n, f0, FELT_STRINGS, attack_ms=attack_ms, amp=amp))
    return out


def pack_felt():
    # A root, its fifth and its octave -- an open voicing, no third, so the
    # chord has no mood for anyone to get tired of.
    #
    # The upper two are weighted much heavier than they would be on a real
    # piano (0.55 and 0.42 against the root's 1.0) for the speaker rather than
    # for the ear. Measured through a 2-pole 250 Hz highpass, which is roughly
    # what a laptop speaker does to a signal, this pack loses 6 dB where every
    # other pack loses one to three -- so although all ten files sit at exactly
    # -14.8 dBFS RMS, felt comes out of a laptop around 3 dB under its
    # neighbours. Weighting the octave up recovers most of one of those dB.
    #
    # It does not recover the rest, and it cannot: loudness through a small
    # speaker *is* energy above 250 Hz, and spectral centroid *is* where the
    # energy sits, so the two constraints are the same quantity pulling in
    # opposite directions. Everything that makes this pack more audible on a
    # laptop makes it measure brighter, and the ceiling on brightness here is
    # hard -- 378 Hz, two semitones under bubble's start.
    #
    # The remaining 3 dB is accepted rather than fixed, for two reasons. It is
    # a difference, not a jump: the thing the loudness matching exists to
    # prevent was a 13 dB step on pack switch, and the two built-in beeps
    # already differ by 1.8 dB through the same filter. And the low slot has to
    # be occupied by something -- it is the only gap left in the map wide
    # enough for a start/stop pair -- so the question is only which pack should
    # take the hit. A marimba that came out 3 dB quiet would sound broken. A
    # piano with cloth over the strings sounds like a piano with cloth over the
    # strings.
    #
    # STOP is the same voicing a whole tone down and 50 ms shorter: the oldest
    # closing gesture there is. The drop puts the pair 2 semitones apart on
    # centroid, so the two never read as the same event fired twice.
    start = felt_chord([(196.00, 1.00), (293.66, 0.55), (392.00, 0.42)], 390.0)
    stop = felt_chord([(174.61, 1.00), (261.63, 0.55), (349.23, 0.42)], 340.0)
    return master(start, fade_out_ms=10.0), master(stop, fade_out_ms=10.0)


# --------------------------------------------------------------------------
# pack 5 -- bubble: water
# --------------------------------------------------------------------------

def bubble_tone(f0: float, f1: float, dur_ms: float, attack_ms: float) -> list:
    """Sine with a fast pitch sweep that settles, plus a soft attack."""
    n = n_samples(dur_ms)
    sweep_n = int(n * 0.62)
    freqs = exp_sweep(sweep_n, f0, f1)
    # after the sweep, drift the last 4% so it settles instead of stopping dead
    freqs += exp_sweep(n - sweep_n, f1, f1 * 1.04)
    fund = osc(freqs, n, "sin")
    h2 = osc([f * 2 for f in freqs], n, "sin")
    env = env_ar(n, attack_ms, dur_ms * 0.16, dur_ms, curve=1.7)
    sig = [(a + 0.13 * b) for a, b in zip(fund, h2)]
    return apply_env(sig, env)


def pack_bubble():
    start = bubble_tone(305.0, 935.0, 235.0, attack_ms=26.0)   # rising: opening
    stop = bubble_tone(935.0, 305.0, 215.0, attack_ms=22.0)    # falling: closing
    return master(start, fade_in_ms=4.0), master(stop, fade_in_ms=4.0)


PACKS = {
    "bowl": pack_bowl,
    "marimba": pack_marimba,
    "submarine": pack_submarine,
    "felt": pack_felt,
    "bubble": pack_bubble,
}


# --------------------------------------------------------------------------
# verification
# --------------------------------------------------------------------------

class CheckFailure(Exception):
    pass


def fft(x: list) -> list:
    """
    Iterative radix-2 Cooley-Tukey, in-place, `cmath` only. Length must be a
    power of two. This exists so the timbre checks below do not need numpy: at
    2048 points it is about eleven thousand butterflies, which is nothing.
    """
    n = len(x)
    if n & (n - 1):
        raise ValueError("fft length must be a power of two, got %d" % n)
    x = list(x)
    j = 0                                      # bit-reversal permutation
    for i in range(1, n):
        bit = n >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j |= bit
        if i < j:
            x[i], x[j] = x[j], x[i]
    size = 2
    while size <= n:
        step = cmath.exp(-2j * math.pi / size)
        half = size // 2
        for i in range(0, n, size):
            w = 1 + 0j
            for k in range(half):
                u, v = x[i + k], x[i + k + half] * w
                x[i + k], x[i + k + half] = u + v, u - v
                w *= step
        size <<= 1
    return x


def timbre_of(samples, rate: int = SR) -> tuple:
    """
    Spectral centroid and flatness over the first 50 ms -- the window in which
    a pack has to announce which pack it is, and the window that decides
    whether it is warm or bright.

    Centroid is the brightness: the amplitude-weighted mean frequency, in Hz,
    restricted to 40 Hz - 16 kHz so that DC offset and inaudible top end cannot
    move it. Flatness is the tonality: the ratio of the geometric to the
    arithmetic mean of the magnitude spectrum, near zero for a note with a few
    partials and approaching one for white noise. Both are computed through a
    Hann window, without which the rectangular edge of the frame smears energy
    across the whole spectrum and every sound measures noisy.
    """
    seg = [float(s) / FULL_SCALE for s in samples[:TIMBRE_WINDOW]]
    seg += [0.0] * (TIMBRE_WINDOW - len(seg))
    seg = [v * (0.5 - 0.5 * math.cos(2.0 * math.pi * i / (TIMBRE_WINDOW - 1)))
           for i, v in enumerate(seg)]
    spec = fft([complex(v, 0.0) for v in seg])

    mags = [abs(spec[k]) for k in range(TIMBRE_WINDOW // 2 + 1)]
    freqs = [k * rate / float(TIMBRE_WINDOW) for k in range(len(mags))]

    band = [(f, m) for f, m in zip(freqs, mags) if 40.0 <= f <= 16000.0]
    total = sum(m for _, m in band)
    centroid = sum(f * m for f, m in band) / total if total > 0 else 0.0

    eps = 1e-20
    geo = math.exp(sum(math.log(m + eps) for m in mags) / len(mags))
    arith = sum(mags) / len(mags)
    flatness = geo / arith if arith > 0 else 0.0

    return centroid, flatness


def semitones(f_a: float, f_b: float) -> float:
    """Absolute distance between two frequencies, in semitones."""
    if f_a <= 0 or f_b <= 0:
        return 0.0
    return abs(12.0 * math.log(f_a / f_b, 2.0))


def verify(path: str) -> dict:
    """
    Re-open a written file and check every property we promised. Returns the
    measured stats; raises CheckFailure on anything out of spec.
    """
    problems = []
    with wave.open(path, "rb") as w:
        p = w.getparams()
        raw = w.readframes(p.nframes)

    if p.nchannels != 1:
        problems.append("channels=%d, expected 1" % p.nchannels)
    if p.sampwidth != 2:
        problems.append("sampwidth=%d bytes, expected 2" % p.sampwidth)
    if p.framerate != SR:
        problems.append("framerate=%d, expected %d" % (p.framerate, SR))
    if p.comptype != "NONE":
        problems.append("compressed: %s" % p.comptype)

    samples = array.array("h")
    samples.frombytes(raw)
    if sys.byteorder == "big":
        samples.byteswap()

    n = len(samples)
    if n == 0:
        raise CheckFailure("%s: empty file" % path)

    dur = n / float(p.framerate)
    peak = max(abs(s) for s in samples)
    rms = math.sqrt(sum(float(s) * s for s in samples) / n)
    peak_db = 20.0 * math.log10(peak / FULL_SCALE) if peak else -999.0
    rms_db = 20.0 * math.log10(rms / FULL_SCALE) if rms else -999.0
    first, last = samples[0], samples[-1]
    centroid, flatness = timbre_of(samples, p.framerate)

    if dur >= MAX_DURATION_S:
        problems.append("duration %.1f ms >= %.0f ms cap" % (dur * 1000, MAX_DURATION_S * 1000))
    if peak >= 32767:
        problems.append("clipping: peak sample %d" % peak)
    if peak_db > PEAK_CEILING_DBFS + 0.05:
        problems.append("peak %.2f dBFS above the %.1f dBFS ceiling"
                        % (peak_db, PEAK_CEILING_DBFS))
    if peak_db < PEAK_FLOOR_DBFS:
        problems.append("peak %.2f dBFS below the %.1f dBFS floor"
                        % (peak_db, PEAK_FLOOR_DBFS))
    if abs(rms_db - REFERENCE_RMS_DBFS) > RMS_TOLERANCE_DB:
        problems.append("RMS %.2f dBFS off the %.1f dBFS reference by more than %.2f dB"
                        % (rms_db, REFERENCE_RMS_DBFS, RMS_TOLERANCE_DB))
    if abs(first) > BOUNDARY_MAX:
        problems.append("first sample %d, expected |x| <= %d" % (first, BOUNDARY_MAX))
    if abs(last) > BOUNDARY_MAX:
        problems.append("last sample %d, expected |x| <= %d" % (last, BOUNDARY_MAX))
    if centroid > CENTROID_CEILING_HZ:
        problems.append("first-50ms centroid %.0f Hz above the %.0f Hz warmth ceiling"
                        % (centroid, CENTROID_CEILING_HZ))
    if flatness > FLATNESS_CEILING:
        problems.append("first-50ms spectral flatness %.3f above the %.2f ceiling "
                        "(noise-led, not pitched)" % (flatness, FLATNESS_CEILING))

    if problems:
        raise CheckFailure("%s: %s" % (os.path.relpath(path, REPO_ROOT), "; ".join(problems)))

    return {
        "path": path,
        "n": n,
        "rate": p.framerate,
        "channels": p.nchannels,
        "bits": p.sampwidth * 8,
        "ms": dur * 1000.0,
        "peak_db": peak_db,
        "rms_db": rms_db,
        "first": first,
        "last": last,
        "centroid": centroid,
        "flatness": flatness,
    }


def print_table(rows: list) -> None:
    hdr = ("%-11s %-6s %-14s %8s %10s %9s %6s %6s %10s %9s"
           % ("pack", "sound", "format", "dur ms", "peak dBFS", "rms dBFS",
              "first", "last", "cent. Hz", "flatness"))
    print()
    print(hdr)
    print("-" * len(hdr))
    for pack, kind, st in rows:
        fmt = "%d/%d-bit/mono" % (st["rate"], st["bits"])
        print("%-11s %-6s %-14s %8.1f %10.2f %9.2f %6d %6d %10.0f %9.3f"
              % (pack, kind, fmt, st["ms"], st["peak_db"], st["rms_db"],
                 st["first"], st["last"], st["centroid"], st["flatness"]))
    print("-" * len(hdr))
    rmss = [st["rms_db"] for _, _, st in rows]
    peaks = [st["peak_db"] for _, _, st in rows]
    durs = [st["ms"] for _, _, st in rows]
    cents = [st["centroid"] for _, _, st in rows]
    flats = [st["flatness"] for _, _, st in rows]
    print("RMS spread   %.2f dB  (%.2f .. %.2f)   reference: built-in beeps at %.1f dBFS"
          % (max(rmss) - min(rmss), min(rmss), max(rmss), REFERENCE_RMS_DBFS))
    print("peak range   %.2f .. %.2f dBFS   (ceiling %.1f, floor %.1f)"
          % (min(peaks), max(peaks), PEAK_CEILING_DBFS, PEAK_FLOOR_DBFS))
    print("duration     %.1f .. %.1f ms      (cap %.0f)"
          % (min(durs), max(durs), MAX_DURATION_S * 1000))
    print("centroid     %.0f .. %.0f Hz       (ceiling %.0f)   flatness %.3f .. %.3f  (ceiling %.2f)"
          % (min(cents), max(cents), CENTROID_CEILING_HZ,
             min(flats), max(flats), FLATNESS_CEILING))


def check_separation(rows: list) -> list:
    """
    Every pack has to be identifiable inside the first 50 ms, so no two packs
    may sit on top of each other on that window's centroid.

    Sounds from the *same* pack are exempt: a matched pair is supposed to share
    a timbre, and bowl deliberately puts its start and its stop on the same
    pitch and separates them by damping instead. What is checked is that you
    can never confuse one pack for another, which is the thing that actually
    goes wrong -- submarine's original 762 Hz ping measured within a semitone
    of the built-in beep and read as "the normal sound, but longer".
    """
    problems = []
    worst = None
    for i, (pack_a, kind_a, st_a) in enumerate(rows):
        for pack_b, kind_b, st_b in rows[i + 1:]:
            if pack_a == pack_b:
                continue
            d = semitones(st_a["centroid"], st_b["centroid"])
            if worst is None or d < worst[0]:
                worst = (d, pack_a, kind_a, pack_b, kind_b)
            if d < MIN_PACK_SEPARATION_ST:
                problems.append(
                    "%s/%s and %s/%s are %.2f semitones apart on first-50ms "
                    "centroid (%.0f vs %.0f Hz), under the %.1f minimum"
                    % (pack_a, kind_a, pack_b, kind_b, d,
                       st_a["centroid"], st_b["centroid"], MIN_PACK_SEPARATION_ST))
    if worst:
        print("separation  closest two packs %.2f semitones apart "
              "(%s/%s vs %s/%s)   minimum %.1f"
              % (worst[0], worst[1], worst[2], worst[3], worst[4],
                 MIN_PACK_SEPARATION_ST))
    return problems


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--verify", action="store_true",
                    help="only verify existing files, do not regenerate")
    ap.add_argument("--out", default=PACKS_DIR, help="output packs directory")
    args = ap.parse_args()

    rows, failures = [], []
    for pack in sorted(PACKS):
        if not args.verify:
            start, stop = PACKS[pack]()
            write_wav(os.path.join(args.out, pack, "start.wav"), start)
            write_wav(os.path.join(args.out, pack, "stop.wav"), stop)
        for kind in ("start", "stop"):
            path = os.path.join(args.out, pack, "%s.wav" % kind)
            try:
                rows.append((pack, kind, verify(path)))
            except (CheckFailure, OSError, wave.Error) as exc:
                failures.append(str(exc))

    if rows:
        print_table(rows)
        failures.extend(check_separation(rows))
    if failures:
        print("\nFAILED:")
        for f in failures:
            print("  " + f)
        return 1
    print("\nOK: %d files, all checks passed." % len(rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
