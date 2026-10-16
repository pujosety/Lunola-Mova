// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Actual adapter alpha acceptance; explicit case accounting, never all-skipped green.
use concat_render::{Compositor, FramePlan, PlannedLayer, WgpuCompositor, detached_clip};
use concat_core::frame::Frame;
use std::sync::Arc;

fn picture(rgba: [u8; 4]) -> PlannedLayer {
    let mut frame = Frame::transparent(4, 2);
    frame.fill(rgba);
    PlannedLayer::picture(detached_clip(), Arc::new(frame))
}

#[test]
fn alpha_single_row_keeps_each_source_pixel() {
    let source = [
        190, 40, 90, 0,
        40, 120, 220, 128,
        15, 210, 70, 255,
    ];
    let mut frame = Frame::transparent(3, 1);
    frame.pixels_mut().copy_from_slice(&source);
    let plan = FramePlan {
        layers: vec![PlannedLayer::picture(detached_clip(), Arc::new(frame))],
        ..FramePlan::empty(3, 1)
    };
    let mut executed = 0;
    for (name, gpu) in [("preferred", WgpuCompositor::new()), ("software", WgpuCompositor::software())] {
        let Some(mut gpu) = gpu else { continue; };
        let out = gpu.render_transparent(&plan).expect("alpha readback");
        assert_eq!((out.width(), out.height()), (3, 1));
        for (i, pixel) in out.pixels().chunks_exact(4).enumerate() {
            assert!(pixel[3].abs_diff(source[i * 4 + 3]) <= 1, "{name} pixel {i}: {pixel:?}");
        }
        executed += 1;
    }
    assert!(executed > 0, "no renderer executed the single-row case");
}

#[test]
fn alpha_partial_colour_is_straight() {
    let mut executed = 0;
    for (name, gpu) in [("preferred", WgpuCompositor::new()), ("software", WgpuCompositor::software())] {
        let Some(mut gpu) = gpu else {
            eprintln!("ALPHA_SKIP adapter={name} case=partial_colour reason=unavailable");
            continue;
        };
        let mut foreground = picture([220, 120, 60, 128]);
        foreground.opacity = 0.5;
        let plan = FramePlan { layers: vec![foreground], ..FramePlan::empty(4, 2) };
        let out = gpu.render_transparent(&plan).unwrap();
        for p in out.pixels().chunks_exact(4) {
            for (actual, expected) in p.iter().zip([220u8, 120, 60, 64]) {
                assert!(actual.abs_diff(expected) <= 2, "straight colour expected, got {p:?}");
            }
        }
        let opaque = gpu.render(&plan);
        assert!(opaque.pixels().chunks_exact(4).all(|p| p[3] == 255));
        assert!(!gpu.is_dead());
        executed += 1;
        eprintln!("ALPHA_EXECUTED adapter={name} case=partial_colour");
    }
    eprintln!("ALPHA_SUMMARY executed={executed} skipped={}", 2-executed);
    assert!(executed > 0, "all partial colour cases skipped");
}

#[test]
fn alpha_resampled_edges_do_not_bleed_hidden_rgb() {
    let mut executed = 0;
    for (name, gpu) in [("preferred", WgpuCompositor::new()), ("software", WgpuCompositor::software())] {
        let Some(mut gpu) = gpu else { eprintln!("ALPHA_SKIP adapter={name} case=colour_edges reason=unavailable"); continue; };
        let mut f = Frame::transparent(2, 2);
        for y in 0..2 { f.set_pixel(0, y, [220, 80, 40, 255]); f.set_pixel(1, y, [0, 0, 255, 0]); }
        let mut l = PlannedLayer::picture(detached_clip(), Arc::new(f));
        l.transform.scale = 2.0;
        let out = gpu.render_transparent(&FramePlan { layers: vec![l], ..FramePlan::empty(4, 2) }).unwrap();
        let mut partial = 0;
        for p in out.pixels().chunks_exact(4) {
            if p[3] > 0 {
                for (a, e) in p[..3].iter().zip([220u8, 80, 40]) { assert!(a.abs_diff(e) <= 2, "halo {p:?}"); }
                if p[3] < 255 { partial += 1; }
            } else { assert_eq!(&p[..3], &[0,0,0]); }
        }
        assert!(partial > 0);
        executed += 1;
        eprintln!("ALPHA_EXECUTED adapter={name} case=colour_edges");
    }
    eprintln!("ALPHA_SUMMARY executed={executed} skipped={}", 2-executed);
    assert!(executed > 0, "all edge cases skipped");
}

#[test]
fn alpha_empty_canvas_adapters() {
    let mut executed = 0;
    let mut skipped = 0;
    for (name, gpu) in [("preferred", WgpuCompositor::new()), ("software", WgpuCompositor::software())] {
        let Some(mut gpu) = gpu else {
            eprintln!("ALPHA_SKIP adapter={name} case=empty_canvas reason=unavailable");
            skipped += 1;
            continue;
        };
        eprintln!("ALPHA_ADAPTER {name}: {:?}", gpu.adapter_info());
        let plan = FramePlan::empty(4, 2);
        let opaque = gpu.render(&plan);
        assert!(opaque.pixels().chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
        assert!(!gpu.is_dead());
        let transparent = gpu.render_transparent(&plan).unwrap();
        assert!(transparent.pixels().chunks_exact(4).all(|p| p == [0, 0, 0, 0]));
        executed += 1;
        eprintln!("ALPHA_EXECUTED adapter={name} case=empty_canvas");
    }
    eprintln!("ALPHA_SUMMARY executed={executed} skipped={skipped}");
    assert!(executed > 0, "all alpha cases skipped: no proof was executed");
}
