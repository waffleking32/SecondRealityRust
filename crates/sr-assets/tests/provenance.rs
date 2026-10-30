//! Audits of the curated tree's provenance (`assets/PROVENANCE.md`, `asset-inventory.md` §A.2,
//! §C, §D, §E): the one recovered asset, the blobs with no twin, and the shipped pack. They read
//! `upstream/SecondReality` and `assets/` and decode nothing but OMF payloads.

mod common;

use std::path::{Path, PathBuf};

use common::{asset, asset_path, upstream, ASSETS};
use sr_assets::omf::extract_ledata;

fn payload(blob: &str) -> Vec<u8> {
    extract_ledata(&upstream(blob)).unwrap_or_else(|e| panic!("{blob}: {e}"))
}

/// Every file under `root`.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).unwrap_or_else(|e| panic!("{}: {e}", root.display())) {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// `TECHNO/_CIRCLE2.OBK`, linked into the shipped TECHNO (`TECHNO/MAKEFILE:6,23`) and read by
/// `init_interference` (`TECHNO/KOEA.ASM:665`), is the recovered `assets/TECHNO/CIRCLE2.EGA`: a
/// 320x200 one-bit mask of 8,000 bytes.
#[test]
fn the_recovered_mask_is_the_shipped_blob_payload() {
    let payload = payload("TECHNO/_CIRCLE2.OBK");
    assert_eq!(payload.len(), 8_000, "= 320 * 200 / 8, one bit per pixel");
    assert_eq!(asset("TECHNO/CIRCLE2.EGA"), payload);
}

/// The mask is exactly what `DDSTARS/DOIT2.C:21-52` generates (`a = (r/12)&1`, high bit first),
/// and not the stale `DDSTARS/PIC2.EGA`, which differs in 7,162 of 8,000 bytes.
#[test]
fn the_recovered_mask_is_what_the_generator_in_the_tree_writes() {
    let mut generated = Vec::with_capacity(8_000);
    for y in 0..200i32 {
        for x in (0..320i32).step_by(8) {
            let mut byte = 0u8;
            for k in 0..8i32 {
                let dx = (x + k - 320) as f64;
                let dy = (y - 200) as f64;
                let r = (dx * dx + dy * dy).sqrt() as i32;
                if (r / 12) & 1 == 1 {
                    byte |= 1 << (7 - k);
                }
            }
            generated.push(byte);
        }
    }
    assert_eq!(asset("TECHNO/CIRCLE2.EGA"), generated, "DDSTARS/DOIT2.C:21-52");

    let stale = upstream("DDSTARS/PIC2.EGA");
    let differing = generated.iter().zip(&stale).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 7_162, "DDSTARS/PIC2.EGA is not the shipped mask, size aside");
}

/// `TECHNO/_PIC2.OBK`, which no build links, is the last 20,859 bytes of `TECHNO/TROLL.UP`.
#[test]
fn the_unlinked_picture_blob_is_a_slice_of_the_curated_picture() {
    let payload = payload("TECHNO/_PIC2.OBK");
    assert_eq!(payload.len(), 20_859);
    assert!(
        upstream("TECHNO/TROLL.UP").ends_with(&payload),
        "_PIC2.OBK is not the tail of TROLL.UP"
    );
}

/// §E.18's nine bytes (APPROXIMATIONS A20): `ALKU/INCLUDE.OBJ`'s 173,786-byte payload differs from
/// `assets/ALKU/HOI.U` only at offsets 19–27, palette entries 1–3, where the shipped build holds
/// three brighter greys than `HOI.LBM`'s `CMAP / 4`.
#[test]
fn alkus_linked_picture_differs_from_the_curated_copy_in_nine_palette_bytes() {
    let payload = payload("ALKU/INCLUDE.OBJ");
    assert_eq!(payload.len(), 173_786, "the measured payload length");
    let curated = asset("ALKU/HOI.U");

    let differing: Vec<(usize, u8, u8)> = payload
        .iter()
        .zip(&curated)
        .enumerate()
        .filter(|(_, (linked, hoit))| linked != hoit)
        .map(|(at, (linked, hoit))| (at, *linked, *hoit))
        .collect();
    assert_eq!(
        differing,
        vec![
            (19, 10, 2),
            (20, 10, 2),
            (21, 10, 3),
            (22, 30, 5),
            (23, 30, 5),
            (24, 30, 7),
            (25, 63, 6),
            (26, 63, 6),
            (27, 63, 9),
        ],
        "§E.18's nine bytes at offsets 19–27: (offset, linked, assets/ALKU/HOI.U)"
    );
}

/// The distributor's two text files, packed and shown by nothing (§C, §E.12, §F).
const DISTRIBUTOR_TEXT: [&str; 2] = ["FCINFO10.TXT", "README.1ST"];


/// `MAIN/PACKFINA.INC:1-81`, the 79-entry table of what `SECOND.EXE` carries: 22 part executables,
/// 55 files whose bytes are in `assets/`, and the two distributor texts.
#[test]
fn the_shipped_pack_carries_part_executables_and_assets() {
    let manifest = "MAIN/PACKFINA.INC";
    let text = String::from_utf8(upstream(manifest)).unwrap();

    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let declared: usize = lines
        .next()
        .unwrap()
        .trim()
        .strip_prefix("dw ")
        .expect("the table opens with `dw <count>`")
        .parse()
        .unwrap();
    assert_eq!(declared, 79, "{manifest}:1");

    let mut names = Vec::new();
    for line in lines {
        let (fields, name) = line.split_once(';').expect("every row ends in ;NAME");
        assert_eq!(fields.split(',').count(), 6, "{line}");
        names.push(name.trim().to_string());
    }
    assert_eq!(names.len(), 79, "one row per declaration");
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 79, "duplicate pack entry");

    let staged: Vec<(String, Vec<u8>)> = names
        .iter()
        .map(|name| {
            (
                name.clone(),
                upstream(&format!("MAIN/DATA/{name}")),
            )
        })
        .collect();

    let curated = files_under(Path::new(ASSETS));

    let (mut executables, mut in_assets, mut distributor) = (0, 0, 0);
    for (name, bytes) in &staged {
        if name.ends_with(".EXE") {
            executables += 1;
            continue;
        }
        if DISTRIBUTOR_TEXT.contains(&name.as_str()) {
            distributor += 1;
            continue;
        }
        let matches = curated
            .iter()
            .filter(|path| path.file_name().and_then(|n| n.to_str()) == Some(name))
            .any(|path| std::fs::read(path).unwrap() == *bytes);
        assert!(
            matches,
            "{name}: packed ({} bytes) and not a part EXE, but no file of that name in \
             assets/ has its bytes",
            bytes.len()
        );
        in_assets += 1;
    }
    assert_eq!((executables, in_assets, distributor), (22, 55, 2));
    assert_eq!(executables + in_assets + distributor, 79);
}

/// Regenerates the recovered asset from its blob; writes into the repository, so run by hand:
///
/// ```text
/// cargo test -p sr-assets --test provenance -- --ignored write_the_recovered_asset
/// ```
#[test]
#[ignore = "writes into assets/; run by hand to regenerate the recovered file"]
fn write_the_recovered_asset() {
    let out = asset_path("TECHNO/CIRCLE2.EGA");
    std::fs::write(&out, payload("TECHNO/_CIRCLE2.OBK")).unwrap();
    println!("wrote {}", out.display());
}
