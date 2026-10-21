//! HARD's dots: the build-time streams, the sprite engine and the grid table
//! (`HARD/BDO.C`, `HARD/BA.ASM`).
//!
//! `BDO.C` turns the 320×200 `FCRZ.U` picture into four `_dots*.tmp` streams,
//! one per 40-row band: each record is `(value, 0, x-160, y-20)` for a pixel
//! with `0 < value < 4`, and the stream ends with a zero byte
//! (`BDO.C:12-26`). The effect reads those bytes directly, so the port does
//! too: the spawner's `dotsp += 4` before the first read is the source's own
//! skip of record 0, and the `default: dotsp = 0` branch makes the stream
//! restart at record 1 when the terminator is met (`B.C:60-70`, `:100-121`,
//! `:251-261`).
//!
//! The dot array is `_dotxyz`, `NUM = 2048` records of six words
//! (`BA.ASM:7-12`): fixed-point x and y, the velocity pair, a word holding the
//! colour in its low byte and a 66-frame life in its high byte, and the last
//! screen offset. [`add_dot`] is `adddot` (`B.C:84-98`); [`dodots`] is
//! `dot3d` (`BA.ASM:75-122`), [`dodots2`] is `dot3d2` (`:124-171`), and
//! [`do3dots`] is part 2's `do3dots` (`:224-280`).
//!
//! Two source behaviours the pictures depend on: `dot3d2` **decrements the
//! colour** every frame (`dec dl`, `BA.ASM:144`; commented out in `dot3d`,
//! `:95`) and never restores the previous pixel, so a mode-4 trail walks down
//! the palette one entry a frame and dies at index 189; and `dot3d` writes a
//! dying dot's colour into the background buffer as well as the screen
//! (`:87-93`), which is the only way the modes-0..3 logo persists.
//!
//! Every number in this file's tests is reproducible from `assets/HARD/FCRZ.U`
//! (SHA-256 `e2c068fe…`) or `upstream/SecondReality/HARD/_GRID1.TMP`, or is
//! produced by the test that names it (`docs/fidelity/hard.md`, D25).

/// `NUM` (`BA.ASM:7`): the dot array's record count.
pub const DOT_COUNT: usize = 2048;

/// One `_dotxyz` record (`BA.ASM:11`).
///
/// The source's word layout is `(x, y, vx, vy, colour|life<<8, last)`;
/// `x`/`y` are 1/64-pixel positions, `last` the screen offset written for the
/// background restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dot {
    pub x: i16,
    pub y: i16,
    pub vx: i16,
    pub vy: i16,
    pub life: u8,
    pub colour: u8,
    pub last: u16,
}

/// `_dotxyz dw NUM dup(0,32767,0,0,100h,0)` (`BA.ASM:11`): y = 32767, colour
/// 0 and life 1, so every record starts dead.
pub fn initial_dots() -> Vec<Dot> {
    vec![
        Dot {
            x: 0,
            y: 32767,
            vx: 0,
            vy: 0,
            life: 1,
            colour: 0,
            last: 0,
        };
        DOT_COUNT
    ]
}

/// `adddot` (`B.C:84-98`): store one dot in `slot`, then advance and wrap the
/// static `doti`.
///
/// `colour == 0` is the source's kill: words 4 and 5 are set to `0x100` and
/// the record's other words are left alone. Otherwise the six words are the
/// two fixed-point positions, the velocities, the colour with a 66-frame life
/// and the initial screen offset — all in the source's 16-bit `int`
/// arithmetic, which wraps.
pub fn add_dot(dots: &mut [Dot], ax: i16, ay: i16, x: i16, y: i16, colour: u8, slot: &mut u16) {
    let at = (*slot as usize) & (DOT_COUNT - 1);
    if colour == 0 {
        dots[at].life = 1;
        dots[at].colour = 0;
        dots[at].last = 0x100;
    } else {
        let dot = &mut dots[at];
        dot.x = x
            .wrapping_mul(64)
            .wrapping_add(32)
            .wrapping_sub(ax.wrapping_sub(x));
        dot.y = y
            .wrapping_mul(64)
            .wrapping_add(32)
            .wrapping_sub(ay.wrapping_sub(y));
        dot.vx = ax.wrapping_sub(x);
        dot.vy = ay.wrapping_sub(y);
        dot.life = 66;
        dot.colour = colour;
        dot.last = (x as u16).wrapping_add((y as u16).wrapping_mul(320));
    }
    *slot = (*slot + 1) & (DOT_COUNT as u16 - 1);
}

/// `dot3d` (`BA.ASM:75-122`), behind `_dodots` (`:18-34`).
///
/// For each of the 2048 records, in order: a dead record (colour 0) is
/// skipped; a record whose life reaches 0 writes its colour into the
/// background and the screen at `last` and is marked dead with `0x100`;
/// otherwise the previous screen pixel is restored from the background, the
/// fixed-point y then x advance, and the dot draws when `y <= 127` and
/// `x <= 319` (unsigned compares: a negative coordinate is out).
///
/// The x advance happens only after the y test has passed, and the y advance
/// happens before it: `BA.ASM`'s stores are inside the branches, not after
/// them.
pub fn dodots(dots: &mut [Dot], background: &mut [u8], screen: &mut [u8]) {
    for dot in dots.iter_mut() {
        if dot.colour == 0 {
            continue;
        }
        // `dec dh; jz a2` (`:96-97`): the life reaches zero this frame.
        dot.life = dot.life.wrapping_sub(1);
        if dot.life == 0 {
            let at = dot.last as usize;
            if at < background.len() {
                background[at] = dot.colour;
            }
            if at < screen.len() {
                screen[at] = dot.colour;
            }
            dot.life = 1;
            dot.colour = 0;
            continue;
        }
        let at = dot.last as usize;
        if at < background.len() && at < screen.len() {
            screen[at] = background[at];
        }
        dot.y = dot.y.wrapping_add(dot.vy);
        let y = dot.y >> 6;
        // `cmp bx,127; ja a5` — an unsigned compare after `sar`.
        if (y as u16) > 127 {
            continue;
        }
        let row = (y as usize) * 320;
        dot.x = dot.x.wrapping_add(dot.vx);
        let x = dot.x >> 6;
        if (x as u16) > 319 {
            continue;
        }
        let pos = row + x as usize;
        if pos < screen.len() {
            screen[pos] = dot.colour;
        }
        dot.last = pos as u16;
    }
}

/// `dot3d2` (`BA.ASM:124-171`), behind `_dodots2` (`:36-53`).
///
/// The same walk without the background: the previous pixel is not restored
/// (`:148-150`), the last offset is not stored (`:167`), and — the one
/// behaviour the mode-4 picture is made of — **the colour decrements every
/// frame** (`dec dl`, `:144`). When the life reaches 0 the record is marked
/// dead and nothing is drawn; the last drawn colour is therefore index 189
/// after 65 frames from a spawn at 254.
pub fn dodots2(dots: &mut [Dot], screen: &mut [u8]) {
    for dot in dots.iter_mut() {
        if dot.colour == 0 {
            continue;
        }
        dot.colour = dot.colour.wrapping_sub(1);
        dot.life = dot.life.wrapping_sub(1);
        if dot.life == 0 {
            dot.life = 1;
            dot.colour = 0;
            continue;
        }
        dot.y = dot.y.wrapping_add(dot.vy);
        let y = dot.y >> 6;
        if (y as u16) > 127 {
            continue;
        }
        let row = (y as usize) * 320;
        dot.x = dot.x.wrapping_add(dot.vx);
        let x = dot.x >> 6;
        if (x as u16) > 319 {
            continue;
        }
        let pos = row + x as usize;
        if pos < screen.len() {
            screen[pos] = dot.colour;
        }
    }
}

/// Part 2's grid dots, `do3dots` (`BA.ASM:224-280`).
///
/// `points` is part 2's projected point list as `crotprojlist` leaves it: a
/// 16-bit count at word 0, then 16-byte records whose word 0 is x, word 1 y,
/// word 3 the previous screen offset, words 4/5 the 32-bit z (so the load at
/// `BA.ASM:232` is a **16-bit** load of z's low word) and word 6 the shadow
/// offset. The routine clears the previous pixel, clears the two shadow words,
/// skips a record whose z low word is greater than 3000 *signed*
/// (`cmp bp,3000; jg`), otherwise reads `depthcol[bp]` and draws at
/// `rows[y] + x` when `y <= 127`.
///
/// `depthcol` is 8192 bytes with only 0..3099 written (`BA.ASM:221-222`,
/// `BP.C:128-133`). A z whose low word is negative (bit 15 set) passes the
/// signed test and indexes past the array; in the source that read lands in
/// BSS and is zero, which is what the `else` below reproduces. Task 7 builds
/// the table; this task pins the rule.
pub fn do3dots(points: &mut [i16], depthcol: &[u8; 8192], screen: &mut [u8]) {
    let count = points[0];
    if count <= 0 {
        return;
    }
    let mut at = 2usize; // `add si,4` past the count word
    for _ in 0..count {
        if at + 8 > points.len() {
            break;
        }
        let old = points[at + 3] as u16 as usize;
        if old < screen.len() {
            screen[old] = 0;
        }
        let shadow = points[at + 6] as u16 as usize;
        if shadow < screen.len() {
            screen[shadow] = 0;
        }
        if shadow + 320 < screen.len() {
            screen[shadow + 320] = 0;
        }
        // `mov bp,ds:[si+8]`: z's low word, 16-bit.
        let bp = points[at + 4];
        if bp > 3000 {
            // Signed `jg @@2`: a positive z above 3000 is skipped.
            points[at + 3] = 0;
            at += 8;
            continue;
        }
        let index = bp as u16 as usize;
        let colour = if index < depthcol.len() {
            depthcol[index]
        } else {
            // The source reads past its 8192-byte BSS array; the bytes there
            // are zero.
            0
        };
        let y = points[at + 1] as u16 as usize;
        if y > 127 {
            points[at + 3] = 0;
            at += 8;
            continue;
        }
        let x = points[at] as u16 as usize;
        let pos = y * 320 + x;
        points[at + 3] = pos as i16;
        if pos < screen.len() {
            screen[pos] = colour;
        }
        at += 8;
    }
}

/// `BDO.C:12-41`: four 40-row bands of the FCRZ.U picture, each streamed as
/// `(value, 0, x-160, y-20)` records for `0 < value < 4` and terminated by a
/// zero byte.
///
/// The records are kept as **bytes**, not as records, because the effect reads
/// them that way: `dotsp += 4` before the first read skips record 0, and a
/// terminator resets `dotsp` to 0 so the next read is record 1 again.
pub fn dots_from_fcrz(image: &[u8; 64_000]) -> [Vec<u8>; 4] {
    let mut streams: [Vec<u8>; 4] = std::array::from_fn(|_| Vec::new());
    for (band, stream) in streams.iter_mut().enumerate() {
        let ybase = band * 40;
        for x in 0..320usize {
            for y in 0..40usize {
                let value = image[x + y * 320 + ybase * 320];
                if value > 0 && value < 4 {
                    stream.push(value);
                    stream.push(0);
                    stream.push((x as i32 - 160) as i8 as u8);
                    stream.push((y as i32 - 20) as i8 as u8);
                }
            }
        }
        stream.push(0);
    }
    streams
}

/// The 400-byte `_GRID1.TMP` table: 200 words, 100 `(x, y)` pairs reused by
/// `doit2` (`B.C:112-121`) as a 200-word cycle.
///
/// The file's last byte is `0x00`; the linked `_grid1.obj` data in `B.EXE` at
/// file offset 69,265 matches the file for 399 of 400 bytes and holds `0x4F`
/// there, so the last word is `0x4F5A` in the binary and `0x005A` here. The
/// table is consumed from the file (`docs/fidelity/hard.md` records the
/// difference; the last word is only the y of the 100th pair).
pub const GRID1: [i16; 200] = [
    120, 31, 128, 31, 136, 31, 144, 31, 152, 31, 160, 31, 167, 31, 175, 31, 183, 31, 191, 31, 120,
    38, 128, 38, 136, 38, 144, 38, 152, 38, 160, 38, 167, 38, 175, 38, 183, 38, 191, 38, 120, 44,
    128, 44, 136, 44, 144, 44, 152, 44, 160, 44, 167, 44, 175, 44, 183, 44, 191, 44, 120, 51, 128,
    51, 136, 51, 144, 51, 152, 51, 160, 51, 167, 51, 175, 51, 183, 51, 191, 51, 120, 58, 128, 58,
    136, 58, 144, 58, 152, 58, 160, 58, 167, 58, 175, 58, 183, 58, 191, 58, 120, 64, 128, 64, 136,
    64, 144, 64, 152, 64, 160, 64, 167, 64, 175, 64, 183, 64, 191, 64, 120, 70, 128, 70, 136, 70,
    144, 70, 152, 70, 160, 70, 167, 70, 175, 70, 183, 70, 191, 70, 120, 77, 128, 77, 136, 77, 144,
    77, 152, 77, 160, 77, 167, 77, 175, 77, 183, 77, 191, 77, 120, 83, 128, 83, 136, 83, 144, 83,
    152, 83, 160, 83, 167, 83, 175, 83, 183, 83, 191, 83, 120, 90, 128, 90, 136, 90, 144, 90, 152,
    90, 160, 90, 167, 90, 175, 90, 183, 90, 191, 90,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The curated FCRZ.U, from the crate's directory (the tests run there).
    fn fcrz() -> [u8; 64_000] {
        let raw = std::fs::read("../../assets/HARD/FCRZ.U").expect("assets/HARD/FCRZ.U");
        let mut image = [0u8; 64_000];
        image.copy_from_slice(&raw);
        image
    }

    fn upstream(path: &str) -> Vec<u8> {
        std::fs::read(format!("../../upstream/SecondReality/HARD/{path}"))
            .unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn the_dot_streams_are_the_bdo_extraction() {
        let image = fcrz();
        // The picture's histogram, `BDO.C:15`'s `value<4 && value` rule.
        let mut histogram = [0usize; 256];
        for &value in &image {
            histogram[value as usize] += 1;
        }
        assert_eq!(
            [
                histogram[0],
                histogram[1],
                histogram[2],
                histogram[3],
                histogram[4]
            ],
            [60_645, 164, 441, 2_737, 13]
        );

        let streams = dots_from_fcrz(&image);
        let counts: Vec<usize> = streams.iter().map(|s| (s.len() - 1) / 4).collect();
        assert_eq!(counts, [859, 690, 722, 1_071]);
        assert_eq!(
            streams.iter().map(Vec::len).collect::<Vec<_>>(),
            [3_437, 2_761, 2_889, 4_285]
        );
        for stream in &streams {
            assert_eq!(*stream.last().unwrap(), 0, "the zero terminator");
            for record in stream[..stream.len() - 1].chunks_exact(4) {
                assert!((1..=3).contains(&record[0]));
                assert_eq!(record[1], 0);
            }
        }

        // The cross-check: `_dots1.obj`..`_dots4.obj` are linked into B.EXE
        // and the four streams sit at 55,875 / 59,315 / 62,082 / 64,977. The
        // first and last match byte for byte; the middle two differ only in
        // their final terminator byte, where the next linked object's bytes
        // abut (`docs/fidelity/hard.md`).
        let exe = upstream("B.EXE");
        for (band, offset) in [55_875usize, 59_315, 62_082, 64_977].iter().enumerate() {
            let stream = &streams[band];
            let read = &exe[*offset..*offset + stream.len()];
            let differing: Vec<usize> = stream
                .iter()
                .zip(read)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(at, _)| at)
                .collect();
            if band == 0 || band == 3 {
                assert!(differing.is_empty(), "band {band}: {differing:?}");
                assert_eq!(read.last(), Some(&0));
            } else {
                assert_eq!(differing, vec![stream.len() - 1], "band {band}");
                assert_eq!(*read.last().unwrap(), [56u8, 72][band - 1]);
            }
        }
    }

    #[test]
    fn the_grid_table_is_the_shipped_grid1() {
        let raw = upstream("_GRID1.TMP");
        assert_eq!(raw.len(), 400);
        let words: Vec<i16> = raw
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(GRID1[..], words[..]);
        assert_eq!((GRID1[0], GRID1[1]), (120, 31));
        assert_eq!((GRID1[198], GRID1[199]), (191, 90));

        // The measured one-byte difference: B.EXE's `_grid1.obj` data at
        // 69,265 matches 399 of 400 bytes; the last byte is 0x4F against the
        // file's 0x00, so the binary's last word would be 0x4F5A.
        let exe = upstream("B.EXE");
        let linked = &exe[69_265..69_265 + 400];
        let differing: Vec<usize> = raw
            .iter()
            .zip(linked)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(at, _)| at)
            .collect();
        assert_eq!(differing, vec![399]);
        assert_eq!(linked[399], 0x4F);
        assert_eq!(raw[399], 0x00);
    }

    fn spawn_at() -> (Vec<Dot>, u16) {
        let mut dots = initial_dots();
        let mut slot = 0u16;
        add_dot(&mut dots, 160, 64, 160, 64, 3, &mut slot);
        (dots, slot)
    }

    #[test]
    fn the_dot_engine_leaves_the_66_frame_trail() {
        // `adddot(x*64+32-(a-x), ...)` for a dot spawned at rest: the six
        // words are the source's (`B.C:84-98`).
        let (mut dots, mut slot) = spawn_at();
        assert_eq!(slot, 1);
        let dot = dots[0];
        assert_eq!((dot.x, dot.y), (160 * 64 + 32, 64 * 64 + 32));
        assert_eq!((dot.vx, dot.vy), (0, 0));
        assert_eq!((dot.life, dot.colour), (66, 3));
        assert_eq!(dot.last, 160 + 64 * 320);

        let mut background = vec![0u8; 64_000];
        let mut screen = vec![0u8; 64_000];
        // A velocity of 64 fixed-point units is one pixel a frame in x.
        add_dot(&mut dots, 224, 64, 160, 64, 3, &mut slot);

        // `adddot` stores the dot one velocity-step behind its nominal
        // position, so the first frame draws both records at 160; from the
        // second frame the moving record restores 160 and draws at 161.
        dodots(&mut dots, &mut background, &mut screen);
        assert_eq!(screen[64 * 320 + 160], 3);
        dodots(&mut dots, &mut background, &mut screen);
        assert_eq!(screen[64 * 320 + 160], 0);
        assert_eq!(screen[64 * 320 + 161], 3);

        // `dot3d` restores the old pixel from the background and draws the
        // new one: after 66 frames the first dot is dead, its death colour is
        // in the background, and the screen keeps it at the last position.
        let mut dots2 = initial_dots();
        let mut slot2 = 0;
        add_dot(&mut dots2, 100, 100, 100, 100, 3, &mut slot2);
        let mut background = vec![0u8; 64_000];
        let mut screen = vec![0u8; 64_000];
        for _ in 0..66 {
            dodots(&mut dots2, &mut background, &mut screen);
        }
        assert_eq!((dots2[0].life, dots2[0].colour), (1, 0), "66 frames");
        assert_eq!(screen[100 * 320 + 100], 3);
        assert_eq!(background[100 * 320 + 100], 3);

        // The clip is unsigned after the `sar`: y = 128 and x = 320 are out,
        // and a negative coordinate is out too (16-bit `ja`).
        for (x, y) in [(160i16, 128i16), (320, 64), (-1, 64), (160, -1)] {
            let mut dots = initial_dots();
            let mut slot = 0;
            add_dot(&mut dots, x, y, x, y, 3, &mut slot);
            let mut background = vec![0u8; 64_000];
            let mut screen = vec![0u8; 64_000];
            dodots(&mut dots, &mut background, &mut screen);
            assert_eq!(
                screen.iter().filter(|&&pixel| pixel != 0).count(),
                0,
                "({x}, {y}) must be clipped"
            );
        }

        // `dodots2` leaves the trail: the previous pixel is never restored,
        // and the colour decrements every frame.
        let mut dots3 = initial_dots();
        let mut slot3 = 0;
        add_dot(&mut dots3, 100, 100, 100, 100, 5, &mut slot3);
        let mut trail = vec![0u8; 64_000];
        for frame in 0..4 {
            dodots2(&mut dots3, &mut trail);
            assert_eq!(trail[100 * 320 + 100], 4 - frame, "frame {frame}");
        }
    }

    #[test]
    fn the_shadow_grid_dot_reads_bss_zero_past_3100() {
        // The mode-4 spawn leaves the trail at index 189 after 65 draws: the
        // colour starts at 254 and `dec dl` runs before every draw
        // (`BA.ASM:144`). The last drawn value is 254 - 65 = 189, which is
        // also the entry `B.C:299` tests when mode 5 rebuilds the band.
        let dot = Dot {
            x: 100 * 64 + 32,
            y: 100 * 64 + 32,
            vx: 0,
            vy: 0,
            life: 66,
            colour: 254,
            last: 0,
        };
        let mut dots = vec![dot; 1];
        let mut screen = vec![0u8; 64_000];
        let mut last = 0;
        for _ in 0..65 {
            dodots2(&mut dots, &mut screen);
            last = dots[0].colour;
        }
        assert_eq!(last, 189);

        // `do3dots`' 16-bit z load and its signed `cmp bp,3000`:
        let depthcol = {
            let mut table = [0u8; 8192];
            for (a, slot) in table.iter_mut().enumerate().take(3100) {
                *slot = if a < 1000 {
                    127
                } else {
                    (127 - (a as i32 - 1000) / 32) as u8
                };
            }
            table
        };
        let mut screen = vec![0u8; 64_000];
        let mut points = vec![0i16; 2 + 8];
        points[0] = 1;
        points[2] = 40; // x
        points[3] = 60; // y
        points[4] = 0; // word 2
        points[5] = 0; // last
        points[6] = 1500; // z low word -> depthcol[1500]
        points[7] = 0; // z high word
        do3dots(&mut points, &depthcol, &mut screen);
        assert_eq!(screen[60 * 320 + 40], depthcol[1500]);
        assert_eq!(points[5], (60 * 320 + 40) as i16);

        // z = 0xFF00 (negative as the 16-bit load): the signed `jg` does not
        // skip it, and the index past the 8,192-byte table reads BSS zero.
        points[6] = 0xFF00u16 as i16;
        points[5] = 0;
        do3dots(&mut points, &depthcol, &mut screen);
        assert_eq!(screen[60 * 320 + 40], 0);

        // z low word 3001..32767 is positive and skipped without a read.
        points[6] = 3001;
        points[5] = 0;
        let before = screen.clone();
        do3dots(&mut points, &depthcol, &mut screen);
        assert_eq!(screen, before);
        assert_eq!(points[5], 0, "a skipped record clears its last offset");
    }
}
