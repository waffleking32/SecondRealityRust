//! The Origin PIC family — the container behind `.UH`, `.U`, `.UP`, `.P`, `.16`
//! and `.X` — pinned against the files the demo shipped.
//!
//! Every file these tests read is a curated copy under `assets/`, verified
//! byte-for-byte against `upstream/SecondReality/` at commit `071a82e` by task
//! A1. The expectations (magic, geometry, colour count, paragraph field) were
//! read off the files, and the format itself off the writers: `GRAB/LBM2U.C`,
//! `GRAB/LBM16.C`, `GRAB/LBM2P.C` and `GRAB/GLVP.C`'s `saveunp` /
//! `saveunpheader` / `saveunpheaderpal`. `ENDPIC/READP.C` is the runtime reader
//! the RLE rows have to satisfy.
//!
//! Two paths the brief names are not in the curated tree, and are substituted
//! where they are used rather than silently dropped: `ENDPIC/PIC.UH` has no
//! consumer in the original and was not copied (`docs/fidelity/asset-inventory.md`
//! §D), so `GLENZ/FC.UH` — the same `Uh` variant, geometry, size and palette
//! shape — stands in for it. And `origin_assets_in` finds **15** files carrying
//! an Origin magic, not the 20 the brief's guard names: 20 is the count over the
//! whole upstream tree (§8 of `.superpowers/sdd/01-assets/a2-format-research-report.md`),
//! while `assets/` is A1's curated subset. The guard below asserts the measured
//! count so that a loop which matches nothing still fails.

use std::path::{Path, PathBuf};

use rstest::rstest;
use sr_assets::{load, load_frames, load_path, raw_surface, six_bit, sniff, AssetError, Format};
use sr_engine::dac_expand;

/// Every file in the curated tree shipped under an extension this family uses.
/// The extension is a *hint* — the sniff in each caller decides whether the file
/// is really one of them, which is the point of
/// `the_extension_never_decides_the_format`. `EXB` and `1BM` are here because
/// A1's tree keeps the two renamed saves the report names (§8): `LENS/LENS.EXB`
/// is `Uh` data under `LENS/MAKEFILE`'s rename, and `GRAB/BLOWFNT.1BM` is a bare
/// dump.
fn origin_assets_in(root: &str) -> Vec<PathBuf> {
    const EXTENSIONS: [&str; 9] = ["UH", "U", "UP", "P", "16", "X", "UX", "EXB", "1BM"];

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

#[test]
fn a_thirty_pixel_tall_font_strip_reads_as_one() {
    // ALKU/FONA.UH, read off the file: magic FC FC, 1500 x 30, colours 256,
    // para 0x31 (= 784), so 16 + 768 palette bytes then 45,000 raw pixels.
    let img = load_path(Path::new("../../assets/ALKU/FONA.UH")).unwrap();
    assert_eq!((img.width(), img.height()), (1500, 30));
    assert_eq!(img.pixels().len(), 45_000);
}

/// The variant table of the research report §1, one shipped file per variant.
/// Only the magic decides which row a file is in; the extension, the geometry
/// and the size all fail to separate them.
#[rstest]
#[case("../../assets/ALKU/FONA.UH", 1500, 30, "FCFC raw")]
#[case("../../assets/DDSTARS/TEXTS.16", 320, 200, "FCFC two bit-planes")]
#[case("../../assets/BEG/SRTITLE.UP", 320, 400, "FCFD RLE")]
#[case("../../assets/COMAN/COMBG.X", 320, 200, "UH, no palette")]
#[case("../../assets/GLENZ/FC.UH", 320, 200, "Uh, 768-byte palette")]
fn each_variant_in_the_family_decodes(
    #[case] path: &str,
    #[case] w: u16,
    #[case] h: u16,
    #[case] what: &str,
) {
    let raw = std::fs::read(path).unwrap();
    assert_eq!(sniff(&raw), Some(Format::Origin), "{what}");
    let img = load_path(Path::new(path)).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!((img.width(), img.height()), (w, h), "{what}");
    assert_eq!(img.pixels().len(), w as usize * h as usize, "{what}");
}

/// The point of this family, in one test: the extension says nothing. `END/PIC.UH`
/// is RLE, `GLENZ/FC.UH` is `Uh` with a palette, and `COMAN/COMBG.X` is `UH`
/// without one — and `.X` is not a format at all, it is `saveunpheader` output
/// renamed. All of them are `.UH`/`.X`.
#[test]
fn the_extension_never_decides_the_format() {
    let rle = load_path(Path::new("../../assets/END/PIC.UH")).unwrap();
    assert_eq!((rle.width(), rle.height()), (320, 400));

    let uh_pal = load_path(Path::new("../../assets/GLENZ/FC.UH")).unwrap();
    assert_eq!((uh_pal.width(), uh_pal.height()), (320, 200));

    let uh = load_path(Path::new("../../assets/COMAN/COMBG.X")).unwrap();
    assert_eq!((uh.width(), uh.height()), (320, 200));

    assert_eq!(
        sniff(&std::fs::read("../../assets/BEG/SRTITLE.LBM").unwrap()),
        Some(Format::Iff)
    );
}

/// The `Uh` variant's palette is the one place a 768-byte palette is guaranteed,
/// and it is already 6-bit — so this also pins that no re-quantising happens.
#[test]
fn a_uh_file_carries_its_own_six_bit_palette() {
    let img = load_path(Path::new("../../assets/GLENZ/FC.UH")).unwrap();
    for i in 0..=255u8 {
        assert!(img.palette.entry(i).iter().all(|&c| c <= 63), "entry {i} is not 6-bit");
    }
    assert_ne!(img.palette.entry(0), img.palette.entry(255), "palette was not read");
}

/// `LBM16.C` writes a fixed 16-colour palette and bit-planes, and `TEXTS.16` was
/// made with it at two planes (`DOTEXTS.BAT:1`).
#[test]
fn a_bit_plane_file_decodes_to_the_geometry_in_its_header() {
    let img = load_path(Path::new("../../assets/DDSTARS/TEXTS.16")).unwrap();
    assert_eq!((img.width(), img.height()), (320, 200));
    assert_eq!(img.pixels().len(), 64_000);
}

/// A bare dump has no header, so `sniff` cannot see it and the geometry is the
/// caller's to supply. `PANIC/MONSTER.U` is exactly 320 x 200 bytes of pixels.
#[test]
fn a_headerless_dump_needs_its_geometry_from_the_caller() {
    let raw = std::fs::read("../../assets/PANIC/MONSTER.U").unwrap();
    assert_eq!(sniff(&raw), None, "a bare dump has no magic to sniff");
    let img = raw_surface(&raw, 320, 200).unwrap();
    assert_eq!((img.width(), img.height()), (320, 200));
    assert!(matches!(raw_surface(&raw, 320, 199), Err(AssetError::Truncated { .. })));
}

/// Every Origin file in the curated tree decodes, and what it decodes to fills the
/// geometry its header claimed. The decoder refuses any payload that is neither
/// `w*h` nor a whole number of bit-planes, so a misread header field cannot pass.
/// `decoded` guards the other failure mode: a loop that matches nothing also
/// passes. The one exception is the tree's one genuinely corrupt asset, which is
/// counted separately and must be refused with a `Truncated` — never loaded.
#[test]
fn every_origin_asset_decodes_and_fills_the_geometry_its_header_claims() {
    let mut decoded = 0;
    let mut refused = 0;
    for path in origin_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        if sniff(&raw) != Some(Format::Origin) {
            continue; // bare dumps are the caller's geometry, tested above
        }
        match load(&raw, Format::Origin) {
            Ok(img) => {
                assert_eq!(
                    img.pixels().len(),
                    img.width() as usize * img.height() as usize,
                    "{}: decoded pixels vs header geometry",
                    path.display()
                );
                decoded += 1;
            }
            Err(e) => {
                // Only one file in the tree is expected here, and it is the
                // 32,768-byte truncation of COMAN/COMBG.X — `asset-inventory.md`
                // §D records it. Anything else failing is a decoder bug.
                assert!(
                    path.to_string_lossy().ends_with("COMAN/COMBG.UH"),
                    "{}: {e}",
                    path.display()
                );
                assert!(
                    matches!(e, AssetError::Truncated { .. }),
                    "{}: expected a Truncated, got {e}",
                    path.display()
                );
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 1, "the tree carries exactly one corrupt Origin asset");
    assert!(decoded >= 14, "expected the whole Origin family, saw {decoded}");
}

#[test]
fn a_truncated_pixel_run_is_refused_not_panicked_on() {
    // Half of an RLE file: the row length word at the cut promises more bytes than
    // remain, which is a truncation the decoder must detect, not a panic.
    let mut bytes = std::fs::read("../../assets/BEG/SRTITLE.UP").unwrap();
    bytes.truncate(bytes.len() / 2);
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::Truncated { .. })));
}

/// A file that is genuinely short in the tree. `COMAN/COMBG.UH` is the first
/// 32,768 bytes of `COMAN/COMBG.X`; its `UH` header claims 320 x 200 and its
/// payload is 32,752 of the 64,000 pixels. It is a corrupt asset, and the decoder
/// refusing it is the correct outcome — see the finding in `asset-inventory.md`.
#[test]
fn the_truncated_asset_in_the_tree_is_refused() {
    let bytes = std::fs::read("../../assets/COMAN/COMBG.UH").unwrap();
    // `needed` and `had` are file offsets: 16 header bytes plus the 64,000
    // pixels the header promises, against the 32,768 bytes that exist.
    let err = load(&bytes, Format::Origin).unwrap_err();
    assert!(
        matches!(
            err,
            AssetError::Truncated { format: Format::Origin, needed: 64_016, had: 32_768 }
        ),
        "the shortfall must name the length it wanted: {err}"
    );
}

#[test]
fn an_oversized_header_is_refused_rather_than_clipped() {
    let mut bytes = std::fs::read("../../assets/BEG/SRTITLE.UP").unwrap();
    bytes[2..4].copy_from_slice(&1024u16.to_le_bytes());
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::TooLarge { .. })));
}

/// `START/HZPIC.P` has 153 of its 400 rows decoding wider than the 640 its header
/// claims, up to 761 pixels, and a shipped file relying on it means this is not a
/// malformed file. The original writes every pixel into a 640-byte row buffer and
/// overflows it; the port takes the first `w` and drops the rest, which is what the
/// visible output shows. That substitution is recorded in APPROXIMATIONS.md.
#[test]
fn a_run_row_wider_than_the_header_is_clipped_not_refused() {
    let img = load_path(Path::new("../../assets/START/HZPIC.P")).unwrap();
    assert_eq!((img.width(), img.height()), (640, 400));
    assert_eq!(img.pixels().len(), 640 * 400);
}

#[test]
fn six_bit_is_the_exact_inverse_of_the_engines_widening() {
    for c in 0u8..=63 {
        assert_eq!(six_bit(dac_expand(c)), c, "6-bit value {c}");
    }
}

// ---------------------------------------------------------------------------
// Beyond the brief's list: the rulings and the error paths they name.
// ---------------------------------------------------------------------------

/// The Origin family stores six-bit values already, so a byte above 63 is out of
/// contract: the writer divides an 8-bit `CMAP` by four (`LBM2U.C:170`) and GLVP
/// copies a palette that is six-bit by construction (`GLVP.C:1003`). Nothing in
/// the tree produces one, and clamping would silently change a colour, so it is
/// refused.
#[test]
fn a_palette_byte_above_six_bits_is_refused_not_clamped() {
    let bytes = fc_fc_raw(2, 1, &[[0, 0, 0], [64, 0, 0]], &[0, 1]);
    assert!(
        matches!(load(&bytes, Format::Origin), Err(AssetError::Palette { .. })),
        "a 64 in the palette is not a six-bit value"
    );
}

/// `0x80` is a run of zero. The two readers in the tree disagree about it — the
/// assembly in `ENDPIC/READP.C:44-51` copies 256 pixels, the C in
/// `START/READP.C:37-38` copies none — and the writer never emits it, so refusing
/// it is the only reading that does not invent behaviour.
#[test]
fn a_run_opcode_of_zerox80_is_refused() {
    let bytes = fc_fd_rle(1, 1, 1, &[0x80]);
    assert!(matches!(
        load(&bytes, Format::Origin),
        Err(AssetError::Unsupported { .. })
    ));
}

/// The other half of the same rule: every run length the file format can express
/// is decoded, including the 2 that only the earlier `LBM2P` revision emitted
/// (2,726 runs of it are in `START/HZPIC.P`) and the 127 that the checked-in
/// writer caps at.
#[test]
fn every_run_length_from_one_to_127_is_decoded() {
    // A run of 1, a literal, a run of 2 and a run of 127: the whole range the
    // format can express, not just the 3..127 the checked-in writer emits.
    let bytes = fc_fd_rle(1 + 1 + 2 + 127, 1, 1, &[0x81, 7, 0x40, 0x82, 9, 0xFF, 11]);
    let img = load(&bytes, Format::Origin).unwrap();
    let mut want = vec![7u8];
    want.push(0x40);
    want.extend_from_slice(&[9u8; 2]);
    want.extend_from_slice(&[11u8; 127]);
    assert_eq!(img.pixels(), &want[..]);
}

/// A bare dump is loaded by construction, never guessed: `PANIC/MONSTER.U`,
/// `LENS/LENS.U` and `HARD/FCKOE3.U` are all exactly 64,000 bytes, so a size match
/// proves nothing. `sniff` returns `None` and `load` has no geometry to use.
#[test]
fn a_headerless_dump_is_not_sniffed_from_its_size() {
    let raw = vec![0u8; 64_000];
    assert_eq!(sniff(&raw), None);
    assert!(matches!(load(&raw, Format::Origin), Err(AssetError::UnknownFormat)));
    assert!(matches!(
        load(&raw, Format::OriginBare),
        Err(AssetError::Unsupported { .. })
    ));
}

#[test]
fn load_path_refuses_a_file_it_cannot_identify() {
    assert!(matches!(
        load_path(Path::new("../../assets/PANIC/MONSTER.U")),
        Err(AssetError::UnknownFormat)
    ));
}

/// The plan's Review Focus #1, on the Origin family: every loader returns an
/// `AssetError`, never panics and never reads past its input. A compiler cannot
/// prove the indexing in the row walker safe, so this cuts each real file at a
/// spread of offsets — including every header boundary and every point a row's
/// length prefix lands on — and requires an error rather than a panic.
///
/// The cut files are also the sharpest test of the length checks: a row whose
/// length word survives the cut while its data does not is exactly the case the
/// original's `readp` would walk off the end of.
#[test]
fn no_prefix_of_an_origin_asset_panics_or_decodes_silently() {
    let mut checked = 0;
    let mut refused = 0;
    for path in origin_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        if sniff(&raw) != Some(Format::Origin) {
            continue;
        }
        // Every header boundary is in the sweep by construction (the step is a
        // few hundred bytes at most) and the rest of it lands mid-row.
        let step = (raw.len() / 200).max(1);
        for cut in (0..raw.len()).step_by(step) {
            match load(&raw[..cut], Format::Origin) {
                Ok(img) => {
                    // The one legal case: a cut of a bit-plane file that happens
                    // to leave a whole number of planes, which is a smaller
                    // image in the same format, not a lie about the bytes.
                    assert_eq!(
                        img.pixels().len(),
                        img.width() as usize * img.height() as usize,
                        "{} cut to {cut}: decoded pixels vs header geometry",
                        path.display()
                    );
                }
                Err(_) => refused += 1,
            }
            checked += 1;
        }
    }
    assert!(checked > 1_000, "expected a lot of cuts, made {checked}");
    // The checks are what refuse them, not an early bail on byte 0.
    assert!(refused * 10 > checked * 9, "only {refused} of {checked} cuts refused");
}

/// A still container is one frame, which is what `load_frames` is for: A6 adds the
/// two formats that carry more than one.
#[test]
fn a_still_image_loads_as_one_frame() {
    let bytes = std::fs::read("../../assets/GLENZ/FC.UH").unwrap();
    let frames = load_frames(&bytes, Format::Origin).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!((frames[0].width(), frames[0].height()), (320, 200));
}
