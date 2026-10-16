// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local model throughput on synthetic input; NOT natural-footage quality.
#![cfg(feature = "infer")]
use concat_core::frame::Frame;
use concat_vision::Segmenter;
use std::{path::Path, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let kind = args.get(1).map(String::as_str).unwrap_or("selfie");
    let start = Instant::now();
    let model = match kind {
        "person" => Segmenter::person(Path::new(args.get(2).ok_or("person model path required")?))?,
        "object" => Segmenter::object(Path::new(args.get(2).ok_or("object model path required")?))?,
        "selfie" => Segmenter::selfie()?,
        _ => return Err("expected selfie, person or object".into()),
    };
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let (w, h) = model.input_size(1280, 720);
    let mut frame = Frame::black(w, h);
    model.begin();
    let mut times = Vec::new();
    let mut checksum = 0u64;
    let mut result_size = (0, 0);
    for index in 0..25 {
        for (i, pixel) in frame.pixels_mut().chunks_exact_mut(4).enumerate() {
            let x = i % w as usize;
            let y = i / w as usize;
            let inside = x >= (w / 4) as usize + index
                && x < (w * 3 / 4) as usize + index
                && y > (h / 4) as usize
                && y < (h * 3 / 4) as usize;
            pixel.copy_from_slice(if inside {
                &[180, 130, 95, 255]
            } else {
                &[40, 70, 120, 255]
            });
        }
        let started = Instant::now();
        let mask = model.mask(&frame)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        result_size = (mask.width(), mask.height());
        checksum += mask.bytes().iter().map(|&v| u64::from(v)).sum::<u64>();
        if index >= 5 {
            times.push(elapsed);
        }
    }
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    times.sort_by(f64::total_cmp);
    let p95 = times[(times.len() * 95).div_ceil(100) - 1];
    println!(
        "{{\"model\":\"{kind}\",\"fixture\":\"synthetic-moving-block\",\"natural_quality_evaluated\":false,\"input\":[{w},{h}],\"output\":[{},{}],\"load_ms\":{load_ms},\"warmup_frames\":5,\"measured_frames\":{},\"mean_ms\":{mean},\"p95_ms\":{p95},\"inference_fps\":{},\"mask_checksum\":{checksum}}}",
        result_size.0,
        result_size.1,
        times.len(),
        1000.0 / mean
    );
    Ok(())
}
