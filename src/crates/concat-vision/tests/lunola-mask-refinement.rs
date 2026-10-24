// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Synthetic actual-mask contracts; not natural-footage quality evidence.
use concat_vision::{
    Mask, SpatialRefinement, TemporalConfig, TemporalSmoother, expand_mask, refine_mask,
};

#[test]
fn temporal_controls_are_sanitized_and_reconfiguration_resets() {
    let mut smoother = TemporalSmoother::new(TemporalConfig {
        strength: 9.0,
        max_delta: 255,
        max_gap_seconds: -1.0,
        scene_change_fraction: -1.0,
    });
    let config = smoother.config();
    assert_eq!(config.strength, 0.95);
    assert_eq!(config.max_delta, 128);
    assert_eq!(config.max_gap_seconds, 0.001);
    assert_eq!(config.scene_change_fraction, 0.01);
    smoother.set_config(TemporalConfig {
        strength: 0.75,
        ..Default::default()
    });
    smoother.smooth(&row(&[100]), 0.0);
    smoother.smooth(&row(&[110]), 0.1);
    smoother.set_config(smoother.config());
    assert_eq!(smoother.smooth(&row(&[120]), 0.2), row(&[120]));
    smoother.set_config(TemporalConfig {
        strength: f32::NAN,
        max_delta: 0,
        max_gap_seconds: f64::INFINITY,
        scene_change_fraction: f32::NAN,
    });
    assert_eq!(
        smoother.config(),
        TemporalConfig {
            max_delta: 0,
            ..Default::default()
        }
    );
    for i in 0..5 {
        assert_eq!(
            smoother.smooth(&row(&[1, 37, 128, 254]), i as f64 * 0.1),
            row(&[1, 37, 128, 254])
        );
    }
    smoother.set_config(TemporalConfig {
        strength: -1.0,
        max_gap_seconds: 999.0,
        scene_change_fraction: 9.0,
        ..Default::default()
    });
    assert_eq!(smoother.config().strength, 0.0);
    assert_eq!(smoother.config().max_gap_seconds, 60.0);
    assert_eq!(smoother.config().scene_change_fraction, 1.0);
}

#[test]
fn widespread_mask_changes_reset_even_unchanged_region_history() {
    let mut smoother = TemporalSmoother::new(TemporalConfig {
        strength: 0.75,
        ..Default::default()
    });
    smoother.smooth(&row(&[0, 0, 0, 100]), 0.0);
    let scene = row(&[255, 255, 255, 110]);
    assert_eq!(smoother.smooth(&scene, 0.1), scene);
    let following = smoother.smooth(&row(&[255, 255, 255, 120]), 0.2);
    assert_eq!(following.bytes()[3], 113);
}

#[test]
fn explicit_reset_starts_new_source_or_run_without_history() {
    let mut smoother = TemporalSmoother::new(TemporalConfig {
        strength: 0.75,
        ..Default::default()
    });
    smoother.smooth(&row(&[100]), 0.0);
    smoother.smooth(&row(&[110]), 0.1);
    smoother.reset();
    assert_eq!(smoother.smooth(&row(&[120]), 0.2), row(&[120]));
    smoother.reset();
    smoother.reset();
    assert_eq!(smoother.smooth(&row(&[130]), 0.3), row(&[130]));
}

#[test]
fn time_discontinuities_reset_and_nonfinite_time_does_not_seed_history() {
    for discontinuity in [0.05, 0.1, 2.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut smoother = TemporalSmoother::new(TemporalConfig {
            strength: 0.75,
            ..Default::default()
        });
        smoother.smooth(&row(&[100]), 0.0);
        smoother.smooth(&row(&[110]), 0.1);
        let current = row(&[120]);
        assert_eq!(
            smoother.smooth(&current, discontinuity),
            current,
            "time {discontinuity}"
        );
        if !discontinuity.is_finite() {
            assert_eq!(smoother.smooth(&row(&[130]), 0.2), row(&[130]));
        } else {
            assert_ne!(
                smoother.smooth(&row(&[130]), discontinuity + 0.1),
                row(&[130])
            );
        }
    }
}

#[test]
fn shape_changes_reset_even_with_equal_pixel_counts() {
    let mut smoother = TemporalSmoother::new(TemporalConfig {
        strength: 0.75,
        ..Default::default()
    });
    smoother.smooth(&Mask::filled(2, 3, 100), 0.0);
    let changed = Mask::filled(3, 2, 110);
    assert_eq!(smoother.smooth(&changed, 0.1), changed);
    assert_ne!(
        smoother.smooth(&Mask::filled(3, 2, 120), 0.2),
        Mask::filled(3, 2, 120)
    );
}

#[test]
fn moving_hard_edges_bypass_history_without_trails() {
    let mut smoother = TemporalSmoother::new(TemporalConfig {
        strength: 0.9,
        ..Default::default()
    });
    for index in 0..8 {
        let mut mask = Mask::filled(12, 1, 0);
        mask.bytes_mut()[index..index + 3].fill(255);
        assert_eq!(smoother.smooth(&mask, index as f64 * 0.1), mask);
    }
}

#[test]
fn temporal_smoothing_reduces_alternating_small_alpha_noise() {
    let mut smoother = TemporalSmoother::new(TemporalConfig {
        strength: 0.75,
        ..Default::default()
    });
    assert_eq!(smoother.smooth(&row(&[100]), 0.0), row(&[100]));
    let mut previous = 100;
    let mut variation = 0u32;
    for index in 1..21 {
        let current = if index % 2 == 0 { 100 } else { 110 };
        let result = smoother.smooth(&row(&[current]), index as f64 * 0.1);
        let value = result.bytes()[0];
        assert!((100..=110).contains(&value));
        variation += u32::from(value.abs_diff(previous));
        previous = value;
    }
    assert!(
        variation < 80,
        "smoothed variation {variation}, input variation 200"
    );
}

#[test]
fn edge_refinement_is_bounded_and_feather_reuses_spatial_blur() {
    let mask = row(&[0, 10, 255]);
    let controls = SpatialRefinement {
        edge_strength: 1.0,
        edge_radius: 1,
        ..Default::default()
    };
    let refined = refine_mask(&mask, controls);
    let target = mask.blurred(1.0).bytes()[1];
    assert_eq!(refined.bytes(), &[0, target.min(42), 255]);
    assert_ne!(refined, mask);
    assert_eq!(refine_mask(&mask, SpatialRefinement::default()), mask);
    assert_eq!(
        refine_mask(
            &mask,
            SpatialRefinement {
                feather: 2.0,
                ..Default::default()
            }
        ),
        mask.blurred(2.0)
    );
}

fn row(values: &[u8]) -> Mask {
    Mask::from_bytes(values.len() as u32, 1, values.to_vec()).unwrap()
}

#[test]
fn separable_extrema_match_square_reference_on_rectangular_soft_masks() {
    for (width, height) in [(1, 9), (9, 1), (2, 3), (9, 7)] {
        let data = (0..width * height)
            .map(|i| ((i * 73 + 19) % 256) as u8)
            .collect();
        let mask = Mask::from_bytes(width, height, data).unwrap();
        for radius in [1i32, 2, 5, 12] {
            for sign in [-1, 1] {
                let result = expand_mask(&mask, radius * sign);
                for y in 0..height {
                    for x in 0..width {
                        let mut expected = if sign > 0 { 0 } else { 255 };
                        for dy in -radius..=radius {
                            for dx in -radius..=radius {
                                let alpha = mask
                                    .at(i64::from(x) + i64::from(dx), i64::from(y) + i64::from(dy));
                                expected = if sign > 0 {
                                    expected.max(alpha)
                                } else {
                                    expected.min(alpha)
                                };
                            }
                        }
                        assert_eq!(
                            result.at(i64::from(x), i64::from(y)),
                            expected,
                            "{width}x{height}, ({x},{y}), {radius}*{sign}"
                        );
                    }
                }
                assert_eq!((result.width(), result.height()), (width, height));
            }
        }
    }
}

#[test]
fn empty_inputs_are_rejected_and_extreme_radii_are_bounded() {
    assert!(Mask::from_bytes(0, 0, vec![]).is_none());
    assert!(Mask::from_bytes(1, 1, vec![]).is_none());
    assert!(Mask::from_bytes(u32::MAX, 1, vec![]).is_none());
    for (width, height) in [(1, 1), (4097, 1), (1, 4097)] {
        let mut mask = Mask::filled(width, height, 127);
        mask.bytes_mut()[0] = 0;
        *mask.bytes_mut().last_mut().unwrap() = 255;
        assert_eq!(expand_mask(&mask, i32::MAX), expand_mask(&mask, 4096));
        assert_eq!(expand_mask(&mask, i32::MIN), expand_mask(&mask, -4096));
        assert!(
            expand_mask(&mask, i32::MAX)
                .bytes()
                .iter()
                .all(|&a| a == 255)
        );
        assert!(
            expand_mask(&mask, i32::MIN)
                .bytes()
                .iter()
                .all(|&a| a == if width == 1 && height == 1 { 255 } else { 0 })
        );
    }
    for alpha in [0, 1, 37, 128, 254, 255] {
        let mask = Mask::filled(5, 3, alpha);
        for expansion in [-9999, -1, 0, 1, 9999] {
            assert_eq!(
                refine_mask(
                    &mask,
                    SpatialRefinement {
                        expansion,
                        feather: 64.0,
                        edge_radius: u32::MAX,
                        edge_strength: 1.0
                    }
                ),
                mask
            );
        }
    }
}

#[test]
fn spatial_controls_are_bounded_and_disabled_effects_preserve_every_byte() {
    let mask = row(&(0..=255).collect::<Vec<u8>>());
    assert_eq!(refine_mask(&mask, SpatialRefinement::default()), mask);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
        assert_eq!(
            refine_mask(
                &mask,
                SpatialRefinement {
                    feather: invalid,
                    edge_strength: invalid,
                    edge_radius: u32::MAX,
                    ..Default::default()
                }
            ),
            mask
        );
    }
    assert_eq!(
        refine_mask(
            &mask,
            SpatialRefinement {
                feather: f32::MAX,
                ..Default::default()
            }
        ),
        mask.blurred(64.0)
    );
    assert_eq!(
        refine_mask(
            &mask,
            SpatialRefinement {
                edge_strength: f32::MAX,
                edge_radius: u32::MAX,
                ..Default::default()
            }
        ),
        refine_mask(
            &mask,
            SpatialRefinement {
                edge_strength: 1.0,
                edge_radius: 8,
                ..Default::default()
            }
        )
    );
    assert_eq!(
        refine_mask(
            &mask,
            SpatialRefinement {
                edge_strength: 1.0,
                edge_radius: 0,
                ..Default::default()
            }
        ),
        mask
    );
    let refined = refine_mask(
        &mask,
        SpatialRefinement {
            edge_strength: 1.0,
            edge_radius: 8,
            ..Default::default()
        },
    );
    for (&before, &after) in mask.bytes().iter().zip(refined.bytes()) {
        assert!(before.abs_diff(after) <= 32);
    }
    let binary = row(&[0, 0, 255, 0, 255, 255]);
    assert_eq!(
        refine_mask(
            &binary,
            SpatialRefinement {
                edge_strength: 1.0,
                edge_radius: 8,
                ..Default::default()
            }
        ),
        binary
    );
    let composed = refine_mask(
        &mask,
        SpatialRefinement {
            expansion: -2,
            feather: 1.0,
            ..Default::default()
        },
    );
    assert_eq!(composed, expand_mask(&mask, -2).blurred(1.0));
}

#[test]
fn refined_soft_alpha_multiplies_existing_alpha_without_changing_rgb() {
    use concat_core::Frame;
    use concat_vision::{Mapping, cut};
    let mask = refine_mask(
        &Mask::filled(2, 2, 128),
        SpatialRefinement {
            feather: 2.0,
            edge_strength: 1.0,
            edge_radius: 1,
            ..Default::default()
        },
    );
    let mut frame = Frame::black(2, 2);
    frame.fill([43, 87, 129, 128]);
    cut(&mut frame, &mask, &Mapping::IDENTITY);
    assert!(
        frame
            .pixels()
            .chunks_exact(4)
            .all(|pixel| pixel == [43, 87, 129, 64])
    );
}

#[test]
fn temporal_zero_strength_preserves_all_alpha_values() {
    let mut smoother = TemporalSmoother::new(TemporalConfig::default());
    for index in 0..4 {
        let values = (0..=255)
            .map(|alpha| ((alpha + index) % 256) as u8)
            .collect::<Vec<_>>();
        let mask = row(&values);
        assert_eq!(smoother.smooth(&mask, index as f64 * 0.1), mask);
    }
}

#[test]
fn expansion_sign_preserves_soft_alpha_and_clamps_borders() {
    let mask = row(&[0, 40, 120, 200, 255]);
    assert_eq!(expand_mask(&mask, 1).bytes(), &[40, 120, 200, 255, 255]);
    assert_eq!(expand_mask(&mask, -1).bytes(), &[0, 0, 40, 120, 200]);
    assert_eq!(expand_mask(&mask, 0), mask);
}
