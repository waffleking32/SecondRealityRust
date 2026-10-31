# PANICEND: the fake ending

PANICEND is the demo's fake ending. Just after [TECHNO](TECHNO)'s troll settles, the picture
squashes vertically into a band that grows brighter as it gets thinner, the way an old CRT
television's picture collapses when it is switched off. The band becomes a single bright
line, the line shrinks from both ends to a dot, the dot pulses twice and goes dark, and the
screen stays black long enough for the viewer to think the demo is over. Then the shell waits for
the music's next cue and [MNTSCRL](MNTSCRL) begins the second half. The effect itself lasts 121 frames (under two seconds),
about four minutes into the demo. The shell labels it `Panicfake (WILDF)`.
Its source is the upstream `PANIC/` directory: `SHUTDOWN.C` with two small assembly helpers.

## What you see

![The picture it starts from: TECHNO's troll](images/techno-15800.png)

![The line, after the collapse: one row of the troll, almost white](images/panicend-16200.png)

## The trick

PANICEND draws no picture of its own. `SHUTDOWN.C` defines `_LOADER_` near its top, so the
file-loading branch (which would read a test picture, `MONSTER.U`) is compiled out. The part works on whatever video memory and [DAC](Hardware-primer) TECHNO left behind. Those hold the troll:
256-colour mode with chain-4 off, 400 lines, and a 640-pixel virtual row (160 bytes), with the
troll in the right half. `getpal` reads all 768 DAC bytes back so that the part has a copy of
the palette.

### Fading to white with 64 tables

The first step builds 64 palettes, each a blend of the troll's palette towards white:

```
fadepals[a][i] = (a·63 + pal[i]·(64 − a)) / 64        a = 0..63
```

`fadepals[0]` is the picture's own palette and `fadepals[63]` is nearly white. The loop starts
at byte 3, so entry 0 stays black in every table, and the background stays black while
everything else bleaches.

### Halving the screen with the CRTC

Two CRTC writes change how memory is displayed:

- Register `09h = 41h` sets the *maximum scan line* to 1, so each memory row is shown on two
  scanlines. The 400-line screen becomes 200 logical rows.
- Register `13h = A0h` (the offset) doubles the distance between displayed rows to two memory
  rows.

Together, displayed row `r` is memory row `2r`, shown twice. The display now shows only the
left 320 pixels of every second memory row. Before the switch, the part copies every fourth
troll row into memory rows 150..249 of the left half. Its even rows are then the troll at a
quarter of its height, centred on display row 100. That is the first squash.

### The collapse

The collapse runs eleven passes with `a = 32, 26, 21, 17, 14, 11, 9, 7, 5, 4, 3`
(`a = a*5/6` in integer arithmetic, stopping when `a` reaches 2). Each pass:

1. Uploads `fadepals[63 − a]`, which is brighter as `a` shrinks.
2. Blacks memory rows `200 ± 2b` for `b = a/2 … a`, which erases the outer edge of the
   previous, taller band.
3. Squeezes the **whole** 400-row troll into `a` rows. Source row `400·b/a` goes to memory
   row `200 + 2(b − a/2)`, for `b = 0 … a−1`.

```
pass a=32:  ████████████████  32 display rows, palette 31/64 of the way to white
pass a=14:  ███████           14 rows, 49/64
pass a=3 :  ██                 3 rows, 60/64, then rows 198 and 202 blacked
                               → one line: memory row 200 = display row 100
```

Each pass is a vertical resample: the image is not cropped, it is sampled more sparsely, so
the band always shows the whole troll compressed.

### Copying four pixels per byte through the latches

With chain-4 off, one byte address covers four pixels, one in each plane. Copying a row with
the CPU would normally mean four passes, one per plane. The part instead sets the graphics
controller to **write mode 1** (`05h = 41h`) with all four planes enabled. In write mode 1, a
read from video memory loads the VGA's four internal *latches* with that address's byte from
every plane, and a write stores the latches into all four planes, whatever the CPU writes.
A plain memory-to-memory copy therefore moves four pixels per byte.

`copyline` copies with `rep movsw`, which moves words. A word read is two byte reads, and the
latches keep only the second one. The word write then stores that second byte into both
destination bytes. Each 8-pixel group of the copy is therefore the source's *second* 4-pixel
group twice. That is why the final line runs from x = 40 to 276, four pixels wider on the left
than the troll's own row, with a three-pixel gap at 273..275.

### The line and the dot

Back in write mode 0, the part blacks four pixels at each end of row 200 every frame, stepping
3 (so one pixel overlaps), for 47 frames from x = 20 to x = 158. The line closes in from both
sides. It then plots colour 1 at (160, 200), display (160, 100), and animates DAC entry 1 for
60 frames:

```
b = cos(a/120 · 3 · 2π) · 31 + 32        a = 0..59
```

That is one and a half cosine periods: bright (63), dark, bright, then close to dark (1) at the
end. The source ends with `sleep(1)`, and the part exits. The demo's shell
then keeps the last screen until the music's next cue before starting MNTSCRL (see
[Demo structure](Demo-structure)).

## How the original did it

The mode change, set while the troll is still showing
([SHUTDOWN.C#L67-L79](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PANIC/SHUTDOWN.C#L67-L79)):

```c
asm {
    mov	dx, 3d4h
    mov	ax, 4109h
    out	dx, ax

    mov	dx, 03ceh
    mov	ax, 4105h
    out	dx, ax			// latch transfer mode

    mov	dx, 03c4h
    mov	ax, 0f02h		// map mask 1111
    out	dx, ax
    }
```

The collapse
([SHUTDOWN.C#L91-L102](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PANIC/SHUTDOWN.C#L91-L102)).
Memory row 0 was blacked earlier, so copying it is how a row gets cleared. `80+…*160` is the
right half of a troll row:

```c
for(a=32;a>2;a=a*5/6)
    {
    dis_waitb();
    tw_setpalette(fadepals[63-a]);
    for(b=a/2;b<=a;b++)
        {
        copyline(MK_FP(0x0a000,0),MK_FP(0x0a000,200*160-(b*320)),80);
        copyline(MK_FP(0x0a000,0),MK_FP(0x0a000,200*160+(b*320)),80);
        }
    for(b=0;b<a;b++)
        copyline(MK_FP(0x0a000,80+(400*b/a)*160),MK_FP(0x0a000,200*160+(b-a/2)*320),80);
    }
```

The whole of `copyline`
([ASMYT.ASM#L15-L20](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PANIC/ASMYT.ASM#L15-L20)):

```asm
	lds	si, [from]
	les	di, [to]
	mov	cx, [count]

	cld
	rep	movsw
```

The dot and its pulse
([SHUTDOWN.C#L129-L135](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PANIC/SHUTDOWN.C#L129-L135)):

```c
tw_putpixel(160,200,1);
for(a=0;a<60;a++)
    {
    dis_waitb();
    b=cos(a/120.0*3*2*3.1415926535)*31.0+32;
    tw_setrgbpalette(1,b,b,b);
    }
```

`SHUTDOWN.C` never calls `dis_exit()`. Pressing ESC reaches it only through the shell.

## How the port does it

The port is [`parts/panicend.rs`](https://github.com/waffleking32/SecondRealityRust/blob/main/bins/secondreality/src/parts/panicend.rs),
a `Panicend` that implements the `Part` trait. The original is a straight line of 121
`dis_waitb()` calls, so the port counts ticks (`step`), and each `frame` call runs one segment
between waits. `init` takes over the engine's surface and palette exactly as TECHNO left them.
Nothing is reloaded, as in the original.

- The engine's surface stores the unchained screen as 640 pixels a row, so one planar byte is
  four consecutive surface pixels. `copyline` implements the latch copy in those terms: for
  each word, four pixels from the second byte are written twice.
- `fade_tables` builds the 64 tables from byte 3. `collapse`, `first_copy`, `black_row_zero`
  and `line_iteration` are the source's loops.
- The CRTC changes become the engine's `View`. `present` sets `start_addr`, the height (400,
  then 200) and `y_line_offset` (640 pixels, then 1,280 for the two-row stride).

The part registers no copper callbacks, because the original installs none. The tick table,
the evidence and the exact frames are in
[docs/fidelity/panicend.md](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/panicend.md).
[How the port works](How-the-port-works) has the engine's side of the hand-over.

## Quirks kept

- `getpal`'s `mov al, 0d` is decimal zero, so the palette is read from entry 0 and not
  rotated ([bugs.md §13.1](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/bugs.md)).
- `copyline`'s word copy through the latches stores each pair's second byte twice, which
  widens the line to x = 40..276 and leaves a gap at 273..275
  ([bugs.md §13.2](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/bugs.md)).
- The fade tables start at byte 3, so entry 0 is black in all 64, and the white entry 0 set
  at `:81` lasts one frame ([bugs.md §13.3](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/bugs.md)).
- The collapse stops at `a = 3` and writes only even memory rows
  ([bugs.md §13.4](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/bugs.md)).
- The part never polls the exit flag. The shell retires it
  ([panicend.md §9](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/panicend.md)).
- A standalone `--part panicend` run has no TECHNO screen to inherit, so its collapse and line
  frames are blank. Through the hand-over they are exact
  ([APPROXIMATIONS.md A63](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/APPROXIMATIONS.md)).

---

**Previous:** [TECHNO](TECHNO) · **Next:** [MNTSCRL](MNTSCRL)
