// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Pure source-space mask refinement, independent of inference and cache storage.
use crate::Mask;
use std::collections::VecDeque;

/// Temporal alpha-noise controls, sanitized by [`TemporalSmoother::new`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemporalConfig {
    /// Weight of history, clamped to 0..=0.95; zero is byte-exact bypass.
    pub strength: f32,
    /// Changes above this many alpha levels bypass history at that pixel;
    /// clamped to 0..=128 so a full hard edge cannot be blended.
    pub max_delta: u8,
    /// Source-time gap in seconds above which history is discarded. Clamped to
    /// 0.001..=60 seconds; nonfinite values use 0.5 seconds.
    pub max_gap_seconds: f64,
    /// Fraction of pixels exceeding `max_delta` that resets the entire mask.
    /// Clamped to 0.01..=1; nonfinite values use 0.5.
    pub scene_change_fraction: f32,
}

impl TemporalConfig {
    fn sanitized(self) -> Self {
        Self {
            strength: finite_clamp(self.strength, 0.95),
            max_delta: self.max_delta.min(128),
            max_gap_seconds: if self.max_gap_seconds.is_finite() {
                self.max_gap_seconds.clamp(0.001, 60.0)
            } else {
                0.5
            },
            scene_change_fraction: if self.scene_change_fraction.is_finite() {
                self.scene_change_fraction.clamp(0.01, 1.0)
            } else {
                0.5
            },
        }
    }
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            strength: 0.0,
            max_delta: 32,
            max_gap_seconds: 0.5,
            scene_change_fraction: 0.5,
        }
    }
}

/// Stateful single-sequence smoother. Never share history across sources/runs.
#[derive(Debug)]
pub struct TemporalSmoother {
    config: TemporalConfig,
    history: Vec<f32>,
    previous: Vec<u8>,
    shape: (u32, u32),
    time: Option<f64>,
}

impl TemporalSmoother {
    /// Construct one smoother per ordered source sequence.
    pub fn new(config: TemporalConfig) -> Self {
        Self {
            config: config.sanitized(),
            history: Vec::new(),
            previous: Vec::new(),
            shape: (0, 0),
            time: None,
        }
    }

    /// Return the effective controls.
    pub fn config(&self) -> TemporalConfig {
        self.config
    }

    /// Replace controls and discard history (including when controls are equal).
    pub fn set_config(&mut self, config: TemporalConfig) {
        self.config = config.sanitized();
        self.reset();
    }

    /// Discard history on every source/run/seek boundary, even at contiguous time.
    pub fn reset(&mut self) {
        self.history.clear();
        self.previous.clear();
        self.shape = (0, 0);
        self.time = None;
    }

    /// Smooth a mask at source time in seconds. Non-increasing time, excessive
    /// gaps and shape changes start a cold sequence; nonfinite time clears state
    /// without seeding the next frame.
    pub fn smooth(&mut self, mask: &Mask, seconds: f64) -> Mask {
        if self.config.strength == 0.0 || !seconds.is_finite() {
            self.reset();
            return mask.clone();
        }
        let shape = (mask.width(), mask.height());
        let discontinuity = self.time.is_none_or(|previous| {
            seconds <= previous || seconds - previous > self.config.max_gap_seconds
        });
        let changed = mask
            .bytes()
            .iter()
            .zip(&self.previous)
            .filter(|(current, previous)| current.abs_diff(**previous) > self.config.max_delta)
            .count();
        let scene_changed = changed as f64
            >= mask.bytes().len() as f64 * f64::from(self.config.scene_change_fraction);
        self.time = Some(seconds);
        if self.shape != shape || discontinuity || scene_changed {
            self.shape = shape;
            self.history = mask.bytes().iter().map(|&alpha| f32::from(alpha)).collect();
            self.previous = mask.bytes().to_vec();
            return mask.clone();
        }
        let mut out = mask.clone();
        for ((value, history), previous) in out
            .bytes_mut()
            .iter_mut()
            .zip(&mut self.history)
            .zip(&mut self.previous)
        {
            let current = f32::from(*value);
            if value.abs_diff(*previous) > self.config.max_delta
                || (*history - current).abs() > f32::from(self.config.max_delta)
            {
                *history = current;
            } else {
                *history = *history * self.config.strength + current * (1.0 - self.config.strength);
            }
            *previous = *value;
            *value = history.round() as u8;
        }
        out
    }
}

/// Spatial controls in mask pixels; all zero-valued effects are byte identities.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpatialRefinement {
    /// Signed square-window grow/shrink radius, clamped to -4096..=4096.
    pub expansion: i32,
    /// Existing three-pass box feather radius, clamped to 0..=64 pixels.
    pub feather: f32,
    /// Blend toward local soft-alpha average, clamped to 0..=1.
    pub edge_strength: f32,
    /// Local average radius, clamped to 0..=8 pixels; zero disables refinement.
    pub edge_radius: u32,
}

/// Expand/contract, bounded soft-edge refinement, then optional spatial feather.
/// Edge refinement changes only alpha strictly between 0 and 255, by at most
/// 32 levels before feathering. It does not recover hair or refine binary edges.
pub fn refine_mask(mask: &Mask, controls: SpatialRefinement) -> Mask {
    let mut out = expand_mask(mask, controls.expansion);
    let strength = finite_clamp(controls.edge_strength, 1.0);
    let radius = controls.edge_radius.min(8);
    if strength > 0.0 && radius > 0 {
        let averaged = out.blurred(radius as f32);
        for (alpha, &target) in out.bytes_mut().iter_mut().zip(averaged.bytes()) {
            if *alpha > 0 && *alpha < 255 {
                let delta = (f32::from(target) - f32::from(*alpha)).clamp(-32.0, 32.0);
                *alpha = (f32::from(*alpha) + delta * strength).round() as u8;
            }
        }
    }
    let feather = finite_clamp(controls.feather, 64.0);
    if feather > 0.0 {
        out.blurred(feather)
    } else {
        out
    }
}

fn finite_clamp(value: f32, maximum: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, maximum)
    } else {
        0.0
    }
}

/// Positive pixels expand the foreground; negative pixels contract it.
/// Uses a square max/min window with replicated borders, without thresholding
/// soft alpha. Magnitude is clamped to 4096 pixels. Two monotonic-deque passes
/// take O(width * height) time independent of window area.
pub fn expand_mask(mask: &Mask, pixels: i32) -> Mask {
    let pixels = pixels.clamp(-4096, 4096);
    if pixels == 0 {
        return mask.clone();
    }
    let radius = pixels.unsigned_abs() as usize;
    let width = mask.width() as usize;
    let height = mask.height() as usize;
    let mut scratch = vec![0; mask.bytes().len()];
    let mut out = mask.clone();
    let mut queue = VecDeque::new();
    for y in 0..height {
        extrema_line(
            mask.bytes(),
            &mut scratch,
            y * width,
            1,
            width,
            radius,
            pixels > 0,
            &mut queue,
        );
    }
    for x in 0..width {
        extrema_line(
            &scratch,
            out.bytes_mut(),
            x,
            width,
            height,
            radius,
            pixels > 0,
            &mut queue,
        );
    }
    out
}

// Truncated extrema windows equal replicated-border extrema: duplicates do not
// affect min/max. Each index enters and leaves the queue at most once.
#[allow(clippy::too_many_arguments)]
fn extrema_line(
    source: &[u8],
    out: &mut [u8],
    start: usize,
    stride: usize,
    len: usize,
    radius: usize,
    maximum: bool,
    queue: &mut VecDeque<usize>,
) {
    queue.clear();
    let radius = radius.min(len - 1);
    let mut next = 0;
    for center in 0..len {
        let right = center.saturating_add(radius).min(len - 1);
        while next <= right {
            let value = source[start + next * stride];
            while let Some(&back) = queue.back() {
                let old = source[start + back * stride];
                if (maximum && old > value) || (!maximum && old < value) {
                    break;
                }
                queue.pop_back();
            }
            queue.push_back(next);
            next += 1;
        }
        let left = center.saturating_sub(radius);
        while queue.front().is_some_and(|&index| index < left) {
            queue.pop_front();
        }
        out[start + center * stride] = source[start + queue[0] * stride];
    }
}
