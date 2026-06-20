//! Pitch-preserving time stretching (WSOLA).
//!
//! `rodio`'s `Source::speed()` simply resamples, which changes the pitch
//! (the "chipmunk" effect). For dialogue we want to change the *tempo* while
//! keeping the *pitch* — exactly like FFmpeg's `atempo` filter.
//!
//! This module implements WSOLA (Waveform Similarity Overlap-Add): it slides
//! overlapping windows from the input at the analysis hop and overlap-adds them
//! at the synthesis hop, searching a small range for the best-correlating offset
//! so successive grains line up in phase. The result is a faster (or slower)
//! signal with the original pitch preserved.

use std::f32::consts::PI;

/// Time-stretch interleaved f32 samples, preserving pitch.
///
/// * `samples`     - interleaved PCM (channel-major per frame)
/// * `channels`    - number of channels
/// * `sample_rate` - sample rate in Hz
/// * `speed`       - playback speed factor: `> 1.0` = faster (shorter),
///                   `< 1.0` = slower (longer). `1.0` returns the input as-is.
///
/// Returns new interleaved samples at the requested tempo.
pub fn time_stretch(samples: &[f32], channels: u16, sample_rate: u32, speed: f32) -> Vec<f32> {
    let channels = channels.max(1) as usize;

    // No-op cases.
    if !speed.is_finite() || (speed - 1.0).abs() < 0.01 || samples.is_empty() {
        return samples.to_vec();
    }

    let frames = samples.len() / channels; // per-channel sample count
    if frames < 8 {
        return samples.to_vec();
    }

    // Deinterleave into per-channel buffers + a mono mix used for alignment.
    let mut chans: Vec<Vec<f32>> = vec![Vec::with_capacity(frames); channels];
    let mut mix = vec![0.0f32; frames];
    for f in 0..frames {
        let mut acc = 0.0f32;
        for c in 0..channels {
            let v = samples[f * channels + c];
            chans[c].push(v);
            acc += v;
        }
        mix[f] = acc / channels as f32;
    }

    // Window / hop sizes. ~25 ms window with 50% synthesis overlap is enough
    // for speech and keeps the per-line cost low (so audio keeps up).
    let mut w = (sample_rate as f32 * 0.025) as usize;
    if w < 512 {
        w = 512;
    }
    if w % 2 != 0 {
        w += 1;
    }
    if frames <= w {
        return samples.to_vec();
    }
    let hs = w / 2; // synthesis hop
    let ha = (((hs as f32) * speed).round() as usize).max(1); // analysis hop
    // Small WSOLA search radius (~one pitch period) — the dominant cost driver,
    // so keep it modest to avoid starving the audio worker.
    let tol = (w as isize / 6).max(64);

    // Hann window.
    let win: Vec<f32> = (0..w)
        .map(|i| 0.5 - 0.5 * ((2.0 * PI * i as f32) / (w as f32 - 1.0)).cos())
        .collect();

    let out_frames = ((frames as f32 / speed).round() as usize) + w;
    let mut out: Vec<Vec<f32>> = vec![vec![0.0f32; out_frames]; channels];
    let mut norm = vec![0.0f32; out_frames];

    let mut ts: usize = 0; // synthesis position
    let mut p: isize = 0; // analysis position of current grain
    let mut m: usize = 0; // grain index

    loop {
        // Overlap-add the current grain (from input position p) at ts.
        for i in 0..w {
            let oi = ts + i;
            if oi >= out_frames {
                break;
            }
            let src = p + i as isize;
            if src < 0 || (src as usize) >= frames {
                continue;
            }
            let wv = win[i];
            let s = src as usize;
            for c in 0..channels {
                out[c][oi] += chans[c][s] * wv;
            }
            norm[oi] += wv;
        }

        ts += hs;
        m += 1;
        if ts + w >= out_frames {
            break;
        }

        // Nominal next analysis position (without alignment correction).
        let nominal = (m * ha) as isize;
        if nominal >= frames as isize {
            break;
        }

        // WSOLA: search a small offset that best matches the natural
        // continuation of the grain we just placed (mix[p+hs ..]).
        let tgt_start = p + hs as isize;
        let mut best_delta: isize = 0;
        let mut best_err = f32::INFINITY;

        let mut d = -tol;
        while d <= tol {
            let cand = nominal + d;
            if cand >= 0 && (cand as usize) + w <= frames {
                let cand_u = cand as usize;
                let mut err = 0.0f32;
                // Decimate the comparison by 4 for speed.
                let mut i = 0;
                while i < w {
                    let a = mix[cand_u + i];
                    let tgt_i = tgt_start + i as isize;
                    let b = if tgt_i >= 0 && (tgt_i as usize) < frames {
                        mix[tgt_i as usize]
                    } else {
                        0.0
                    };
                    let diff = a - b;
                    err += diff * diff;
                    if err >= best_err {
                        break;
                    }
                    i += 4;
                }
                if err < best_err {
                    best_err = err;
                    best_delta = d;
                }
            }
            d += 1;
        }

        p = (nominal + best_delta).max(0);
        if p as usize >= frames {
            break;
        }
    }

    // Normalize by the accumulated window energy and reinterleave.
    let mut last = out_frames;
    while last > 0 && norm[last - 1] <= 1e-6 {
        last -= 1;
    }

    let mut result = Vec::with_capacity(last * channels);
    for f in 0..last {
        let n = norm[f];
        for c in 0..channels {
            let v = if n > 1e-6 { out[c][f] / n } else { 0.0 };
            result.push(v.clamp(-1.0, 1.0));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_one_is_noop() {
        let samples = vec![0.1f32, -0.2, 0.3, -0.4, 0.5, -0.6, 0.7, -0.8];
        let out = time_stretch(&samples, 2, 44100, 1.0);
        assert_eq!(out, samples);
    }

    #[test]
    fn test_faster_is_shorter() {
        // 1 second mono sine at 44.1k
        let sr = 44100u32;
        let n = sr as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * 220.0 * i as f32 / sr as f32).sin())
            .collect();
        let out = time_stretch(&samples, 1, sr, 1.5);
        // ~1.5x faster -> ~2/3 the length (allow generous tolerance)
        let expected = (n as f32 / 1.5) as usize;
        let diff = (out.len() as isize - expected as isize).abs();
        assert!(diff < (sr as isize / 5), "len {} vs expected {}", out.len(), expected);
    }

    #[test]
    fn test_slower_is_longer() {
        let sr = 22050u32;
        let n = sr as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * 200.0 * i as f32 / sr as f32).sin())
            .collect();
        let out = time_stretch(&samples, 1, sr, 0.75);
        assert!(out.len() > n, "slower output should be longer: {} vs {}", out.len(), n);
    }

    #[test]
    fn test_empty_and_tiny() {
        assert!(time_stretch(&[], 2, 44100, 1.5).is_empty());
        let tiny = vec![0.1f32, 0.2];
        assert_eq!(time_stretch(&tiny, 2, 44100, 1.5), tiny);
    }
}
