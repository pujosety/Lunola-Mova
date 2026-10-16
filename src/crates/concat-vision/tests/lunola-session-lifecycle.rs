// SPDX-License-Identifier: AGPL-3.0-or-later
//! Real native-session lifecycle regression, not model-quality evaluation.
#![cfg(feature = "infer")]
use concat_core::frame::Frame;
use concat_vision::Segmenter;

#[test]
fn repeated_session_creation_without_inference_remains_safe() {
    for index in 0..4 {
        eprintln!("construct-only session {index}");
        let model = Segmenter::selfie().expect("embedded model loads");
        drop(model);
    }
}

#[test]
fn repeated_session_creation_and_inference_remains_safe() {
    for index in 0..4 {
        eprintln!("session {index}: loading");
        let model = Segmenter::selfie().expect("embedded model loads");
        eprintln!("session {index}: inference");
        let mask = model
            .mask(&Frame::black(64, 48))
            .expect("inference succeeds");
        assert_eq!((mask.width(), mask.height()), (256, 256));
        eprintln!("session {index}: dropping");
        drop(model);
    }
}
