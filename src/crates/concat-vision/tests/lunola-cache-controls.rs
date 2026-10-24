//! Generation-aware raw/resolved publication and persisted controls.
use concat_project::model::Cutout;
use concat_vision::{Mask, MaskStore};

fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("lunola-controls-{name}-{}", std::process::id()))
}
#[test]
fn overwrite_refreshes_raw_and_resolved_masks() {
    let dir = scratch("overwrite");
    let mut writer = MaskStore::open(&dir);
    writer.clear();
    writer.put(0, &Mask::filled(4, 4, 80)).unwrap();
    let reader = MaskStore::open(&dir);
    let mut cutout = Cutout::auto();
    cutout.feather = 0.0;
    assert_eq!(reader.resolved(0.0, &cutout, 1.0).unwrap().at(0, 0), 80);
    writer.put(0, &Mask::filled(4, 4, 180)).unwrap();
    assert_eq!(reader.mask_at(0.0).unwrap().at(0, 0), 180);
    assert_eq!(reader.resolved(0.0, &cutout, 1.0).unwrap().at(0, 0), 180);
    writer.clear();
}

#[test]
fn persisted_spatial_controls_change_resolved_not_raw_mask() {
    let dir = scratch("spatial");
    let mut writer = MaskStore::open(&dir);
    writer.clear();
    let raw = Mask::from_bytes(3, 3, vec![0, 0, 0, 0, 128, 0, 0, 0, 0]).unwrap();
    writer.put(0, &raw).unwrap();
    let mut cutout = Cutout::auto();
    cutout.feather = 0.0;
    assert_eq!(
        writer.resolved(0.0, &cutout, 1.0).unwrap().bytes(),
        raw.bytes()
    );
    cutout.expansion = 1;
    assert_eq!(
        writer.resolved(0.0, &cutout, 1.0).unwrap().bytes(),
        &[128; 9]
    );
    cutout.expansion = 0;
    cutout.edge_strength = 1.0;
    cutout.edge_radius = 1;
    let refined = writer.resolved(0.0, &cutout, 1.0).unwrap();
    assert!(refined.at(1, 1) < 128);
    assert_eq!(refined.at(0, 0), 0);
    assert_eq!(writer.mask_at(0.0).unwrap().bytes(), raw.bytes());
    writer.clear();
}
