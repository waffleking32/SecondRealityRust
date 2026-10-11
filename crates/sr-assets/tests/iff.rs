//! IFF — `ILBM` and `PBM `, both flavours the curated tree ships — pinned
//! against the files the demo shipped.
//!
//! Every file these tests read is a curated copy under `assets/`, verified
//! byte-for-byte against `upstream/SecondReality/` at commit `071a82e` by task
//! A1. The format itself was read off the readers the toolchain uses: the three
//! copies of one `loadlbm2` in `GRAB/LOADLBM.C:92-263`, `GRAB/LBM2U.C:161-336`
//! and `GRAB/VIEWLBM.C:148-323`, and the writers that made the files —
//! `GRAB/LBM2P.C` (chunky → `.UP`/`.P`), `GRAB/LBM2U.C` (→ `.UH`/`.U`) and
//! `GRAB/LBM16.C` (→ `.16`).
//!
//! Measured over the curated tree, and worth stating once: **37 IFF files, 24
//! `ILBM` and 13 `PBM `**; every `masking` byte 0; every `compression` byte 1;
//! **no `CAMG` chunk in the tree at all** (nor in any of the 86 upstream IFF
//! files); every `FORM`'s size exactly the file length less eight; and every
//! `ILBM` file a multiple of 16 wide, which is why the row-padding question in
//! `decode_planar`'s doc comment cannot be answered by any shipped file.
//!
//! The palette rule, which the twins settle: an IFF `CMAP` holds 8-bit values
//! and the original truncates them with `/4` (`LBM2U.C:170`, and the same line
//! at `LBM2P.C:195`, `LBM16.C:194`, `VIEWLBM.C:157`, `VPL.C:97`). Every
//! converted twin in the tree holds those `/4` values byte for byte —
//! `the_palette_is_the_originals_own_truncation_of_the_eight_bit_cmap` checks
//! 768 bytes of `LENS/LENS.EXB` and of `BEG/SRTITLE.UP` against this decoder's
//! own palette. It matters: `six_bit`, the *display* rule's inverse, would
//! differ on 457 of the 768 bytes for the first pair and 1 for the second, and
//! over the curated tree the two disagree on 2,067 of 22,760 `CMAP` bytes.

use std::path::{Path, PathBuf};

use rstest::rstest;
use sr_assets::{load, load_frames, load_path, six_bit, sniff, AssetError, Format};
use sr_engine::dac_expand;

/// Every file in the curated tree shipped under an extension one of the two
/// trees uses for IFF images. The extension is a *hint*: `GRAB/BLOWFNT.1BM` is
/// a bare dump, not IFF, and upstream's two IFF `.bbk` files sit beside eight
/// `.bbk` files that are C source — so every caller here lets `sniff` decide.
fn iff_assets_in(root: &str) -> Vec<PathBuf> {
    const EXTENSIONS: [&str; 4] = ["LBM", "1BM", "BBM", "BBK"];

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if EXTENSIONS.iter().any(|k| ext.eq_ignore_ascii_case(k)) {
                    out.push(path);
                }
            }
        }
    }

    let mut out = Vec::new();
    walk(Path::new(root), &mut out);
    out.sort();
    out
}

fn be16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

/// The geometry a file's own header claims, read independently of the decoder:
/// big-endian, the way `getaw` reads it (`LOADLBM.C:84-90`). What makes
/// `every_iff_asset_…` worth running is that this and the decoder can disagree.
fn bmhd_of(file: &[u8]) -> (u16, u16, u8) {
    let mut at = 12;
    while at + 8 <= file.len() {
        let size = u32::from_be_bytes([file[at + 4], file[at + 5], file[at + 6], file[at + 7]]);
        if &file[at..at + 4] == b"BMHD" {
            return (
                be16(file, at + 8),
                be16(file, at + 10),
                file[at + 16],
            );
        }
        at += 8 + size as usize + (size as usize & 1);
    }
    panic!("no BMHD chunk in this file");
}

/// The payload of a file's `CMAP` chunk, walked the same way the decoder walks
/// it, so a test can compare a palette with the bytes it came from.
fn cmap_of(file: &[u8]) -> &[u8] {
    let mut at = 12;
    while at + 8 <= file.len() {
        let size = u32::from_be_bytes([file[at + 4], file[at + 5], file[at + 6], file[at + 7]])
            as usize;
        if &file[at..at + 4] == b"CMAP" {
            return &file[at + 8..at + 8 + size];
        }
        at += 8 + size + (size & 1);
    }
    panic!("no CMAP chunk in this file");
}

/// The payload offset and declared length of a file's `BODY` chunk.
fn body_at(file: &[u8]) -> (usize, usize) {
    let mut at = 12;
    while at + 8 <= file.len() {
        let size = u32::from_be_bytes([file[at + 4], file[at + 5], file[at + 6], file[at + 7]]);
        if &file[at..at + 4] == b"BODY" {
            return (at + 8, size as usize);
        }
        at += 8 + size as usize + (size as usize & 1);
    }
    panic!("no BODY chunk in this file");
}

/// One IFF chunk, the standard way: id, big-endian length, payload, and a pad
/// byte when the payload is odd. The tree's own writers count that pad byte *in*
/// the length instead (`chunk_counting_pad` below), and both readings have to
/// walk to the same next chunk.
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

/// A chunk whose declared length counts its own pad byte, which is what
/// `CREDITS/PICS/PIC01.LBM`'s 334-byte `CMAP` (111 triples and a zero) and its
/// 7,488-byte `BODY` (a row stream of 7,487) do.
fn chunk_counting_pad(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = chunk(id, payload);
    if payload.len() % 2 == 1 {
        out[4..8].copy_from_slice(&(payload.len() as u32 + 1).to_be_bytes());
    }
    out
}

/// A whole `FORM`: `FORM`, the size, the type, and the chunks. The size is
/// written the way every file in the tree writes it — the length less the eight
/// bytes `FORM` and the size field take, with no slack.
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

/// `file` with `extra` spliced in after its `BMHD`, and the `FORM` size
/// corrected to match. Every file in the tree puts `BMHD` first, and the helper
/// says so rather than assuming it.
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

/// `file` with its `BODY` cut to `keep` bytes, both the chunk's length and the
/// `FORM`'s rewritten so that the container is self-consistent and the shortfall
/// is the picture's rather than the container's.
fn with_short_body(file: &[u8], keep: usize) -> Vec<u8> {
    let (at, size) = body_at(file);
    let keep = keep.min(size);
    let mut out = file[..at + keep].to_vec();
    out[at - 4..at].copy_from_slice(&(keep as u32).to_be_bytes());
    let form_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&form_size.to_be_bytes());
    out
}

// ---------------------------------------------------------------------------
// The brief's tests.
// ---------------------------------------------------------------------------

/// A 4x2 `ILBM` built here, so the expected pixels are known independently of
/// any asset: two bitplanes, ByteRun1, and a `CMAP`.
///
/// The plane rows are worked out in the comment rather than read from a file, so
/// a decoder that ran the planes in the wrong order or shifted the bits the
/// wrong way fails here and nowhere else. w = 4 gives `((4 + 15) / 16) * 2` = 2
/// bytes per plane row, filled by one literal opcode each (`0x01` is two literal
/// bytes, `LOADLBM.C:217-231`).
///
/// Row 0 is pixels 0, 1, 2, 3: colour bit 0 is 0,1,0,1 — eight pixels a byte,
/// most significant leftmost, so `0101_0000` = 0x50 — and bit 1 is 0,0,1,1 =
/// `0011_0000` = 0x30. Row 1 is 3, 2, 1, 0: bit 0 = 1,0,1,0 = 0xA0, bit 1 =
/// 1,1,0,0 = 0xC0.
///
/// The palette's third entry, `[83, 3, 7]`, is there for the conversion rule:
/// `83 / 4` is 20, and `six_bit(83)` would be 21 — the display rule's inverse,
/// which is not what the original's converters do (`LBM2U.C:170` is `a =
/// getc(f1)/4`). The fourth is the brief's `[255, 0, 0]`, which gives 63 either
/// way.
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
    assert_eq!(img.palette.entry(3), [63, 0, 0]); // from the hand-built CMAP
    assert_eq!(img.palette.entry(1), [63, 63, 63]);
    assert_eq!(
        img.palette.entry(2),
        [20, 0, 1],
        "eight bits are truncated by four, as every converter in the tree does"
    );
}

/// Any chunky file from `assets/`; A1's inventory names them. The three width
/// classes the tree uses are here — 1500 for the font strips, 640 and 320 — and
/// each one's pixels have to be more than one value, or a decoder that returned
/// a zeroed surface would pass.
#[rstest]
#[case("../../assets/ALKU/FONA.LBM", 1500, 30)]
#[case("../../assets/GLENZ/FC.LBM", 320, 200)]
#[case("../../assets/START/HZPIC.LBM", 640, 400)]
#[case("../../assets/BEG/SRTITLE.LBM", 320, 400)]
fn a_pbm_decodes_to_the_dimensions_in_its_header(
    #[case] path: &str,
    #[case] w: u16,
    #[case] h: u16,
) {
    let raw = std::fs::read(path).unwrap();
    assert_eq!(sniff(&raw), Some(Format::Iff));
    assert_eq!(&raw[8..12], b"PBM ", "the chunky type, not ILBM");
    let img = load_path(Path::new(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!((img.width(), img.height()), (w, h));
    assert_eq!(img.pixels().len(), w as usize * h as usize);
    let mut values = img.pixels().to_vec();
    values.sort_unstable();
    values.dedup();
    assert!(values.len() > 1, "{path}: decoded to a single value");
}

/// **The phase's strongest single piece of evidence**, moved here from A2 because
/// it needs both decoders: the title screen the demo actually shows, and the
/// source art it was converted from, are the same picture.
///
/// `BEG/SRTITLE.UP` is the RLE-packed `FCFD` picture the BEGLOGO part links in
/// (`BEG/MAKEFILE:21`, `link ...+_pic.obk`, which `asset-inventory.md` §D
/// identifies as this file); `BEG/SRTITLE.LBM` is the Deluxe Paint original it
/// came from. 128,000 pixels, no difference — the measurement the ruling
/// quotes, re-run here. The planar half of the same argument is
/// `the_planar_scene_bitmap_and_its_origin_twin_are_the_same_picture`, which a
/// chunky file cannot make.
#[test]
fn the_packed_title_and_the_iff_title_are_the_same_picture() {
    let up = load_path(Path::new("../../assets/BEG/SRTITLE.UP")).unwrap();
    let lbm = load_path(Path::new("../../assets/BEG/SRTITLE.LBM")).unwrap();
    assert_eq!((up.width(), up.height()), (320, 400));
    assert_eq!((up.width(), up.height()), (lbm.width(), lbm.height()));
    assert_eq!(up.pixels(), lbm.pixels());
}

/// The same check for the **planar** half, which the title cannot make: the
/// title is chunky, so nothing above would notice bitplanes interleaved in the
/// wrong order, bits within a byte the wrong way round, or a plane row read at
/// the wrong width. `VISU/C/U2ABG.LBM` is the source art of `VISU/C/U2ABG.UH`
/// (`INCBIN`ed by the 3D viewer's build), so the two have to agree pixel for
/// pixel — 64,000 of them, none differing.
#[test]
fn the_planar_scene_bitmap_and_its_origin_twin_are_the_same_picture() {
    let lbm = load_path(Path::new("../../assets/VISU/C/U2ABG.LBM")).unwrap();
    let uh = load_path(Path::new("../../assets/VISU/C/U2ABG.UH")).unwrap();
    assert_eq!(&std::fs::read("../../assets/VISU/C/U2ABG.LBM").unwrap()[8..12], b"ILBM");
    assert_eq!((lbm.width(), lbm.height()), (320, 200));
    assert_eq!((lbm.width(), lbm.height()), (uh.width(), uh.height()));
    assert_eq!(lbm.pixels(), uh.pixels());
    // Palette too, across the two decoders: the `.UH` was written from this
    // `.LBM` by `LBM2U.C`, which truncates its `CMAP` with `/4` — so the Origin
    // door (verbatim 6-bit) and the IFF door (`/4`) have to land on the same 256
    // entries. This assertion is the one a `six_bit` conversion of the `CMAP`
    // fails, on 56 of this file's 768 bytes.
    assert_eq!(lbm.palette, uh.palette);

    // Thirteen file pairs like this exist in the curated tree — `COMAN/COMBG.LBM`
    // has two twins, and `LENS/LENSPIC.LBM`'s is `LENS/LENS.EXB`, named after
    // the part rather than after its source — and nine of the eleven whose
    // geometries match agree to zero pixels: `FONA` (ALKU), `HOI`, `SRTITLE`,
    // `PIC` (END), `FONA` (ENDSCRL), `FC`, `ICEKNGDM`, `LENSPIC` and this one.
    // The two that do not are the converters' doing, not the decoders':
    // `DDSTARS/TEXTS.LBM` went through `lbm16 texts.lbm texts.16 2`
    // (`DDSTARS/DOTEXTS.BAT:1`, sixteen colours, so 7,573 of 16,000 pixels
    // differ), and `START/HZPIC.LBM`'s `.P` twin is the file A2's
    // clipped-overflow approximation is about (66,269 of 256,000, every one of
    // them a clipped row; `docs/fidelity/APPROXIMATIONS.md`, A2).
    // `COMAN/COMBG.LBM`'s 320x90 source has no 320x90 twin to compare with.
}

/// An IFF `BODY` is ByteRun1, so A2's payload-equality property does not
/// transfer to this container: there is no fixed relationship between the bytes
/// stored and the bytes decoded. What does hold, and catches the same class of
/// misread header field, is that every IFF file decodes, that what it decodes to
/// is the geometry **its own `BMHD` claims** — read here independently, big
/// endian, so the two readings cannot be wrong together — and that the pixels
/// are a picture rather than a blank.
///
/// Measured: 37 files, 24 planar and 13 chunky. The guard is a floor, the split
/// an equality, so a loop that matches nothing fails as loudly as one that
/// stopped matching half the tree.
#[test]
fn every_iff_asset_decodes_and_fills_the_geometry_its_bmhd_claims() {
    let mut seen = 0;
    let (mut planar, mut chunky) = (0, 0);
    for path in iff_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        if sniff(&raw) != Some(Format::Iff) {
            continue; // GRAB/BLOWFNT.1BM is a bare dump: A2's raw_surface is its door
        }
        let (w, h, planes) = bmhd_of(&raw);
        let img = load(&raw, Format::Iff).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            (img.width(), img.height()),
            (w, h),
            "{}: decoded geometry vs its own BMHD",
            path.display()
        );
        assert_eq!(
            img.pixels().len(),
            img.width() as usize * img.height() as usize,
            "{}: decoded pixels vs BMHD geometry",
            path.display()
        );
        let mut values = img.pixels().to_vec();
        values.sort_unstable();
        values.dedup();
        assert!(values.len() > 1, "{}: decoded to a single value", path.display());

        if &raw[8..12] == b"ILBM" {
            planar += 1;
            // Every curated ILBM uses its top plane — measured — so a plane
            // whose stream was lost, misordered or read at the wrong row width
            // shows up as a pixel range that never reaches `1 << (planes - 1)`.
            assert!(
                values.last().copied().unwrap() >= 1 << (planes - 1),
                "{}: {planes} planes but no pixel uses the top one",
                path.display()
            );
        } else {
            chunky += 1;
        }
        seen += 1;
    }
    assert!(seen >= 37, "expected both IFF flavours, saw {seen}");
    assert_eq!((planar, chunky), (24, 13), "the curated tree's own tally");
}

/// `six_bit` is the exact inverse of the engine's DAC widening — the 6→8
/// *display* rule inverted — in both directions: an eight-bit value widened from
/// the six-bit answer comes back within the rounding (at most 2), and every
/// six-bit value survives the round trip exactly.
///
/// It is **not** the IFF `CMAP` rule. The original's converters truncate by four
/// (`LBM2U.C:170`), and
/// `the_palette_is_the_originals_own_truncation_of_the_eight_bit_cmap` pins this
/// decoder against their output; A2's
/// `six_bit_is_the_exact_inverse_of_the_engines_widening` pins the 0..=63 half
/// from the palette side.
#[test]
fn six_bit_is_the_inverse_of_dac_expand_over_all_256_values() {
    for v in 0u8..=255 {
        let six = six_bit(v);
        assert!((dac_expand(six) as i32 - v as i32).abs() <= 2, "8-bit {v} -> 6-bit {six}");
        assert_eq!(six, six_bit(v), "stable");
    }
    for c in 0u8..=63 {
        assert_eq!(six_bit(dac_expand(c)), c, "6-bit value {c}");
    }
}

/// The opcode that fills the last row is consumed whole, and the row stream then
/// ends exactly where the `BODY` does. A decoder that read one byte past the
/// last row — to look at the next opcode, or to check for a pad byte — would
/// refuse this file; the original's `while(lp < xsz)` (`LOADLBM.C:161-179`) stops
/// on the opcode boundary and reads nothing more.
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

/// Every run length ByteRun1 can express, and the row's end as the bound: the
/// last op here wants 126 bytes of a row that has four left, and the opcode is
/// consumed whole with the excess dropped. That is what the original leaves
/// visible — it writes the whole run into `linebuf` and copies `xsz` bytes out
/// of it (`LOADLBM.C:181`) — so the *next* row starts after the run's opcode and
/// not part way through its output, which is what the second row of this file
/// pins: four runs of two, read from the byte after the overshooting run.
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

/// The same rule for the other opcode: `0x0B` is twelve literal bytes, and a row
/// with seven left takes seven of them. The original reads all twelve and writes
/// them past the row (`LOADLBM.C:171-178`) — the copy takes `xsz` — and the next
/// row starts after the whole opcode, not after the part that fit.
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

/// The `BODY` stops one byte short of the row the header promises. `needed` and
/// `had` are file offsets — the byte the decoder wanted against the last byte
/// the `BODY` holds — and the refusal has to come from the row walk, not from
/// the `FORM` check: this file's container is self-consistent, and only its
/// picture is short.
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
    let (at, size) = body_at(&file);
    let body_end = at + size;
    assert!(
        matches!(
            load(&file, Format::Iff),
            Err(AssetError::Truncated { format: Format::Iff, needed, had })
                if needed == body_end + 1 && had == body_end
        ),
        "expected a Truncated naming the row stream's end"
    );
}

/// `CRNG`, `DPPS`, `DPPV` and `TINY` are Deluxe Paint's colour-range and tool
/// chunks: they change no pixel, the original skips them (`LOADLBM.C:139-150`),
/// and so does this decoder — including a chunk id it has never seen, and
/// including an odd-sized one, whose pad byte the chunk walk has to skip.
///
/// `GLENZ/FC.LBM` already carries seventeen `CRNG` chunks, so this inserts two
/// more — one of them unheard of — and requires the picture not to change.
#[test]
fn unknown_chunks_are_skipped_not_fatal() {
    let raw = std::fs::read("../../assets/GLENZ/FC.LBM").unwrap();
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

// ---------------------------------------------------------------------------
// Beyond the brief's list: the rulings, the review focus, and the quirks the
// measurements turned up.
// ---------------------------------------------------------------------------

/// The palette rule, pinned against the original's own converted twins rather
/// than against a transcription of it.
///
/// The two doors lead to the same picture — one loads the `.LBM`, the other the
/// `.UH`/`.EXB` the `GRAB/` toolchain made from it — so their palettes have to
/// be the same 768 bytes, and each assertion below fails for a `six_bit`
/// implementation of the `CMAP`. The first case is the one with teeth: on
/// `LENS/LENSPIC.LBM` the two rules differ on **457 of the 768 bytes**, against
/// 1 on the title. Both twins put their palette at offset 16 (`Uh`, `GRAB/
/// GLVP.C:988-1014`, and `FCFD`, `LBM2P.C`), and the third assertion keeps the
/// discrimination honest by counting what is at stake in each pair — a case that
/// stopped discriminating would still pass the first two.
#[rstest]
#[case("../../assets/LENS/LENSPIC.LBM", "../../assets/LENS/LENS.EXB", 457)]
#[case("../../assets/BEG/SRTITLE.LBM", "../../assets/BEG/SRTITLE.UP", 1)]
fn the_palette_is_the_originals_own_truncation_of_the_eight_bit_cmap(
    #[case] lbm_path: &str,
    #[case] twin_path: &str,
    #[case] at_stake: usize,
) {
    let lbm = load_path(Path::new(lbm_path)).unwrap();
    let twin = load_path(Path::new(twin_path)).unwrap();
    assert_eq!(lbm.palette, twin.palette, "the IFF door and the Origin door must agree");

    // And the same thing in raw bytes, so a shared mistake in both decoders
    // cannot pass: the twin's stored palette is the `CMAP` divided by four, byte
    // for byte (`LBM2U.C:170`, `LBM2P.C:195`).
    let raw = std::fs::read(lbm_path).unwrap();
    let cmap = cmap_of(&raw);
    assert_eq!(cmap.len(), 768, "256 entries, as every `PBM ` file in the tree");
    let twin_raw = std::fs::read(twin_path).unwrap();
    for i in 0..768 {
        assert_eq!(twin_raw[16 + i], cmap[i] / 4, "byte {i} of the twin's palette");
    }

    // What `six_bit` would have cost, which is why this is not the rule: the
    // bytes where the display widening's inverse disagrees with the conversion
    // the original ran.
    let divergent = (0..768).filter(|&i| six_bit(cmap[i]) != cmap[i] / 4).count();
    assert_eq!(divergent, at_stake, "{lbm_path} against {twin_path}");
    assert!(at_stake > 0, "a pair where the two rules agree proves nothing");
}

/// `masking` is a *mask plane*: nonzero means one is stored in each row after
/// the colour planes, so it is decoded into the row stream the rows are taken
/// from. Ignoring the byte would read the next row's pixels out of the mask —
/// which is why this is refused rather than skipped even though the original
/// skips it (`LOADLBM.C:234-251`).
///
/// Measured: every one of the curated 37 files has `masking` 0; upstream has ten
/// with 1 or 2 (`ENDPIC/PIC.LBM`, `GLENZ/FCX.LBM`, `JPLOGO/PIC.LBM`,
/// `LENS/LENSPIC1.LBM`, `WATER/FONA.LBM`, `FOREST/FINAL2.LBM` and the four
/// tools' `TMP.BBM` files), and none of them was curated. Both values are
/// refused here, because both put a plane in the stream.
#[rstest]
#[case(1)]
#[case(2)]
fn a_stencil_or_mask_plane_is_refused_rather_than_skipped(#[case] masking: u8) {
    let file = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(4, 2, 2, masking, 1)),
            chunk(b"BODY", &[0x01, 0x50, 0x00]),
        ],
    );
    assert!(
        matches!(load(&file, Format::Iff), Err(AssetError::Unsupported { .. })),
        "masking {masking} must be refused, not guessed at"
    );
}

/// `CAMG` is the Amiga display mode, and its two colour-changing bits are EHB
/// (0x80) and HAM (0x800): the first halves the palette at the display end — the
/// same index is two different colours — and the second builds a colour out of
/// the previous pixel's. This decoder produces *indexed* pixels and a palette,
/// so it can represent neither, and refuses both.
///
/// No file in either tree carries a `CAMG` chunk at all (measured: 0 of 37 and 0
/// of 86), so the refusal cannot reject a real asset. A mode without those bits
/// — the Amiga's own NTSC/PAL flags are others — is not a colour question and
/// decodes like any other file, which the last case pins.
#[rstest]
#[case(0x80, true)]
#[case(0x800, true)]
#[case(0x0000, false)]
#[case(0x1004, false)]
fn an_ehb_or_ham_display_mode_is_refused(#[case] mode: u32, #[case] refused: bool) {
    let raw = std::fs::read("../../assets/GLENZ/FC.LBM").unwrap();
    let file = with_chunks_after_bmhd(&raw, &chunk(b"CAMG", &mode.to_be_bytes()));
    assert_eq!(sniff(&file), Some(Format::Iff));
    match (load(&file, Format::Iff), refused) {
        (Err(AssetError::Unsupported { .. }), true) => {}
        (Ok(img), false) => assert_eq!((img.width(), img.height()), (320, 200)),
        (got, want) => panic!("CAMG {mode:#x}: wanted refused={want}, got {got:?}"),
    }
}

/// Deluxe Paint writes `compression` 1 (ByteRun1) and nothing else: every
/// `compression` byte in the curated tree and upstream is 1. The original never
/// reads the field as such — `LOADLBM.C:132` takes it and the pad byte after it
/// together as the misnamed `colors` — and byte-run-decodes whatever is there,
/// which is only safe because 1 is what is there. Uncompressed `BODY` data (0)
/// would be read as opcodes and produce a picture nobody wrote.
#[test]
fn a_compression_the_writers_never_emit_is_refused() {
    let file = form(
        b"PBM ",
        &[
            chunk(b"BMHD", &bmhd(4, 1, 8, 0, 0)),
            chunk(b"BODY", &[1, 2, 3, 4]),
        ],
    );
    assert!(matches!(load(&file, Format::Iff), Err(AssetError::Unsupported { .. })));
}

/// `nPlanes` sizes the colour index, so 0 names no colours and more than 8
/// cannot fit an indexed byte. The count is only consulted for the planar
/// flavour, exactly as in the original: its chunky branch never reads it
/// (`LOADLBM.C:151-185`), which the last case pins — a `PBM `'s bytes are pixel
/// values whatever its header says the plane count is.
#[rstest]
#[case(0)]
#[case(9)]
fn a_plane_count_that_cannot_index_a_pixel_is_refused(#[case] planes: u8) {
    let file = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(4, 1, planes, 0, 1)),
            chunk(b"BODY", &[0x01, 0x11, 0x22]),
        ],
    );
    assert!(matches!(load(&file, Format::Iff), Err(AssetError::Unsupported { .. })));
}

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

/// The engine's surface holds 256,000 pixels and the rule is an *area* one
/// (plan §Review Focus 3): 1500 x 30 is fine, 640 x 400 is exactly the bound,
/// and 1024 x 400 is 409,600 and has to be refused before anything is allocated
/// for it, with the geometry in the message.
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

/// A `FORM` holding a picture's chunks and no `BMHD` — nothing tells a decoder
/// how wide or tall it is, and the size of a dump proves nothing (three 64,000
/// byte assets in this tree have three geometries; A2's tests say so at length).
#[test]
fn a_container_without_a_bmhd_is_refused() {
    let file = form(
        b"ILBM",
        &[chunk(b"CMAP", &[1, 2, 3]), chunk(b"BODY", &[0x42])],
    );
    assert!(matches!(load(&file, Format::Iff), Err(AssetError::Unsupported { .. })));
}

/// …and one with a header and no `BODY`, which has a geometry and no pixels.
#[test]
fn a_container_without_a_body_is_refused() {
    let file = form(
        b"ILBM",
        &[chunk(b"BMHD", &bmhd(4, 1, 2, 0, 1)), chunk(b"CMAP", &[1, 2, 3])],
    );
    assert!(matches!(load(&file, Format::Iff), Err(AssetError::Unsupported { .. })));
}

/// The writers' pad-byte quirk, which is what eight of the curated files'
/// palettes look like: an odd palette made even by counting the pad byte in the
/// chunk's length (`CREDITS/PICS/PIC01.LBM` is 334 bytes of `CMAP` for 111
/// triples, and thirteen files' `BODY` chunks have the same shape). The entries
/// are the first `size / 3` triples and the leftover byte is padding — not an
/// error, not a fourth of an entry, and not a chunk the walk loses its place in.
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

/// The other end of the same rule, and the reason the walk's check is
/// `at != end` rather than `at < end`: a `FORM` whose size stops one byte short
/// of its last, odd-sized chunk — the pad byte the tree's writers count *in* the
/// chunk is not counted in the `FORM` — is refused rather than read past. No
/// file in either tree is shaped like this (measured over all 37 curated and all
/// 86 upstream IFF files: every walk lands exactly on the `FORM`'s end), and the
/// padded form of the same file decodes, which is what makes the refusal a
/// decision about the contract rather than a limitation.
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

/// The `FORM` size has to describe chunks. These writers make it exactly the
/// file length less eight (measured, all 37), so bytes left over after the last
/// chunk mean the size and the content disagree — and a decoder that shrugged
/// and kept going would be reading a picture out of a container it had already
/// mistaken the shape of.
#[test]
fn a_form_size_that_promises_bytes_that_are_not_a_chunk_is_refused() {
    let raw = std::fs::read("../../assets/GLENZ/FC.LBM").unwrap();
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

/// The other half of the walk's contract: a chunk whose declared size runs past
/// the `FORM`'s end. The `FORM` check above cannot catch this one — the size
/// field still describes the file exactly — so it is the chunk that is lying,
/// and its payload is that much shorter than it says.
#[test]
fn a_chunk_that_claims_more_than_the_form_holds_is_refused() {
    let raw = std::fs::read("../../assets/GLENZ/FC.LBM").unwrap();
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

#[test]
fn a_still_iff_image_loads_as_one_frame() {
    let bytes = std::fs::read("../../assets/BEG/SRTITLE.LBM").unwrap();
    let frames = load_frames(&bytes, Format::Iff).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!((frames[0].width(), frames[0].height()), (320, 400));
}

/// The plan's Review Focus 1, on IFF: every loader returns an `AssetError`,
/// never panics and never reads past its input. Two shapes of cut, because the
/// container catches most of them and the row walk has to catch the rest.
///
/// **A raw prefix** is refused by the `FORM` size alone — `size + 8` is the file
/// length for all 37 curated files (measured), so anything shorter than the
/// whole file claims more bytes than it has — which is why the count here is
/// exact rather than a floor.
///
/// **A `BODY` cut short** is the interesting one: the container is rewritten to
/// be consistent with itself, so the refusal has to come from the rows. Every
/// cut must be an error or a complete picture: a decoder that zero-filled the
/// missing rows, stopped quietly at the shortfall, or read a byte past the
/// `BODY` would decode a picture here that nothing in the tree contains.
#[test]
fn no_prefix_of_an_iff_asset_panics_or_decodes_silently() {
    let mut assets = 0;
    let mut cuts = 0;
    let mut refused = 0;
    for path in iff_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        if sniff(&raw) != Some(Format::Iff) {
            continue;
        }
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

        let (_, body_size) = body_at(&raw);
        let step = (body_size / 12).max(1);
        for keep in (0..body_size).step_by(step) {
            let cut = with_short_body(&raw, keep);
            match load(&cut, Format::Iff) {
                Ok(img) => {
                    // Only legal when the cut removed nothing the rows needed —
                    // but then the picture is complete, not short.
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
    // The `FORM`-size cuts are all refused, and so are nearly all the `BODY`
    // cuts: for one to decode, it has to land exactly on a row boundary — and
    // then the container has to still walk, which an odd length with no pad byte
    // does not.
    assert!(refused * 10 > cuts * 9, "only {refused} of {cuts} cuts refused");
}

/// The truncated file the brief names, at file scale rather than hand-built:
/// half of `BEG/SRTITLE.LBM` is a `FORM` promising 53,238 bytes it does not
/// have, and that is caught before a chunk is read.
#[test]
fn a_halved_iff_asset_is_refused() {
    let mut bytes = std::fs::read("../../assets/BEG/SRTITLE.LBM").unwrap();
    bytes.truncate(bytes.len() / 2);
    assert!(matches!(load(&bytes, Format::Iff), Err(AssetError::Truncated { .. })));
}

/// A file that is not an IFF at all, handed to the IFF door by a caller that
/// did not sniff first. `load` is told the format; it still checks the magic.
#[test]
fn load_refuses_bytes_that_are_not_iff() {
    let origin = std::fs::read("../../assets/BEG/SRTITLE.UP").unwrap();
    assert!(matches!(load(&origin, Format::Iff), Err(AssetError::UnknownFormat)));
    assert!(matches!(load(b"FORM", Format::Iff), Err(AssetError::Truncated { .. })));
    let not_an_iff = {
        let mut f = std::fs::read("../../assets/BEG/SRTITLE.LBM").unwrap();
        f[8..12].copy_from_slice(b"ANIM"); // a FORM, but not a picture type
        f
    };
    assert!(matches!(load(&not_an_iff, Format::Iff), Err(AssetError::Unsupported { .. })));
}
