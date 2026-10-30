//! PAM's animation: the FLI the part was authored in (`PAM/PRAX4.FLI`) and the `.ANI` record
//! stream its packer made from it (`PAM/OUT.ANI`), each against the tree's reader and against
//! the other.

mod common;

use common::{asset, fnv1a};
use sr_assets::{load, load_frames, pal, sniff, AssetError, Format, Image};

fn fli_frames() -> Vec<Image> {
    load_frames(&asset("PAM/PRAX4.FLI"), Format::Fli).unwrap_or_else(|e| panic!("PRAX4.FLI: {e}"))
}

fn ani_frames() -> Vec<Image> {
    load_frames(&asset("PAM/OUT.ANI"), Format::Ani).unwrap_or_else(|e| panic!("OUT.ANI: {e}"))
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

// The FLI

/// The stored magic is `0xAF11` (`asset-inventory.md` §E.5); the header's frame word says 41
/// (`VFLI.C:66` reads it and nothing uses it) and the record walk finds 42, the last the first
/// again. The key frame's pixel sum and digest are regression pins.
#[test]
fn the_fli_decodes_to_the_frames_its_own_record_walk_finds() {
    let fli = asset("PAM/PRAX4.FLI");
    assert_eq!(
        &fli[4..6],
        &[0x11, 0xAF],
        "the stored magic is 0xAF11 — the classic FLI, not 0xAF12"
    );
    assert_eq!(
        sniff(&fli),
        Some(Format::Fli),
        "the FLI magic is 11 AF at offset 4"
    );
    assert_eq!(
        word(&fli, 6),
        41,
        "the header's frame word, which VFLI.C:66 reads and drops"
    );
    let frames = load_frames(&fli, Format::Fli).unwrap();
    assert_eq!(
        frames.len(),
        42,
        "42 records: VFLI.C:79-84 counts a frame per 0xF1FA record"
    );
    assert!(frames.iter().all(|f| (f.width(), f.height()) == (320, 200)));

    let first = frames[0].pixels();
    assert_eq!(first.len(), 64_000);
    assert_eq!(
        first.iter().filter(|&&b| b != 0).count(),
        48_320,
        "the key frame is a byte-run chunk (VFLI.C:247-272) that writes the whole screen"
    );
    assert_eq!(first.iter().map(|&b| b as u64).sum::<u64>(), 1_378_265);
    assert_eq!(fnv1a(first), 0xd527_b14f_0319_fd6a);

    assert_eq!(
        frames[41].pixels(),
        frames[0].pixels(),
        "the 42nd frame is the first again — the animation's loop point"
    );
}

/// A `0x000C` delta covers a band of lines (frame 1's opens `lb=96, count=8`, `VFLI.C:178-179`);
/// pixel 8000, outside every band here and not black, keeps the previous frame's value.
#[rstest::rstest]
#[case(1, 67)]
#[case(2, 134)]
#[case(3, 202)]
#[case(20, 4270)]
fn a_delta_frame_leaves_the_pixels_it_does_not_touch_alone(
    #[case] frame: usize,
    #[case] differing: usize,
) {
    let frames = fli_frames();
    let prev = frames[frame - 1].pixels();
    let now = frames[frame].pixels();
    let changed = (0..prev.len()).filter(|&i| prev[i] != now[i]).count();
    assert_eq!(
        changed, differing,
        "frame {frame}'s delta covers {differing} pixels"
    );
    assert_eq!(
        prev.len() - changed,
        64_000 - differing,
        "the rest are carried over"
    );

    assert_eq!(
        prev[8000], 36,
        "row 25 is outside frame {frame}'s band and is not black"
    );
    assert_eq!(
        now[8000], 36,
        "so the untouched pixel still holds the previous frame's 36"
    );
}

/// The only colour chunk is a `0x000B`, copied verbatim: every frame's palette is
/// `PAM/PAL.PAL` (`PAM/ANIM.C:17-21`), entry for entry.
#[test]
fn the_fli_colour_chunk_is_six_bit_and_comes_out_verbatim() {
    let pal_pal = pal::decode(&asset("PAM/PAL.PAL")).unwrap();
    assert_eq!(pal_pal.entry(3), [6, 6, 7]);
    for (i, frame) in fli_frames().iter().enumerate() {
        assert_eq!(frame.palette, pal_pal, "frame {i}'s palette");
    }
}

/// A FLI cut short of its header, inside a frame record's header (`VFLI.C:74-82`), or inside a
/// frame its size word promises (`VFLI.C:73-76`) is refused with the byte it wanted.
#[rstest::rstest]
#[case(0, 128)]
#[case(100, 128)]
#[case(5000, 15258)]
#[case(15268, 15274)]
#[case(15300, 15376)]
fn a_short_fli_is_refused_at_the_byte_it_stops(#[case] cut: usize, #[case] needed: usize) {
    let mut fli = asset("PAM/PRAX4.FLI");
    fli.truncate(cut);
    match load_frames(&fli, Format::Fli) {
        Err(AssetError::Truncated {
            format,
            needed: n,
            had,
        }) => {
            assert_eq!(format, Format::Fli);
            assert_eq!(n, needed, "the byte the decoder wanted");
            assert_eq!(had, cut, "the last byte it had");
        }
        other => panic!("a FLI cut at {cut} must be Truncated, not {other:?}"),
    }
}

/// Every cut fails or decodes a prefix of the whole file's frames. The header alone (128 bytes)
/// is `Ok` of no frames.
#[test]
fn no_fli_prefix_panics_or_produces_a_frame_the_whole_file_does_not() {
    let fli = asset("PAM/PRAX4.FLI");
    let whole = fli_frames();

    for len in (0..=200).filter(|&len| len != 128) {
        assert!(
            load_frames(&fli[..len], Format::Fli).is_err(),
            "a {len}-byte FLI has no complete record"
        );
    }
    assert!(load_frames(&fli[..128], Format::Fli).unwrap().is_empty());
    for cut in (0..fli.len()).step_by(4999) {
        if let Ok(prefix) = load_frames(&fli[..cut], Format::Fli) {
            assert!(prefix.len() <= whole.len(), "cut at {cut}");
            for (i, frame) in prefix.iter().enumerate() {
                assert_eq!(frame.pixels(), whole[i].pixels(), "cut at {cut}, frame {i}");
            }
        }
    }
}

// The ANI

/// The `.ANI` was packed from the frames the FLI player dumped (`PAM/ANIM.C:24-33` reads what
/// `PAM/VFLI.C:111` writes), so two readers that share no code must agree on all 42.
#[test]
fn the_ani_decodes_to_the_fli_frames() {
    let fli = fli_frames();
    let ani = ani_frames();
    assert_eq!(ani.len(), 42);
    assert_eq!(ani.len(), fli.len());
    for (i, (a, b)) in ani.iter().zip(&fli).enumerate() {
        assert_eq!((a.width(), a.height()), (320, 200));
        assert_eq!(a.pixels(), b.pixels(), "frame {i} of OUT.ANI vs PRAX4.FLI");
    }
}

/// Every frame starts on a 16-byte boundary (`ANIM.C:42`, `ASMYT.ASM:191-198`) and the padding
/// is zeros, 303 bytes in all; the last frame's terminator ends the file.
#[test]
fn the_ani_starts_every_frame_on_a_sixteen_byte_boundary_and_pads_with_zeros() {
    let ani = asset("PAM/OUT.ANI");
    let (mut frames, mut padding, mut pos) = (0, 0, 0);
    while pos < ani.len() {
        let mut p = pos;
        loop {
            let run = ani[p] as i8;
            p += 1;
            if run == 0 {
                break;
            }
            p += usize::from(run > 0); // a positive run carries one colour byte
        }
        let next = (p + (16 - p % 16) % 16).min(ani.len());
        assert!(ani[p..next].iter().all(|&b| b == 0), "the pad after frame {frames}");
        padding += next - p;
        frames += 1;
        pos = next;
    }
    assert_eq!((frames, padding), (42, 303));
    assert_eq!(*ani.last().unwrap(), 0x00);
}

/// `PAM/INCLUDE.ASM:9-26` includes `OUT.ANI` as five TASM `db` fragments of at most 65,535
/// bytes. They join to the file exactly, plus `OUT.IN4`'s trailing `db 0,-1`: the last
/// terminator and one `0xFF` that is no part of any frame.
#[test]
fn the_five_in_fragments_are_the_ani_as_tasm_db_source() {
    fn parse_db(text: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for line in String::from_utf8_lossy(text).lines() {
            let Some(rest) = line.trim().strip_prefix("db") else {
                continue;
            };
            for value in rest.split(',') {
                let value: i32 = value.trim().parse().unwrap();
                assert!((-128..=255).contains(&value), "{value} is not a byte");
                out.push(value as u8);
            }
        }
        out
    }

    let parts: Vec<Vec<u8>> = (0..5).map(|i| parse_db(&asset(&format!("PAM/OUT.IN{i}")))).collect();
    assert_eq!(
        parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
        vec![65_535, 65_535, 65_535, 65_535, 62_486],
    );
    let joined: Vec<u8> = parts.concat();
    let ani = asset("PAM/OUT.ANI");
    assert_eq!(joined.len(), ani.len() + 1);
    assert_eq!(
        &joined[..ani.len()],
        &ani[..],
        "the fragments are the animation"
    );
    assert_eq!(
        joined[ani.len()],
        0xFF,
        "and the one byte past it is the db 0,-1 artefact"
    );

    let in4 = String::from_utf8(asset("PAM/OUT.IN4")).unwrap();
    assert!(
        in4.trim_end().ends_with("db\t136,136,136,241,0,-1"),
        "the 0xFF is OUT.IN4's last db line, not a byte of the animation"
    );
}

/// A run with no colour byte, a skip with no terminator after it (`0x80`, `i8::MIN`, which
/// `ANIM.C`'s 120 cap never writes), and a frame with no terminator are refused where they stop.
#[rstest::rstest]
#[case(&[0x7F], 2, 1)]
#[case(&[0x80], 2, 1)]
#[case(&[0x05, 0x09, 0x03], 4, 3)]
fn a_short_ani_is_refused_at_the_byte_it_stops(
    #[case] bytes: &[u8],
    #[case] needed: usize,
    #[case] had: usize,
) {
    match load_frames(bytes, Format::Ani) {
        Err(AssetError::Truncated {
            format,
            needed: n,
            had: h,
        }) => {
            assert_eq!(format, Format::Ani);
            assert_eq!((n, h), (needed, had));
        }
        other => panic!("{bytes:?} must be Truncated, not {other:?}"),
    }
}

/// Runs are bounded by the 64,000-pixel canvas (`ANIM.C:44`); skips alone within it are a frame.
#[test]
fn an_ani_run_past_the_canvas_is_refused() {
    let over = [0x7Fu8, 0x09].repeat(505); // 505 x 127 = 64,135 pixels
    assert!(matches!(
        load_frames(&over, Format::Ani),
        Err(AssetError::Unsupported {
            format: Format::Ani,
            ..
        })
    ));
    let skips = [0x88u8].repeat(501); // 501 x 120 = 60,120: skips alone are allowed
    let mut skips = skips;
    skips.push(0x00); // the frame's terminator (ANIM.C:59)
    assert_eq!(load_frames(&skips, Format::Ani).unwrap().len(), 1);
}

/// Every cut fails or decodes a prefix of the whole file's frames.
#[test]
fn no_ani_prefix_panics_or_produces_a_frame_the_whole_file_does_not() {
    let ani = asset("PAM/OUT.ANI");
    let whole = ani_frames();
    for cut in (0..ani.len()).step_by(4099) {
        if let Ok(prefix) = load_frames(&ani[..cut], Format::Ani) {
            assert!(prefix.len() <= whole.len(), "cut at {cut}");
            for (i, frame) in prefix.iter().enumerate() {
                assert_eq!(frame.pixels(), whole[i].pixels(), "cut at {cut}, frame {i}");
            }
        }
    }
    assert!(
        load_frames(&[], Format::Ani).unwrap().is_empty(),
        "no records, no frames"
    );
}

/// Malformed input is an `AssetError`, never a panic, for any bytes: 512 seeded strings through
/// both decoders, some drawn from the extreme length and run bytes (`0x80` overflows a naive
/// `-run`). Chunk dispatch is `synthetic_fli`'s job, below.
#[test]
fn random_byte_strings_are_refused_rather_than_panicked_on() {
    // xorshift64, fixed seed: reproducible, and no dependency for a test helper.
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    for case in 0..512 {
        let len = (next() % 512) as usize;
        let mut bytes = Vec::with_capacity(len);
        while bytes.len() < len {
            bytes.extend_from_slice(&next().to_le_bytes());
        }
        bytes.truncate(len);
        if case % 8 == 0 {
            for byte in bytes.iter_mut() {
                *byte = [0x00u8, 0x01, 0x7F, 0x80, 0x88, 0xFF][(*byte as usize) % 6];
            }
        }
        let _ = load_frames(&bytes, Format::Ani);
        let _ = load_frames(&bytes, Format::Fli);
    }
}

// Synthetic input and the doors

/// One frame record (`VFLI.C:74-87`) carrying one chunk, on the real header.
fn synthetic_fli(chunk_type: u16, body: &[u8]) -> Vec<u8> {
    let mut file = asset("PAM/PRAX4.FLI");
    file.truncate(128);
    let chunk = 6 + body.len();
    file.extend_from_slice(&((16 + chunk) as u32).to_le_bytes());
    file.extend_from_slice(&0xF1FAu16.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&[0u8; 8]);
    file.extend_from_slice(&(chunk as u32).to_le_bytes());
    file.extend_from_slice(&chunk_type.to_le_bytes());
    file.extend_from_slice(body);
    file
}

/// A chunk type `VFLI.C` does not implement (`:275-277` prints it and skips it) is refused.
#[rstest::rstest]
#[case(0x0011)]
#[case(0x0012)]
#[case(0x000D)]
fn a_chunk_type_vfli_does_not_implement_is_refused(#[case] chunk_type: u16) {
    let file = synthetic_fli(chunk_type, &[0u8; 8]);
    match load_frames(&file, Format::Fli) {
        Err(AssetError::Unsupported { format, what }) => {
            assert_eq!(format, Format::Fli);
            assert!(
                what.contains(&format!("{chunk_type:#06x}")),
                "names the type: {what}"
            );
        }
        other => panic!("chunk {chunk_type:#06x} must be Unsupported, not {other:?}"),
    }
}

/// A chunk that outruns its frame is refused at the frame's end (`VFLI.C:87-88` has no bound).
#[test]
fn a_chunk_that_outruns_its_frame_is_refused() {
    let mut file = asset("PAM/PRAX4.FLI");
    file.truncate(128);
    let body = 10;
    file.extend_from_slice(&((16 + 6 + body) as u32).to_le_bytes());
    file.extend_from_slice(&0xF1FAu16.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&[0u8; 8]);
    file.extend_from_slice(&400u32.to_le_bytes()); // the chunk claims 400 bytes in a 32-byte frame
    file.extend_from_slice(&0x000Bu16.to_le_bytes());
    file.extend_from_slice(&[0u8; 10]);
    match load_frames(&file, Format::Fli) {
        Err(AssetError::Truncated { needed, had, .. }) => {
            assert_eq!(
                needed,
                128 + 16 + 400,
                "the byte past the chunk the file stops short of"
            );
            assert_eq!(had, 128 + 32, "which is the frame's end");
        }
        other => panic!("must be Truncated, not {other:?}"),
    }
}

/// A colour range past 256, a byte above 63, and a body shorter than its count are refused.
#[test]
fn a_colour_chunk_outside_its_contract_is_refused() {
    // 256 entries from entry 1: one past the end.
    let mut body = vec![0x01, 0x00, 0x00, 0x01];
    body.extend_from_slice(&[0u8; 768]);
    assert!(matches!(
        load_frames(&synthetic_fli(0x000B, &body), Format::Fli),
        Err(AssetError::Palette { .. })
    ));

    // One entry, from 0, whose red is 64 — not a six-bit DAC value.
    let mut body = vec![0x00, 0x01, 0x00, 0x00];
    body.extend_from_slice(&[64, 0, 0]);
    assert!(matches!(
        load_frames(&synthetic_fli(0x000B, &body), Format::Fli),
        Err(AssetError::Palette { .. })
    ));

    // 256 entries declared, three bytes present: the body stops short, and both
    // fields are file offsets — the body starts at 128 + 16 + 6 = 150.
    let body = vec![0x01, 0x00, 0x00, 0x00, 0, 0, 0];
    match load_frames(&synthetic_fli(0x000B, &body), Format::Fli) {
        Err(AssetError::Truncated { needed, had, .. }) => {
            assert_eq!(needed, 150 + 4 + 768);
            assert_eq!(had, 150 + 7);
        }
        other => panic!("must be Truncated, not {other:?}"),
    }
}

/// `load` refuses both animations and names `load_frames`; the `.ANI` has no magic; a FLI without
/// one is `UnknownFormat`; and the formats with their own readers refuse `load_frames`.
#[test]
fn load_frames_is_the_door_and_load_says_so() {
    for (rel, format) in [("PAM/PRAX4.FLI", Format::Fli), ("PAM/OUT.ANI", Format::Ani)] {
        match load(&asset(rel), format) {
            Err(AssetError::Unsupported { what, .. }) => {
                assert!(what.contains("load_frames"), "{rel}: {what}")
            }
            other => panic!("{rel} through load must be Unsupported, not {other:?}"),
        }
    }
    assert_eq!(sniff(&asset("PAM/PRAX4.FLI")), Some(Format::Fli));
    assert_eq!(
        sniff(&asset("PAM/OUT.ANI")),
        None,
        "the .ANI scheme has no magic"
    );
    assert!(matches!(
        load_frames(&vec![0u8; 500], Format::Fli),
        Err(AssetError::UnknownFormat)
    ));
    for (bytes, format) in [
        (asset("PAM/PAL.PAL"), Format::Pal),
        (asset("PAM/OUT.ANI"), Format::Omf),
    ] {
        assert!(matches!(
            load_frames(&bytes, format),
            Err(AssetError::Unsupported { .. })
        ));
    }
}
