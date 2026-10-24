// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The frame with its background taken away.

use concat_core::frame::{BYTES_PER_PIXEL, Frame};

use crate::Mask;

/// How a decoded pixel finds its place in the source picture.
///
/// A decoded frame is the source after its crop and its flips, fitted to
/// some size. The mask covers the source as it was shot, so a pixel of the
/// frame has to be walked back through both before it can be looked up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mapping {
    /// Fractions cut off the source's left, top, right and bottom before
    /// the fit, as the document stores them.
    pub crop: [f32; 4],
    /// The frame is the source mirrored left to right.
    pub flip_h: bool,
    /// The frame is the source mirrored top to bottom.
    pub flip_v: bool,
}

impl Mapping {
    /// The frame is the source as it was shot.
    pub const IDENTITY: Mapping = Mapping {
        crop: [0.0; 4],
        flip_h: false,
        flip_v: false,
    };

    /// The source fraction under a frame fraction.
    pub fn source_of(&self, x: f32, y: f32) -> (f32, f32) {
        let x = if self.flip_h { 1.0 - x } else { x };
        let y = if self.flip_v { 1.0 - y } else { y };
        let [left, top, right, bottom] = self.crop;
        (
            left + x * (1.0 - left - right).max(0.0),
            top + y * (1.0 - top - bottom).max(0.0),
        )
    }

    /// The frame fraction a source fraction lands on, or `None` when the
    /// crop cut it away. The inverse of [`Mapping::source_of`], for a brush
    /// that paints in the source's terms and shows on the frame.
    pub fn frame_of(&self, u: f32, v: f32) -> Option<(f32, f32)> {
        let [left, top, right, bottom] = self.crop;
        let width = (1.0 - left - right).max(1e-6);
        let height = (1.0 - top - bottom).max(1e-6);
        let x = (u - left) / width;
        let y = (v - top) / height;
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return None;
        }
        Some((
            if self.flip_h { 1.0 - x } else { x },
            if self.flip_v { 1.0 - y } else { y },
        ))
    }
}

impl Default for Mapping {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Multiplies the frame's alpha by the mask, sampled through `mapping`.
/// The frame keeps its straight alpha; a pixel the mask calls background
/// becomes transparent and its colour is left for the compositor to weigh
/// by that alpha.
pub fn cut(frame: &mut Frame, mask: &Mask, mapping: &Mapping) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if width == 0 || height == 0 {
        return;
    }
    let pixels = frame.pixels_mut();
    let row_bytes = width * BYTES_PER_PIXEL;
    // Rows split across the machine's cores for a frame worth the threads;
    // a small preview frame is faster done on the one it arrived on.
    let workers = if width * height >= 400_000 {
        std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
            .clamp(1, 8)
    } else {
        1
    };
    let rows_per = height.div_ceil(workers);
    std::thread::scope(|scope| {
        for (chunk, band) in pixels.chunks_mut(rows_per * row_bytes).enumerate() {
            let first_row = chunk * rows_per;
            scope.spawn(move || {
                for (row_index, row) in band.chunks_exact_mut(row_bytes).enumerate() {
                    let y = (first_row + row_index) as f32 + 0.5;
                    let fy = y / height as f32;
                    for (x, pixel) in row.chunks_exact_mut(BYTES_PER_PIXEL).enumerate() {
                        let fx = (x as f32 + 0.5) / width as f32;
                        let (u, v) = mapping.source_of(fx, fy);
                        let keep = mask.sample(u, v);
                        pixel[3] = (f32::from(pixel[3]) * keep + 0.5) as u8;
                    }
                }
            });
        }
    });
}

/// Blurs the background while keeping the foreground sharp.
/// `radius` is in source mask pixels, as a fraction of picture width.
/// The frame keeps its alpha; only the background pixels are softened.
pub fn cut_blur(frame: &mut Frame, mask: &Mask, mapping: &Mapping, radius: f32) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if width == 0 || height == 0 {
        return;
    }
    let radius = (radius * width as f32).round().max(0.0) as usize;
    if radius == 0 {
        return;
    }
    // Blur the whole frame, then mix: foreground where mask=1, blurred where mask=0
    let mut blurred = frame.clone();
    blur_frame(&mut blurred, radius);
    let pixels = frame.pixels_mut();
    let blurred_pixels = blurred.pixels();
    let row_bytes = width * BYTES_PER_PIXEL;
    for (y, row) in pixels.chunks_exact_mut(row_bytes).enumerate() {
        let fy = (y as f32 + 0.5) / height as f32;
        for (x, pixel) in row.chunks_exact_mut(BYTES_PER_PIXEL).enumerate() {
            let fx = (x as f32 + 0.5) / width as f32;
            let (u, v) = mapping.source_of(fx, fy);
            let keep = mask.sample(u, v);
            if keep < 1.0 {
                let idx = (y * width + x) * BYTES_PER_PIXEL;
                for c in 0..3 {
                    pixel[c] = (f32::from(pixel[c]) * keep
                        + f32::from(blurred_pixels[idx + c]) * (1.0 - keep)
                        + 0.5) as u8;
                }
            }
        }
    }
}

/// Replaces the background with a solid colour.
/// The frame keeps its alpha; only the background pixels are replaced.
pub fn cut_replace(frame: &mut Frame, mask: &Mask, mapping: &Mapping, colour: [u8; 3]) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if width == 0 || height == 0 {
        return;
    }
    let pixels = frame.pixels_mut();
    let row_bytes = width * BYTES_PER_PIXEL;
    for (y, row) in pixels.chunks_exact_mut(row_bytes).enumerate() {
        let fy = (y as f32 + 0.5) / height as f32;
        for (x, pixel) in row.chunks_exact_mut(BYTES_PER_PIXEL).enumerate() {
            let fx = (x as f32 + 0.5) / width as f32;
            let (u, v) = mapping.source_of(fx, fy);
            let keep = mask.sample(u, v);
            if keep < 1.0 {
                for c in 0..3 {
                    pixel[c] = (f32::from(pixel[c]) * keep
                        + f32::from(colour[c]) * (1.0 - keep)
                        + 0.5) as u8;
                }
            }
        }
    }
}

/// Box blur for a frame, in place. Three passes approximate a gaussian.
fn blur_frame(frame: &mut Frame, radius: usize) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if width == 0 || height == 0 || radius == 0 {
        return;
    }
    let mut temp = frame.clone();
    for _ in 0..3 {
        blur_rows(frame, &mut temp, width, height, radius);
        blur_cols(&mut temp, frame, width, height, radius);
    }
}

fn blur_rows(source: &mut Frame, out: &mut Frame, width: usize, height: usize, radius: usize) {
    let src = source.pixels();
    let dst = out.pixels_mut();
    let row_bytes = width * BYTES_PER_PIXEL;
    let window = (2 * radius + 1) as u32;
    for y in 0..height {
        let row_start = y * row_bytes;
        let mut sums = [0u32; 3];
        for offset in 0..window as usize {
            let x = (offset as isize - radius as isize).clamp(0, width as isize - 1) as usize;
            let idx = row_start + x * BYTES_PER_PIXEL;
            for c in 0..3 { sums[c] += u32::from(src[idx + c]); }
        }
        for x in 0..width {
            let idx = row_start + x * BYTES_PER_PIXEL;
            for c in 0..3 { dst[idx + c] = (sums[c] / window) as u8; }
            if x + 1 < width {
                let old_x = (x as isize - radius as isize).clamp(0, width as isize - 1) as usize;
                let new_x = (x as isize + radius as isize + 1).clamp(0, width as isize - 1) as usize;
                let old_idx = row_start + old_x * BYTES_PER_PIXEL;
                let new_idx = row_start + new_x * BYTES_PER_PIXEL;
                for c in 0..3 {
                    sums[c] -= u32::from(src[old_idx + c]);
                    sums[c] += u32::from(src[new_idx + c]);
                }
            }
        }
    }
}

fn blur_cols(source: &mut Frame, out: &mut Frame, width: usize, height: usize, radius: usize) {
    let src = source.pixels();
    let dst = out.pixels_mut();
    let row_bytes = width * BYTES_PER_PIXEL;
    let window = (2 * radius + 1) as u32;
    for x in 0..width {
        let mut sums = [0u32; 3];
        for offset in 0..window as usize {
            let y = (offset as isize - radius as isize).clamp(0, height as isize - 1) as usize;
            let idx = y * row_bytes + x * BYTES_PER_PIXEL;
            for c in 0..3 { sums[c] += u32::from(src[idx + c]); }
        }
        for y in 0..height {
            let idx = y * row_bytes + x * BYTES_PER_PIXEL;
            for c in 0..3 { dst[idx + c] = (sums[c] / window) as u8; }
            if y + 1 < height {
                let old_y = (y as isize - radius as isize).clamp(0, height as isize - 1) as usize;
                let new_y = (y as isize + radius as isize + 1).clamp(0, height as isize - 1) as usize;
                let old_idx = old_y * row_bytes + x * BYTES_PER_PIXEL;
                let new_idx = new_y * row_bytes + x * BYTES_PER_PIXEL;
                for c in 0..3 {
                    sums[c] -= u32::from(src[old_idx + c]);
                    sums[c] += u32::from(src[new_idx + c]);
                }
            }
        }
    }
}

/// The frame with what the mask keeps tinted `colour`, and the rest as
/// shot: the view a person paints against, where the picture stays whole
/// and the cutout is drawn over it rather than taken out of it. The same
/// mapping as [`cut`], so the tint lies exactly where the cut would.
pub fn highlight(frame: &mut Frame, mask: &Mask, mapping: &Mapping, colour: [u8; 3]) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if width == 0 || height == 0 {
        return;
    }
    let pixels = frame.pixels_mut();
    let row_bytes = width * BYTES_PER_PIXEL;
    for (y, row) in pixels.chunks_exact_mut(row_bytes).enumerate() {
        let fy = (y as f32 + 0.5) / height as f32;
        for (x, pixel) in row.chunks_exact_mut(BYTES_PER_PIXEL).enumerate() {
            let fx = (x as f32 + 0.5) / width as f32;
            let (u, v) = mapping.source_of(fx, fy);
            // Half way to the colour where the mask is sure, less where
            // it is not: the tint reads as the mask's own edge.
            let mix = mask.sample(u, v) * 0.55;
            for c in 0..3 {
                pixel[c] =
                    (f32::from(pixel[c]) * (1.0 - mix) + f32::from(colour[c]) * mix + 0.5) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_highlight_tints_the_kept_half_and_leaves_the_rest() {
        let mut frame = Frame::black(4, 2);
        frame.fill([200, 200, 200, 255]);
        let mut mask = Mask::filled(2, 1, 0);
        mask.bytes_mut()[1] = 255;
        highlight(&mut frame, &mask, &Mapping::default(), [0, 200, 200]);
        let left = frame.pixel(0, 0).unwrap();
        let right = frame.pixel(3, 0).unwrap();
        assert_eq!(left, [200, 200, 200, 255]);
        assert!(right[0] < 120 && right[1] == 200, "{right:?}");
        assert_eq!(right[3], 255);
    }

    fn left_half_kept() -> Mask {
        let mut mask = Mask::filled(8, 8, 0);
        for y in 0..8 {
            for x in 0..4 {
                mask.bytes_mut()[y * 8 + x] = 255;
            }
        }
        mask
    }

    fn alpha(frame: &Frame, x: usize, y: usize) -> u8 {
        frame.pixels()[(y * frame.width() as usize + x) * BYTES_PER_PIXEL + 3]
    }

    #[test]
    fn the_background_half_goes_transparent() {
        let mut frame = Frame::black(16, 8);
        cut(&mut frame, &left_half_kept(), &Mapping::IDENTITY);
        assert_eq!(alpha(&frame, 2, 4), 255);
        assert_eq!(alpha(&frame, 13, 4), 0);
        // The colour is untouched: only the alpha is the mask's business.
        assert_eq!(frame.pixels()[(4 * 16 + 13) * 4], 0);
    }

    #[test]
    fn a_flip_looks_the_mask_up_mirrored() {
        let mut frame = Frame::black(16, 8);
        let mapping = Mapping {
            flip_h: true,
            ..Mapping::IDENTITY
        };
        cut(&mut frame, &left_half_kept(), &mapping);
        assert_eq!(alpha(&frame, 2, 4), 0);
        assert_eq!(alpha(&frame, 13, 4), 255);
    }

    #[test]
    fn a_crop_looks_the_mask_up_inside_the_cropped_source() {
        // The frame is the right two fifths of the source, all of which the
        // mask calls background.
        let mut frame = Frame::black(16, 8);
        let mapping = Mapping {
            crop: [0.6, 0.0, 0.0, 0.0],
            ..Mapping::IDENTITY
        };
        cut(&mut frame, &left_half_kept(), &mapping);
        assert_eq!(alpha(&frame, 1, 4), 0);
        assert_eq!(alpha(&frame, 14, 4), 0);
    }

    #[test]
    fn frame_of_inverts_source_of() {
        let mapping = Mapping {
            crop: [0.1, 0.2, 0.3, 0.0],
            flip_h: true,
            flip_v: false,
        };
        let (u, v) = mapping.source_of(0.25, 0.75);
        let (x, y) = mapping.frame_of(u, v).expect("inside the crop");
        assert!((x - 0.25).abs() < 1e-5 && (y - 0.75).abs() < 1e-5);
        assert!(mapping.frame_of(0.05, 0.5).is_none());
    }

    #[test]
    fn a_large_frame_is_cut_the_same_on_many_threads() {
        let mut big = Frame::black(1000, 500);
        cut(&mut big, &left_half_kept(), &Mapping::IDENTITY);
        assert_eq!(alpha(&big, 10, 250), 255);
        assert_eq!(alpha(&big, 990, 250), 0);
        assert_eq!(alpha(&big, 990, 499), 0);
    }

    #[test]
    fn cut_blur_softens_background_not_foreground() {
        let mut frame = Frame::black(16, 8);
        frame.fill([200, 100, 50, 255]);
        for y in 0..8 { for x in 8..16 { frame.set_pixel(x, y, [0, 0, 0, 255]); } }
        cut_blur(&mut frame, &left_half_kept(), &Mapping::IDENTITY, 0.05);
        assert_eq!(frame.pixel(2, 4).unwrap(), [200, 100, 50, 255]);
        let near_edge = frame.pixel(8, 4).unwrap();
        assert!(near_edge[0] > 0, "background edge should be softened: {near_edge:?}");
        assert_eq!(frame.pixel(14, 4).unwrap(), [0, 0, 0, 255]);
    }

    #[test]
    fn blur_uses_three_separable_passes_without_dropping_extent() {
        let mut frame = Frame::black(5, 5);
        frame.set_pixel(2, 2, [255, 255, 255, 255]);
        blur_frame(&mut frame, 1);
        assert_eq!(frame.pixel(2, 2).unwrap(), [16, 16, 16, 255]);
        assert_eq!(frame.pixel(0, 0).unwrap(), [5, 5, 5, 255]);
    }

    #[test]
    fn cut_blur_zero_radius_is_noop() {
        let mut frame = Frame::black(16, 8);
        frame.fill([200, 100, 50, 255]);
        cut_blur(&mut frame, &left_half_kept(), &Mapping::IDENTITY, 0.0);
        // Nothing should change
        assert_eq!(frame.pixel(13, 4).unwrap(), [200, 100, 50, 255]);
    }

    #[test]
    fn cut_replace_paints_background_with_colour() {
        let mut frame = Frame::black(16, 8);
        frame.fill([200, 100, 50, 255]);
        cut_replace(&mut frame, &left_half_kept(), &Mapping::IDENTITY, [0, 255, 0]);
        // Foreground (left half) should be unchanged
        assert_eq!(frame.pixel(2, 4).unwrap(), [200, 100, 50, 255]);
        // Background (right half) should be replaced with green
        let bg = frame.pixel(13, 4).unwrap();
        assert_eq!(bg[0], 0);
        assert_eq!(bg[1], 255);
        assert_eq!(bg[2], 0);
        assert_eq!(bg[3], 255);
    }

    #[test]
    fn cut_replace_preserves_alpha() {
        let mut frame = Frame::black(16, 8);
        frame.fill([200, 100, 50, 128]);
        cut_replace(&mut frame, &left_half_kept(), &Mapping::IDENTITY, [0, 0, 255]);
        // Alpha should be preserved
        assert_eq!(alpha(&frame, 13, 4), 128);
        assert_eq!(alpha(&frame, 2, 4), 128);
    }
}
