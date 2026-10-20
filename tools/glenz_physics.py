#!/usr/bin/env python3
"""GLENZ's opening: the source's main-loop recurrence and its first drawn frame.

Task 1 of Phase 7's measurement, kept as a runnable tool (D25): the note
`docs/fidelity/reference-indices.md` §7.6 quotes this tool's `entry` output beside
the captures' first object pixels.

    python3 -I tools/glenz_physics.py entry   # frame 44 and frames 44..49's boxes
    python3 -I tools/glenz_physics.py drawn   # the first drawn main-loop frame only

It reads only the vendored tree: `GLENZ/MATHSIN.INC`'s sin/cos words and
`GLENZ/MAIN.C`'s `points`/`epolys`. The arithmetic is the source's, not a model of
it: `cmatrix_yxz`'s nine `shld dx,ax,1` products, `rotlist`'s 64-bit sum shifted
right 15, `projlist`'s `imul`/`idiv` with the 16-bit wrapped centre add, and
`NEW.ASM`'s per-row edge spans (x clamped to 1..319, the XOR walk, each pixel
`colour | bgpic`). The first object pixel is the first frame on which a span with a
non-zero colour covers a screen pixel.

The main loop's phase is `MAIN.C:409-641`: `rx += 32`, `ry += 7` (`%= 10800`), and
for `frame <= 800` the `ypos`/`yposa`/`jello` recurrence of `:502-536` with the
initialisers of `:275-279`. Only the first object (`xscale > 4`) is needed: the
second appears after frame 800.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MATHSIN = os.path.join(ROOT, 'upstream/SecondReality/GLENZ/MATHSIN.INC')
MAIN_C = os.path.join(ROOT, 'upstream/SecondReality/GLENZ/MAIN.C')


def parse_tables(path):
    """`sintable16` (900 words) followed by `costable16` (3,600)."""
    words = []
    cost = None
    seen_sin = False
    with open(path) as handle:
        for line in handle:
            text = line.strip()
            if text.startswith('sintable16'):
                seen_sin = True
                continue
            if text.startswith('costable16'):
                cost = len(words)
                continue
            if not text.startswith('dw') or not seen_sin:
                continue
            words.extend(int(v) for v in re.findall(r'-?\d+', text[2:]))
    assert cost is not None, 'MATHSIN.INC has no costable16'
    assert len(words) == cost + 3600, (len(words), cost)
    sin_full = words[:cost + 2700]   # sin(a) = costable16[a - 900] past the quarter
    cos_full = words[cost:cost + 3600]
    return sin_full, cos_full


SIN, COS = parse_tables(MATHSIN)


def s16(value):
    value &= 0xFFFF
    return value - 0x10000 if value >= 0x8000 else value


def trunc_div(a, b):
    """C's `/` for ints: truncation toward zero."""
    quotient = abs(a) // abs(b)
    return quotient if (a >= 0) == (b >= 0) else -quotient


def shl15(product):
    """`shld dx,ax,1`: the low 16 bits of the signed 32-bit product >> 15."""
    return s16((product >> 15) & 0xFFFF)


def checkdeg(angle):
    while angle >= 3600:
        angle -= 3600
    while angle < 0:
        angle += 3600
    return angle


def matrix_yxz(a0, a1, a2):
    """`cmatrix_yxz(a0,a1,a2)`: a1 fills calcmatrix's X slot, a0 its Y, a2 its Z."""
    first, second, third = checkdeg(a1), checkdeg(a0), checkdeg(a2)
    sx, cx = SIN[first], COS[first]
    sy, cy = SIN[second], COS[second]
    sz, cz = SIN[third], COS[third]
    m = [0] * 9                       # m[0..8] = matrix[0],[2],[4],[6],[8],[10],[12],[14],[16]
    m[7] = shl15(sy * sz)             # matrix[14]
    m[0] = shl15(cy * cz)             # matrix[0]
    m[7] = s16(m[7] - shl15(m[0] * sx))
    cxr = shl15(sx * sy)              # calcmatrix's `cx`
    m[0] = s16(m[0] - shl15(sz * cxr))
    m[1] = shl15(cz * cxr)            # matrix[2]
    m[1] = s16(m[1] + shl15(cy * sz))
    m[6] = shl15(sx * shl15(cy * sz))  # matrix[12]
    m[6] = s16(m[6] + shl15(sy * cz))
    m[3] = s16(-shl15(cx * sz))       # matrix[6]
    m[4] = shl15(cx * cz)             # matrix[8]
    m[2] = s16(-shl15(cx * sy))       # matrix[4]
    m[8] = shl15(cx * cy)             # matrix[16]
    m[5] = sx                         # matrix[10]
    return m


def rotate(points, matrix, add):
    out = []
    for x, y, z in points:
        out.append((
            shl15(x * matrix[0] + y * matrix[1] + z * matrix[2]) + add[0],
            shl15(x * matrix[3] + y * matrix[4] + z * matrix[5]) + add[1],
            shl15(x * matrix[6] + y * matrix[7] + z * matrix[8]) + add[2],
        ))
    return out


def project(points, mul_x=256, mul_y=213, add_x=160, add_y=130, min_z=128):
    """`projlist` with `init320x200`'s constants: 16-bit wrapped centre adds."""
    out = []
    for x, y, z in points:
        zz = z if z >= min_z else min_z
        out.append((s16(trunc_div(x * mul_x, zz) + add_x),
                    s16(trunc_div(y * mul_y, zz) + add_y), z))
    return out


def points_from_main():
    with open(MAIN_C) as handle:
        src = handle.read()
    body = re.search(r'long\s+points\[64\]=\{14,(.*?)\};', src, re.S).group(1)
    body = body.replace('ZZZ', '50').replace('QQQ', '99')
    values = [eval(tok.strip(), {'__builtins__': {}}) for tok in body.split(',') if tok.strip()]
    assert len(values) == 42, len(values)
    return [(values[i], values[i + 1], values[i + 2]) for i in range(0, 42, 3)]


def epolys():
    with open(MAIN_C) as handle:
        src = handle.read()
    body = re.search(r'int epolys\[\]=\{(.*?)\n0\};', src, re.S).group(1)
    values = [int(v, 0) for v in re.findall(r'0x[0-9a-fA-F]+|\d+', body)]
    polys, at = [], 0
    while at < len(values):
        count, colour = values[at], values[at + 1]
        if count == 0:
            break
        polys.append((colour, values[at + 2:at + 2 + count]))
        at += 2 + count
    return polys


PTS = points_from_main()
EPOLYS = epolys()


def poly_lists():
    """`ceasypolylist`: dedupe consecutive points (carried across polygons), drop the
    closing duplicate. The cross is taken on the first three points of each."""
    out = []
    previous = None
    for colour, indices in EPOLYS:
        points = []
        for index in indices:
            if index == previous:
                continue
            previous = index
            points.append(index)
        if len(points) >= 2 and points[-1] == points[0]:
            points.pop()
        if len(points) >= 3:
            out.append((colour, points))
    return out


POLYLISTS = poly_lists()


def cross3(points):
    (x0, y0, _), (x1, y1, _), (x2, y2, _) = points[0], points[1], points[2]
    return (x0 - x1) * (y0 - y2) - (y0 - y1) * (x0 - x2)


def painted_pixels(projected):
    """`NEW.ASM`'s edge list and span walk: the rows 0..199 pixels a non-zero colour fills."""
    bounds = {}
    for colour_word, indices in POLYLISTS:
        points = [projected[i] for i in indices]
        hidden = cross3(points) < 0
        colour = ((colour_word >> 1) & 1) << 2 if hidden else 16
        count = len(points)
        for k in range(count):
            x1, y1, _ = points[k]
            x2, y2, _ = points[(k + 1) % count]
            if y1 > y2:
                x1, x2 = x2, x1
                y1, y2 = y2, y1
            if y1 == y2:
                continue
            x = x1 << 16
            dx = trunc_div((x2 - x1) << 16, y2 - y1)
            if y1 < 0:
                if y2 <= 0:
                    continue
                x += dx * (-y1)
                y1 = 0
            for row in range(y1, y2):
                if 0 <= row <= 199:
                    clamped = (x >> 16) if x >= 0 else -((-x) >> 16)
                    clamped = min(319, max(1, clamped))
                    bounds.setdefault(row, []).append((clamped, colour))
                x += dx
    pixels = set()
    for row, entries in bounds.items():
        entries.sort()
        colour = 0
        previous = None
        for x, edge in entries:
            if previous is not None and x > previous and colour != 0:
                pixels.update((pixel, row) for pixel in range(previous, x))
            if previous is not None and x == previous:
                colour ^= edge
                continue
            colour ^= edge
            previous = x
        if previous is not None and colour != 0:
            pixels.update((pixel, row) for pixel in range(previous, 320))
    return pixels


def bbox(pixels):
    if not pixels:
        return None
    xs = [p[0] for p in pixels]
    ys = [p[1] for p in pixels]
    return (min(xs), max(xs), min(ys), max(ys), len(pixels),
            round(sum(xs) / len(pixels), 1), round(sum(ys) / len(pixels), 1))


def state_at(frame):
    """The main-loop state and projected list at main-loop frame `frame` (1-based)."""
    ypos, yposa = -9000, 0
    jello, jelloa = 0, 0
    boingm, boingd = 6, 7
    xscale = yscale = zscale = 120
    rx = ry = 0
    zpos = 7500
    for current in range(1, frame + 1):
        rx = (rx + 32) % 10800
        ry = (ry + 7) % 10800
        if current <= 800:
            if current < 710:
                yposa += 31
                ypos += trunc_div(yposa, 40)
                if ypos > -300:
                    ypos -= trunc_div(yposa, 40)
                    yposa = trunc_div(-yposa * boingm, boingd)
                    boingm += 2
                    boingd += 1
                if ypos > -900 and yposa > 0:
                    jello = trunc_div((ypos + 900) * 5, 3)
                    jelloa = 0
            else:
                if ypos > -2800:
                    ypos -= 16
                elif ypos < -2800:
                    ypos += 16
            xscale = yscale = 120 + trunc_div(jello, 30)
            zscale = 120 - trunc_div(jello, 30)
            previous = jello
            jello += jelloa
            if (previous < 0 and jello > 0) or (previous > 0 and jello < 0):
                jelloa = trunc_div(jelloa * 5, 6)
            jelloa -= trunc_div(jello, 20)
        matrix = matrix_yxz(rx, ry, 0)
        rotated = rotate(PTS, matrix, (0, 0, 0))
        scale = [0] * 9
        scale[0], scale[4], scale[8] = xscale * 64, yscale * 64, zscale * 64
        moved = rotate(rotated, scale, (0, ypos + 1500, zpos))
        if current < 800:
            moved = [(x, min(y, 1500), z) for (x, y, z) in moved]
        projected = project(moved)
    return dict(frame=frame, ypos=ypos, jello=jello, xscale=xscale, proj=projected)


def first_drawn(last=400):
    for frame in range(1, last + 1):
        pixels = painted_pixels(state_at(frame)['proj'])
        if pixels:
            return frame, state_at(frame), pixels
    return None


def entry():
    first, state, pixels = first_drawn()
    print(f"first drawn main-loop frame: {first} "
          f"(ypos {state['ypos']}, jello {state['jello']}, xscale {state['xscale']})")
    print('frame  x0,x1,y0,y1        n    centroid')
    for frame in range(first, first + 6):
        box = bbox(painted_pixels(state_at(frame)['proj']))
        print(f"{frame:5d}  ({box[0]},{box[1]},{box[2]},{box[3]})  {box[4]:4d}  "
              f"({box[5]}, {box[6]})")


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else 'entry'
    if mode == 'drawn':
        first, state, _ = first_drawn()
        print(f"first drawn main-loop frame: ({first}, {state['ypos']}, "
              f"{state['jello']}, {state['xscale']})")
    elif mode == 'entry':
        entry()
    else:
        raise SystemExit(f"unknown mode {mode!r}: use entry or drawn")


if __name__ == '__main__':
    main()
