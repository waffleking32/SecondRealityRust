# Approximations register

Every place the port knowingly differs from the original. An entry requires a
justification and a measured visual difference; it is not a parking place for "close
enough".

## A1 — Overscan border is padded, not scanned out

**Part:** all; used deliberately by PANIC and the 320×400 parts.
**Original:** the border is the region of the display outside the active area, set by
AC 0x11 and scanned out by the CRTC.
**Port:** `View::border` is a colour; `present()` draws it as a frame of 8 source pixels
per side, scaled with the picture.
**Why:** the engine composes the active display only (spec §4.2, plan decision 8). The
border must still appear, but its exact thickness depends on CRTC timings the port does
not model.
**Measured difference:** 8 source pixels per side at every geometry. To be re-measured
against the original when PANIC is ported (Phase 5).
