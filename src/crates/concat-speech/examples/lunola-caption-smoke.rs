// SPDX-License-Identifier: AGPL-3.0-or-later
// Standalone QA entry point for the unchanged Concat transcription engine.
// Copy into concat-speech/examples only after the active build has exited.

use std::error::Error;
use std::path::Path;
use std::time::Instant;

use concat_host::AppDirs;
use concat_speech::Transcriber;
use concat_speech::transcribe::TranscribeRequest;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: lunola-caption-smoke MODEL_DATA_ROOT WAV_PATH DURATION_SECONDS".into());
    }
    let dirs = AppDirs::under(Path::new(&args[1]));
    let window: f64 = args[3].parse()?;
    let model_file = dirs.data.join("whisper-models/ggml-tiny.en.bin");
    if !model_file.is_file() {
        return Err("verified local model is missing; this harness never downloads models".into());
    }
    let request = TranscribeRequest {
        path: args[2].clone(),
        audio_stream: None,
        source_start: 0.0,
        window,
        model_id: "tiny.en".to_owned(),
    };
    let clock = Instant::now();
    let segments = Transcriber::new().transcribe(&dirs, &request, |percent| {
        eprintln!("transcribe progress: {percent}%");
    })?;
    if segments.is_empty() {
        return Err("transcription produced no caption segments".into());
    }
    for segment in &segments {
        if !segment.start.is_finite()
            || !segment.end.is_finite()
            || segment.start < 0.0
            || segment.end < segment.start
            || segment.end > window + 1.0
            || segment.text.trim().is_empty()
        {
            return Err(format!("invalid caption segment: {segment:?}").into());
        }
    }
    let text = segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let lower = text.to_lowercase();
    if !lower.contains("video") && !lower.contains("frame") {
        return Err(format!("transcript did not recognise the fixture's key words: {text}").into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "product": "Lunola Mova",
            "engine": "Concat / whisper.cpp",
            "model": "tiny.en",
            "model_file": model_file,
            "local_model_inference": true,
            "runtime_model_download": false,
            "elapsed_seconds": clock.elapsed().as_secs_f64(),
            "segments": segments,
            "text": text,
            "result": "PASS"
        }))?
    );
    Ok(())
}
