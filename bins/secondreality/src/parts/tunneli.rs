//! TUNNELI — the dot tunnel: 77 rings of 64 dots, chained, one plot per dot.
//!
//! `TUNNELI/TUN10.PAS` (227 lines of Turbo Pascal with an inline-assembly
//! inner loop) is the part; `TUNNELI/X.BAT:1` stages `tun10.exe` as
//! `tunneli.exe`, so the shipped build is TUN10 (`TUN9.PAS` is the earlier
//! revision: `veke` 980, a different palette ramp at `:118-119`, the chain copy
//! commented out at `:204`, and the music exit at `:223` — the one TUN10
//! removes).
//!
//! The source's own acts, in order:
//!
//! 1. `:99-104` — the tables: `rows[x] := x*320`, the two sine tables copied
//!    out of `sinit.obj`, and `pcalc` copied out of `tunneli.obj`.
//! 2. `:110-115` — `mov ax,$13 / int $10`, mode 13h, which the *part* sets
//!    rather than the shell.
//! 3. `:121-128` — the DAC: two 65-entry grey ramps at 64 and 128, entries 68
//!    and 132 forced black, entry 255 green.
//! 4. `:150-227` — the loop. One vblank (`waitr`, `:44-53`) per iteration; draw
//!    all 77 rings (`:152-198`); then one animation step (`:200-225`) that
//!    computes a new head at `putki[100]`, shifts the chain one record along,
//!    and sets the head's colour. `veke = 1060` (`:3`) frames, and the part
//!    quits at `:221`.
//!
//! Four things about that loop are the port's real work, and each is
//! `docs/fidelity/tunneli.md`'s:
//!
//! - **The clip is one-sided** (`:179-190`). `cmp di,319 / ja @yli` tests the
//!   x sum only; the y term then indexes `rows[bx]` **unchecked**, and the
//!   word that read lands on is whatever the data segment holds there. The
//!   port keeps that shape (§[`Tunneli::rows_word`]) because the original
//!   *draws* those dots: on the reference's own frames the unchecked read
//!   accounts for 85–146 pixels that no bounded port reproduces.
//! - **The colour immediate is self-modified** (`:168`
//!   `mov byte ptr cs:[@c+3],al`). The port makes it a variable, which is what
//!   spec §5.2 says to do with self-modifying code.
//! - **`waitr` rewrites DAC entry 0 every frame** (`:46`, `:52`), from the BIOS
//!   keyboard flags: black, or `(0,0,15)` when Scroll Lock is set. The port has
//!   no keyboard flags and writes the black (`frame`).
//! - **The last 102 frames set the new ring's colour to 0** (`:220`), and with
//!   `c = 0` a ring's `bbc` is below 64 — so those rings are not drawn at all
//!   (`:161`) and the screen is left to the erases of the rings still moving.
//!
//! The picture is written through `Surface::pixels_mut()` **at the original's
//! own `LINE_LEN` of 320** (Part B's second convention): mode 13h addresses
//! video memory as `x + y*320`, so `View::y_line_offset` is set to 320 and the
//! surface's first 64 KB *is* the original's video memory, wraparound and all.

use std::path::Path;

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "tunneli";

/// The programs' two data files, read in their original form. Both are the
/// linked data segments of the originals' `.OBJ`s (`asset-inventory.md` records
/// `TUNNELI/SINIT.OBJ`→`SINIT.DAT` and `TUNNELI/TUNNELI.OBJ`→`TUNNEL.DAT`), and
/// **neither is a picture**: they carry no magic, so `sr_assets::sniff` returns
/// `None` and `load_path` refuses them; `raw_surface` would need a geometry the
/// caller invents. `the_data_files_are_not_pictures` pins both facts, and
/// `docs/fidelity/tunneli.md` §2 records why the port reads the record arrays
/// itself.
const TUNNEL_PATH: &str = "assets/TUNNELI/TUNNEL.DAT";
const SINIT_PATH: &str = "assets/TUNNELI/SINIT.DAT";

/// `:3` `const veke = 1060; { frame count to exit }`.
const VEKE: u32 = 1060;

/// `:220` `if frame >= veke-102 then putki[99].c := 0`: the part's tick 958 is
/// the first whose new ring is drawn in colour 0.
const FADE_FIRST_FRAME: u32 = VEKE - 102;

/// `:17` `putki : array[0..102] of rengas` — 103 records, of which `[0..100]`
/// are the chain.
const CHAIN: usize = 103;

/// `:204`'s `move(putki[1], putki[0], ofs(putki[100])-ofs(putki[0]))`: the
/// count is the *offset difference*, so it is 100 records — `putki[0..99]`
/// take `putki[1..100]`, and each is the previous one's data six bytes along.
const CHAIN_COPY: usize = 100;

/// `:153` `for x := 80 downto 4 do`: rings 4..=80 inclusive, 77 of them, each
/// drawn with the ellipse `sade[x]` selects.
const RING_BOTTOM: usize = 4;
const RING_TOP: usize = 80;
/// Rings drawn by one pass of `:153` — 77 — which the ring-count test pins.
#[cfg(test)]
const RINGS: usize = RING_TOP - RING_BOTTOM + 1;

/// `:170` `mov cx,64`: every ring's ellipse is 64 dots.
const DOTS_PER_RING: usize = 64;

/// `:18` `pcalc : array[0..137,0..63] of bc` — 138 rows of 64 `(x, y)` pairs,
/// which is exactly the 35,328 bytes of `TUNNEL.DAT`.
const PCALC_ROWS: usize = 138;

/// `:67` `oldpos : array[0..7500] of word`. The inner loop indexes it with the
/// *byte* offset `ax` (`:175-176` `mov bx,ax / mov di,word ptr ds:oldpos[bx]`),
/// so the 77 rings' 4,928 dots use its first 9,856 bytes.
const OLDPOS_WORDS: usize = 7501;

/// `:24-25` `sinit : array[0..4096] of word; cosit : array[0..2048] of word`,
/// and `:102-103` copy 4,097 and 2,048 words of them out of `sinit.obj`.
/// `SINIT.DAT` is 12,292 bytes = 4,097 + 2,049 words: the file has one word
/// more than the second `move` reads, and the port keeps the same count.
const SINIT_WORDS: usize = 4097;
const COSIT_WORDS: usize = 2049;
const COSIT_COPIED: usize = 2048;

/// `:21`/`:101` `rows : array[0..200] of word`, `rows[x] := x*320`: the byte
/// offset of row `x` in mode 13h's linear video memory.
const ROWS_LAST: usize = 200;
const ROW_BYTES: usize = 320;

/// `:110-115`'s mode 13h is 320×200 of one byte per pixel, addressed
/// `x + y*320` — `View::y_line_offset` in the port's pixel units.
const LINE_LEN: u16 = 320;
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;

/// Mode 13h's video memory: 64 KB, which is what makes the 16-bit address
/// arithmetic at `:180`/`:186` wrap where it does.
const VIDEO_BYTES: usize = 65_536;

/// `:71`'s global `y`, set to 0 at `:93` and never written again. It appears in
/// `:203` as `sinit[y and 4095]`, which is therefore `sinit[0]` — 0.
const UNWRITTEN_Y: u16 = 0;

/// `:148` `sade[z] := round(16384 div ((Z*7)+95))` for `z = 0..100`; `:26`
/// declares 103 of them and only those are written.
const SADE_WORDS: usize = 103;

/// The window words around `rows` that [`Tunneli::rows_word`] models, and the
/// two the match arms below use: forward to the end of `sade`, and — past the
/// 64 KB wrap — back to `pcalc`'s first word. Words outside those two ranges
/// are not modelled and read as 0; the test
/// `the_unchecked_read_covers_everything_the_arithmetic_can_reach` measures
/// that the arithmetic can never ask for one.
const MODEL_FORWARD_LAST: usize = SADE_LAST;
const MODEL_BACKWARD_FIRST: usize = PCALC_FIRST;

/// The window's word offsets, counted from `rows[0]`: `aa2` — 101 words of
/// 64000 then 100 never-written zeros — then `sinit`, `cosit` and `sade`; and,
/// past the 64 KB wrap, `pcalc` read backwards and then `aa1`.
const AA2_FIRST: usize = ROWS_LAST + 1;
const AA1_COPY: usize = 101;
/// The last of `aa2`'s copied words, and the first of its never-written ones.
const AA2_COPY_LAST: usize = AA2_FIRST + AA1_COPY - 1;
const AA2_ZERO_FIRST: usize = AA2_COPY_LAST + 1;
/// `aa2` is `array[0..200]` (`:22`): 201 words, of which the first 101 are the
/// copy of `aa1`.
const AA2_WORDS: usize = 2 * AA1_COPY - 1;
const AA2_LAST: usize = AA2_FIRST + AA2_WORDS - 1;
const SINIT_FIRST: usize = AA2_LAST + 1;
const SINIT_LAST: usize = SINIT_FIRST + SINIT_WORDS - 1;
const COSIT_FIRST: usize = SINIT_LAST + 1;
const COSIT_LAST: usize = COSIT_FIRST + COSIT_WORDS - 1;
const SADE_FIRST: usize = COSIT_LAST + 1;
const SADE_LAST: usize = SADE_FIRST + SADE_WORDS - 1;
const PCALC_WORDS: usize = PCALC_ROWS * DOTS_PER_RING * 2;
const AA1_FIRST: usize = 32_768 - AA1_COPY;
const PCALC_LAST: usize = AA1_FIRST - 1;
const PCALC_FIRST: usize = PCALC_LAST + 1 - PCALC_WORDS;

/// `:99-100`'s 64000, the value every word of `aa1` — and so of `aa2` — holds.
const AA1_VALUE: u16 = 64_000;

/// One ring's record, `:11-14` `rengas = record x, y: integer; c: byte; end` —
/// six bytes, which is the "six bytes further along" of `:204`. `x` and `y` are
/// `integer` in the source and every use of them is 16-bit (`_bx`, `by`, `di`,
/// `bx` are all words, and `:203` wraps), so the port stores them unsigned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Ring {
    x: u16,
    y: u16,
    c: u8,
}

/// The dot tunnel.
pub struct Tunneli {
    /// Where the two data files are read from. The paths are fields so a unit
    /// test can point them at the same files through a path that resolves from
    /// this crate's directory (a unit test's working directory is not the
    /// repository root, and `tests/tunneli.rs` runs the real binary from there).
    tunnel_path: &'static str,
    sinit_path: &'static str,

    /// `:24` `sinit`, and `:25` `cosit`, as `:102-103` copy them.
    sinit: Vec<u16>,
    cosit: Vec<u16>,
    /// `:18` `pcalc`, in the file's own word layout: two words per dot, `x`
    /// then `y`. Kept as words rather than pairs because the unchecked read of
    /// [`Tunneli::rows_word`]'s neighbourhood walks this memory as words.
    pcalc: Vec<u16>,
    /// `:26` `sade`, computed at `:148`.
    sade: [u16; SADE_WORDS],
    /// `:17` `putki`.
    putki: [Ring; CHAIN],
    /// `:67` `oldpos`.
    oldpos: Vec<u16>,
    /// `:69` `ry` — the inner loop's **byte** offset into `oldpos`, reset to 0
    /// at `:152` and advanced by 2 per dot at `:192`.
    ry: u16,
    /// `:76` `sx`, `:76` `sy`: the two animation counters, one step per tick.
    sx: u16,
    sy: u16,
}

impl Tunneli {
    /// A part with no tables: `init` is what reads them.
    pub fn new() -> Tunneli {
        Tunneli {
            tunnel_path: TUNNEL_PATH,
            sinit_path: SINIT_PATH,
            sinit: Vec::new(),
            cosit: Vec::new(),
            pcalc: Vec::new(),
            sade: [0; SADE_WORDS],
            putki: [Ring::default(); CHAIN],
            oldpos: Vec::new(),
            ry: 0,
            sx: 0,
            sy: 0,
        }
    }

    /// [`new`](Tunneli::new) reading the two data files by `path`-relative
    /// names. A unit test's working directory is this crate's, not the
    /// repository root, so the part's own relative paths do not resolve there.
    #[cfg(test)]
    fn at(tunnel_path: &'static str, sinit_path: &'static str) -> Tunneli {
        Tunneli { tunnel_path, sinit_path, ..Tunneli::new() }
    }

    /// The word `:186`'s `add di, word ptr ds:rows[bx]` reads for the 16-bit
    /// index `yy` — **the original's read is unchecked, and this is it**.
    ///
    /// `rows` is `array[0..200] of word` and the assembly indexes it with
    /// `bx = (by + pcalc[br][i].y) * 2`, a byte offset it never tests. The
    /// effective address is `DS:rows_off + bx`, and the data segment is 64 KB,
    /// so the offset wraps; the word grid is two bytes, so the *word* index
    /// wraps at 32,768. What that lands on is the neighbouring globals, whose
    /// layout the declaration order fixes (`TUN10.PAS:16-29`):
    ///
    /// ```text
    ///   … putki, pcalc, aa1[0..100], rows[0..200], aa2[0..200], sinit[0..4096],
    ///     cosit[0..2048], sade[0..102], frame, bor, frames, oldpos, …
    /// ```
    ///
    /// `aa1` is filled with 64000 at `:99` and copied into `aa2` at `:100`, so
    /// the words just either side of `rows` are 64000 (the rest of `aa2` is
    /// never written and is zero), the words after that are the sine tables,
    /// and the words *before* `aa1` are `pcalc` — this part's own 35,328-byte
    /// table, read backwards.
    ///
    /// **Nothing here is guessed at the port's convenience: the reference's own
    /// frames decide it.** On `frame_004200.rgb`, `frame_004500.rgb` and
    /// `frame_004600.rgb`, 85, 146 and 90 pixels of the port's frame at the
    /// aligned tick are *only* drawn if this read is the original's — a port
    /// that treated an out-of-range y as 0, or that clipped it away, agrees
    /// with the reference on 0–4 of them (`docs/fidelity/tunneli.md` §4).
    ///
    /// The reachable range is `-680..=680` (the head's y is
    /// `sinit[..] - cosit[..] ∈ [-222, 211]`, `by` is a difference of two of
    /// those, and `pcalc`'s own y runs to ±247), so the windows this needs are
    /// the first 680 words after `rows` and the first 680 before it.
    /// `the_unchecked_read_covers_everything_the_arithmetic_can_reach` measures
    /// that bound against the tables themselves.
    fn rows_word(&self, yy: u16) -> u16 {
        // The wrap the 16-bit offset performs, in words.
        let k = (yy % 32_768) as usize;
        match k {
            // `:101` `rows[x] := x*320`.
            0..=ROWS_LAST => (k * ROW_BYTES) as u16,
            // `aa2` — `:100`'s copy of `aa1`, and the rest of it is never
            // written.
            AA2_FIRST..=AA2_COPY_LAST => AA1_VALUE,
            AA2_ZERO_FIRST..=AA2_LAST => 0,
            // `:24`'s `sinit`, as `:102` copies it.
            SINIT_FIRST..=SINIT_LAST => self.sinit[k - SINIT_FIRST],
            // `:25`'s `cosit`, 2,048 words of it: the 2,049th is the one
            // `:103`'s `move` never writes.
            COSIT_FIRST..=COSIT_LAST => self.cosit.get(k - COSIT_FIRST).copied().unwrap_or(0),
            // `:26`'s `sade`, of which `:148` writes the first 101.
            SADE_FIRST..=MODEL_FORWARD_LAST => self.sade[k - SADE_FIRST],
            // Before `aa1`, the memory is `pcalc`'s last words, read backwards:
            // this is the part's own 35,328-byte table.
            MODEL_BACKWARD_FIRST..=PCALC_LAST => self.pcalc[PCALC_WORDS - 1 - (PCALC_LAST - k)],
            // `aa1` itself, every word of it 64000 (`:99`).
            AA1_FIRST.. => AA1_VALUE,
            // Between `sade` and `pcalc` lie `frame`, `bor`, `frames`, `oldpos`
            // and the scalars, which the arithmetic above cannot reach.
            _ => 0,
        }
    }

    /// `:152-198`: one pass over the rings, newest first, one plot per dot.
    fn draw(&mut self, memory: &mut [u8]) {
        // `:152` `ry := 0`, and `:196` `mov RY,ax` leaves it where this stops.
        self.ry = 0;
        for x in (RING_BOTTOM..=RING_TOP).rev() {
            let ring = self.putki[x];
            // `:155` `_bx := putki[x].x-putki[5].x` — the ring's offset from
            // `putki[5]`, in words.
            let bx = ring.x.wrapping_sub(self.putki[5].x);
            // `:156` `by := putki[x].y-putki[5].y`.
            let by = ring.y.wrapping_sub(self.putki[5].y);
            // `:157` `br := sade[x]` — which of `pcalc`'s ellipses this ring is.
            let br = self.sade[x] as usize;
            // `:158` `bbc := putki[x].c+round(x / 1.3)`, a byte.
            let colour = ring.c.wrapping_add(ring_colour(x));
            // `:161` `if bbc >= 64 then` — the whole assembly block, erase and
            // plot alike, is skipped for a ring whose colour is below 64. That
            // is what the fade's `c = 0` does: it stops drawing the ring
            // rather than drawing it in entry 0.
            if colour < 64 {
                continue;
            }

            // `:159` `pcp := ofs(pcalc[br][0])`, and `:191` `add si,4`.
            let base = br * DOTS_PER_RING * 2;
            for dot in 0..DOTS_PER_RING {
                // `:175-176` `mov bx,ax / mov di,word ptr ds:oldpos[bx]`: `ax`
                // is a byte offset, so this is word `ry / 2`.
                let slot = (self.ry / 2) as usize;
                // `:177` `mov byte ptr es:[di],0` — the erase, which runs
                // whether or not the dot plots.
                memory[self.oldpos[slot] as usize] = 0;

                // `:179-180` `mov di,word ptr ds:[si] / add di,dx`.
                let mut di = self.pcalc[base + dot * 2].wrapping_add(bx);
                // `:181-182` `cmp di,319 / ja @yli` — **the only bound**, and
                // it is on the x sum alone. A y that is out of range is not
                // clipped; it is read out of the data segment below.
                if di <= 319 {
                    // `:183-186`: the y term, unchecked, through `rows`.
                    let y = by.wrapping_add(self.pcalc[base + dot * 2 + 1]);
                    di = di.wrapping_add(self.rows_word(y));
                    // `:187` `mov byte ptr es:[di],15` — the colour immediate
                    // `:168` patched. The port passes `colour`.
                    memory[di as usize] = colour;
                }
                // `:189-190` `mov bx,ax / mov word ptr ds:oldpos[bx],di` — the
                // offset stored is the *final* one, plotted or not.
                self.oldpos[slot] = di;
                // `:192` `add ax,2`.
                self.ry = self.ry.wrapping_add(2);
            }
        }
    }

    /// `:200-225`: one iteration of the sync loop, which is one vblank of
    /// `waitr`'s count — and so, by the engine's one-tick-one-frame rule, one
    /// tick.
    fn sync(&mut self, frame: u32) {
        let sx = self.sx as usize;
        let sy = self.sy as usize;
        // `:202-203` — the new head, summed in 16 bits. `TUN10.PAS:203`'s third
        // term is `sinit[y and 4095]` with the global `y` of `:71` that nothing
        // ever writes; the port writes the term out as the source has it.
        // `sy*3` and `sx*2` are word products in the source, so they wrap
        // before the mask; 4096 divides 65,536, so masking first is the same.
        let x = self.cosit[sy & 2047]
            .wrapping_sub(self.sinit[sy.wrapping_mul(3) & 4095])
            .wrapping_sub(self.cosit[sx & 2047]);
        let y = self.sinit[sx.wrapping_mul(2) & 4095]
            .wrapping_sub(self.cosit[sx & 2047])
            .wrapping_add(self.sinit[UNWRITTEN_Y as usize & 4095]);
        self.putki[100].x = x;
        self.putki[100].y = y;

        // `:204` the chain copy, which is what carries the head down into the
        // drawn rings `4..80` and the colour wave with it.
        self.putki.copy_within(1..=CHAIN_COPY, 0);

        // `:205-206` `inc(sy); inc(sx)`.
        self.sy = self.sy.wrapping_add(1);
        self.sx = self.sx.wrapping_add(1);
        // `:219` `if (sy and 15) > 7 then putki[99].c := 128 else putki[99].c
        // := 64` — eight new rings bright, eight dim, and the pattern walks
        // down the chain a record per frame.
        self.putki[99].c = if (self.sy & 15) > 7 { 128 } else { 64 };
        // `:220` the fade: from `veke-102` the new ring's colour is 0, and a
        // ring whose colour is 0 is not drawn at all (`:161`).
        if frame >= FADE_FIRST_FRAME {
            self.putki[99].c = 0;
        }
    }

    /// `:121-128`: the part's own DAC — two 65-entry grey ramps at 64 and 128,
    /// entries 68 and 132 black inside them, and entry 255 green.
    ///
    /// Entries 0..63 are **deliberately not written**: they are the mode set's
    /// own table, which is the same one `shell::hardware_default_palette`
    /// installs, and the reference's background — entry 0 — is black in every
    /// one of the four frames (`tunneli.md` §5).
    fn install_palette(ctx: &mut PartCtx) {
        // `:121` `setrgb(64+x, (64-x),(64-x),(64-x))`. `x = 0` writes 64,
        // which the DAC has no bit for: `Palette::set_entry` masks to six bits,
        // as the hardware does.
        for x in 0..=64u32 {
            let value = (64 - x) as u8;
            ctx.set_palette_entry(64 + x as u8, [value, value, value]);
        }
        // `:122` `setrgb(128+x, (64-x) *3 div 4, …)`, the same ramp at three
        // quarters, integer-truncated.
        for x in 0..=64u32 {
            let value = ((64 - x) * 3 / 4) as u8;
            ctx.set_palette_entry(128 + x as u8, [value, value, value]);
        }
        // `:124-125` — the two dark rings inside the ramps, black.
        ctx.set_palette_entry(68, [0, 0, 0]);
        ctx.set_palette_entry(132, [0, 0, 0]);
        // `:128` `setrgb(255,0,63,0)` — green, and the only non-grey entry the
        // part writes.
        ctx.set_palette_entry(255, [0, 63, 0]);
    }
}

impl Part for Tunneli {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `:99-148`: the tables, mode 13h, and the DAC.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // `:102-103` — the two sine tables, `move`d out of `sinit.obj`.
        let sinit_bytes = read_file(self.sinit_path)?;
        let sinit_words = words_of(&sinit_bytes);
        if sinit_words.len() < SINIT_WORDS + COSIT_COPIED {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} holds {} word(s); :102-103 copy {} of sinit and {} of cosit",
                    self.sinit_path,
                    sinit_words.len(),
                    SINIT_WORDS,
                    COSIT_COPIED
                ),
            });
        }
        self.sinit = sinit_words[..SINIT_WORDS].to_vec();
        // `:103` copies 2,048 of `SINIT.DAT`'s 2,049 words; the array's last
        // word is whatever the data segment held, which is 0 in the image.
        self.cosit = sinit_words[SINIT_WORDS..SINIT_WORDS + COSIT_COPIED].to_vec();
        self.cosit.resize(COSIT_WORDS, 0);

        // `:104` — `pcalc`, `move`d out of `tunneli.obj`: 138 rows of 64
        // `(x, y)` word pairs.
        let pcalc_bytes = read_file(self.tunnel_path)?;
        self.pcalc = words_of(&pcalc_bytes);
        let needed = PCALC_ROWS * DOTS_PER_RING * 2;
        if self.pcalc.len() != needed {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} holds {} word(s); pcalc is {}x{} of two-word records = {}",
                    self.tunnel_path,
                    self.pcalc.len(),
                    PCALC_ROWS,
                    DOTS_PER_RING,
                    needed
                ),
            });
        }

        // `:148` `for Z := 0 to 100 do sade[z] := round(16384 div ((Z*7)+95))`
        // — integer division, so `round` has nothing to do.
        for z in 0..=100usize {
            self.sade[z] = (16_384 / ((z * 7) + 95)) as u16;
        }

        // `:130-135` zeroes `putki` explicitly, and `:67`'s `oldpos` is *not*
        // cleared — `:97`'s `fillchar` is commented out — so it starts as the
        // data segment's image holds it, which is zeroes. The port zeroes both,
        // which is each of those states: the first frame's erases write colour
        // 0 at offset 0, exactly as the original's do.
        self.oldpos = vec![0; OLDPOS_WORDS];
        self.putki = [Ring::default(); CHAIN];

        // `:110-115` `mov ax,$13 / int $10`. The mode set is the BIOS's, not
        // the part's, and three of its effects are visible here: video memory
        // is cleared, the screen becomes 320x200 of one byte per pixel
        // addressed `x + y*320`, and the overscan entry goes back to AC
        // 0x11 = 0 (entry 0, which is black).
        // `y_line_offset` is 320 — Part B's second convention, the original's
        // own LINE_LEN — so the surface's first 64 KB is this part's video
        // memory and `View` reads exactly its first 200 rows of 320.
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = SCREEN_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = Some(LINE_LEN);
        ctx.view.border = Some(0);
        // The part addresses 64 KB of video memory flat, so the surface has to
        // hold it: this is the engine's own surface size (640x400 = 256,000
        // pixels), and a smaller one would put `draw`'s writes out of bounds
        // rather than clip them the way `Surface::set` does. Refused loudly.
        if ctx.surface.pixels().len() < VIDEO_BYTES {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "this part addresses {VIDEO_BYTES} bytes of video memory and the surface holds \
                     {}",
                    ctx.surface.pixels().len()
                ),
            });
        }
        ctx.surface.pixels_mut()[..VIDEO_BYTES].fill(0);

        Self::install_palette(ctx);
        Ok(())
    }

    /// One vblank of `:150`'s loop: draw the rings, then advance the chain once.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `:151` `waitr`, whose `:46` `setrgb(0,0,0,0)` rewrites DAC entry 0
        // every frame — and whose `:52` makes it `(0,0,15)` when bit 4 of the
        // BIOS keyboard flags (`mem[$40:$17]`, Scroll Lock) is set. **The port
        // has no keyboard flags**, so it writes the black branch, which is the
        // state the reference was captured in: every non-dot pixel of all four
        // of its TUNNELI frames is (0,0,0) (`tunneli.md` §5).
        ctx.set_palette_entry(0, [0, 0, 0]);

        // `:152-198`, then `:200-225`. `frames` from `waitr`'s DIS call is the
        // vblanks elapsed since the last one; one tick is one frame, so the
        // sync loop runs exactly once and `frame` (`:27`, incremented at
        // `:221`) is this part's `PartCtx::frame`.
        let memory = ctx.surface.pixels_mut();
        self.draw(memory);
        self.sync(ctx.frame);

        // `:221` `if frame = veke then quit := true` — the tick at `veke` draws
        // and then ends the part, so the part draws `veke + 1` frames.
        if ctx.frame >= VEKE {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }
}

/// `:158`'s `round(x / 1.3)`.
///
/// A real division in the source, so the port divides too rather than
/// tabulating it. The rounding is unambiguous over the loop's range: the
/// distance from the nearest half is at least 0.05 for every `x` in `4..=80`
/// (`the_ring_colour_rounds_unambiguously`), so no float subtlety can change
/// the integer.
fn ring_colour(x: usize) -> u8 {
    (x as f64 / 1.3).round() as u8
}

/// A file's bytes, or the part's own error naming the file.
fn read_file(path: &str) -> Result<Vec<u8>, PartError> {
    std::fs::read(Path::new(path)).map_err(|error| PartError {
        part: NAME,
        message: format!("{path}: {error}"),
    })
}

/// `bytes` as the little-endian words the original's `move` copies. A trailing
/// odd byte is dropped, which is what a word-wise `move` of a whole file does;
/// the two files here are both even and their lengths are checked by the
/// caller.
fn words_of(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use sr_engine::copper::CopperRegistry;
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::surface::Surface;
    use sr_engine::view::View;
    use sr_engine::Palette;

    use crate::shell::{SURFACE_HEIGHT, SURFACE_WIDTH};

    /// The two tables by the path that resolves from this crate's directory —
    /// a unit test's working directory is not the repository root, which is
    /// where the part's own `assets/...` paths resolve (`tests/tunneli.rs` runs
    /// the real binary from there).
    const TUNNEL_LOCAL: &str = "../../assets/TUNNELI/TUNNEL.DAT";
    const SINIT_LOCAL: &str = "../../assets/TUNNELI/SINIT.DAT";

    /// An entry no path through the part writes, as a marker for "the shell's
    /// DAC is still there".
    const UNWRITTEN: u8 = 200;

    /// The engine's pieces held together, so a `PartCtx` can be built for one
    /// call at a time — the same disjoint borrows `Engine::tick` makes.
    struct Rig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
    }

    impl Rig {
        fn new() -> Rig {
            let mut palette = Palette::new();
            // Two entries of the mode set's own table, as markers: 0 (which
            // `waitr` rewrites) and 14 (which nothing does).
            palette.set_entry(0, [1, 2, 3]);
            palette.set_entry(14, [1, 2, 3]);
            palette.set_entry(15, [63, 63, 63]);
            palette.set_entry(UNWRITTEN, [1, 2, 3]);
            Rig {
                surface: Surface::new(SURFACE_WIDTH, SURFACE_HEIGHT),
                view: View::new(320, 200),
                palette,
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
            }
        }

        fn ctx(&mut self, frame: u32) -> PartCtx<'_> {
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                frame,
                0,
                1,
                true,
                false,
            )
        }
    }

    /// A part whose `init` has run, and the rig it ran against.
    fn started() -> (Rig, Tunneli) {
        let mut rig = Rig::new();
        let mut part = Tunneli::at(TUNNEL_LOCAL, SINIT_LOCAL);
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    /// Runs the part for `ticks` ticks, numbered from zero.
    fn run_ticks(rig: &mut Rig, part: &mut Tunneli, ticks: u32) {
        for at in 0..ticks {
            part.frame(&mut rig.ctx(at));
        }
    }

    /// One blank, hairlined dot's worth of ink: how many of the first 64,000
    /// pixels are not the background.
    fn ink(surface: &Surface) -> usize {
        surface.pixels()[..64_000].iter().filter(|p| **p != 0).count()
    }

    #[test]
    fn the_tunnel_draws_seventy_seven_rings_of_sixty_four_dots() {
        // TUN10.PAS:153 `for x := 80 downto 4 do` and :170 `mov cx,64`. The
        // witness is `ry` (:69), the inner loop's byte offset into `oldpos`
        // (:152 `ry := 0`, :192 `add ax,2`), which advances only for a ring
        // that draws: 77 rings x 64 dots x 2 bytes.
        let (mut rig, mut part) = started();
        // The tunnel forms over its first ~96 ticks — ring x's colour arrives
        // down the chain from `putki[99]`, and with c = 0 (:130-135) a ring is
        // not drawn at all (:161) — so measure once every ring has one.
        run_ticks(&mut rig, &mut part, 120);
        part.frame(&mut rig.ctx(120));

        assert_eq!(RINGS, 77, "80 downto 4 inclusive is 77 rings");
        assert_eq!(RINGS * DOTS_PER_RING, 4_928, "77 x 64 dots");
        assert_eq!(
            part.ry as usize,
            RINGS * DOTS_PER_RING * 2,
            "a ring count or a dot count that moved would move ry"
        );
        // And nothing past the 4,928th slot was touched: the loop's last write
        // is that slot, because `ax` (:174) is 2 per dot and `oldpos` is word
        // indexed by it — 77 x 64 words, not 7,501.
        assert_eq!(part.oldpos.len(), OLDPOS_WORDS);
        assert!(
            part.oldpos[RINGS * DOTS_PER_RING..].iter().all(|slot| *slot == 0),
            "a dot was written past the 4,928th slot"
        );
    }

    #[test]
    fn the_ring_chain_copies_the_last_ring_forward() {
        // :204 `move(putki[1], putki[0], ofs(putki[100])-ofs(putki[0]))`: the
        // count is the offset difference, 100 records, so putki[0..99] take
        // putki[1..100] — each ring is the previous one's data six bytes along,
        // the record's own size (`rengas`, :11-14).
        let (_, mut part) = started();
        for (i, slot) in part.putki.iter_mut().enumerate() {
            *slot = Ring { x: 1000 + i as u16, y: 2000 + i as u16, c: i as u8 };
        }
        part.sync(0);

        // Every field moves together, so the stride is the whole record. The
        // loop stops at 98: `putki[99]` takes `putki[100]`, which `:202-203`
        // has just overwritten with the new head, and that is the next
        // assertion's business.
        for i in 0..CHAIN_COPY - 1 {
            assert_eq!(part.putki[i].x, 1001 + i as u16, "ring {i} x");
            assert_eq!(part.putki[i].y, 2001 + i as u16, "ring {i} y");
        }
        assert_eq!(part.putki[98].x, 1099, "putki[98] took the old putki[99]");
        // putki[99] took the old putki[100] **and then had its colour set**
        // (:219-220), which is the order the source runs them in: the head's
        // position is the one `:202-203` just computed, not a copy of itself.
        assert_eq!(part.putki[99].x, part.putki[100].x, "putki[99] is the new head");
        assert_eq!(part.putki[99].y, part.putki[100].y);
        assert_ne!(part.putki[99].x, 1100, "a 99-record copy would have kept the old x");
        assert!(matches!(part.putki[99].c, 64 | 128), "the colour is the new one");
    }

    #[test]
    fn the_animation_advances_once_per_tick() {
        // :200 `for sync := 1 to frames do` with `frames` from `waitr`'s DIS
        // call — the vblanks elapsed. One tick is one frame (Global
        // Constraints), so the loop runs exactly once per tick and advances
        // `sx`/`sy` (:205-206) by one.
        let (mut rig, mut part) = started();
        assert_eq!((part.sx, part.sy), (0, 0));
        run_ticks(&mut rig, &mut part, 7);
        part.frame(&mut rig.ctx(7));
        assert_eq!((part.sx, part.sy), (8, 8), "eight ticks, eight steps");

        // And the head is the function of the *pre-increment* counters: after
        // one tick it is :202-203 evaluated at sx = sy = 0.
        let (_, mut fresh) = started();
        let (x, y) = (
            fresh.cosit[0].wrapping_sub(fresh.sinit[0]).wrapping_sub(fresh.cosit[0]),
            fresh.sinit[0].wrapping_sub(fresh.cosit[0]).wrapping_add(fresh.sinit[0]),
        );
        fresh.frame(&mut rig.ctx(0));
        assert_eq!(fresh.putki[99].x, x);
        assert_eq!(fresh.putki[99].y, y);
    }

    #[test]
    fn the_last_hundred_and_two_frames_set_the_new_rings_colour_to_zero() {
        // :220 `if frame >= veke-102 then putki[99].c := 0`. veke - 102 = 958:
        // tick 957's new ring is 64 or 128, tick 958's is 0 — and a ring whose
        // colour is 0 has `bbc = round(x/1.3) < 64`, so :161 skips the whole
        // assembly block for it: the fade is the rings *stopping*, not being
        // drawn in entry 0. What the screen keeps is the erases of the rings
        // still moving, which is why it goes black a few ticks later.
        assert_eq!(FADE_FIRST_FRAME, 958);
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, FADE_FIRST_FRAME - 1);
        part.frame(&mut rig.ctx(FADE_FIRST_FRAME - 1));
        assert!(matches!(part.putki[99].c, 64 | 128), "tick 957 is still coloured");
        part.frame(&mut rig.ctx(FADE_FIRST_FRAME));
        assert_eq!(part.putki[99].c, 0, "tick 958's new ring is colour 0");

        // The consequence on the screen: with every ring's colour 0 none is
        // drawn, and the erases of the rings that still were take the rest —
        // the screen is black before the part ends (tick 1,052 here; `tunneli.md`
        // §6 has the port's and the reference's own black-out).
        for at in FADE_FIRST_FRAME..=1_052 {
            part.frame(&mut rig.ctx(at));
        }
        assert_eq!(ink(&rig.surface), 0, "the last frames hold a black screen");
    }

    #[test]
    fn the_part_retires_after_veke_frames() {
        // :3 `const veke = 1060`, and :221 `if frame = veke then quit := true`
        // — the tick at veke draws and syncs, and *then* the loop ends, so the
        // part draws 1,061 frames and the last of them is tick 1,060.
        assert_eq!(VEKE, 1060);
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, VEKE - 1);
        assert_eq!(
            part.frame(&mut rig.ctx(VEKE - 1)),
            PartStatus::Continuing,
            "tick 1,059 is not the last"
        );
        assert_eq!(
            part.frame(&mut rig.ctx(VEKE)),
            PartStatus::Finished,
            "tick 1,060 ends the part"
        );
        assert_eq!(part.sy, (VEKE + 1) as u16, "the last tick advanced the animation");
    }

    #[test]
    fn a_dot_with_an_out_of_range_y_is_plotted_at_the_words_the_read_lands_on() {
        // :179-190: `cmp di,319 / ja @yli` bounds the *x* sum only, and the y
        // term then indexes `rows` with whatever the 16-bit sum was. Ring 80
        // (the smallest ellipse, `br = sade[80] = 25`) with dot 0's y summing
        // to 445: in a bounded port that is off screen; in the original it is
        // past `rows`' 201 words, where the data segment holds `sinit`, and
        // `sinit[43]` is 1 — so the dot lands one pixel to the right.
        let (mut rig, mut part) = started();
        part.putki = [Ring::default(); CHAIN];
        let base = part.sade[RING_TOP] as usize * DOTS_PER_RING * 2;
        let (dot_x, dot_y) = (part.pcalc[base], part.pcalc[base + 1]);
        assert_eq!(part.sinit[43], 1, "the word at rows[445] is sinit[43]");
        part.putki[RING_TOP] = Ring { x: 0, y: 445u16.wrapping_sub(dot_y), c: 64 };

        part.draw(rig.surface.pixels_mut());

        let colour = 64 + ring_colour(RING_TOP);
        assert_eq!(part.oldpos[0], dot_x.wrapping_add(1), "dot 0 landed at x + sinit[43]");
        assert_eq!(
            rig.surface.get(dot_x as usize + 1, 0),
            Some(colour),
            "the dot is drawn where the unchecked read puts it"
        );
    }

    #[test]
    fn a_dot_leaves_its_previous_position_black() {
        // `:176-177`: the erase runs before the plot, at the offset the dot had
        // last time, and it is the only thing that ever removes ink — the
        // screen is never cleared and the tunnel erases its own trail. Move one
        // hand-built ring by one pixel between two draws: the dot's old pixel
        // goes back to 0 and its new one takes the colour.
        let (mut rig, mut part) = started();
        part.putki = [Ring::default(); CHAIN];
        part.putki[RING_TOP] = Ring { x: 0, y: 0, c: 64 };

        part.draw(rig.surface.pixels_mut());
        let colour = 64 + ring_colour(RING_TOP);
        let first = part.oldpos[0];
        assert_eq!(rig.surface.pixels()[first as usize], colour, "dot 0 is drawn");

        part.putki[RING_TOP] = Ring { x: 1, y: 0, c: 64 };
        part.draw(rig.surface.pixels_mut());
        assert_eq!(part.oldpos[0], first.wrapping_add(1), "the dot moved one pixel right");
        assert_eq!(
            rig.surface.pixels()[first as usize],
            0,
            "the position it left is black again"
        );
        assert_eq!(rig.surface.pixels()[(first + 1) as usize], colour);
    }

    #[test]
    fn the_unchecked_read_reads_the_data_segments_own_words() {
        // The layout is the declaration order of `TUN10.PAS:16-29`, and every
        // boundary below is a word of it. The window index is `yy` wrapped at
        // 32,768: the assembly's address is `DS:rows_off + 2*yy` in a 64 KB
        // segment, so the *byte* offset wraps at 65,536 and the word grid at
        // half that.
        let (_, part) = started();

        // `rows` itself, `:101`.
        assert_eq!(part.rows_word(0), 0);
        assert_eq!(part.rows_word(1), 320);
        assert_eq!(part.rows_word(200), 64_000);

        // `aa2` (`:22`), `:100`'s copy of `aa1`; the rest of it is never
        // written and is zero.
        assert_eq!(part.rows_word(201), 64_000);
        assert_eq!(part.rows_word(301), 64_000);
        assert_eq!(part.rows_word(302), 0);
        assert_eq!(part.rows_word(401), 0);

        // `sinit` (`:24`), then `cosit` (`:25`).
        assert_eq!(part.rows_word(402), part.sinit[0]);
        assert_eq!(part.rows_word(445), part.sinit[43]);
        assert_eq!(part.rows_word(4_498), part.sinit[4_096]);
        assert_eq!(part.rows_word(4_499), part.cosit[0]);
        assert_eq!(part.rows_word(6_547), 0, "the word :103's move never writes");

        // `sade` (`:26`): :148 writes 0..=100, so 6650 is unwritten.
        assert_eq!(part.rows_word(6_548), 16_384 / 95);
        assert_eq!(part.rows_word(6_650), 0);

        // Past the wrap, walking backwards: `aa1`'s 101 words are all 64,000,
        // and before them is `pcalc` — this part's own table, last word first.
        assert_eq!(part.rows_word(65_535), 64_000);
        assert_eq!(part.rows_word(65_435), 64_000);
        assert_eq!(part.rows_word(65_434), part.pcalc[17_663]);
        assert_eq!(part.rows_word(15_003), part.pcalc[0]);
        assert_eq!(part.rows_word(32_666), part.pcalc[17_663]);
    }

    #[test]
    fn the_unchecked_read_covers_everything_the_arithmetic_can_reach() {
        // The model above only covers part of the data segment, so the bound
        // has to be measured rather than assumed. Every y the assembly can
        // index with is `by + pcalc[br][i].y`, where `by` is the difference of
        // two chain positions and a chain position is :203's
        // `sinit[sx*2 and 4095] - cosit[sx and 2047]` (its third term is the
        // global `y` of :71, which nothing writes) or the initial 0 (:134).
        let (_, part) = started();
        let range = |words: &[u16]| {
            words.iter().fold((0i32, 0i32), |(lo, hi), word| {
                let value = *word as i16 as i32;
                (lo.min(value), hi.max(value))
            })
        };
        // `pcalc`'s words include each dot's x as well as its y, so this is a
        // superset of the y the loop can add — which is all a bound needs.
        let (sinit_lo, sinit_hi) = range(&part.sinit);
        let (cosit_lo, cosit_hi) = range(&part.cosit);
        let (pcalc_lo, pcalc_hi) = range(&part.pcalc);

        let head = (sinit_lo - cosit_hi, sinit_hi - cosit_lo);
        let by = (2 * head.0, 2 * head.1);
        let y_lo = by.0 + pcalc_lo;
        let y_hi = by.1 + pcalc_hi;

        assert!(
            y_hi as usize <= MODEL_FORWARD_LAST,
            "the forward reach is {y_hi}, past the modelled {MODEL_FORWARD_LAST}"
        );
        assert!(
            (y_lo + 32_768) as usize >= MODEL_BACKWARD_FIRST,
            "the wrapped reach is {}, before the modelled {MODEL_BACKWARD_FIRST}",
            y_lo + 32_768
        );
        assert!(y_lo < 0, "the wrapped half is reachable at all: y can be {y_lo}");
    }

    #[test]
    fn the_ring_colour_rounds_unambiguously() {
        // :158 `round(x / 1.3)` over the loop's range: the port divides in
        // floating point, so the test is that no `x` lands near a half.
        assert_eq!(ring_colour(4), 3);
        assert_eq!(ring_colour(80), 62);
        let nearest = (RING_BOTTOM..=RING_TOP)
            .map(|x| (x as f64 / 1.3).fract().max(1.0 - (x as f64 / 1.3).fract()))
            .fold(1.0f64, f64::min);
        assert!(nearest > 0.02, "the closest call is {nearest} from a half");
    }

    #[test]
    fn the_palette_is_the_parts_own_ramps_and_leaves_0_to_63_alone() {
        // :121-128. Two 65-entry ramps at 64 and 128; entries 68 and 132 black
        // inside them; entry 255 green. Entries 0..63 are the mode set's own
        // table, which the part does not touch.
        let (mut rig, mut part) = started();
        assert_eq!(rig.palette.entry(15), [63, 63, 63], "the shell's entry 15");

        // The ramps, at both ends and where the black writes land.
        assert_eq!(rig.palette.entry(64), [0, 0, 0], "64 six-bit-masked, as the DAC does");
        assert_eq!(rig.palette.entry(65), [63, 63, 63]);
        assert_eq!(rig.palette.entry(68), [0, 0, 0], "setrgb(68,0,0,0)");
        assert_eq!(rig.palette.entry(128), [48, 48, 48], "64 * 3 div 4");
        assert_eq!(rig.palette.entry(129), [47, 47, 47], "63 * 3 div 4");
        assert_eq!(rig.palette.entry(132), [0, 0, 0], "setrgb(132,0,0,0)");
        assert_eq!(rig.palette.entry(192), [0, 0, 0]);
        assert_eq!(rig.palette.entry(255), [0, 63, 0], "setrgb(255,0,63,0)");
        assert_eq!(rig.palette.entry(UNWRITTEN), [1, 2, 3], "an entry no ramp reaches");

        // `waitr`'s `:46` then rewrites entry 0 on the first tick, and nothing
        // else of 0..63 (`:52`'s Scroll Lock branch is the one the port does
        // not model — see `frame`).
        part.frame(&mut rig.ctx(0));
        assert_eq!(rig.palette.entry(0), [0, 0, 0], ":46 setrgb(0,0,0,0)");
        assert_eq!(rig.palette.entry(14), [1, 2, 3], "and only entry 0");
    }

    #[test]
    fn the_mode_set_puts_the_screen_at_320x200_and_clears_video_memory() {
        // :110-115 `mov ax,$13 / int $10`. The mode set is the BIOS's, and the
        // port models the three things a part can see: the geometry, the line
        // stride mode 13h addresses with (`x + y*320`), and the cleared
        // memory. The stride is Part B's second convention — the original's own
        // LINE_LEN — not the surface's row stride.
        let mut rig = Rig::new();
        rig.surface.pixels_mut().fill(0xAA);
        rig.view = View::new(640, 400);
        rig.view.y_line_offset = Some(640);
        let mut part = Tunneli::at(TUNNEL_LOCAL, SINIT_LOCAL);
        part.init(&mut rig.ctx(0)).unwrap();

        assert_eq!((rig.view.width, rig.view.height), (320, 200));
        assert_eq!(rig.view.y_line_offset, Some(320));
        assert_eq!(rig.view.start_addr, 0);
        assert_eq!(rig.view.border, Some(0), "AC 0x11 back to entry 0");
        assert!(rig.surface.pixels()[..VIDEO_BYTES].iter().all(|p| *p == 0));
        assert_eq!(rig.surface.pixels()[VIDEO_BYTES], 0xAA, "and no more than video memory");
    }

    #[test]
    fn the_data_files_are_not_pictures() {
        // `docs/fidelity/tunneli.md` §2: the port decodes these two itself
        // because `sr-assets` has no door for them, and the reason is a
        // measurement rather than a preference. Neither file carries a magic,
        // so `load_path` refuses them; and `raw_surface` would need a geometry
        // the caller invents — 35,328 is 384x92 and 12,292 is 4x3,073, which
        // are record counts, not pictures.
        for path in [TUNNEL_LOCAL, SINIT_LOCAL] {
            let bytes = std::fs::read(path).unwrap();
            assert!(sr_assets::sniff(&bytes).is_none(), "{path} is not sniffable");
        }
        let tunnel = std::fs::read(TUNNEL_LOCAL).unwrap();
        let sinit = std::fs::read(SINIT_LOCAL).unwrap();
        assert_eq!(tunnel.len(), PCALC_ROWS * DOTS_PER_RING * 2 * 2, "pcalc's bytes");
        assert_eq!(sinit.len(), (SINIT_WORDS + COSIT_WORDS) * 2, "both sine tables");
    }
}
