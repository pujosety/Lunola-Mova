// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! End-to-end acceptance for alpha-preserving PNG sequence export.

use concat_export::{ClipKind, ExportClip, ExportFormat, ExportRequest, Reporter, render_on};
use concat_media::{RateMode, VideoCodec};
use concat_project::model::ColorSpace;
use concat_render::WgpuCompositor;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let scratch = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    scratch.join(format!(
        "concat-png-sequence-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ))
}

fn write_rgba_png(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    let file = File::create(path).expect("creates source PNG");
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("writes PNG header")
        .write_image_data(pixels)
        .expect("writes PNG pixels");
}

fn read_rgba_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let mut decoder = png::Decoder::new(File::open(path).expect("opens output PNG"));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("reads PNG header");
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).expect("decodes output PNG");
    assert_eq!(info.color_type, png::ColorType::Rgba, "sequence must retain RGBA");
    bytes.truncate(info.buffer_size());
    (info.width, info.height, bytes)
}

#[test]
fn legacy_export_request_without_format_stays_a_movie() {
    let request: ExportRequest = serde_json::from_str(
        r#"{"output":"old.mp4","width":4,"height":2,"rateNum":1,"rateDen":1,"crf":18,"preset":"medium","clips":[]}"#,
    )
    .expect("deserializes the legacy movie request");
    assert_eq!(request.format, ExportFormat::Movie);
}

#[test]
fn png_sequence_rejects_a_single_pixel_output_side() {
    let request = ExportRequest {
        output: "unused-sequence".into(),
        format: ExportFormat::PngSequence,
        width: 3,
        height: 1,
        rate_num: 1,
        rate_den: 1,
        crf: 18,
        preset: "medium".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Limited,
        color_space: ColorSpace::Sdr,
        hdr: false,
        clips: vec![ExportClip::blank(ClipKind::Image, 0.0, 1.0, 0)],
    };
    let cancel = AtomicBool::new(false);
    let mut progress = |_: i64, _: i64, _: &'static str| {};
    let error = render_on(
        &request,
        None,
        Reporter {
            progress: &mut progress,
            cancel: &cancel,
        },
    )
    .expect_err("one-pixel height is not a renderable export size");
    assert!(error.contains("2 pixels on each side"), "{error}");
}

#[test]
fn decoder_preserves_alpha_of_single_row_png_sources() {
    use concat_media::{DecodeOptions, Decoder, FrameSource};

    let root = temp_dir();
    fs::create_dir_all(&root).expect("creates temp root");
    let source = root.join("single-row.png");
    let expected = [190, 40, 90, 0, 40, 120, 220, 128, 15, 210, 70, 255];
    write_rgba_png(&source, 3, 1, &expected);
    let options = DecodeOptions::default().scaled_to(3, 1).limited_to(1);
    let mut decoder = Decoder::open(&source, &options).expect("opens single-row PNG");
    let frame = decoder.next_frame().expect("decodes PNG").expect("one frame");
    assert_eq!(frame.pixels(), expected);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cancelled_png_sequence_preserves_existing_output_and_removes_staging() {
    let root = temp_dir();
    fs::create_dir_all(&root).expect("creates temp root");
    let source = root.join("source.png");
    let output = root.join("alpha-sequence");
    fs::create_dir_all(&output).expect("creates prior output directory");
    let marker = output.join("old.txt");
    fs::write(&marker, b"keep me").expect("writes prior output marker");
    write_rgba_png(&source, 4, 2, &[40; 32]);

    let mut clip = ExportClip::blank(ClipKind::Image, 0.0, 1.0, 0);
    clip.path = source.to_string_lossy().into_owned();
    clip.media_width = Some(4);
    clip.media_height = Some(2);
    let request = ExportRequest {
        output: output.to_string_lossy().into_owned(),
        format: ExportFormat::PngSequence,
        width: 4,
        height: 2,
        rate_num: 1,
        rate_den: 1,
        crf: 18,
        preset: "ultrafast".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Full,
        color_space: ColorSpace::Sdr,
        hdr: false,
        clips: vec![clip],
    };
    let Some(compositor) = WgpuCompositor::new() else {
        let _ = fs::remove_dir_all(&root);
        return; // no usable GPU/software renderer on this host
    };
    let cancel = AtomicBool::new(true);
    let mut progress = |_: i64, _: i64, _: &'static str| {};
    let error = render_on(
        &request,
        Some(compositor),
        Reporter {
            progress: &mut progress,
            cancel: &cancel,
        },
    )
    .expect_err("pre-cancelled sequence must stop");
    assert!(error.contains("cancelled"), "{error}");
    assert_eq!(fs::read(&marker).expect("prior output remains"), b"keep me");
    assert!(!output.join("frame_000001.png").exists());
    let staging_left = fs::read_dir(&root)
        .expect("lists parent")
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().contains("concat-sequence-"));
    assert!(!staging_left, "cancel cleans the staging directory");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn png_sequence_exports_numbered_rgba_frames_with_transparent_and_partial_alpha() {
    let root = temp_dir();
    fs::create_dir_all(&root).expect("creates temp root");
    let source = root.join("source.png");
    let output = root.join("alpha-sequence");
    let source_pixels = [
        190, 40, 90, 0, 40, 120, 220, 128, 15, 210, 70, 255, 190, 40, 90, 0,
        190, 40, 90, 0, 40, 120, 220, 128, 15, 210, 70, 255, 190, 40, 90, 0,
    ];
    write_rgba_png(&source, 4, 2, &source_pixels);
    fs::create_dir_all(&output).expect("creates prior output directory");
    let old_marker = output.join("old.txt");
    fs::write(&old_marker, b"old export").expect("writes prior output marker");

    let mut clip = ExportClip::blank(ClipKind::Image, 0.0, 2.0, 0);
    clip.path = source.to_string_lossy().into_owned();
    clip.media_width = Some(4);
    clip.media_height = Some(2);
    let request = ExportRequest {
        output: output.to_string_lossy().into_owned(),
        format: ExportFormat::PngSequence,
        width: 4,
        height: 2,
        rate_num: 1,
        rate_den: 1,
        crf: 18,
        preset: "ultrafast".into(),
        codec: VideoCodec::H264,
        ten_bit: false,
        rate_mode: RateMode::Vbr,
        bitrate_kbps: 0,
        color_range: concat_media::ColorRange::Full,
        color_space: ColorSpace::Sdr,
        hdr: false,
        clips: vec![clip],
    };
    let Some(compositor) = WgpuCompositor::new() else {
        let _ = fs::remove_dir_all(&root);
        return; // no usable GPU/software renderer on this host
    };
    let cancel = AtomicBool::new(false);
    let mut progress = |_: i64, _: i64, _: &'static str| {};
    let result = render_on(
        &request,
        Some(compositor),
        Reporter {
            progress: &mut progress,
            cancel: &cancel,
        },
    )
    .expect("renders the sequence");
    assert_eq!(Path::new(&result), output);
    assert!(!old_marker.exists(), "successful overwrite replaces prior sequence");

    let first_path = output.join("frame_000001.png");
    let second_path = output.join("frame_000002.png");
    assert!(first_path.is_file(), "first numbered frame exists");
    assert!(second_path.is_file(), "second numbered frame exists");
    let (width, height, pixels) = read_rgba_png(&first_path);
    assert_eq!((width, height), (4, 2));
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        let expected = [0u8, 128, 255, 0][index % 4];
        assert!(pixel[3].abs_diff(expected) <= 1, "alpha at {index}: {pixel:?}");
    }
    let (second_width, second_height, second_pixels) = read_rgba_png(&second_path);
    assert_eq!((second_width, second_height), (4, 2));
    assert_eq!(second_pixels, pixels, "still repeats at every output frame");

    let _ = fs::remove_dir_all(&root);
}
