//! The Origin PIC family (`.UH`, `.U`, `.UP`, `.P`, `.16`, `.X`) against the files the demo
//! ships, and against hand-built files for the writers' rules (`GRAB/LBM2U.C`, `GRAB/LBM16.C`,
//! `GRAB/LBM2P.C`, `GRAB/GLVP.C`).

mod common;

use std::path::PathBuf;

use common::{asset, asset_path, assets_with};
use rstest::rstest;
use sr_assets::{load, load_frames, load_path, raw_surface, sniff, AssetError, Format};

/// Every curated file under an extension this family uses. The extension is only a hint:
/// callers sniff (`LENS/LENS.EXB` is `Uh` data, `GRAB/BLOWFNT.1BM` a bare dump).
fn origin_assets() -> Vec<(PathBuf, Vec<u8>)> {
    assets_with(&["UH", "U", "UP", "P", "16", "X", "UX", "EXB", "1BM"])
        .into_iter()
        .map(|path| {
            let raw = std::fs::read(&path).unwrap();
            (path, raw)
        })
        .filter(|(_, raw)| sniff(raw) == Some(Format::Origin))
        .collect()
}

/// A hand-built `FCFC` raw file: the 16-byte header, `colors` RGB triples at 16,
/// `'X'` fill out to the paragraph boundary, then raw pixels. The offset
/// arithmetic is the writer's own (`LBM2U.C:57-67`).
fn fc_fc_raw(width: u16, height: u16, palette: &[[u8; 3]], pixels: &[u8]) -> Vec<u8> {
    let para = (16 + palette.len() * 3).div_ceil(16);
    let mut out = Vec::new();
    out.extend_from_slice(&0xFCFCu16.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&(palette.len() as u16).to_le_bytes());
    out.extend_from_slice(&(para as u16).to_le_bytes());
    out.extend_from_slice(&[0u8; 6]);
    for rgb in palette {
        out.extend_from_slice(rgb);
    }
    while out.len() < para * 16 {
        out.push(b'X');
    }
    out.extend_from_slice(pixels);
    out
}

/// One row of an RLE file, expanded here by the writer's rule (`LBM2P.C:74-94`).
fn expand_row(file: &[u8], pixels_at: usize, row: usize) -> Vec<u8> {
    let mut p = pixels_at;
    for _ in 0..row {
        let len = u16::from_le_bytes([file[p], file[p + 1]]) as usize;
        p += 2 + len;
    }
    let len = u16::from_le_bytes([file[p], file[p + 1]]) as usize;
    let ops = &file[p + 2..p + 2 + len];
    let mut out = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        let op = ops[i];
        i += 1;
        if op < 0x80 {
            out.push(op);
        } else {
            let run = (op & 0x7f) as usize;
            out.resize(out.len() + run, ops[i]);
            i += 1;
        }
    }
    out
}

/// A hand-built `FCFD` RLE file with one row, whose length prefix is the count of
/// opcode-stream bytes that follow (`LBM2P.C:72-94`).
fn fc_fd_rle(width: u16, height: u16, colors: u16, row_ops: &[u8]) -> Vec<u8> {
    let para = (16 + colors as usize * 3).div_ceil(16);
    let mut out = Vec::new();
    out.extend_from_slice(&0xFCFDu16.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&colors.to_le_bytes());
    out.extend_from_slice(&(para as u16).to_le_bytes());
    out.extend_from_slice(&[0u8; 6]);
    out.resize(16 + colors as usize * 3, 0);
    while out.len() < para * 16 {
        out.push(b'X');
    }
    for _ in 0..height {
        out.extend_from_slice(&(row_ops.len() as u16).to_le_bytes());
        out.extend_from_slice(row_ops);
    }
    out
}

/// One shipped file per variant. Only the magic separates them: `END/PIC.UH` is RLE under the
/// `.UH` extension, and `.X` is `saveunpheader` output renamed. Each decodes to more than one
/// value, which a misread stride or an empty palette lookup would not.
#[rstest]
#[case("ALKU/FONA.UH", 1500, 30, "FCFC raw")]
#[case("DDSTARS/TEXTS.16", 320, 200, "FCFC two bit-planes")]
#[case("BEG/SRTITLE.UP", 320, 400, "FCFD RLE")]
#[case("END/PIC.UH", 320, 400, "FCFD RLE under .UH")]
#[case("COMAN/COMBG.X", 320, 200, "UH, no palette")]
#[case("GLENZ/FC.UH", 320, 200, "Uh, 768-byte palette")]
fn each_variant_in_the_family_decodes(#[case] file: &str, #[case] w: u16, #[case] h: u16, #[case] what: &str) {
    assert_eq!(sniff(&asset(file)), Some(Format::Origin), "{what}");
    let img = load_path(&asset_path(file)).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!((img.width(), img.height()), (w, h), "{what}");
    assert_eq!(img.pixels().len(), w as usize * h as usize, "{what}");
    let mut values = img.pixels().to_vec();
    values.sort_unstable();
    values.dedup();
    assert!(values.len() > 1, "{what}: decoded to a single value");
}

/// Every Origin file in the tree decodes to the geometry its header claims, but one:
/// `COMAN/COMBG.UH`, the first 32,768 bytes of `COMAN/COMBG.X` (`asset-inventory.md` §D), is
/// refused — 16 header bytes plus 64,000 pixels wanted, 32,768 present.
#[test]
fn every_origin_asset_decodes_but_the_one_truncated_copy() {
    let mut decoded = 0;
    for (path, raw) in origin_assets() {
        if path.ends_with("COMAN/COMBG.UH") {
            let err = load(&raw, Format::Origin).unwrap_err();
            assert!(
                matches!(err, AssetError::Truncated { format: Format::Origin, needed: 64_016, had: 32_768 }),
                "{err}"
            );
            continue;
        }
        let img = load(&raw, Format::Origin).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(img.pixels().len(), img.width() as usize * img.height() as usize);
        decoded += 1;
    }
    assert_eq!(decoded, 14, "the curated Origin family");
}

/// The `Uh` palette is the file's own 768 bytes at 16 (`GLVP.C:1003`), copied verbatim.
#[test]
fn a_uh_file_carries_its_own_six_bit_palette() {
    let bytes = asset("GLENZ/FC.UH");
    let img = load(&bytes, Format::Origin).unwrap();
    for (i, rgb) in bytes[16..16 + 768].chunks_exact(3).enumerate() {
        assert_eq!(img.palette.entry(i as u8), [rgb[0], rgb[1], rgb[2]], "entry {i}");
    }
    assert_ne!(img.palette.entry(0), img.palette.entry(255), "palette was not read");
}

/// `LBM16.C` writes a fixed 16-colour palette and bit-planes, and `TEXTS.16` was
/// made with it at two planes (`DOTEXTS.BAT:1`).
#[test]
fn a_bit_plane_file_decodes_to_the_geometry_in_its_header() {
    let bytes = asset("DDSTARS/TEXTS.16");
    let img = load(&bytes, Format::Origin).unwrap();
    assert_eq!((img.width(), img.height()), (320, 200));
    assert_eq!(img.pixels().len(), 64_000);

    assert!(
        img.pixels().iter().all(|&p| p < 4),
        "a plane count above 2 would show here"
    );

    // Row 11, the first that is not flat, from the file's own plane bytes: plane 0 first, the
    // most significant bit leftmost (`LBM16.C:76-92`).
    let pixels_at = 4 * 16; // `para` = 4, as `LBM16.C:61-67` computes it for colors=16
    let planes = &bytes[pixels_at..];
    let plane_row = 320 / 8;
    for x in 0..40usize {
        let byte = x / 8;
        let bit = 7 - (x % 8);
        let low = (planes[11 * 2 * plane_row + byte] >> bit) & 1;
        let high = (planes[11 * 2 * plane_row + plane_row + byte] >> bit) & 1;
        assert_eq!(
            img.surface.get(x, 11),
            Some(low | (high << 1)),
            "row 11, x = {x}"
        );
    }
}

/// A bare dump has no magic and no geometry: the caller supplies it, `bytes.len()` must be
/// `w * h`, and nothing sniffs it from its size (`PANIC/MONSTER.U`, `LENS/LENS.U` and
/// `HARD/FCKOE3.U` are all 64,000 bytes).
#[test]
fn a_headerless_dump_takes_its_geometry_from_the_caller() {
    let raw = asset("PANIC/MONSTER.U");
    assert_eq!(sniff(&raw), None);
    assert!(matches!(load_path(&asset_path("PANIC/MONSTER.U")), Err(AssetError::UnknownFormat)));
    let img = raw_surface(&raw, 320, 200).unwrap();
    assert_eq!((img.width(), img.height()), (320, 200));
    assert!(matches!(raw_surface(&raw, 320, 199), Err(AssetError::Truncated { .. })));
    assert!(matches!(load(&raw, Format::Origin), Err(AssetError::UnknownFormat)));
    assert!(matches!(load(&raw, Format::OriginBare), Err(AssetError::Unsupported { .. })));
}

#[test]
fn an_oversized_header_is_refused_rather_than_clipped() {
    let mut bytes = asset("BEG/SRTITLE.UP");
    bytes[2..4].copy_from_slice(&1024u16.to_le_bytes());
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::TooLarge { .. })));
}

/// `START/HZPIC.P` has 153 of its 400 rows decoding wider than its 640; the port keeps the
/// first 640 pixels of each (APPROXIMATIONS A2).
#[test]
fn a_run_row_wider_than_the_header_is_clipped_not_refused() {
    let bytes = asset("START/HZPIC.P");
    let img = load(&bytes, Format::Origin).unwrap();
    assert_eq!((img.width(), img.height()), (640, 400));
    assert_eq!(img.pixels().len(), 640 * 400);

    // Row 302 is 757 pixels wide and its head differs from its tail.
    let full = expand_row(&bytes, 0x31 * 16, 302);
    assert!(full.len() > 640, "row 302 must be over-wide for this test to mean anything");
    let row = img.surface.row(302);
    assert_eq!(row, &full[..640], "the clip keeps the head of the row");
    assert_ne!(row, &full[full.len() - 640..], "…and not its tail");
    assert!(row.iter().any(|&p| p != 0), "nor is the row zero-filled");
}

/// The family's palettes are six bits (`LBM2U.C:170`, `GLVP.C:1003`), so 64 is refused.
#[test]
fn a_palette_byte_above_six_bits_is_refused_not_clamped() {
    let bytes = fc_fc_raw(2, 1, &[[0, 0, 0], [64, 0, 0]], &[0, 1]);
    assert!(
        matches!(load(&bytes, Format::Origin), Err(AssetError::Palette { .. })),
        "a 64 in the palette is not a six-bit value"
    );
}

/// `0x80`, on which `ENDPIC/READP.C:44-51` and `START/READP.C:37-38` disagree, is refused.
#[test]
fn a_run_opcode_of_zerox80_is_refused() {
    let bytes = fc_fd_rle(1, 1, 1, &[0x80]);
    assert!(matches!(
        load(&bytes, Format::Origin),
        Err(AssetError::Unsupported { .. })
    ));
}

/// Every run length the format can express is decoded, including the 2 an earlier `LBM2P`
/// emitted (`START/HZPIC.P` has such runs) and the 127 the checked-in writer caps at.
#[test]
fn every_run_length_from_one_to_127_is_decoded() {
    // A run of 1, a literal, a run of 2 and a run of 127.
    let bytes = fc_fd_rle(1 + 1 + 2 + 127, 1, 1, &[0x81, 7, 0x40, 0x82, 9, 0xFF, 11]);
    let img = load(&bytes, Format::Origin).unwrap();
    let mut want = vec![7u8];
    want.push(0x40);
    want.extend_from_slice(&[9u8; 2]);
    want.extend_from_slice(&[11u8; 127]);
    assert_eq!(img.pixels(), &want[..]);
}

/// Every writer's `para` points past the palette (`LBM2U.C:57`), so one inside it is refused.
#[test]
fn a_pixel_offset_inside_the_palette_is_refused() {
    let mut bytes = fc_fc_raw(2, 1, &[[0, 0, 0], [1, 2, 3]], &[0, 1]);
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes()); // para 1 -> offset 16
    assert!(matches!(
        load(&bytes, Format::Origin),
        Err(AssetError::Unsupported { .. })
    ));
}

/// Every loader returns an `AssetError` rather than panicking or reading past its input: each
/// real file is cut at ~200 evenly spread offsets. A cut may decode only as a bit-plane file
/// with fewer whole planes, a smaller picture in the same format.
#[test]
fn no_prefix_of_an_origin_asset_panics_or_decodes_silently() {
    let (mut checked, mut refused) = (0, 0);
    for (path, raw) in origin_assets() {
        let step = (raw.len() / 200).max(1);
        for cut in (0..raw.len()).step_by(step) {
            match load(&raw[..cut], Format::Origin) {
                Ok(img) => assert_eq!(
                    img.pixels().len(),
                    img.width() as usize * img.height() as usize,
                    "{} cut to {cut}",
                    path.display()
                ),
                Err(_) => refused += 1,
            }
            checked += 1;
        }
    }
    assert!(checked > 1_000, "made {checked} cuts");
    assert!(refused * 10 > checked * 9, "only {refused} of {checked} cuts refused");
}

/// A still container is one frame.
#[rstest]
#[case("GLENZ/FC.UH", Format::Origin, (320, 200))]
#[case("BEG/SRTITLE.LBM", Format::Iff, (320, 400))]
#[case("WATER/BKG.CLX", Format::Rix3, (320, 200))]
fn a_still_image_loads_as_one_frame(#[case] file: &str, #[case] format: Format, #[case] size: (u16, u16)) {
    let frames = load_frames(&asset(file), format).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!((frames[0].width(), frames[0].height()), size);
}
