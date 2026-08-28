#!/usr/bin/env python3
"""
Synthesise TTP's cosmetic sound packs from scratch.

Everything here is generated with the Python standard library only (`wave`,
`math`, `struct`, `array`, `random`). No samples, no downloads, no numpy
dependency -- if numpy happens to be installed it is not used, so the output
bytes are identical on every machine.

Writes, for each pack:
    src-tauri/sounds/packs/<pack>/start.wav
    src-tauri/sounds/packs/<pack>/stop.wav

and then verifies every file it wrote (format, duration, peak, clipping,
boundary samples) and prints a loudness table.

    python3 scripts/synth_sounds.py            # generate + verify
    python3 scripts/synth_sounds.py --verify   # verify existing files only

Design constraints (see docs/ttp-pro-design.md, "1. A voice -- sound packs"):

  * 44100 Hz, 16-bit signed, mono -- same as the built-in start.wav/stop.wav.
  * Under 400 ms. These fire on every dictation; length is the enemy.
  * First and last sample are exactly zero, with a raised-cosine fade at both
    edges, so no pack ever ticks on playback.
  * Loudness-matched to the built-in beeps at -14.8 dBFS RMS, under a -3.0
    dBFS peak ceiling.

Why loudness and not peak is the normalisation target: the built-in beeps peak
at -10.5 dBFS, not -3. Peak-normalising all ten files to -3 dBFS produces a
13 dB RMS spread across packs -- the bit-crushed arcade square lands at -5 dBFS
RMS while the radio squelch lands at -19 -- which is exactly the volume jump on
pack switch that we are trying to avoid. So the chain peak-normalises to the
-3 dBFS ceiling first (that is the never-clip guarantee, and transient-led
packs end up sitting right on it) and then trims down to the reference RMS.
Attenuation only: nothing is ever pushed back up through the ceiling.

For packs that come out quieter than the reference at the ceiling, gain is not
available -- it would breach -3 dBFS -- so the crest factor is reduced instead,
by searching for the mild tanh drive that lands the sound on target loudness
while still peaking at -3. That is a synthesis change, not a gain change.
"""

from __future__ import annotations

import argparse
import array
import math
import os
import random
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


# --------------------------------------------------------------------------
# tiny DSP kit
# --------------------------------------------------------------------------

def n_samples(ms: float) -> int:
    """Milliseconds -> whole samples."""
    return int(round(ms * SR / 1000.0))


def silence(n: int) -> list:
    return [0.0] * n


def noise(n: int, seed: int) -> list:
    """Deterministic white noise in [-1, 1). Seeded explicitly, always."""
    rng = random.Random(seed)
    return [rng.uniform(-1.0, 1.0) for _ in range(n)]


def one_pole_lp(sig: list, cutoff: float) -> list:
    a = 1.0 - math.exp(-2.0 * math.pi * cutoff / SR)
    out, y = [], 0.0
    for x in sig:
        y += a * (x - y)
        out.append(y)
    return out


def one_pole_hp(sig: list, cutoff: float) -> list:
    return [x - y for x, y in zip(sig, one_pole_lp(sig, cutoff))]


def biquad_bp(sig: list, freq: float, q: float) -> list:
    """RBJ constant-peak-gain bandpass. Cascade it for steeper skirts."""
    w0 = 2.0 * math.pi * freq / SR
    alpha = math.sin(w0) / (2.0 * q)
    cw = math.cos(w0)
    b0, b1, b2 = alpha, 0.0, -alpha
    a0, a1, a2 = 1.0 + alpha, -2.0 * cw, 1.0 - alpha
    b0, b1, b2, a1, a2 = b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0
    out = []
    x1 = x2 = y1 = y2 = 0.0
    for x in sig:
        y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2
        x2, x1 = x1, x
        y2, y1 = y1, y
        out.append(y)
    return out


def band_noise(n: int, seed: int, freq: float, q: float, stages: int = 2) -> list:
    """Band-limited noise -- the raw material for every squelch and click."""
    sig = noise(n, seed)
    for _ in range(stages):
        sig = biquad_bp(sig, freq, q)
    return normalise(sig, 1.0)


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


def bitcrush(sig: list, bits: int, hold: int) -> list:
    """Quantise to `bits` and sample-and-hold every `hold` samples. 8-bit charm."""
    levels = float(2 ** (bits - 1))
    out, held = [], 0.0
    for i, x in enumerate(sig):
        if i % hold == 0:
            held = max(-1.0, min(1.0, x))
            held = round(held * levels) / levels
        out.append(held)
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
# pack 1 -- radio: 1970s walkie-talkie
# --------------------------------------------------------------------------

def radio_squelch(n: int, seed: int, freq: float, q: float,
                  env: list, crackle_seed: int) -> list:
    """Band-limited hiss with slow amplitude crackle, the squelch texture."""
    sig = band_noise(n, seed, freq, q, stages=2)
    crackle = one_pole_lp(noise(n, crackle_seed), 90.0)
    cp = peak_of(crackle) or 1.0
    sig = [s * (0.80 + 0.20 * (c / cp)) for s, c in zip(sig, crackle)]
    return apply_env(sig, env)


def radio_click(seed: int, freq: float, ring: float) -> list:
    """The relay click: 4 ms of hard transient plus a short damped ring."""
    n = n_samples(14.0)
    trans = band_noise(n, seed, freq, 0.7, stages=1)
    trans = apply_env(trans, env_exp(n, 0.12, 1.4, tail_ms=4.0))
    tone = apply_env(osc(ring, n, "sin"), env_exp(n, 0.15, 4.5, tail_ms=4.0))
    return [0.85 * a + 0.35 * b for a, b in zip(trans, tone)]


def pack_radio():
    # START -- squelch opens, then the relay clicks shut on it. The squelch
    # holds its body before decaying: a spikier envelope would peak on the
    # click and leave the noise too quiet to carry the sound's loudness.
    n = n_samples(205.0)
    start = silence(n)
    sq_n = n_samples(152.0)
    sq_env = env_ar(sq_n, 4.0, 62.0, 86.0, curve=1.25)
    mix(start, radio_squelch(sq_n, 1001, 1750.0, 0.85, sq_env, 1002), 0.0, 0.80)
    mix(start, radio_click(1003, 2400.0, 1450.0), 152.0, 0.92)

    # STOP -- click first, squelch tail dies away behind it. Duller band and a
    # shorter body so the pair reads as closing rather than opening.
    n2 = n_samples(195.0)
    stop = silence(n2)
    mix(stop, radio_click(2003, 2200.0, 1180.0), 5.0, 0.80)   # 5 ms clear of the fade
    t_n = n_samples(172.0)
    t_env = env_ar(t_n, 3.0, 34.0, 135.0, curve=1.45)
    mix(stop, radio_squelch(t_n, 2001, 1250.0, 0.80, t_env, 2002), 15.0, 0.92)

    return master(start), master(stop)


# --------------------------------------------------------------------------
# pack 2 -- arcade: 8-bit
# --------------------------------------------------------------------------

def arcade_blip(f_lo: float, f_hi: float, rising: bool,
                first_ms: float, second_ms: float) -> list:
    """
    Two-tone square blip, phase-continuous across the note change so the
    interval steps without a click. Bit-crushed, then tamed with a lowpass.
    """
    f1, f2 = (f_lo, f_hi) if rising else (f_hi, f_lo)
    n1, n2 = n_samples(first_ms), n_samples(second_ms)
    freqs = [f1] * n1 + [f2] * n2
    sig = osc(freqs, n1 + n2, "square", duty=0.5)
    sig = bitcrush(sig, bits=5, hold=3)          # 5-bit, ~14.7 kHz S&H
    # Two gentle poles rather than one: a square plus S&H aliasing is very
    # bright, and a single 6 dB/oct pole leaves enough top to read as harsh.
    sig = one_pole_lp(one_pole_lp(sig, 4300.0), 4300.0)
    env = env_ar(n1 + n2, 2.0, first_ms + second_ms * 0.42,
                 second_ms * 0.58, curve=2.2)
    # 4 ms of lead-in silence: the master fade-in then shapes silence rather
    # than smearing the attack that makes the blip read as 8-bit.
    return silence(n_samples(4.0)) + apply_env(sig, env)


def pack_arcade():
    # B5 -> E6, the coin interval. Start rises, stop falls and is shorter.
    start = arcade_blip(987.77, 1318.51, rising=True, first_ms=52.0, second_ms=132.0)
    stop = arcade_blip(987.77, 1318.51, rising=False, first_ms=44.0, second_ms=106.0)
    return master(start), master(stop)


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
# pack 4 -- typewriter
# --------------------------------------------------------------------------

def key_click(seed: int) -> list:
    """Type-bar strike: 3 ms of highpassed noise over a short wooden thock."""
    n = n_samples(16.0)
    tick = one_pole_hp(noise(n, seed), 1900.0)
    tick = normalise(apply_env(tick, env_exp(n, 0.08, 1.25, tail_ms=5.0)), 1.0)
    thock = apply_env(osc(232.0, n, "sin"), env_exp(n, 0.4, 5.0, tail_ms=5.0))
    return [a + 0.30 * b for a, b in zip(tick, thock)]


def pack_typewriter():
    # START -- key click, then the small inharmonic bell it sets ringing.
    n = n_samples(220.0)
    start = silence(n)
    mix(start, key_click(3001), 5.0, 1.0)   # 5 ms clear of the master fade-in
    bn = n_samples(205.0)
    start = mix(start, bell(bn, [
        (2274.0, 1.00, 152.0),
        (3416.0, 0.55, 108.0),
        (4688.0, 0.30, 78.0),
        (6130.0, 0.17, 58.0),
    ], attack_ms=1.2), 11.0, 0.52)

    # STOP -- the carriage-return ding alone. Same bell family, no strike
    # transient, pitched down and left to ring: a closing gesture.
    n2 = n_samples(340.0)
    stop = bell(n2, [
        (2102.0, 1.00, 235.0),
        (3157.0, 0.50, 168.0),
        (4334.0, 0.27, 120.0),
        (5668.0, 0.14, 88.0),
    ], attack_ms=2.6)

    return master(start), master(stop, fade_out_ms=10.0)


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
    "radio": pack_radio,
    "arcade": pack_arcade,
    "submarine": pack_submarine,
    "typewriter": pack_typewriter,
    "bubble": pack_bubble,
}


# --------------------------------------------------------------------------
# verification
# --------------------------------------------------------------------------

class CheckFailure(Exception):
    pass


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
    }


def print_table(rows: list) -> None:
    hdr = ("%-11s %-6s %-14s %8s %10s %9s %7s %7s"
           % ("pack", "sound", "format", "dur ms", "peak dBFS", "rms dBFS", "first", "last"))
    print()
    print(hdr)
    print("-" * len(hdr))
    for pack, kind, st in rows:
        fmt = "%d/%d-bit/mono" % (st["rate"], st["bits"])
        print("%-11s %-6s %-14s %8.1f %10.2f %9.2f %7d %7d"
              % (pack, kind, fmt, st["ms"], st["peak_db"], st["rms_db"],
                 st["first"], st["last"]))
    print("-" * len(hdr))
    rmss = [st["rms_db"] for _, _, st in rows]
    peaks = [st["peak_db"] for _, _, st in rows]
    durs = [st["ms"] for _, _, st in rows]
    print("RMS spread   %.2f dB  (%.2f .. %.2f)   reference: built-in beeps at %.1f dBFS"
          % (max(rmss) - min(rmss), min(rmss), max(rmss), REFERENCE_RMS_DBFS))
    print("peak range   %.2f .. %.2f dBFS   (ceiling %.1f, floor %.1f)"
          % (min(peaks), max(peaks), PEAK_CEILING_DBFS, PEAK_FLOOR_DBFS))
    print("duration     %.1f .. %.1f ms      (cap %.0f)"
          % (min(durs), max(durs), MAX_DURATION_S * 1000))


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
    if failures:
        print("\nFAILED:")
        for f in failures:
            print("  " + f)
        return 1
    print("\nOK: %d files, all checks passed." % len(rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
