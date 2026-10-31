"""CRTC probe: boot the shipped original under the capture recipe and read the
VGA CRTC registers per frame.

Starts the unmodified original (`reference/original`, `SECOND <ARG>`, the
`AUTOTYPE left enter` boot of `cargo xtask reference capture`) in the
dosbox-automation emulator at 30,000 cycles, waits for boot frame START_FRAME,
then samples SAMPLES consecutive frames: display start (CRTC 0x0C/0x0D),
offset (0x13), max scan line (0x09), vertical display end (0x12), line compare
(0x18) and overflow (0x07), written to OUTDIR/crtc.json. It also dumps the
first 640 KB of guest memory to OUTDIR/conventional.bin.

It is how `docs/fidelity/APPROXIMATIONS.md` A23 established that the release's
PLZPART cube runs with CRTC 0x09 = 2 (three scanlines a row, a 134-row screen,
`PLZPART/TWEAK.ASM:31-33`): `python3 -I tools/crtc_probe.py 3 8500 30 <dir>`.
A sample taken between the demo's own CRTC writes can read a transient value
(max scan line 170 = 0xAA is the demo's own write racing the poll), so take the
majority reading.

Needs the emulator built at `$SR_DOSBOX` (default
`../dosbox-automation` beside the repository, `build/release-linux/dosbox-automation`)
and **no emulator already running** on port 8386.

usage: python3 -I tools/crtc_probe.py ARG START_FRAME SAMPLES OUTDIR
"""
import json, os, secrets, subprocess, sys, tempfile, time, urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOSBOX = os.environ.get("SR_DOSBOX", os.path.join(os.path.dirname(REPO), "dosbox-automation"))
BIN = DOSBOX + "/build/release-linux/dosbox-automation"
ORIG = REPO + "/reference/original"
BASE = "http://127.0.0.1:8386/api/v1"

arg, start, samples, outdir = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
os.makedirs(outdir, exist_ok=True)
token = secrets.token_hex(32)


def req(method, path, body=None, ctype="application/json"):
    data = None
    if body is not None:
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
    r = urllib.request.Request(BASE + path, data=data, method=method)
    r.add_header("Authorization", "Bearer " + token)
    if data is not None:
        r.add_header("Content-Type", ctype)
    with urllib.request.urlopen(r, timeout=30) as resp:
        return resp.read()


def jget(path):
    return json.loads(req("GET", path))


def outp(port, value):
    req("PUT", "/io/port", {"port": port, "value": value})


def inp(port):
    return jget(f"/io/port?port={port}")["value"]


def crtc(index):
    outp(0x3D4, index)
    return inp(0x3D5)


def frame():
    return jget("/script/status")["frame"]


scratch = tempfile.mkdtemp(prefix="sr-probe-")
os.makedirs(scratch + "/xdg/dosbox-automation")
os.makedirs(scratch + "/run")
prog = "SECOND" if arg == "1" else f"SECOND {arg}"
with open(scratch + "/xdg/dosbox-automation/dosbox-automation.conf", "w") as f:
    f.write(f"[webserver]\nwebserver_enabled = true\nmount_allowed_bases = {ORIG}\n"
            f"[autoexec]\nMOUNT C -RO {ORIG}\nC:\nAUTOTYPE -w 5 left enter\n{prog}\n")
deps = DOSBOX + "/build/release-linux/_deps"
libs = ":".join(sorted(os.path.join(deps, d) for d in os.listdir(deps) if d.endswith("-build")))
env = dict(os.environ, XDG_CONFIG_HOME=scratch + "/xdg", LD_LIBRARY_PATH=libs,
           SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy", DOSBOX_API_TOKEN=token)
log = open(outdir + "/emulator.log", "w")
proc = subprocess.Popen([BIN, "--set", "cpu_cycles=30000"], env=env, cwd=scratch + "/run",
                        stdout=log, stderr=log, stdin=subprocess.DEVNULL)
try:
    for _ in range(100):
        try:
            req("GET", "/dosbox/info")
            break
        except Exception:
            time.sleep(0.1)
    req("POST", "/script/load?name=watch", b"while true do\n  dosbox.wait_frames(60)\nend\n",
        "text/plain")
    req("POST", "/script/start", {})
    while frame() < start - 30:
        time.sleep(0.2)
    while frame() < start:
        time.sleep(0.005)
    rows = []
    last = None
    while len(rows) < samples:
        fr = frame()
        if fr == last:
            time.sleep(0.002)
            continue
        last = fr
        hi, lo = crtc(0x0C), crtc(0x0D)
        rows.append({"frame": fr, "start": (hi << 8) | lo, "offset": crtc(0x13),
                     "maxscan": crtc(0x09), "vde": crtc(0x12), "linecmp": crtc(0x18),
                     "overflow": crtc(0x07)})
    with open(outdir + "/crtc.json", "w") as f:
        json.dump(rows, f, indent=0)
    mem = bytearray()
    for off in range(0, 0xA0000, 0x10000):
        mem += req("GET", f"/memory/{off}/{min(0x10000, 0xA0000 - off)}")
    with open(outdir + "/conventional.bin", "wb") as f:
        f.write(mem)
    print("sampled", len(rows), "frames; memory", len(mem), "bytes")
finally:
    proc.terminate()
    proc.wait(timeout=10)
    subprocess.run(["rm", "-rf", scratch])
