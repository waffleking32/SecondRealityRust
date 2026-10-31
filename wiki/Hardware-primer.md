# Hardware primer: the VGA as Second Reality uses it

Every effect page in this wiki assumes the few VGA facts on this page. The 1993 VGA has no
blitter, no sprites and no 3D hardware. What it has is 256 KB of video memory, a palette, and a
display controller (the CRTC) whose registers say *where* in memory the picture is read from and
*how*. Second Reality gets most of its tricks from writing those registers at the right moment of
the frame. The CPU draws everything else, pixel by pixel.

All of it is driven through I/O ports. The ones the demo touches:

| port | what it is |
|---|---|
| `3C4h`/`3C5h` | the **sequencer**: index, data. Register 2 is the **map mask**, register 4 the memory mode (chain-4). |
| `3D4h`/`3D5h` | the **CRTC**: index, data. It holds the start address, the line offset, the line compare and the scan doubling. |
| `3C0h` | the **attribute controller**. It holds pel panning (`13h`) and the overscan colour (`11h`). |
| `3C7h`/`3C8h`/`3C9h` | the **DAC**: read index, write index, data. This is the palette. |
| `3DAh` | the input status register. Bit 3 is set while the beam is in **vertical retrace**. |

A 16-bit `out dx,ax` with the index in `al` and the value in `ah` writes an index and its data in
one instruction. Most of the excerpts below do that.

## Mode 13h, and why the demo leaves it

BIOS Mode 13h is the famous 320×200, 256-colour mode. Memory at segment `A000h` is one byte per
pixel, row after row, so pixel `(x, y)` is byte `y*320 + x`. It is simple to draw in. It reaches
only 64,000 bytes of the card's 256 KB, so it has room for one screen and nothing else.

The card really stores its memory as **four planes** of 64 KB. Mode 13h hides them with
**chain-4**: the low two bits of an address pick the plane. Switch chain-4 off and you get the
"unchained" layout that Michael Abrash popularised as **Mode X** (the demo calls it "tweaked"):

```
pixel (x, y) lives in plane  x & 3,  at byte  y*80 + x/4

          plane 0   plane 1   plane 2   plane 3
byte 0:   x=0       x=1       x=2       x=3
byte 1:   x=4       x=5       x=6       x=7
...
```

The cost is that a write goes to whichever planes the **map mask** (sequencer register 2) enables.
To set one pixel you select its plane and then write its byte. What you gain:

- **All 256 KB.** A 320×200 screen is only 16,000 bytes per plane, so four whole screens fit, and
  the display can be pointed at any of them (page flipping, below).
- **Four pixels per write.** With the map mask at `0Fh`, one byte written lands in all four planes:
  four identical pixels in one store. Clears and wide fills run four times faster.
- **400 lines.** The same layout extends to 320×400.

The shell's `inittwk` makes the switch, starting from the BIOS's Mode 13h. The shell calls it when a
run starts partway through, at [BEGLOGO](BEGLOGO)'s or [ENDLOGO](ENDLOGO)'s block, so that those parts find the mode in
place
([MAIN/U2.ASM:805-808](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/U2.ASM#L805-L808)):

```asm
	;400 rows
	mov	dx,3d4h
	mov	ax,00009h       ; CRTC 9 = 0: scan every row once
	out	dx,ax
	;tweak
	mov	dx,3d4h
	mov	ax,00014h       ; CRTC 14h = 0: doubleword addressing off
	out	dx,ax
	mov	ax,0e317h       ; CRTC 17h = E3h: byte addressing
	out	dx,ax
	mov	dx,3c4h
	mov	ax,0604h        ; sequencer 4 = 06h: chain-4 off
	out	dx,ax
```

[MAIN/U2A.ASM:1074-1086](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/U2A.ASM#L1074-L1086)
(comments added). It then opens all four planes and clears all of video memory, 32,768 words
(comment added):

```asm
	mov	dx,3c4h
	mov	ax,0f02h        ; map mask = 0Fh: write all four planes
	out	dx,ax
	mov	ax,0a000h
	mov	es,ax
	xor	di,di
	mov	cx,32768
	xor	ax,ax
	rep	stosw
```

[MAIN/U2A.ASM:1088-1096](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/U2A.ASM#L1088-L1096)

Several parts open their own variant of the mode with a routine of the same shape, `tw_opengraph`
in the part's `TWEAK.ASM`.

## 320×400 and double scan

The VGA's 256-colour modes always send **400 lines** to the monitor. Mode 13h gets its 200 rows by
scanning each row twice. CRTC register 9, *maximum scan line*, sets how many times each row of
memory is repeated (its bit 7 is a separate "double scan" switch that doubles every line again).
`inittwk` writes 0 there, so each row is shown once and the screen becomes **320×400**: square-ish
pixels at twice the vertical resolution. [ALKU](ALKU), BEGLOGO, [JPLOGO](JPLOGO), ENDLOGO and [DDSTARS](DDSTARS) are among the
parts that show 320×400 screens. Changing the doubling changes the picture's height without touching memory. [PANICEND](PANICEND)
writes `41h` to register 9 (each row scanned twice again) just before its screen collapses
([PANIC/SHUTDOWN.C:67-70](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PANIC/SHUTDOWN.C#L67-L70)).

## The DAC: a 6-bit palette

A pixel byte is not a colour. It is an index into the **DAC**'s 256-entry table. Each entry is
three **6-bit** values, 0..63, for red, green and blue. To write a run of entries, send the first
index to `3C8h` and then R, G, B, R, G, B... to `3C9h`. The index advances after each triple.
Reading works the same way through `3C7h`. This is the shell's helper that sets entry 0, for
example:

```asm
setblack0:
	mov	dx,3c8h
	mov	al,0
	out	dx,al
	inc	dx
	mov	al,cl
	out	dx,al
	mov	al,ch
	out	dx,al
	xor	al,al
	out	dx,al
```

[MAIN/COPPER.ASM:263-273](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/COPPER.ASM#L263-L273)

Because the palette is separate from the pixels, rewriting it is the cheapest animation there is.
In 768 `out`s every pixel on the screen changes colour, and none of them is touched:

- **Fades** step every entry towards a target, black or white, a little each frame.
- **Colour cycling** rotates a range of entries, so a static picture appears to flow.
- **Raster bars** rewrite an entry while the beam is partway down the screen, so the same index
  shows a different colour on different lines.

[PAM](PAM)'s white flash works this way. It precomputes 64 palettes, from the picture's own (number 0) to
pure white (number 63), each a weighted average of the two, and then uploads one per step as its
`wfade` table walks from white into the picture and back out to white:

```c
	for(a=1;a<64;a++) for(b=0;b<768;b++)
		pal[a*768+b]=(63*a+(64-a)*pal[b])/64;
```

[PAM/OUTTAA.C:36-37](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PAM/OUTTAA.C#L36-L37)

Six bits matter. A fade has only 64 levels per channel, and the original's integer rounding
decides which level each step lands on. The port therefore keeps the palette at 6 bits and widens
it to 8 only when the picture reaches the screen. It widens the way the emulator it is checked
against does, `(c*259 + 33) >> 6`, which is `round(255c/63)`.

## Retrace, and the "copper"

The monitor's beam draws the screen top to bottom about **70 times a second** (70.086 Hz in these
modes), then spends a short **vertical retrace** returning to the top. Anything written during
retrace takes effect cleanly at the next frame. Bit 3 of `3DAh` says whether the beam is in
retrace, and the classic wait for the start of retrace is two loops:

```asm
copperwaitborder PROC NEAR
	mov	dx,03dah
cbl1:	in	al,dx
	test	al,8
	jnz	cbl1
cbl2:	in	al,dx
	test	al,8
	jz	cbl2
	ret
copperwaitborder ENDP
```

[MAIN/COPPER.ASM:72-81](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/COPPER.ASM#L72-L81)

Spinning on that port wastes the CPU, and a part busy drawing a polygon cannot stop to watch it.
Second Reality's shell therefore has a **"copper simulator"**, named after the Amiga's display
coprocessor ("Don't play with this one!", the file warns). At start-up `syncronize` measures how
many PIT timer counts one frame lasts. The timer interrupt is then reprogrammed every frame to
fire at fixed fractions of it. The fractions come from a small list in hundredths of a frame:

```asm
copperlst0 LABEL WORD
	dw	0
	dw	20,OFFSET copper_doit
	dw	95,0
```

[MAIN/COPPER.ASM:40-43](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/COPPER.ASM#L40-L43)

So a fifth of a frame after retrace, near scan line 25 by the DIS's own comment, the timer calls
`copper_doit`. That runs the part's
first copper routine, then the music player's poll, then the shell's own check for a held Ctrl
key:

```asm
	cmp	cs:copper_int0flag,0
	jne	@@1
	mov	cs:copper_int0flag,1
	PUSHALL
	call	cs:copper_int0
	POPALL
	mov	cs:copper_int0flag,0
@@1:
	call	_zpollme
	
	call	u2copperint
```

[MAIN/COPPER.ASM:458-468](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/COPPER.ASM#L458-L468)

At 95/100 the list ends in `copper_end`. It calls the part's second routine, waits for the
retrace to begin, re-arms the timer from there, counts the frame and calls the third routine
([MAIN/COPPER.ASM:289-325](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/COPPER.ASM#L289-L325)):

```asm
	PUSHALL
	call	cs:copper_int1
	POPALL
	pop	bx
	mov	dx,03dah
cel3:	in	al,dx
	test	al,8
	jnz	cel3
cel4:	in	al,dx
	test	al,8
	jz	cel4
```

[MAIN/COPPER.ASM:301-311](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/MAIN/COPPER.ASM#L301-L311)

A part installs its three routines through the shell's interrupt service, `dis_setcopper`:

```asm
;entry: AX=number of copper interrupt to capture:
;               0=after display start (about scan line 25)
;               1=just before retrace (AVOID USING THIS IF POSSIBLE)
;               2=in the retrace
```

[DIS/DISINT.ASM:284-287](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/DIS/DISINT.ASM#L284-L287)

The parts use these slots for everything that has to happen at a fixed point of the frame:
uploading a palette in retrace, flipping pages, setting a split. The music player's poll,
`_zpollme`, also runs from here once per frame (see [Music](Music)). The frame
count that `copper_end` keeps is what a part's `dis_waitb()` waits on (see
[Demo structure](Demo-structure)). Two earlier ports of the demo left the copper out, and they
lost visible effects. This port runs all three slots every frame (see
[How the port works](How-the-port-works)).

## Start address, line offset and pel panning: scrolling for free

The CRTC does not have to start reading at address 0. Registers `0Ch`/`0Dh` (high and low byte)
hold the **start address**. Change it and the whole picture moves without a pixel being copied.
In Mode X the address counts plane bytes, so one step is 4 pixels horizontally, or 80 for a full
320-pixel row.

- **Vertical scroll:** add a multiple of the row length to the start address. JPLOGO scrolls its
  logo this way, and [ENDSCRL](ENDSCRL) scrolls through a circular buffer of 401 lines.
- **Horizontal scroll:** CRTC `13h`, the **line offset**, sets the row stride. In Mode X a row is
  twice the register's value in plane bytes, so 40 gives the normal 80 and a larger value makes a
  canvas wider than the screen. Moving the start address then pans across it. ALKU's horizon pan
  moves across a picture twice the screen's width.
- **Sub-step scroll:** attribute register `13h`, **pel panning**, shifts the picture left by a few
  pixels within one address step. In 256-colour modes it counts half pixels, so the values 0, 2,
  4 and 6 shift 0..3 pixels.

PLZPART's slot-1 copper routine sets both, just before retrace, so they take effect together on
the next frame. (Writing `33h` to `3C0h` selects register `13h` with bit 5 set, which keeps the
display enabled.)

```asm
	mov	dx, 03d4h
	mov	al, 0dh
	mov	ah, [Byte cs:cop_start]
	out	dx, ax
	mov	al, 0ch
	mov	ah, [Byte cs:cop_start+1]
	out	dx, ax

	mov	dx, 3c0h
	mov	al, 33h
	out	dx, al
	mov	ax, [cs:cop_scrl]
	out	dx, al
```

[PLZPART/COPPER.ASM:69-81](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PLZPART/COPPER.ASM#L69-L81)

## Page flipping

With four screens in memory there is no need to draw where the viewer can see. A part draws the
next frame into a hidden page and, once it is finished, points the start address at it. The
switch happens at the next retrace, so the viewer never sees a half-drawn frame. PAM alternates
between the page at `A000h` and the one at `A400h` (16,384 bytes further on) and shows whichever
it has just finished:

```c
	while(!dis_exit() && f++<45)
		{
		while(frame_count<4); frame_count=0;
		if(f<=40)
			{
			if(p)   {
				p=0;
				ulosta_frame(0x0a400);
				tw_setstart(16384);
				}
			else	{
				p=1;
				ulosta_frame(0x0a000);
				tw_setstart(0U);
				}
```

[PAM/OUTTAA.C:46-60](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PAM/OUTTAA.C#L46-L60)

(`frame_count` is incremented by PAM's own copper routine, so the animation advances every fourth
frame.)

## Split screen: line compare

The **line compare** register is a scan line number. When the beam reaches that line, the CRTC's
address counter resets to **0**, so everything below shows memory from address 0, whatever the
start address says. The screen becomes two independent views: an upper one that can scroll and a
lower one fixed at the start of memory. Moving the compare line up or down slides the boundary
between them.

The value is ten bits, spread over three registers: the low 8 bits in CRTC `18h`, bit 8 in bit 4
of the overflow register `07h`, and bit 9 in bit 6 of register `09h`. PLZPART's plasma sits in the
lower view, with the upper view pointed at black memory. When the plasma "falls off" the screen,
the slot-2 copper routine `do_drop` moves the line compare down a little each frame:

![PLZPART: the line compare split moves down, carrying the plasma off the screen](images/plzpart-24500.png)

```asm
	mov	bx, [cs:cop_drop]
	shl	bx, 1d
	add	bx, OFFSET dtau

	mov	bx, [cs:bx]
	mov	dx, 3d4h
	mov	al, 18h		; linecompare
	mov	ah, bl
	out	dx, ax
	mov	al, 07h
	mov	ah, bh
	shl	ah, 4d
	and	ah, 10h
	or	ah, 0fh
	out	dx, ax			; 8th bit
```

[PLZPART/COPPER.ASM:196-210](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PLZPART/COPPER.ASM#L196-L210)

Bit 8 of the value is shifted into bit 4 of register `07h`. The compare lines come from a table
the assembler builds, and it is a parabola: the screen falls the way a dropped object does.

```asm
LABEL	dtau Word
ccc=0
cccc=0
REPT	65				; 43=dy*512/dt²
	dw	ccc*ccc/4*43/128+60
ccc=ccc+1
```

[PLZPART/COPPER.ASM:175-180](https://github.com/mtuomi/SecondReality/blob/071a82eddb7a7390dbaa491a8a473e0e923a55c2/PLZPART/COPPER.ASM#L175-L180)

CRED uses a split as well. The [PLZPART](PLZPART) and [CRED](CRED) pages say what each does with
it.

## The overscan border

Attribute register `11h` gives the colour of the **border** around the active picture. Like a
pixel, it holds a palette index, not a colour. [TUNNELI](TUNNELI), PANICEND and ENDLOGO set it to entry 0,
and BEGLOGO to entry 255, which its palette makes black. The port draws the border as a flat frame
around the picture (its approximation
[A1](https://github.com/waffleking32/SecondRealityRust/blob/main/docs/fidelity/APPROXIMATIONS.md)).

## What the port keeps, and what it drops

The planes, the map mask and the four-pixels-per-write tricks only make drawing faster. They never
change *which* pixel ends up with *which* value. The port draws into a plain 8-bit canvas with the
original's own coordinate maths and drops the plane arithmetic. It keeps everything that changes
what the viewer sees:

1. palette writes, including ones made mid-frame;
2. the start address, line offset and panning;
3. the line compare split;
4. the scan doubling and the 400-line modes;
5. the border colour.

The port's `View` type holds these as one register set. [How the port works](How-the-port-works)
covers it.
