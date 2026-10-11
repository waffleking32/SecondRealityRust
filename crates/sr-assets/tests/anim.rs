//! PAM's animation, in both the formats the tree shows: the FLI the part was
//! authored in (`PAM/PRAX4.FLI`) and the `.ANI` record scheme the part's own
//! packer made from it (`PAM/OUT.ANI`).
//!
//! Every file these tests read is a curated copy under `assets/`, verified
//! byte-for-byte against `upstream/SecondReality/` at commit `071a82e` by task
//! A1. The two decoders are pinned against the tree's own reader and writer, and
//! against each other — the `.ANI` was packed from the frames the FLI player
//! dumped (`PAM/ANIM.C:24-33` reads `out.u`, the 64,000-byte frames
//! `PAM/VFLI.C:111` writes), so the 42 `.ANI` frames and the 42 FLI frames must
//! come out byte-identical, and a decoder that got the delta base, the record
//! forms or the canvas wrong would show it here rather than in a rendering
//! comparison four phases later.
//!
//! The measurements the tests pin, all taken from the shipped bytes:
//!
//! - `PRAX4.FLI`'s header says 41 frames at offset 6 and the record walk finds
//!   42 — one `0x0F` byte-run key frame and 41 `0x0C` line-compressed deltas —
//!   and the last of the 42 is the first again, pixel for pixel.
//! - Its only colour chunk is a `0x000B` in frame 0, and its 768 bytes are
//!   `PAM/PAL.PAL` exactly. `VFLI.C` has both conversions — `:158` writes
//!   `0x000B`'s bytes to the DAC untouched and `:170` shifts `0x0004`'s down two
//!   — so the two rules the crate documents are both in the authority, and this
//!   file takes the verbatim one.
//! - `OUT.ANI`'s five `db` fragments in the assembly source split it at 65,535
//!   bytes and add one trailing `0xFF` (`PAM/OUT.IN4`'s `db 0,-1`), which is
//!   source-file padding, not a byte of the animation.

use sr_assets::{load, load_frames, pal, sniff, AssetError, Format, Image};

fn asset(rel: &str) -> Vec<u8> {
    let path = format!("../../assets/{rel}");
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn fli_frames() -> Vec<Image> {
    let fli = asset("PAM/PRAX4.FLI");
    load_frames(&fli, Format::Fli).unwrap_or_else(|e| panic!("PRAX4.FLI: {e}"))
}

fn ani_frames() -> Vec<Image> {
    let ani = asset("PAM/OUT.ANI");
    load_frames(&ani, Format::Ani).unwrap_or_else(|e| panic!("OUT.ANI: {e}"))
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

/// FNV-1a over a frame's 64,000 pixels: one number that pins the whole picture,
/// so "the first frame is stable" is checked against all of it and not against
/// the corners.
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

// -------------------------------------------------------------------------------------------
// The FLI
// -------------------------------------------------------------------------------------------

/// The frame count is the record walk's, not the header word's. `VFLI.C:66`
/// reads offset 6 into `pics` and nothing in the tree ever reads `pics` again
/// (the player walks records until `feof` at `VFLI.C:71-77`); the word says 41,
/// the walk finds 42, and the last frame repeats the first, which is what a
/// looping animation's writer would leave behind. Both numbers are pinned here
/// so that "41" and "42" can never quietly swap.
#[test]
fn the_fli_decodes_to_the_frames_its_own_record_walk_finds() {
    let fli = asset("PAM/PRAX4.FLI");
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
    assert_eq!(fnv1a64(first), 0xd527_b14f_0319_fd6a);

    assert_eq!(
        frames[41].pixels(),
        frames[0].pixels(),
        "the 42nd frame is the first again — the animation's loop point"
    );
}

/// The property that separates a delta decoder from a per-frame renderer. Each
/// of these frames is one `0x000C` chunk covering a band of lines (frame 1's
/// body opens `lb=96, count=8`, `VFLI.C:178-179`), so everything outside the band
/// has to hold the previous frame's value — and pixel 8000 (row 25, column 0) is
/// a non-zero one, so a decoder that rendered each frame on a fresh canvas would
/// put black there and fail rather than merely differ.
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

/// The ruling that has bitten this crate five times: which palette rule a format
/// needs is measured, not assumed, and the test has to fail under the wrong one.
///
/// `VFLI.C` holds both rules: `0x000B` goes to the DAC untouched (`:158`) and
/// `0x0004` is shifted down two (`:170`). This file's only colour chunk is a
/// `0x000B` (`VFLI.C:149-160`), and its payload is `PAM/PAL.PAL` byte for byte —
/// the palette `pal.rs` already rules on as six-bit verbatim, the one
/// `PAM/ANIM.C:17-21` reads and hands to `tw_setpalette` (`PAM/TWEAK.ASM:151-156`
/// writes it to ports 0x3C8/0x3C9). So the frames' palette is that file's, entry
/// for entry.
///
/// The counts make the failure mode explicit: 743 of the palette's 768 bytes
/// change under `six_bit` (the 6→8 display rule run backwards) and 743 under the
/// IFF's `/4`, and entry 3's `(6, 6, 7)` becomes `(1, 1, 2)` and `(1, 1, 1)`
/// respectively. A `<= 63` assertion would pass under all three rules.
#[test]
fn the_fli_colour_chunk_is_six_bit_and_comes_out_verbatim() {
    let raw = asset("PAM/PAL.PAL");
    let pal_pal = pal::decode(&raw).unwrap();

    for (i, frame) in fli_frames().iter().enumerate() {
        assert_eq!(frame.palette, pal_pal, "frame {i}'s palette");
    }

    assert_eq!(pal_pal.entry(3), [6, 6, 7]);
    assert_eq!(
        [
            sr_assets::six_bit(6),
            sr_assets::six_bit(6),
            sr_assets::six_bit(7)
        ],
        [1, 1, 2],
        "six_bit run over an already-six-bit palette darkens it"
    );
    assert_eq!([6 / 4, 6 / 4, 7 / 4], [1, 1, 1], "and so does the 8→6 /4");
    assert_eq!(
        raw.iter().filter(|&&v| sr_assets::six_bit(v) != v).count(),
        743
    );
    assert_eq!(raw.iter().filter(|&&v| v / 4 != v).count(), 743);
}

/// Truncation is refused at the byte the decoder stopped, and the two fields are
/// file offsets (`sr_assets::AssetError`). Three shapes: a file shorter than the
/// 128-byte header; a file that stops inside a frame record's own 16-byte header
/// (`VFLI.C:74-82`); and one that stops inside a frame, whose size word
/// (`VFLI.C:73-76`) promises the rest.
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

/// No input panics, and a truncated one never invents a frame: every cut either
/// fails or decodes a prefix of the whole file's frames, each identical to its
/// counterpart. The stride is prime so the cuts land all over the record
/// structure rather than on its boundaries.
///
/// A 128-byte file is the header alone: no records, no frames, and `Ok` of an
/// empty list rather than a refusal — there is nothing short about it. One byte
/// less is short of the header, and one byte more is short of the first record.
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

// -------------------------------------------------------------------------------------------
// The ANI
// -------------------------------------------------------------------------------------------

/// The cross-check that makes both decoders evidence for each other: the `.ANI`
/// was packed from the very frames the FLI player dumped, so all 42 have to come
/// out identical, byte for byte, from two readers that share no code. A wrong
/// delta base, a misread run form or an off-by-one in the alignment would break
/// the pair long before frame 42.
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

/// A test-local walk of `ANIM.C`'s record stream — an independent decoder, so if
/// the crate's own walk were wrong this one would have to be wrong the same way
/// to hide it. `base` returns the canvas a frame starts from, which is the one
/// thing the file itself cannot say.
fn walk_records(ani: &[u8], base: impl Fn(&[Vec<u8>]) -> Vec<u8>) -> Vec<Vec<u8>> {
    const CANVAS: usize = 64_000;
    let mut frames: Vec<Vec<u8>> = Vec::new();
    let mut pos = 0;
    while pos < ani.len() {
        let mut canvas = base(&frames);
        assert_eq!(canvas.len(), CANVAS);
        let mut cursor = 0;
        let mut p = pos;
        loop {
            let run = ani[p] as i8;
            p += 1;
            if run == 0 {
                break;
            }
            if run > 0 {
                canvas[cursor..cursor + run as usize].fill(ani[p]);
                cursor += run as usize;
                p += 1;
            } else {
                cursor += -run as usize;
            }
        }
        frames.push(canvas);
        pos = p + (16 - p % 16) % 16; // ANIM.C:42 pads, ASMYT.ASM:191-198 rounds up
    }
    frames
}

/// The record scheme `ANIM.C` writes and `ASMYT.ASM` reads, plus the base that
/// makes it work. The shipped decoder and this walk agree frame for frame, and
/// both agree with the FLI — so the scheme is pinned from three directions.
///
/// The base is the frame *two* back (`PAM/OUTTAA.C:51-60` alternates two screen
/// pages with `tw_setstart`, decoding each frame into the page the frame two
/// back is still on), which is why `PAM/ANIM.C:28-30` copies the previous frame
/// into `VRAM` while `mpage1` still holds the one before, and why frames 0 and 1
/// come off a cleared screen (`PAM/TWEAK.ASM:20-33` clears it).
#[test]
fn the_ani_record_scheme_is_the_one_anim_c_writes_and_asmy_reads() {
    let ani = asset("PAM/OUT.ANI");
    let walked = walk_records(&ani, |out| match out.len() {
        0 | 1 => vec![0u8; 64_000],
        n => out[n - 2].clone(),
    });
    let decoded = ani_frames();
    let fli = fli_frames();
    assert_eq!(walked.len(), 42);
    for (i, (w, d)) in walked.iter().zip(&decoded).enumerate() {
        assert_eq!(
            w,
            d.pixels(),
            "the test-local walk and the decoder, frame {i}"
        );
        assert_eq!(w, fli[i].pixels(), "and the FLI, frame {i}");
    }
}

/// The base, measured rather than asserted by construction: with a
/// one-frame-back base — the obvious reading of "a delta from the previous
/// frame" — only 4 of the 42 frames come out right, because the page the frame
/// lands in still shows the frame two back. If the shipped decoder ever
/// regressed to a one-frame-back base, `the_ani_decodes_to_the_fli_frames` would
/// catch it; this test says why that base is wrong.
#[test]
fn a_one_frame_back_base_reproduces_only_four_of_the_frames() {
    let ani = asset("PAM/OUT.ANI");
    let fli = fli_frames();
    let wrong = walk_records(&ani, |out| {
        out.last().cloned().unwrap_or_else(|| vec![0u8; 64_000])
    });
    let matching = (0..fli.len())
        .filter(|&i| wrong[i] == fli[i].pixels())
        .count();
    assert_eq!(
        matching, 4,
        "the frames whose delta happens to rewrite every pixel"
    );
}

/// `ANIM.C:42` pads the output to a 16-byte boundary before every frame and
/// `ASMYT.ASM:191-198` computes the next frame's segment the same way, so a
/// frame start is a fact about the file the decoder and the reader must agree
/// on. Frame 0 opens `0x88` — a 120-pixel skip, the encoder's own cap
/// (`ANIM.C:48,54`, `a<120`) — and the last frame's `0x00` terminator is the
/// file's last byte: the padding comes before a frame, so there is none after
/// the last one.
#[test]
fn the_ani_starts_every_frame_on_a_sixteen_byte_boundary_and_pads_with_zeros() {
    let ani = asset("PAM/OUT.ANI");
    assert_eq!(ani[0], 0x88, "frame 0 opens with a skip of 120");
    assert_eq!(
        *ani.last().unwrap(),
        0x00,
        "the last frame's terminator ends the file"
    );

    // Walk the records to find each frame's start and the terminator after it.
    let mut starts = Vec::new();
    let mut pos = 0;
    let mut padding = 0;
    while pos < ani.len() {
        starts.push(pos);
        let mut p = pos;
        loop {
            let run = ani[p] as i8;
            p += 1;
            if run == 0 {
                break;
            }
            p += usize::from(run > 0); // a positive run carries one colour byte
        }
        let next = p + (16 - p % 16) % 16;
        if next <= ani.len() {
            padding += next - p;
            assert!(
                ani[p..next].iter().all(|&b| b == 0),
                "the {} pad bytes after frame {} are zeros (ANIM.C:42 writes fputc(0))",
                next - p,
                starts.len() - 1
            );
        }
        pos = next;
    }
    assert_eq!(starts.len(), 42);
    assert!(
        starts.iter().all(|s| s % 16 == 0),
        "every frame start is 16-byte aligned"
    );
    assert_eq!(
        padding, 303,
        "the file's total padding between its 42 frames; the padding comes before a frame, \
         so the last one has none and the file ends on its terminator"
    );
}

/// `PAM/INCLUDE.ASM:9-26` includes the five fragments into five para-aligned
/// segments so TASM could carry the animation inside the part's own executable
/// (`PAM/PAM.PRJ`'s `memblock` is the first), which is the same trick
/// `PAM/PAL.INC` plays with the palette. They are TASM `db` source, not raw
/// bytes: values above 127 are written unsigned (`136` for `0x88`), and the one
/// negative is `OUT.IN4`'s trailing `db 0,-1`, which is `0x00 0xFF` — the last
/// frame's terminator and then a byte that is not part of any frame
/// (`ALKU/HOI.IN1` shows the same `db 0,-1`). So the five join to `OUT.ANI`
/// exactly, plus that one `0xFF`.
#[test]
fn the_five_in_fragments_are_the_ani_as_tasm_db_source() {
    fn parse_db(path: &str) -> Vec<u8> {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let mut out = Vec::new();
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix("db") else {
                continue;
            };
            for value in rest.split(',') {
                let value: i32 = value
                    .trim()
                    .parse()
                    .unwrap_or_else(|e| panic!("{path}: {e}"));
                assert!(
                    (-128..=255).contains(&value),
                    "{path}: {value} is not a byte"
                );
                out.push(value as u8);
            }
        }
        out
    }

    let parts: Vec<Vec<u8>> = (0..5)
        .map(|i| parse_db(&format!("../../assets/PAM/OUT.IN{i}")))
        .collect();
    assert_eq!(
        parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
        vec![65_535, 65_535, 65_535, 65_535, 62_486],
        "65,535 bytes is what TASM's INCLUDE took at a time; the fifth is the remainder"
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

    let in4 = std::fs::read_to_string("../../assets/PAM/OUT.IN4").unwrap();
    assert!(
        in4.trim_end().ends_with("db\t136,136,136,241,0,-1"),
        "the 0xFF is OUT.IN4's last db line, not a byte of the animation"
    );
}

/// A short `.ANI` is refused at the byte it stops, in the same shape as the FLI:
/// a run with no colour byte, and a frame with no terminator. A frame's records
/// end where `ANIM.C:59` wrote the `0x00`; nothing else bounds them.
#[rstest::rstest]
#[case(&[0x7F], 2, 1)]
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

/// Runs are bounded by the canvas the record stream describes, 64,000 pixels
/// (`ANIM.C:44`) — and the encoder cannot overrun it (`cnt+a<64000` caps the
/// last run), so a stream that does is malformed and is refused rather than
/// wrapped (`ASMYT.ASM:64-105` would write past the screen, into the planar
/// words the pixels do not own).
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

/// No input panics, and a truncated `.ANI` never invents a frame either: every
/// cut fails or decodes a prefix of the whole file's frames. The stride is prime
/// so the cuts land inside records, on terminators and in the padding.
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

// -------------------------------------------------------------------------------------------
// The doors, and synthetic input
// -------------------------------------------------------------------------------------------

/// One frame record carrying one chunk, on the real header, so a chunk the tree
/// never uses can be handed to the decoder. `VFLI.C:74-87` fixes both headers'
/// shape: a frame record is a `u32` size, the `0xF1FA` type, a chunk count and
/// eight ignored bytes (`:82`), and a chunk is a `u32` size, its type, and a body
/// that the size includes.
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

/// A colour chunk counts and indexes big-endian — the one place in either format
/// that does. `VFLI.C:152-156` builds both fields as `getc()*256 + getc()`, and
/// the file agrees: `01 00` is 256 colours, and 4 + 256*3 is the chunk's 778
/// bytes less its six-byte header exactly. Reading them little-endian would give
/// one colour and leave 765 bytes unread; the tests above, which compare the
/// palette with `PAM/PAL.PAL` entry for entry, are what catches that.
#[test]
fn the_colour_chunk_counts_and_indexes_are_big_endian() {
    let fli = asset("PAM/PRAX4.FLI");
    let body = &fli[150..922]; // frame 0's 0x000B chunk: 144 + 6 header, size 778
    assert_eq!(word(&fli, 148), 0x000B, "the chunk type");
    assert_eq!([body[0], body[1]], [0x01, 0x00], "count, big-endian");
    assert_eq!([body[2], body[3]], [0x00, 0x00], "first entry, big-endian");
    assert_eq!(4 + 256 * 3, 778 - 6, "the payload fills the chunk exactly");
}

/// The forms `VFLI.C` does not implement are refused, not skipped: `:275-277`
/// prints the type and `getch()`s, and the caller then seeks past the chunk
/// (`:88`), which would leave a frame with missing pixels — a picture no file in
/// the tree asks for. `0x0011` is `FLI_COPY` and `0x0012` the postage stamp;
/// neither appears in `PRAX4.FLI`, whose chunks are one `0x000B`, one `0x000F`
/// and 41 `0x000C`s (measured).
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

/// A chunk that outruns its own frame is refused at the frame's end, not read
/// into the next frame's bytes the way `VFLI.C:87-88` would (it decodes the body
/// with no bound at all and only re-seeks afterwards).
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

/// The colour chunk's own contract: a range that leaves the palette, a byte that
/// is not six bits, and a body shorter than the count it declares. `VFLI.C`
/// would write past its 768-byte array, hand a nine-bit value to a six-bit DAC
/// and read the next chunk's bytes; `Palette::set_entry` would mask, and `Pal`
/// is the variant the crate already uses for a palette outside its contract.
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

/// `load_frames` is the door for both formats, and `load` — which returns one
/// picture — refuses them by naming it. `sniff` still answers from magic alone,
/// so the `.ANI`, which has none, is never sniffed; and a FLI with no magic is
/// `UnknownFormat` rather than a zero-frame answer.
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
    // The three formats with their own door keep refusing `load_frames`.
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
