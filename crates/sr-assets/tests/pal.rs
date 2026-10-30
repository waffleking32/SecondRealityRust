//! `.PAL`: 768 bytes of six-bit VGA palette, against the five files the demo ships.

mod common;

use common::{asset, assets_with, iff_chunk};
use rstest::rstest;
use sr_assets::{load, pal, sniff, AssetError, Format};

fn pal_assets() -> Vec<Vec<u8>> {
    let files: Vec<Vec<u8>> = assets_with(&["PAL"]).iter().map(|p| std::fs::read(p).unwrap()).collect();
    assert_eq!(files.len(), 5, "the curated tree carries five .PAL files");
    files
}

/// Any length but 768 is refused; `needed` and `had` are the wanted and the actual length.
#[rstest]
#[case(0)]
#[case(767)]
#[case(769)]
fn a_pal_file_that_is_not_768_bytes_is_refused(#[case] len: usize) {
    let err = pal::decode(&vec![0u8; len]).unwrap_err();
    assert!(
        matches!(err, AssetError::Truncated { format: Format::Pal, needed: 768, had } if had == len),
        "{err}"
    );
}

/// Every entry is the file's own three bytes. A `<= 63` check would pass a palette divided by
/// four a second time; only the equality catches that.
#[test]
fn every_pal_asset_decodes_to_its_own_bytes() {
    for raw in pal_assets() {
        let p = pal::decode(&raw).unwrap();
        for (i, rgb) in raw.chunks_exact(3).enumerate() {
            assert_eq!(p.entry(i as u8), [rgb[0], rgb[1], rgb[2]], "entry {i}");
        }
    }
}

/// Three `.PAL` files are the `/4` of the `CMAP` of the picture beside them (`GRAB/LBM2U.C:170`),
/// all 768 bytes — and the IFF decoder, which converts the `CMAP` itself, agrees.
#[rstest]
#[case("START/HZPIC.PAL", "START/HZPIC.LBM")]
#[case("VISU/C/SCENE/U2E.PAL", "VISU/C/SCENE/U2E.LBM")]
#[case("VISU/C/U2A.PAL", "VISU/C/SCENE/U2A.LBM")]
fn a_pal_file_is_the_originals_truncation_of_the_pictures_cmap(#[case] pal_file: &str, #[case] lbm_file: &str) {
    let lbm = asset(lbm_file);
    let (_, cmap) = iff_chunk(&lbm, b"CMAP");
    assert_eq!(cmap.len(), 768, "{lbm_file}");
    let p = pal::decode(&asset(pal_file)).unwrap();
    for i in 0..256 {
        let want = [cmap[i * 3] / 4, cmap[i * 3 + 1] / 4, cmap[i * 3 + 2] / 4];
        assert_eq!(p.entry(i as u8), want, "{pal_file}: entry {i}");
    }
    assert_eq!(load(&lbm, Format::Iff).unwrap().palette, p, "{pal_file}");
}

/// A byte above 63 is refused, in any channel, and the message names the entry and the value.
#[rstest]
#[case(0)]
#[case(1)]
#[case(2)]
fn a_palette_byte_above_six_bits_is_refused(#[case] channel: usize) {
    let mut bytes = vec![0u8; 768];
    bytes[100 * 3 + channel] = 200;
    let err = pal::decode(&bytes).unwrap_err();
    let AssetError::Palette { what } = &err else {
        panic!("expected a Palette error, got {err}");
    };
    assert!(what.contains("100") && what.contains("200"), "{what}");
}

/// A `.PAL` has no magic, and a size identifies nothing, so `sniff` never answers `Pal` and
/// `load` names `pal::decode`.
#[test]
fn a_pal_is_loaded_by_construction_and_never_sniffed() {
    for raw in pal_assets() {
        assert_eq!(sniff(&raw), None);
        let err = load(&raw, Format::Pal).unwrap_err();
        assert!(
            matches!(err, AssetError::Unsupported { format: Format::Pal, ref what } if what.contains("pal::decode")),
            "{err}"
        );
    }
    assert_eq!(sniff(&[0u8; 768]), None);
    // A RIX3 file, whose own palette sits at offset 10, is Rix3.
    assert_eq!(sniff(&asset("WATER/BKG.CLX")), Some(Format::Rix3));
}

#[test]
fn no_prefix_of_a_pal_asset_is_accepted() {
    for raw in pal_assets() {
        for cut in (0..raw.len()).step_by(7) {
            assert!(pal::decode(&raw[..cut]).is_err(), "cut to {cut}");
        }
    }
}
