//! IFF (`ILBM` and `PBM `) against the files the demo ships, against their converted twins, and
//! against hand-built files for the toolchain reader's rules (`GRAB/LOADLBM.C:92-263`).

mod common;

use std::path::PathBuf;

use common::{asset, asset_path, assets_with, iff_chunk};
use rstest::rstest;
use sr_assets::{load, load_path, sniff, AssetError, Format};

/// Every curated IFF file. The extension is a hint (`GRAB/BLOWFNT.1BM` is a bare dump), so the
/// magic decides.
fn iff_assets() -> Vec<(PathBuf, Vec<u8>)> {
    assets_with(&["LBM", "1BM", "BBM", "BBK"])
        .into_iter()
        .map(|path| {
            let raw = std::fs::read(&path).unwrap();
            (path, raw)
        })
        .filter(|(_, raw)| sniff(raw) == Some(Format::Iff))
        .collect()
}

fn be16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

/// One IFF chunk: id, big-endian length, payload, and a pad byte when the payload is odd.
fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(id);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// A chunk whose declared length counts its own pad byte, as `CREDITS/PICS/PIC01.LBM`'s 334-byte
/// `CMAP` (111 triples and a zero) does.
fn chunk_counting_pad(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = chunk(id, payload);
    if payload.len() % 2 == 1 {
        out[4..8].copy_from_slice(&(payload.len() as u32 + 1).to_be_bytes());
    }
    out
}

/// A whole `FORM`, its size the length less eight, as every file in the tree writes it.
fn form(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(kind);
    for c in chunks {
        out.extend_from_slice(c);
    }
    let size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&size.to_be_bytes());
    out
}

/// The 20-byte picture header (`LOADLBM.C:123-136`: w, h, x, y, nPlanes,
/// masking, compression, pad, transparent, xAspect, yAspect, pageW, pageH).
fn bmhd(w: u16, h: u16, planes: u8, masking: u8, compression: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&w.to_be_bytes());
    out.extend_from_slice(&h.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // x
    out.extend_from_slice(&0u16.to_be_bytes()); // y
    out.push(planes);
    out.push(masking);
    out.push(compression);
    out.push(0); // pad
    out.extend_from_slice(&0u16.to_be_bytes()); // transparent colour
    out.push(1);
    out.push(1); // aspect
    out.extend_from_slice(&w.to_be_bytes()); // page
    out.extend_from_slice(&h.to_be_bytes());
    out
}

/// `file` with `extra` spliced in after its `BMHD` (first in every file), the `FORM` size
/// corrected.
fn with_chunks_after_bmhd(file: &[u8], extra: &[u8]) -> Vec<u8> {
    assert_eq!(&file[12..16], b"BMHD", "this helper needs BMHD first");
    let size = u32::from_be_bytes([file[16], file[17], file[18], file[19]]) as usize;
    let at = 12 + 8 + size + (size & 1);
    let mut out = file[..at].to_vec();
    out.extend_from_slice(extra);
    out.extend_from_slice(&file[at..]);
    let form_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&form_size.to_be_bytes());
    out
}

/// `file` with its `BODY` cut to `keep` bytes and the container rewritten to match, so the
/// shortfall is the picture's.
fn with_short_body(file: &[u8], keep: usize) -> Vec<u8> {
    let (at, body) = iff_chunk(file, b"BODY");
    let size = body.len();
    let keep = keep.min(size);
    let mut out = file[..at + keep].to_vec();
    out[at - 4..at].copy_from_slice(&(keep as u32).to_be_bytes());
    let form_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&form_size.to_be_bytes());
    out
}

/// A 4x2, two-plane `ILBM` built here. A plane row is 2 bytes (`((4 + 15) / 16) * 2`), each one
/// literal opcode (`LOADLBM.C:217-231`), most significant bit leftmost. Row 0 is 0,1,2,3 (bit 0
/// `0x50`, bit 1 `0x30`), row 1 is 3,2,1,0 (`0xA0`, `0xC0`). `83 / 4` is 20 (`LBM2U.C:170`).
#[test]
fn a_hand_built_planar_ilbm_decodes_to_its_known_pixels() {
    let palette = [[0, 0, 0], [255, 255, 255], [83, 3, 7], [255, 0, 0]];
    let mut cmap = Vec::new();
    for rgb in palette {
        cmap.extend_from_slice(&rgb);
    }
    let body = [
        0x01, 0x50, 0x00, // row 0, plane 0
        0x01, 0x30, 0x00, // row 0, plane 1
        0x01, 0xA0, 0x00, // row 1, plane 0
        0x01, 0xC0, 0x00, // row 1, plane 1
    ];
    let file = form(
        b"ILBM",
        &[chunk(b"BMHD", &bmhd(4, 2, 2, 0, 1)), chunk(b"CMAP", &cmap), chunk(b"BODY", &body)],
    );

    let img = load(&file, Format::Iff).unwrap();
    assert_eq!((img.width(), img.height()), (4, 2));
    assert_eq!(img.pixels(), &[0, 1, 2, 3, 3, 2, 1, 0]);
    assert_eq!(img.palette.entry(1), [63, 63, 63]);
    assert_eq!(img.palette.entry(2), [20, 0, 1]);
    assert_eq!(img.palette.entry(3), [63, 0, 0]);
}

/// The chunky files at the three widths the tree uses, each more than one value.
#[rstest]
#[case("ALKU/FONA.LBM", 1500, 30)]
#[case("GLENZ/FC.LBM", 320, 200)]
#[case("START/HZPIC.LBM", 640, 400)]
#[case("BEG/SRTITLE.LBM", 320, 400)]
fn a_pbm_decodes_to_the_dimensions_in_its_header(
    #[case] path: &str,
    #[case] w: u16,
    #[case] h: u16,
) {
    let raw = asset(path);
    assert_eq!(sniff(&raw), Some(Format::Iff));
    assert_eq!(&raw[8..12], b"PBM ", "the chunky type, not ILBM");
    let img = load_path(&asset_path(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!((img.width(), img.height()), (w, h));
    assert_eq!(img.pixels().len(), w as usize * h as usize);
    let mut values = img.pixels().to_vec();
    values.sort_unstable();
    values.dedup();
    assert!(values.len() > 1, "{path}: decoded to a single value");
}

/// The title the demo shows (`BEG/SRTITLE.UP`, linked by `BEG/MAKEFILE:21`) and the Deluxe Paint
/// original it was converted from are the same 128,000 pixels; the planar `VISU/C/U2ABG.LBM` and
/// its `.UH` are the same 64,000 pixels and palette. Of the thirteen such pairs in the tree, the
/// two that differ are the converters' doing (`DDSTARS/DOTEXTS.BAT:1`'s sixteen colours, and
/// `START/HZPIC.P`'s clipped rows, A2).
#[rstest]
#[case("BEG/SRTITLE.LBM", "BEG/SRTITLE.UP", b"PBM ", (320, 400))]
#[case("VISU/C/U2ABG.LBM", "VISU/C/U2ABG.UH", b"ILBM", (320, 200))]
fn the_iff_art_and_its_converted_twin_are_the_same_picture(
    #[case] lbm_file: &str,
    #[case] twin_file: &str,
    #[case] kind: &[u8; 4],
    #[case] size: (u16, u16),
) {
    assert_eq!(&asset(lbm_file)[8..12], kind);
    let lbm = load_path(&asset_path(lbm_file)).unwrap();
    let twin = load_path(&asset_path(twin_file)).unwrap();
    assert_eq!((lbm.width(), lbm.height()), size);
    assert_eq!((twin.width(), twin.height()), size);
    assert_eq!(lbm.pixels(), twin.pixels());
    assert_eq!(lbm.palette, twin.palette);
}

/// The palette rule against the toolchain's own output: each twin's stored palette (at 16) is the
/// `CMAP` divided by four, byte for byte (`LBM2U.C:170`, `LBM2P.C:195`), and the two decoders agree.
#[rstest]
#[case("LENS/LENSPIC.LBM", "LENS/LENS.EXB")]
#[case("BEG/SRTITLE.LBM", "BEG/SRTITLE.UP")]
fn the_palette_is_the_originals_own_truncation_of_the_eight_bit_cmap(#[case] lbm_file: &str, #[case] twin_file: &str) {
    let raw = asset(lbm_file);
    let (_, cmap) = iff_chunk(&raw, b"CMAP");
    assert_eq!(cmap.len(), 768);
    let twin_raw = asset(twin_file);
    for i in 0..768 {
        assert_eq!(twin_raw[16 + i], cmap[i] / 4, "byte {i} of the twin's palette");
    }
    let lbm = load_path(&asset_path(lbm_file)).unwrap();
    let twin = load_path(&asset_path(twin_file)).unwrap();
    assert_eq!(lbm.palette, twin.palette);
}

/// Every curated IFF file (37: 24 planar, 13 chunky) decodes to the geometry its own `BMHD`
/// claims, read here big-endian (`LOADLBM.C:84-90`), and to more than one value. Every curated
/// `ILBM` uses its top plane, so a lost or misordered plane shows.
#[test]
fn every_iff_asset_decodes_and_fills_the_geometry_its_bmhd_claims() {
    let (mut planar, mut chunky) = (0, 0);
    for (path, raw) in iff_assets() {
        let (_, header) = iff_chunk(&raw, b"BMHD");
        let (w, h, planes) = (be16(header, 0), be16(header, 2), header[8]);
        let img = load(&raw, Format::Iff).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!((img.width(), img.height()), (w, h), "{}", path.display());
        let mut values = img.pixels().to_vec();
        values.sort_unstable();
        values.dedup();
        assert!(values.len() > 1, "{}: decoded to a single value", path.display());
        if &raw[8..12] == b"ILBM" {
            planar += 1;
            assert!(*values.last().unwrap() >= 1 << (planes - 1), "{}: top plane unused", path.display());
        } else {
            chunky += 1;
        }
    }
    assert_eq!((planar, chunky), (24, 13), "the curated tree's own tally");
}

/// The opcode that fills the last row ends the stream exactly where the `BODY` does; nothing past
/// it is read (`while(lp < xsz)`, `LOADLBM.C:161-179`).
#[test]
fn a_run_that_ends_exactly_at_the_row_boundary_is_not_a_truncation() {
    // 4 x 1: a run of two (0xFF) and a literal of two (0x01), filling the row
    // with the last byte of the stream.
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(4, 1, 8, 0, 1)),
            chunk(b"BODY", &[0xFF, 0x11, 0x01, 0x22, 0x33]),
        ],
    );
    let img = load(&file, Format::Iff).unwrap();
    assert_eq!(img.pixels(), &[0x11, 0x11, 0x22, 0x33]);
}

/// A run longer than the row has left is consumed whole and its excess dropped
/// (`LOADLBM.C:181`), so the next row starts after the opcode.
#[test]
fn a_run_op_decodes_to_the_repeated_byte_and_the_row_ends_where_the_op_does() {
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(8, 2, 8, 0, 1)),
            chunk(
                b"BODY",
                &[
                    0xFF, 0x2A, // a run of two
                    0x01, 0x00, 0x00, // a literal of two
                    0x83, 0x7F, // a run of 126 into a row with four bytes left
                    0xFF, 0x11, 0xFF, 0x11, 0xFF, 0x11, 0xFF, 0x11, // row 1
                ],
            ),
        ],
    );
    let img = load(&file, Format::Iff).unwrap();
    assert_eq!(
        img.pixels(),
        &[0x2A, 0x2A, 0x00, 0x00, 0x7F, 0x7F, 0x7F, 0x7F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11]
    );
}

/// The same for a literal: twelve bytes into a row with seven left (`LOADLBM.C:171-178`).
#[test]
fn a_literal_longer_than_the_row_ends_the_row_it_fills() {
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(8, 2, 8, 0, 1)),
            chunk(
                b"BODY",
                &[
                    0x00, 0x2A, // a literal of one
                    0x0B, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
                    0x1C, // a literal of twelve, seven of which fit
                    0xFF, 0x22, 0xFF, 0x22, 0xFF, 0x22, 0xFF, 0x22, // row 1
                ],
            ),
        ],
    );
    let img = load(&file, Format::Iff).unwrap();
    assert_eq!(
        img.pixels(),
        &[0x2A, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22]
    );
}

/// A self-consistent container whose `BODY` holds one of two promised rows: `needed` and `had`
/// name the byte past the `BODY`'s end and its end.
#[test]
fn a_body_shorter_than_bmhd_promises_is_refused() {
    // Two rows promised, one row's stream stored.
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(4, 2, 8, 0, 1)),
            chunk(b"BODY", &[0xFF, 0x11, 0x01, 0x22, 0x33]),
        ],
    );
    let (at, body) = iff_chunk(&file, b"BODY");
    let body_end = at + body.len();
    assert!(
        matches!(
            load(&file, Format::Iff),
            Err(AssetError::Truncated { format: Format::Iff, needed, had })
                if needed == body_end + 1 && had == body_end
        ),
        "expected a Truncated naming the row stream's end"
    );
}

/// Deluxe Paint's colour-range and tool chunks change no pixel and are skipped
/// (`LOADLBM.C:139-150`), as is an unknown, odd-sized one.
#[test]
fn unknown_chunks_are_skipped_not_fatal() {
    let raw = asset("GLENZ/FC.LBM");
    let before = load(&raw, Format::Iff).unwrap();

    let extra = [
        chunk(b"CRNG", &[0, 0, 1, 2, 0, 3, 4, 5]),
        chunk(b"XYZW", &[1, 2, 3, 4, 5]), // five bytes: odd, so padded
    ];
    let spliced = with_chunks_after_bmhd(&raw, &extra.concat());
    assert_eq!(sniff(&spliced), Some(Format::Iff));
    let after = load(&spliced, Format::Iff).unwrap();
    assert_eq!((after.width(), after.height()), (320, 200));
    assert_eq!(before.pixels(), after.pixels());
    assert_eq!(before.palette.entry(7), after.palette.entry(7));
}

/// Constructs no curated file carries, refused rather than guessed at: a mask plane (`masking` 1
/// or 2, which `LOADLBM.C:234-251` reads into the row stream), a `compression` other than
/// ByteRun1, a plane count that cannot index a byte, and a container with no `BMHD` or no `BODY`.
#[rstest]
#[case("masking 1", form(b"ILBM", &[chunk(b"BMHD", &bmhd(4, 2, 2, 1, 1)), chunk(b"BODY", &[0x01, 0x50, 0x00])]))]
#[case("masking 2", form(b"ILBM", &[chunk(b"BMHD", &bmhd(4, 2, 2, 2, 1)), chunk(b"BODY", &[0x01, 0x50, 0x00])]))]
#[case("compression 0", form(b"PBM ", &[chunk(b"BMHD", &bmhd(4, 1, 8, 0, 0)), chunk(b"BODY", &[1, 2, 3, 4])]))]
#[case("0 planes", form(b"ILBM", &[chunk(b"BMHD", &bmhd(4, 1, 0, 0, 1)), chunk(b"BODY", &[0x01, 0x11, 0x22])]))]
#[case("9 planes", form(b"ILBM", &[chunk(b"BMHD", &bmhd(4, 1, 9, 0, 1)), chunk(b"BODY", &[0x01, 0x11, 0x22])]))]
#[case("no BMHD", form(b"ILBM", &[chunk(b"CMAP", &[1, 2, 3]), chunk(b"BODY", &[0x42])]))]
#[case("no BODY", form(b"ILBM", &[chunk(b"BMHD", &bmhd(4, 1, 2, 0, 1)), chunk(b"CMAP", &[1, 2, 3])]))]
fn a_construct_no_curated_file_carries_is_refused(#[case] what: &str, #[case] file: Vec<u8>) {
    assert!(matches!(load(&file, Format::Iff), Err(AssetError::Unsupported { .. })), "{what}");
}

/// `CAMG`'s EHB (0x80) and HAM (0x800) bits make an index not its colour, and are refused; other
/// modes decode. No file in either tree carries a `CAMG`.
#[rstest]
#[case(0x80, true)]
#[case(0x800, true)]
#[case(0x0000, false)]
#[case(0x1004, false)]
fn an_ehb_or_ham_display_mode_is_refused(#[case] mode: u32, #[case] refused: bool) {
    let raw = asset("GLENZ/FC.LBM");
    let file = with_chunks_after_bmhd(&raw, &chunk(b"CAMG", &mode.to_be_bytes()));
    assert_eq!(sniff(&file), Some(Format::Iff));
    match (load(&file, Format::Iff), refused) {
        (Err(AssetError::Unsupported { .. }), true) => {}
        (Ok(img), false) => assert_eq!((img.width(), img.height()), (320, 200)),
        (got, want) => panic!("CAMG {mode:#x}: wanted refused={want}, got {got:?}"),
    }
}

/// The chunky branch never reads the plane count (`LOADLBM.C:151-185`).
#[test]
fn a_chunky_file_ignores_its_plane_count_like_the_original_does() {
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(4, 1, 0, 0, 1)),
            chunk(b"BODY", &[0x01, 0x11, 0x22, 0x01, 0x33, 0x44]),
        ],
    );
    assert_eq!(load(&file, Format::Iff).unwrap().pixels(), &[0x11, 0x22, 0x33, 0x44]);
}

/// The engine bound is an area: 1024x400 is refused before anything is allocated.
#[test]
fn an_image_too_large_for_the_engine_is_refused() {
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(1024, 400, 8, 0, 1)),
            chunk(b"BODY", &[0]),
        ],
    );
    assert!(
        matches!(
            load(&file, Format::Iff),
            Err(AssetError::TooLarge { width: 1024, height: 400 })
        ),
        "a 1024x400 header is 409,600 pixels"
    );
}

/// A `CMAP` whose size counts its pad byte is `size / 3` entries; the pad is no entry.
#[test]
fn a_cmap_that_counts_its_pad_byte_in_the_size_reads_its_entries() {
    let mut cmap = vec![10, 20, 30, 40, 50, 60, 70, 80, 90];
    cmap.push(0); // the pad, counted in the size
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(2, 1, 8, 0, 1)),
            chunk_counting_pad(b"CMAP", &cmap),
            chunk_counting_pad(b"BODY", &[0x01, 0x01, 0x00]), // four bytes, one row of two
        ],
    );
    let img = load(&file, Format::Iff).unwrap();
    assert_eq!(img.pixels(), &[0x01, 0x00]);
    assert_eq!(img.palette.entry(0), [2, 5, 7], "10/4, 20/4, 30/4");
    assert_eq!(img.palette.entry(2), [17, 20, 22], "70/4, 80/4, 90/4");
    // 334 / 3 = 111 entries for `PIC01.LBM`; here it is 3, and the pad bought no
    // fourth.
    assert_eq!(img.palette.entry(3), [0, 0, 0], "the pad must not become an entry");
}

/// A `FORM` that ends before its last, odd chunk's pad byte is refused; the padded form of the
/// same file, as the writers make it, decodes.
#[test]
fn a_form_that_stops_before_its_last_chunks_pad_byte_is_refused() {
    // A one-row file whose BODY is three bytes: odd, so the writers would have
    // written a fourth (the pad) and counted it in the chunk's size.
    let bmhd_chunk = chunk(b"BMHD", &bmhd(2, 1, 8, 0, 1));
    let body_id = 12 + bmhd_chunk.len();
    let mut unpadded = form(b"PBM ", &[bmhd_chunk]);
    unpadded.extend_from_slice(b"BODY");
    unpadded.extend_from_slice(&3u32.to_be_bytes());
    unpadded.extend_from_slice(&[0xFF, 0x11, 0x01]); // a run of two and a literal
    let form_size = (unpadded.len() - 8) as u32;
    unpadded[4..8].copy_from_slice(&form_size.to_be_bytes());
    assert!(
        matches!(load(&unpadded, Format::Iff), Err(AssetError::Unsupported { .. })),
        "a FORM that ends before its last chunk's pad byte does not describe its own bytes"
    );

    // The same file as the writers make it: a four-byte chunk, the pad counted
    // in the size, and the FORM's size even.
    let mut padded = unpadded.clone();
    padded.push(0);
    padded[body_id + 4..body_id + 8].copy_from_slice(&4u32.to_be_bytes());
    let form_size = (padded.len() - 8) as u32;
    padded[4..8].copy_from_slice(&form_size.to_be_bytes());
    assert_eq!(load(&padded, Format::Iff).unwrap().pixels(), &[0x11, 0x11]);
}

/// Bytes after the last chunk that the `FORM` counts are not a chunk, and are refused.
#[test]
fn a_form_size_that_promises_bytes_that_are_not_a_chunk_is_refused() {
    let raw = asset("GLENZ/FC.LBM");
    assert!(load(&raw, Format::Iff).is_ok());

    // Two bytes appended to the file and counted by the `FORM`: not a chunk
    // header, and not a chunk's payload either.
    let mut extended = raw.clone();
    extended.extend_from_slice(&[0, 0]);
    let form_size = (extended.len() - 8) as u32;
    extended[4..8].copy_from_slice(&form_size.to_be_bytes());
    assert!(matches!(
        load(&extended, Format::Iff),
        Err(AssetError::Unsupported { .. })
    ));
}

/// A chunk whose size runs past the `FORM` is truncated at its claimed end.
#[test]
fn a_chunk_that_claims_more_than_the_form_holds_is_refused() {
    let raw = asset("GLENZ/FC.LBM");
    assert!(load(&raw, Format::Iff).is_ok());

    // The BMHD (first chunk, 20 bytes at 12) is told it is as long as the whole
    // file: its payload then ends past the FORM, and there is no reading of the
    // container that makes both numbers true.
    let mut inflated = raw.clone();
    let claimed = inflated.len() as u32;
    inflated[16..20].copy_from_slice(&claimed.to_be_bytes());
    assert!(
        matches!(
            load(&inflated, Format::Iff),
            Err(AssetError::Truncated { format: Format::Iff, needed, had })
                if needed == 20 + raw.len() && had == raw.len()
        ),
        "expected a Truncated naming the chunk's claimed end and the file's length"
    );
}

/// Every loader returns an `AssetError` and never reads past its input. A raw prefix is refused
/// by the `FORM` size alone, so always as `Truncated`; a `BODY` cut short inside a consistent
/// container has to be caught by the row walk, or decode a complete picture.
#[test]
fn no_prefix_of_an_iff_asset_panics_or_decodes_silently() {
    let (mut assets, mut cuts, mut refused) = (0, 0, 0);
    for (path, raw) in iff_assets() {
        assets += 1;

        let step = (raw.len() / 12).max(1);
        for cut in (0..raw.len()).step_by(step) {
            match load(&raw[..cut], Format::Iff) {
                Err(AssetError::Truncated { .. }) => refused += 1,
                Err(e) => panic!("{} cut to {cut}: expected Truncated, got {e}", path.display()),
                Ok(_) => panic!("{} cut to {cut}: a prefix must not decode", path.display()),
            }
            cuts += 1;
        }

        let body_size = iff_chunk(&raw, b"BODY").1.len();
        let step = (body_size / 12).max(1);
        for keep in (0..body_size).step_by(step) {
            let cut = with_short_body(&raw, keep);
            match load(&cut, Format::Iff) {
                Ok(img) => {
                    assert_eq!(
                        img.pixels().len(),
                        img.width() as usize * img.height() as usize,
                        "{} with a {keep}-byte BODY decoded short",
                        path.display()
                    );
                }
                Err(_) => refused += 1,
            }
            cuts += 1;
        }
    }
    assert_eq!(assets, 37, "every curated IFF asset is swept");
    assert!(cuts > 500, "expected a lot of cuts, made {cuts}");
    assert!(refused * 10 > cuts * 9, "only {refused} of {cuts} cuts refused");
}

/// `load` checks the magic even when told the format.
#[test]
fn load_refuses_bytes_that_are_not_iff() {
    let origin = asset("BEG/SRTITLE.UP");
    assert!(matches!(load(&origin, Format::Iff), Err(AssetError::UnknownFormat)));
    assert!(matches!(load(b"FORM", Format::Iff), Err(AssetError::Truncated { .. })));
    let not_an_iff = {
        let mut f = asset("BEG/SRTITLE.LBM");
        f[8..12].copy_from_slice(b"ANIM"); // a FORM, but not a picture type
        f
    };
    assert!(matches!(load(&not_an_iff, Format::Iff), Err(AssetError::Unsupported { .. })));
}
