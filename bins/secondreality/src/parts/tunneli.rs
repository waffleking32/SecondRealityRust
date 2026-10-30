//! TUNNELI — the dot tunnel: 77 rings of 64 dots, chained, one plot per dot.
//!
//! `TUNNELI/TUN10.PAS` (Turbo Pascal with an inline-assembly inner loop) is the
//! part; `TUNNELI/X.BAT:1` stages `tun10.exe`, not the earlier `TUN9.PAS`. Its
//! acts:
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
//! What the port keeps of that loop (`docs/fidelity/tunneli.md`):
//!
//! - **The clip is one-sided** (`:179-190`): the y term indexes `rows`
//!   unchecked. The port models the data-segment words it can reach
//!   ([`Tunneli::rows_word`]); on this art they are all 64,000, so the stray
//!   dots land off screen, where substituting 0 would paint row 0 (`bugs.md` §22).
//! - **The colour immediate is self-modified** (`:168`); the port makes it a
//!   variable (design §5.2).
//! - **`waitr` rewrites DAC entry 0 every frame** (`:46`, `:52`): black, or
//!   blue under Scroll Lock, which the port has no flag for.
//! - **The last 102 frames set the new ring's colour to 0** (`:220`), and a
//!   ring whose `bbc` is below 64 is not drawn (`:161`): the screen empties
//!   through the erases of the rings still moving.
//! - **The second grey ramp is the release's 3/5**, not the tree's 3/4 (D12,
//!   A6; [`second_ramp`]).
//!
//! The picture is written at mode 13h's own stride, `x + y*320`
//! (`View::y_line_offset = 320`), so the surface's first 64 KB is the
//! original's video memory, wraparound and all.

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

use crate::parts::read_asset;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "tunneli";

/// The linked data segments of `TUNNELI.OBJ` and `SINIT.OBJ`: record arrays,
/// not pictures, so the part reads them itself (`tunneli.md` §2).
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

/// The window [`Tunneli::rows_word`] models, in word offsets from `rows[0]`:
/// `aa2` (101 words of 64000, then 100 never-written zeros), `sinit`, `cosit`
/// and `sade`; and, past the 64 KB wrap, `pcalc` read backwards and then `aa1`.
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

    /// The word `:186`'s `add di, word ptr ds:rows[bx]` reads for the 16-bit
    /// index `yy`: **unchecked**. The 64 KB data segment wraps the word index at
    /// 32,768, and the neighbours are the globals in declaration order
    /// (`TUN10.PAS:16-29`):
    ///
    /// ```text
    ///   … putki, pcalc, aa1[0..100], rows[0..200], aa2[0..200], sinit[0..4096],
    ///     cosit[0..2048], sade[0..102], frame, bor, frames, oldpos, …
    /// ```
    ///
    /// The arithmetic reaches ±680 words
    /// (`the_unchecked_read_covers_everything_the_arithmetic_can_reach`); the
    /// words between `sade` and `pcalc` are unreachable and read as 0. Every
    /// read the run makes outside `rows` is 64,000, off screen
    /// (`the_out_of_range_read_is_invisible_and_that_is_measured`,
    /// `tunneli.md` §4).
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
            SADE_FIRST..=SADE_LAST => self.sade[k - SADE_FIRST],
            // Before `aa1`: `pcalc`'s last words, read backwards.
            PCALC_FIRST..=PCALC_LAST => self.pcalc[PCALC_WORDS - 1 - (PCALC_LAST - k)],
            // `aa1` itself, every word of it 64000 (`:99`).
            AA1_FIRST.. => AA1_VALUE,
            // `frame`, `bor`, `frames`, `oldpos` and the scalars: unreachable.
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
            // `:161` `if bbc >= 64 then`: below 64 the whole block, erase and
            // plot alike, is skipped.
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
                // `:181-182` `cmp di,319 / ja @yli`: the only bound, on x alone.
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

    /// `:200-225`: one iteration of the sync loop, one vblank, one tick.
    fn sync(&mut self, frame: u32) {
        let sx = self.sx as usize;
        let sy = self.sy as usize;
        // `:202-203`, the new head, summed in 16 bits; `sy*3` and `sx*2` wrap
        // before the mask, which 4096 dividing 65,536 makes the same.
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

    /// `:121-128`: the part's own DAC. Entries 0..63 are the mode set's table
    /// and are not written (`tunneli.md` §5).
    fn install_palette(ctx: &mut PartCtx) {
        for (entry, rgb) in dac_writes(second_ramp) {
            ctx.set_palette_entry(entry, rgb);
        }
    }
}

/// `:121-128`'s writes in order, with `second` standing in for `:122`'s ramp:
/// two 65-entry grey ramps at 64 and 128, entries 68 and 132 black inside them,
/// and entry 255 green.
fn dac_writes(second: impl Fn(u32) -> u8) -> Vec<(u8, [u8; 3])> {
    let mut writes = Vec::with_capacity(133);
    // `:121` `setrgb(64+x, (64-x),(64-x),(64-x))`. `x = 0` writes 64, which the
    // six-bit DAC masks to 0, as `Palette::set_entry` does.
    for x in 0..=64u32 {
        let value = (64 - x) as u8;
        writes.push((64 + x as u8, [value; 3]));
    }
    // `:122`, the same ramp dimmed: the release's, [`second_ramp`].
    for x in 0..=64u32 {
        let value = second(64 - x);
        writes.push((128 + x as u8, [value; 3]));
    }
    // `:124-125`, the two dark rings; `:128`, green.
    writes.push((68, [0, 0, 0]));
    writes.push((132, [0, 0, 0]));
    writes.push((255, [0, 63, 0]));
    writes
}

impl Part for Tunneli {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `:99-148`: the tables, mode 13h, and the DAC.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // `:102-103` — the two sine tables, `move`d out of `sinit.obj`.
        let sinit_bytes = read_asset(NAME, SINIT_PATH)?;
        let sinit_words = words_of(&sinit_bytes);
        if sinit_words.len() < SINIT_WORDS + COSIT_COPIED {
            return Err(PartError::new(
                NAME,
                format!(
                    "{SINIT_PATH} holds {} word(s); :102-103 copy {} of sinit and {} of cosit",
                    sinit_words.len(),
                    SINIT_WORDS,
                    COSIT_COPIED
                ),
            ));
        }
        self.sinit = sinit_words[..SINIT_WORDS].to_vec();
        // `:103` copies 2,048 of `SINIT.DAT`'s 2,049 words; the array's last
        // word is whatever the data segment held, which is 0 in the image.
        self.cosit = sinit_words[SINIT_WORDS..SINIT_WORDS + COSIT_COPIED].to_vec();
        self.cosit.resize(COSIT_WORDS, 0);

        // `:104` — `pcalc`, `move`d out of `tunneli.obj`: 138 rows of 64
        // `(x, y)` word pairs.
        let pcalc_bytes = read_asset(NAME, TUNNEL_PATH)?;
        self.pcalc = words_of(&pcalc_bytes);
        let needed = PCALC_ROWS * DOTS_PER_RING * 2;
        if self.pcalc.len() != needed {
            return Err(PartError::new(
                NAME,
                format!(
                    "{TUNNEL_PATH} holds {} word(s); pcalc is {}x{} of two-word records = {}",
                    self.pcalc.len(),
                    PCALC_ROWS,
                    DOTS_PER_RING,
                    needed
                ),
            ));
        }

        // `:148` `for Z := 0 to 100 do sade[z] := round(16384 div ((Z*7)+95))`
        // — integer division, so `round` has nothing to do.
        for z in 0..=100usize {
            self.sade[z] = (16_384 / ((z * 7) + 95)) as u16;
        }

        // `:130-135` zeroes `putki`; `oldpos` is not cleared (`:97` is
        // commented out) but the image holds zeroes there.
        self.oldpos = vec![0; OLDPOS_WORDS];
        self.putki = [Ring::default(); CHAIN];

        // `:110-115` `mov ax,$13 / int $10`: video memory cleared, 320x200
        // addressed `x + y*320`, and AC 0x11 back to entry 0.
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = SCREEN_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = Some(LINE_LEN);
        ctx.view.border = Some(0);
        // `draw` addresses 64 KB flat, unclipped.
        if ctx.surface.pixels().len() < VIDEO_BYTES {
            return Err(PartError::new(
                NAME,
                format!(
                    "this part addresses {VIDEO_BYTES} bytes of video memory and the surface holds \
                     {}",
                    ctx.surface.pixels().len()
                ),
            ));
        }
        ctx.surface.pixels_mut()[..VIDEO_BYTES].fill(0);

        Self::install_palette(ctx);
        Ok(())
    }

    /// One vblank of `:150`'s loop: draw the rings, then advance the chain once.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `:151` `waitr`'s `:46` `setrgb(0,0,0,0)`; `:52`'s Scroll Lock blue
        // has no keyboard flag to read, and the reference is black.
        ctx.set_palette_entry(0, [0, 0, 0]);

        // `:152-198`, then `:200-225` once (one vblank a tick); `frame` (`:27`)
        // is `PartCtx::frame`.
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

/// The shipped build's second ramp, `(64 - x) * 3 div 5`, where `TUN10.PAS:122`
/// and the tree's own `TUN10.EXE` (image 0x1d0-0x1e3) have `* 3 div 4`. The
/// reference's frames fit 3/5 uniquely
/// (`the_second_ramp_is_derived_by_matching_the_reference_bit_for_bit`); D12
/// follows the release (`tunneli.md` §5, A6). Truncating: 38 gives 22, as the
/// reference shows, not 23.
fn second_ramp(value: u32) -> u8 {
    (value * 3 / 5) as u8
}

/// `:158`'s `round(x / 1.3)`, unambiguous over `4..=80`
/// (`the_ring_colour_rounds_unambiguously`).
fn ring_colour(x: usize) -> u8 {
    (x as f64 / 1.3).round() as u8
}

/// `bytes` as the little-endian words the original's `move` copies.
fn words_of(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parts::test_rig::{reference, Rig};
    use sr_engine::surface::Surface;
    use sr_engine::view::View;
    use sr_engine::Palette;

    /// `:153`'s 77 rings.
    const RINGS: usize = RING_TOP - RING_BOTTOM + 1;

    /// An entry no path through the part writes, as a marker for "the shell's
    /// DAC is still there".
    const UNWRITTEN: u8 = 200;

    /// A 320×200 view over the shell's DAC. Entries 0 (which `waitr` rewrites)
    /// and 14 (which nothing does) of the mode set's table are markers, as is
    /// [`UNWRITTEN`]; 15 is white. No player: the part asks for no cue.
    fn shell_rig() -> Rig {
        Rig::new(320, 200)
            .palette_entry(0, [1, 2, 3])
            .palette_entry(14, [1, 2, 3])
            .palette_entry(15, [63, 63, 63])
            .palette_entry(UNWRITTEN, [1, 2, 3])
    }

    /// A part whose `init` has run, and the rig it ran against.
    fn started() -> (Rig, Tunneli) {
        let mut rig = shell_rig();
        let mut part = Tunneli::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    /// Runs the part for `ticks` ticks, numbered from zero.
    fn run_ticks(rig: &mut Rig, part: &mut Tunneli, ticks: u32) {
        for at in 0..ticks {
            part.frame(&mut rig.ctx(at));
        }
    }

    /// How many of the first 64,000 pixels are not the background.
    fn ink(surface: &Surface) -> usize {
        surface.pixels()[..64_000].iter().filter(|p| **p != 0).count()
    }

    #[test]
    fn the_tunnel_draws_seventy_seven_rings_of_sixty_four_dots() {
        // `:153` and `:170`. The witness is `ry` (`:69`, `:192`), which
        // advances 2 a dot for a ring that draws; by tick 120 every ring has
        // its colour.
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, 120);
        part.frame(&mut rig.ctx(120));

        assert_eq!(
            part.ry as usize,
            RINGS * DOTS_PER_RING * 2,
            "a ring count or a dot count that moved would move ry"
        );
        // And nothing past the 4,928th slot was touched.
        assert!(
            part.oldpos[RINGS * DOTS_PER_RING..].iter().all(|slot| *slot == 0),
            "a dot was written past the 4,928th slot"
        );
    }

    #[test]
    fn the_ring_chain_copies_the_last_ring_forward() {
        // `:204`: the count is the offset difference, 100 records.
        let (_, mut part) = started();
        for (i, slot) in part.putki.iter_mut().enumerate() {
            *slot = Ring { x: 1000 + i as u16, y: 2000 + i as u16, c: i as u8 };
        }
        part.sync(0);

        // `putki[99]` takes the new head `:202-203` just wrote, checked below.
        for i in 0..CHAIN_COPY - 1 {
            assert_eq!(part.putki[i].x, 1001 + i as u16, "ring {i} x");
            assert_eq!(part.putki[i].y, 2001 + i as u16, "ring {i} y");
        }
        assert_eq!(part.putki[98].x, 1099, "putki[98] took the old putki[99]");
        // ... and then had its colour set (`:219-220`).
        assert_eq!(part.putki[99].x, part.putki[100].x, "putki[99] is the new head");
        assert_eq!(part.putki[99].y, part.putki[100].y);
        assert_ne!(part.putki[99].x, 1100, "a 99-record copy would have kept the old x");
        assert!(matches!(part.putki[99].c, 64 | 128), "the colour is the new one");
    }

    #[test]
    fn the_animation_advances_once_per_tick() {
        // `:200` runs once a vblank, so `sx`/`sy` (`:205-206`) step once a tick.
        let (mut rig, mut part) = started();
        assert_eq!((part.sx, part.sy), (0, 0));
        run_ticks(&mut rig, &mut part, 7);
        part.frame(&mut rig.ctx(7));
        assert_eq!((part.sx, part.sy), (8, 8), "eight ticks, eight steps");

        // The head is `:202-203` at the pre-increment counters.
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
        // `:220`: from tick 958 the new ring's colour is 0, and `:161` skips it.
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, FADE_FIRST_FRAME - 1);
        part.frame(&mut rig.ctx(FADE_FIRST_FRAME - 1));
        assert!(matches!(part.putki[99].c, 64 | 128), "tick 957 is still coloured");
        part.frame(&mut rig.ctx(FADE_FIRST_FRAME));
        assert_eq!(part.putki[99].c, 0, "tick 958's new ring is colour 0");

        // The erases take the rest: black by tick 1,052 (`tunneli.md` §6).
        for at in FADE_FIRST_FRAME..=1_052 {
            part.frame(&mut rig.ctx(at));
        }
        assert_eq!(ink(&rig.surface), 0, "the last frames hold a black screen");
    }

    #[test]
    fn the_part_retires_after_veke_frames() {
        // `:221`: the tick at veke draws and syncs, then the loop ends.
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
        // Ring 80's dot 0 with its y summing to 445: past `rows` the data
        // segment holds `sinit`, and `sinit[43]` is 1, so the dot lands one
        // pixel to the right.
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
        // `:176-177`: the erase at the dot's last offset is the only thing that
        // removes ink.
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
        // Every boundary of `TUN10.PAS:16-29`'s layout, `yy` wrapped at 32,768.
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
        // Every y is `by + pcalc[br][i].y`, `by` the difference of two chain
        // positions, each `:203`'s `sinit[..] - cosit[..]` or the initial 0.
        let (_, part) = started();
        let range = |words: &[u16]| {
            words.iter().fold((0i32, 0i32), |(lo, hi), word| {
                let value = *word as i16 as i32;
                (lo.min(value), hi.max(value))
            })
        };
        // `pcalc`'s x words too: a superset, which is all a bound needs.
        let (sinit_lo, sinit_hi) = range(&part.sinit);
        let (cosit_lo, cosit_hi) = range(&part.cosit);
        let (pcalc_lo, pcalc_hi) = range(&part.pcalc);

        let head = (sinit_lo - cosit_hi, sinit_hi - cosit_lo);
        let by = (2 * head.0, 2 * head.1);
        let y_lo = by.0 + pcalc_lo;
        let y_hi = by.1 + pcalc_hi;

        assert!(
            y_hi as usize <= SADE_LAST,
            "the forward reach is {y_hi}, past the modelled {SADE_LAST}"
        );
        assert!(
            (y_lo + 32_768) as usize >= PCALC_FIRST,
            "the wrapped reach is {}, before the modelled {PCALC_FIRST}",
            y_lo + 32_768
        );
        assert!(y_lo < 0, "the wrapped half is reachable at all: y can be {y_lo}");
    }

    /// The whole run, calling `visit(x_sum, y)` for every dot of every ring
    /// that draws, with the x sum `:181` bounds and the y `:186` reads.
    fn for_each_drawn_dot(mut visit: impl FnMut(u16, u16)) {
        let (mut rig, mut part) = started();
        for tick in 0..=VEKE {
            part.frame(&mut rig.ctx(tick));
            for x in RING_BOTTOM..=RING_TOP {
                if part.putki[x].c.wrapping_add(ring_colour(x)) < 64 {
                    continue;
                }
                let bx = part.putki[x].x.wrapping_sub(part.putki[5].x);
                let by = part.putki[x].y.wrapping_sub(part.putki[5].y);
                let base = part.sade[x] as usize * DOTS_PER_RING * 2;
                for dot in 0..DOTS_PER_RING {
                    let x_sum = part.pcalc[base + dot * 2].wrapping_add(bx);
                    let y = by.wrapping_add(part.pcalc[base + dot * 2 + 1]);
                    visit(x_sum, y);
                }
            }
        }
    }

    #[test]
    fn the_out_of_range_read_is_invisible_and_that_is_measured() {
        // Every y the run puts outside `rows` reads 64,000 (`aa2`'s copy of
        // `aa1`, or `aa1` past the wrap), so the plot lands in bytes
        // 64,002..64,319, below the 200-line screen.
        let (_, part) = started();
        let mut off_screen = 0usize;
        let mut on_screen = 0usize;
        for_each_drawn_dot(|_, y| {
            if y > 200 {
                if part.rows_word(y) == 64_000 {
                    off_screen += 1;
                } else {
                    on_screen += 1;
                }
            }
        });
        assert!(
            off_screen > 100_000,
            "the run should read out of range often: {off_screen}"
        );
        assert_eq!(on_screen, 0, "an out-of-range read that a screen would show");
    }

    #[test]
    fn zeroing_the_read_would_paint_row_zero() {
        // `aa2`'s unwritten half is 0, so substituting 0 for the out-of-range
        // word would plot every such dot at its x sum, which `:181-182` has
        // already bounded to row 0: 356,042 dots, where the reference's four
        // frames ink 34 pixels of row 0 in all (`tunneli.md` §4).
        let mut moved = 0usize;
        for_each_drawn_dot(|x_sum, y| {
            if x_sum <= 319 && y > 200 {
                moved += 1;
            }
        });
        assert_eq!(moved, 356_042, "the run's out-of-range reads that a plot would move");

        let mut reference_ink = 0usize;
        let mut port_ink = 0usize;
        for (tick, name) in ALIGNED {
            let (picture, expected) = aligned_frame(tick, name);
            for column in 0..SCREEN_WIDTH as usize {
                port_ink += usize::from(picture[column] != 0);
                reference_ink += usize::from(expected[column * 3..column * 3 + 3] != [0, 0, 0]);
            }
        }
        assert_eq!(reference_ink, 34, "the reference's ink in row 0, four frames");
        assert!(
            port_ink <= 64,
            "the port draws the same picture, so its row 0 is nearly empty too: {port_ink}"
        );
    }

    #[test]
    fn the_alignment_is_whole_tick_or_nothing() {
        // `tunneli.md` §7.2: at 3800 tick 78 leaves 179 mis-coloured pixels
        // and its neighbours four times as many, so the offset is a whole tick.
        let at = |tick: u32| {
            let (picture, expected) = aligned_frame(tick, "003800");
            colour_error(second_ramp, &[(picture, expected)])
        };
        assert_eq!(at(78), 179, "the aligned tick");
        let (before, after) = (at(77), at(79));
        assert_eq!((before, after), (754, 953), "the neighbouring ticks");
    }

    #[test]
    fn the_ring_colour_rounds_unambiguously() {
        assert_eq!(ring_colour(4), 3);
        assert_eq!(ring_colour(80), 62);
        let nearest = (RING_BOTTOM..=RING_TOP)
            .map(|x| (x as f64 / 1.3).fract().max(1.0 - (x as f64 / 1.3).fract()))
            .fold(1.0f64, f64::min);
        assert!(nearest > 0.02, "the closest call is {nearest} from a half");
    }

    #[test]
    fn the_palette_is_the_parts_own_ramps_and_leaves_0_to_63_alone() {
        let (mut rig, mut part) = started();
        assert_eq!(rig.palette.entry(15), [63, 63, 63], "the shell's entry 15");

        assert_eq!(rig.palette.entry(64), [0, 0, 0], "64 six-bit-masked, as the DAC does");
        assert_eq!(rig.palette.entry(65), [63, 63, 63]);
        assert_eq!(rig.palette.entry(68), [0, 0, 0], "setrgb(68,0,0,0)");
        // The second ramp is the release's 3/5 ([`second_ramp`]).
        assert_eq!(rig.palette.entry(128), [38, 38, 38], "64 * 3 div 5");
        assert_eq!(rig.palette.entry(129), [37, 37, 37], "63 * 3 div 5");
        assert_eq!(rig.palette.entry(166), [15, 15, 15], "26 * 3 div 5, and 16 rounded");
        assert_eq!(rig.palette.entry(154), [22, 22, 22], "38 * 3 div 5, and 23 rounded");
        assert_eq!(rig.palette.entry(132), [0, 0, 0], "setrgb(132,0,0,0)");
        assert_eq!(rig.palette.entry(192), [0, 0, 0]);
        assert_eq!(rig.palette.entry(255), [0, 63, 0], "setrgb(255,0,63,0)");
        assert_eq!(rig.palette.entry(UNWRITTEN), [1, 2, 3], "an entry no ramp reaches");

        // `waitr`'s `:46` rewrites entry 0 on the first tick, and only it.
        part.frame(&mut rig.ctx(0));
        assert_eq!(rig.palette.entry(0), [0, 0, 0], ":46 setrgb(0,0,0,0)");
        assert_eq!(rig.palette.entry(14), [1, 2, 3], "and only entry 0");
    }

    #[test]
    fn the_mode_set_puts_the_screen_at_320x200_and_clears_video_memory() {
        // `:110-115`: the geometry, mode 13h's own stride, the cleared memory.
        let mut rig = shell_rig();
        rig.surface.pixels_mut().fill(0xAA);
        rig.view = View::new(640, 400);
        rig.view.y_line_offset = Some(640);
        let mut part = Tunneli::new();
        part.init(&mut rig.ctx(0)).unwrap();

        assert_eq!((rig.view.width, rig.view.height), (320, 200));
        assert_eq!(rig.view.y_line_offset, Some(320));
        assert_eq!(rig.view.start_addr, 0);
        assert_eq!(rig.view.border, Some(0), "AC 0x11 back to entry 0");
        assert!(rig.surface.pixels()[..VIDEO_BYTES].iter().all(|p| *p == 0));
        assert_eq!(rig.surface.pixels()[VIDEO_BYTES], 0xAA, "and no more than video memory");
    }

    /// The four reference keyframes, each with the part tick that answers it:
    /// reference index = tick + 3,722 (`docs/fidelity/tunneli.md` §7.2).
    const ALIGNED: [(u32, &str); 4] = [
        (78, "003800"),
        (478, "004200"),
        (778, "004500"),
        (878, "004600"),
    ];

    /// One keyframe as the port draws it: the part run to that tick, and the
    /// reference's own RGB frame beside it.
    fn aligned_frame(tick: u32, name: &str) -> (Vec<u8>, Vec<u8>) {
        let (mut rig, mut part) = started();
        for at in 0..=tick {
            part.frame(&mut rig.ctx(at));
        }
        let memory = rig.surface.pixels();
        let mut picture = Vec::with_capacity(SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize);
        for y in 0..SCREEN_HEIGHT as usize {
            let row = y * LINE_LEN as usize;
            picture.extend_from_slice(&memory[row..row + SCREEN_WIDTH as usize]);
        }
        let expected = reference("tunneli", name.parse().unwrap(), "rgb");
        assert_eq!(picture.len() * 3, expected.len(), "{name} is not 320x200");
        (picture, expected)
    }

    /// The DAC `install_palette` leaves, with `second` as the candidate ramp.
    fn dac_with(second: impl Fn(u32) -> u8) -> Palette {
        let mut dac = Palette::new();
        for (entry, rgb) in dac_writes(second) {
            dac.set_entry(entry, rgb);
        }
        dac
    }

    /// How far a candidate ramp leaves the port's colour field from the frames:
    /// the pixels the port inks whose colour no reference frame shows.
    ///
    /// Only pixels the port inks are counted: the reference's ink the port does
    /// not draw (`tunneli.md` §7.2) is the same for every candidate.
    fn colour_error(second: impl Fn(u32) -> u8, frames: &[(Vec<u8>, Vec<u8>)]) -> usize {
        let dac = dac_with(second);
        let mut wrong = 0;
        for (picture, expected) in frames {
            for (index, rgb) in picture.iter().zip(expected.chunks_exact(3)) {
                let entry = dac.entry(*index);
                if entry == [0, 0, 0] {
                    continue;
                }
                let shown = [
                    sr_engine::dac_expand(entry[0]),
                    sr_engine::dac_expand(entry[1]),
                    sr_engine::dac_expand(entry[2]),
                ];
                if shown != rgb {
                    wrong += 1;
                }
            }
        }
        wrong
    }

    /// Every `(64-x) * mul div div` candidate rendered over the four keyframes:
    /// 3/5 leaves 1,108 pixels (all but 17 the ink-presence residual of §7.2)
    /// and every other candidate at least 3,500 (`tunneli.md` §5).
    #[test]
    fn the_second_ramp_is_derived_by_matching_the_reference_bit_for_bit() {
        let frames: Vec<(Vec<u8>, Vec<u8>)> =
            ALIGNED.iter().map(|(tick, name)| aligned_frame(*tick, name)).collect();

        let best = colour_error(second_ramp, &frames);
        assert!(
            best <= 1_500,
            "the port's own ramp leaves {best} pixels wrong; the search's answer should be ~1,108"
        );

        // `mul` 1..=7 against `div` 1..=15, less 3/5's other spellings and the
        // duplicates of smaller pairs: 81 candidates.
        let mut worse = Vec::new();
        for mul in 1..=7u32 {
            for div in 1..=15u32 {
                if mul * 5 == 3 * div {
                    continue; // the shipped ramp itself, in another spelling
                }
                if div % mul == 0 && mul != 1 {
                    continue; // a duplicate of a smaller pair
                }
                let error = colour_error(|n| (n * mul / div) as u8, &frames);
                assert!(
                    error >= 3_500,
                    "{mul}/{div} is {error}, too close to the 3/5 ramp's {best}"
                );
                worse.push((error, mul, div));
            }
        }
        assert_eq!(worse.len(), 81, "the search's size, as the record states it");
        worse.sort();
        assert_eq!(
            (worse[0].1, worse[0].2),
            (7, 12),
            "the runner-up should be 7/12, and it is {:?}",
            worse[0]
        );
    }
}
