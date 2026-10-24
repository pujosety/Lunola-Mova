// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Local-only production Cutouts::analyse QA against a JSON manifest of MOVs.
//! Does not copy, alter, upload, or write beside source media.
use concat_host::cutout::{AnalyseRequest, Cutouts, Progress};
use concat_project::model::Subject;
use concat_vision::{mask_dir, models::ModelId, MaskStore, MASK_RATE};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

const RVM_SHA256: &str = "88d4531297118f595bf2fd60f6f566aec2e559393802d1f436c380f0cbbd2828";

#[derive(Deserialize)]
struct Clip {
    path: String,
    width: u32,
    height: u32,
    duration: f64,
}

#[derive(Deserialize)]
struct Manifest {
    clips: Vec<Clip>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: lunola-cutout-folder-qa <data-dir> <isolated-project-dir> <manifest.json>".into());
    }
    let data_dir = Path::new(&args[1]);
    let project_dir = PathBuf::from(&args[2]);
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    let model_path = concat_vision::models::model_file(data_dir, ModelId::Person);
    let model_bytes = std::fs::read(&model_path)?;
    let model_hash = format!("{:x}", Sha256::digest(&model_bytes));
    if model_hash != RVM_SHA256 {
        return Err(format!("RVM SHA-256 mismatch: {model_hash}").into());
    }
    std::fs::create_dir_all(&project_dir)?;
    let cutouts = Cutouts::new(data_dir);
    if !cutouts.installed(ModelId::Person) {
        return Err("pinned RVM model is not installed; network fetch is prohibited in this QA run".into());
    }
    println!("{{\"event\":\"run_start\",\"clips\":{},\"model\":\"rvm-mobilenetv3\",\"sha256\":\"{}\",\"mask_rate\":{}}}", manifest.clips.len(), model_hash, MASK_RATE);
    let mut total_written = 0usize;
    let mut total_frames = 0usize;
    let mut total_blank = 0usize;
    let overall = Instant::now();

    for (index, clip) in manifest.clips.iter().enumerate() {
        if !Path::new(&clip.path).is_file() {
            return Err(format!("manifest source missing at index {index}").into());
        }
        let request = AnalyseRequest {
            project: project_dir.clone(),
            media_path: clip.path.clone(),
            media_size: (clip.width, clip.height),
            still: false,
            subject: Subject::Person,
            ranges: vec![(0.0, clip.duration)],
        };
        let outstanding_before = Cutouts::outstanding(&request);
        let started = Instant::now();
        let written = cutouts
            .analyse(&request, &mut |progress| {
                if let Progress::Fetching { .. } = progress {
                    eprintln!("unexpected model fetch requested; QA is configured offline");
                }
            })
            .map_err(std::io::Error::other)?;
        let elapsed = started.elapsed().as_secs_f64();
        let store = MaskStore::open(&mask_dir(&project_dir, &clip.path, Subject::Person));
        let stored_model = store.model().unwrap_or("unknown");
        if stored_model != ModelId::Person.name() {
            return Err(format!("clip index {index} used unexpected model {stored_model:?}").into());
        }
        let expected = (clip.duration * f64::from(MASK_RATE)).ceil() as usize;
        let mut sampled = 0usize;
        let mut blank = 0usize;
        let mut foreground_pixels = 0u64;
        let mut pixels = 0u64;
        for frame_index in 0..expected {
            let time = frame_index as f64 / f64::from(MASK_RATE);
            if let Some(mask) = store.mask_at(time) {
                sampled += 1;
                if mask.is_blank() {
                    blank += 1;
                }
                for &value in mask.bytes() {
                    pixels += 1;
                    foreground_pixels += u64::from(value > 127);
                }
            }
        }
        let coverage = if pixels == 0 { 0.0 } else { foreground_pixels as f64 / pixels as f64 };
        total_written += written;
        total_frames += sampled;
        total_blank += blank;
        println!("{{\"event\":\"clip_done\",\"index\":{},\"source_resolution\":[{},{}],\"duration_s\":{},\"outstanding_before\":{},\"frames_written\":{},\"frames_read_back\":{},\"blank_masks\":{},\"mean_binary_foreground_coverage\":{},\"elapsed_s\":{},\"model\":\"{}\"}}", index + 1, clip.width, clip.height, clip.duration, outstanding_before, written, sampled, blank, coverage, elapsed, stored_model);
    }
    println!("{{\"event\":\"run_done\",\"clips\":{},\"frames_written\":{},\"frames_read_back\":{},\"blank_masks\":{},\"elapsed_s\":{},\"inference_pipeline_fps_including_decode\":{}}}", manifest.clips.len(), total_written, total_frames, total_blank, overall.elapsed().as_secs_f64(), total_frames as f64 / overall.elapsed().as_secs_f64());
    Ok(())
}
