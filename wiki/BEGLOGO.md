# BEGLOGO: the title picture, out of white

BEGLOGO is the demo's title card. It follows [PAM](PAM)'s explosion: on a white screen, the
*Second Reality* title picture (the lettering and the crouching creature) fades in from that white
over about two seconds. Then it holds while [GLENZ](GLENZ) loads and waits for the music. In a full
run it starts about 1:53 in (boot frame 7,885) and lasts 172 frames, roughly 2.5 seconds. The
upstream directory is `BEG/`. The demo's own part list gives it no label, and the planning file
`SCRIPT` credits the "Second Reality Title" picture to Pixel, with the code by "WF/PSI"
([SCRIPT#L32](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/SCRIPT#L32-L32)).
BEGLOGO is also the first part of the demo's second half: the loader's `SECOND 2` entry starts
here (see [Demo structure](Demo-structure)).

## What you see

![Halfway through the fade: each palette entry is part way from white to its own colour](images/beglogo-8000.png)

![The finished picture under its own palette, held while GLENZ waits for the music](images/glenz-8100.png)

## The trick

This part does not animate. It draws the picture once and then changes only the palette. That
works because of how VGA colour works (see [Hardware primer](Hardware-primer)): a pixel in video
memory is an 8-bit *index*, and the DAC turns each index into a colour through a 256-entry table of
6-bit red, green and blue values (0..63). If you rewrite the table, every pixel on the screen
changes colour at once, and not one byte of video memory is touched.

The part has four steps:

1. **Fill video memory with index 15.** The part does not write the DAC yet, so the field shows
   whatever colour entry 15 already has, which is white. Then it waits 32 vertical retraces.
2. **Make the whole table white.** Entries 0..254 are set to (63,63,63) and entry 255 to black.
   Every index now looks white, whatever value it holds.
3. **Draw the picture.** All 400 rows are decoded into video memory. Nobody can see this step,
   because every colour the picture uses is still white.
4. **Crossfade the table.** For `c` = 0..128, one step per vertical retrace, every channel of every
   entry is set to

   ```
   pal2[a] = ((128 − c) · 63 + palette[a] · c) / 128
   ```

   That is linear interpolation in integer arithmetic. At `c = 0` it gives 63 (white). At
   `c = 128` it gives the picture's own value. The two weights `(128 − c)/128` and `c/128` always
   add to 1, 128 steps make a power-of-two denominator, and the integer division truncates. After
   the loop the picture's own palette is written once more.

Every entry arrives at the same step, but each moves at a rate set by its distance from white.
Halfway through, a dark colour is still only half as dark as it will be, so the whole picture
looks washed out until near the end. The middle frame above shows this.

### A 320×400 screen

The picture is 320×400, twice the usual 200 lines. Standard Mode 13h is really a 400-line signal
in which each row is drawn twice: the "maximum scan line" field of CRTC register 9 says two
scanlines per row. BEGLOGO clears that field (and the double-scan bit beside it), so each row of
memory fills one scanline and the screen shows 400 distinct rows. 128,000 pixels do not fit in the
64 KB window of chained Mode 13h, so the screen is in unchained ("Mode X") memory, where each byte
address holds four pixels, one in each of four planes. `lineblit` writes a row in four passes: the
sequencer's map mask register selects plane `p`, and pixels `p`, `p+4`, `p+8`, … of the row go into
that plane's 80 bytes.

### The picture format

The picture is in `.UP` format, a simple run-length encoding. A small header gives a magic
number, the width, height, colour count and a data offset in 16-byte units. The palette
follows at byte 16. Each row is stored as a 16-bit byte count followed by codes: a byte with its
top bit clear is a literal pixel, and a byte with its top bit set is `0x80 | n` followed by one
value, which means that value repeated `n` times.

## How the original did it

`BEG/BEG.C` is 87 lines of C (built with Microsoft's `cl /qc`). It begins by filling video memory with white. Writing 15
to sequencer register 2 (the map mask) enables all four planes, so each byte stored by `memset`
paints four pixels:

```c
	dis_partstart();
	outp(0x3c4,2);
	outp(0x3c5,15);
	memset(vram,15,32768);
	memset(vram+32768,15,32768);
	//_asm mov ax,80h+13h
	//_asm int 10h
	for(a=0;a<32;a++) dis_waitb();
```
[BEG/BEG.C#L17-L24](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/BEG/BEG.C#L17-L24)

Then the picture is drawn and the palette is faded, one step per `dis_waitb()` (the demo system's
"wait for the next vertical retrace"):

```c
	readp(palette,-1,pic);
	for(y=0;y<400;y++)
	{
		readp(rowbuf,y,pic);
		lineblit(vram+(unsigned)y*80U,rowbuf);
	}

	for(c=0;c<=128;c++)
	{
		for(a=0;a<768-3;a++) pal2[a]=((128-c)*63+palette[a]*c)/128;
		dis_waitb();
		setpalarea(pal2,0,254);
	}
	setpalarea(palette,0,254);
```
[BEG/BEG.C#L73-L86](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/BEG/BEG.C#L73-L86)

`readp` (`BEG/READP.C`) decodes one row of the RLE. With row `-1` it copies the palette out of the
header instead. Its inner loop is short inline assembly. `jns` tests the top bit, which separates
literals from runs:

```asm
	l1:	mov	al,ds:[si]
		inc	si
		or	al,al
		jns	l2
		mov	ah,al
		and	ah,7fh
		mov	al,ds:[si]
		inc	si
	l4:	mov	es:[di],al
		inc	di
		dec	ah
		jnz	l4
		cmp	si,cx
		jb	l1
		jmp	l3
```
[BEG/READP.C#L40-L54](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/BEG/READP.C#L40-L54)

`setpalarea` (`BEG/ASM.ASM`) points the DAC write index at `start` (port `0x3C8`) and streams
bytes to `0x3C9`. Look at how the byte count is computed:

```asm
	mov	si,[bp+6]
	mov	ds,[bp+8]
	mov	ax,[bp+10]
	mov	dx,3c8h
	out	dx,al
	inc	dx
	mov	cx,[bp+12]
	shl	cx,1
	add	cx,ax
	REPOUTSB
```
[BEG/ASM.ASM#L100-L109](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/BEG/ASM.ASM#L100-L109)

Three bytes per entry would be `count*3`. The code computes `count*2 + start`, so
`setpalarea(pal2,0,254)` sends 508 bytes: entries 0..168 in full and the red channel of entry 169.
The picture only uses indices 1-3, 14-58 and 71-79, all inside that range, so the bug cannot be
seen here. Entries 170..254 do stay white, and GLENZ inherits them.

## How the port does it

The part is
[`bins/secondreality/src/parts/beglogo.rs`](https://github.com/waffleking32/SecondRealityRust/blob/main/bins/secondreality/src/parts/beglogo.rs)
(`Beglogo`). It shares its machinery with [ENDLOGO](ENDLOGO), which is a near-copy of the same program, through
[`parts/logo.rs`](https://github.com/waffleking32/SecondRealityRust/blob/main/bins/secondreality/src/parts/logo.rs):

- `load_picture` decodes `assets/BEG/SRTITLE.UP` in one call, through the asset crate's `.UP`
  reader. It refuses any picture that is not 320×400.
- `set_400_line_view` stands in for the CRTC writes. It sets the engine's `View` to 400 rows with
  no double scan, display start 0 and overscan entry 255.
- `blit` writes the rows with `Surface::set`. The engine's surface is chunky (one byte per pixel),
  so the four-plane interleave of `lineblit` has nothing left to do.
- `white_to_picture(palette, c)` is the crossfade formula in the same integer arithmetic.
- `setpalarea` copies the original's `2*count + start` byte count exactly.

`Part::init` clears the surface to 15 and reads the picture. `Part::frame` runs one engine tick per
`dis_waitb`: ticks 0..31 hold the white field, tick 32 installs the white DAC and blits, and the
129 fade steps run from tick 42. The step after the last one writes the picture's own palette and
returns `Finished`. The part does not touch the DAC during the first 32 ticks, so the white comes
through the palette the previous part left, as in the original.

The part's notes are in
[`docs/fidelity/beglogo.md`](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/beglogo.md).
They cover the timeline and the bit-for-bit match that fixes when the fade starts. They also show
that `BEG/` is the build the release shipped, not the other `beg.exe` under `ENDPIC/` that waits
on the music.

## Quirks kept

- `setpalarea` sends `2*count + start` bytes, so entries 170..254 are never faded and stay white
  for the next part
  ([bugs.md §4](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/bugs.md)).
- The white field shows index 15 through the inherited palette for 32 frames before the part
  writes the DAC at all
  ([beglogo.md §4](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/beglogo.md)).
- The original's fade starts nine frames later than the source explains. The port holds white for
  those nine frames but does not model their cause, which is unknown
  ([APPROXIMATIONS.md A5](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/APPROXIMATIONS.md)).

---

**Previous:** [PAM](PAM) · **Next:** [GLENZ](GLENZ)
