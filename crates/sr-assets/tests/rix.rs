//! RIX3 (`.CLX`, `.SCI`) against the four files the demo ships and the parts that read them:
//! palette at 10, pixels at 778 (`WATER/KOE.PAS:35-40`, `FOREST/READ2.PAS:110-121`,
//! `WATER/DEMO.PAS:108-109`).

mod common;

use std::path::PathBuf;

use common::{asset, assets_with};
use rstest::rstest;
use sr_assets::{load, sniff, AssetError, Format};

const PIXELS_AT: usize = 778;

fn rix3_assets() -> Vec<(PathBuf, Vec<u8>)> {
    let files: Vec<_> = assets_with(&["CLX", "SCI"])
        .into_iter()
        .map(|path| {
            let raw = std::fs::read(&path).unwrap();
            (path, raw)
        })
        .filter(|(_, raw)| sniff(raw) == Some(Format::Rix3))
        .collect();
    assert_eq!(files.len(), 4, "the curated tree carries four RIX3 files");
    files
}

/// The header read here, not by the decoder: width, height, palette type, storage type.
fn header_of(file: &[u8]) -> (u16, u16, u8, u8) {
    (u16::from_le_bytes([file[4], file[5]]), u16::from_le_bytes([file[6], file[7]]), file[8], file[9])
}

/// Every RIX3 file decodes to the header's width, the rows it carries, its own pixel bytes and
/// its own palette bytes. Three fill their canvas; `WATER/MIEKKA.SCI` declares 400x250 and
/// carries 35 rows, the strip `WATER/DEMO.PAS:109` reads `400*34` bytes of.
#[test]
fn every_rix3_asset_decodes_to_its_own_bytes() {
    let mut cropped = Vec::new();
    for (path, raw) in rix3_assets() {
        let (w, h, palette_type, storage) = header_of(&raw);
        assert_eq!((palette_type, storage), (0xAF, 0x00), "{}", path.display());
        let img = load(&raw, Format::Rix3).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let payload = raw.len() - PIXELS_AT;
        assert_eq!(img.width(), w, "{}", path.display());
        assert_eq!(img.height() as usize * w as usize, payload, "{}: whole rows", path.display());
        assert_eq!(img.pixels(), &raw[PIXELS_AT..], "{}", path.display());
        for (i, rgb) in raw[10..PIXELS_AT].chunks_exact(3).enumerate() {
            assert_eq!(img.palette.entry(i as u8), [rgb[0], rgb[1], rgb[2]], "{}: entry {i}", path.display());
        }
        if img.height() != h {
            cropped.push((path.file_name().unwrap().to_owned(), img.width(), img.height()));
        }
    }
    assert_eq!(cropped, [("MIEKKA.SCI".into(), 400, 35)]);
}

#[test]
fn a_palette_byte_above_six_bits_is_refused() {
    let mut raw = asset("WATER/BKG.CLX");
    raw[10 + 3] = 64;
    assert!(matches!(load(&raw, Format::Rix3), Err(AssetError::Palette { .. })));
}

/// Every RIX3 file in either tree carries `AF 00`; any other palette or storage type is refused.
#[rstest]
#[case(8, 0x00)]
#[case(8, 0x8F)]
#[case(9, 0x80)]
#[case(9, 0x40)]
#[case(9, 0x01)]
fn a_header_type_byte_this_decoder_cannot_read_is_refused(#[case] at: usize, #[case] value: u8) {
    let mut raw = asset("WATER/BKG.CLX");
    raw[at] = value;
    let err = load(&raw, Format::Rix3).unwrap_err();
    assert!(matches!(err, AssetError::Unsupported { format: Format::Rix3, .. }), "{err}");
}

/// The byte wanted and the byte present, for a file cut inside its header, inside its palette,
/// part-way through row 199 (wanting that row's end), and one byte longer than its canvas.
#[rstest]
#[case(7, 10)]
#[case(500, 778)]
#[case(64_678, 64_778)]
#[case(64_779, 64_778)]
fn a_file_that_is_not_whole_rows_of_its_canvas_is_refused(#[case] len: usize, #[case] needed: usize) {
    let mut raw = asset("WATER/BKG.CLX");
    assert_eq!(raw.len(), 64_778);
    raw.resize(len, 0);
    let err = load(&raw, Format::Rix3).unwrap_err();
    assert!(
        matches!(err, AssetError::Truncated { format: Format::Rix3, needed: n, had } if n == needed && had == len),
        "{err}"
    );
}

#[test]
fn an_oversized_canvas_is_refused_rather_than_clipped() {
    let mut raw = asset("WATER/BKG.CLX");
    raw[4..6].copy_from_slice(&1024u16.to_le_bytes());
    raw[6..8].copy_from_slice(&400u16.to_le_bytes());
    assert!(matches!(load(&raw, Format::Rix3), Err(AssetError::TooLarge { width: 1024, height: 400 })));
}

#[test]
fn a_file_that_is_not_rix3_is_refused() {
    let mut bytes = asset("WATER/BKG.CLX");
    bytes[0..4].copy_from_slice(b"RIX4");
    assert_eq!(sniff(&bytes), None);
    assert!(matches!(load(&bytes, Format::Rix3), Err(AssetError::UnknownFormat)));
    assert_eq!(sniff(b"RIX3"), Some(Format::Rix3));
}

/// Every prefix is refused, or — cut on a row boundary — decodes to whole rows of the file's own
/// bytes; never a padded row or a panic.
#[test]
fn no_prefix_of_a_rix3_asset_decodes_to_anything_but_the_files_own_rows() {
    let (mut checked, mut refused, mut decoded) = (0, 0, 0);
    for (path, raw) in rix3_assets() {
        let step = (raw.len() / 200).max(1);
        for cut in (0..raw.len()).step_by(step) {
            match load(&raw[..cut], Format::Rix3) {
                Ok(img) => {
                    assert_eq!(img.pixels(), &raw[PIXELS_AT..cut], "{} cut to {cut}", path.display());
                    decoded += 1;
                }
                Err(_) => refused += 1,
            }
            checked += 1;
        }
    }
    assert!(checked > 700 && decoded > 0, "{checked} cuts, {decoded} decoded");
    assert!(refused * 10 > checked * 9, "only {refused} of {checked} cuts refused");
}
