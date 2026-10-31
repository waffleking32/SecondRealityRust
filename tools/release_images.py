"""Release-image check: does `sr_assets::exe::unpack` give the image DOS really runs?

For every shipped part EXE in `assets/MAIN/DATA/`, boots DOS in the dosbox-automation emulator
(30,000 cycles), mounts that directory read-only, runs the EXE alone, waits FRAMES frames and
dumps 640 KB of guest memory. Run alone, a part unpacks itself (PKLITE, then LZEXE or EXEPACK),
finds no DIS and exits with `ERROR: DIS not loaded`, leaving its image in memory. The script
finds the port's image (from `cargo test -p sr-assets --test exe -- --ignored
dump_the_unpacked_images`, in `out/release-images/`) in that memory, applies its relocations at
the load segment found, and prints how many bytes differ: the C runtime's startup writes only
(tens of bytes), and MNTSCRL's in-place `RIX3` decode.

Needs the emulator built at `$SR_DOSBOX` (default `../dosbox-automation`, beside the
repository) and
**no emulator already running** on port 8386. Memory dumps go to `out/release-images/mem/`.

usage: python3 -I tools/release_images.py [NAME ...] [--frames N]
"""
import json, os, secrets, subprocess, sys, tempfile, time, urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOSBOX = os.environ.get("SR_DOSBOX", os.path.join(os.path.dirname(REPO), "dosbox-automation"))
BIN = DOSBOX + "/build/release-linux/dosbox-automation"
DATA = REPO + "/assets/MAIN/DATA"
IMAGES = REPO + "/out/release-images"
BASE = "http://127.0.0.1:8386/api/v1"

args = sys.argv[1:]
frames = 250
if "--frames" in args:
    i = args.index("--frames"); frames = int(args[i + 1]); del args[i:i + 2]
names = args or sorted(n[:-4] for n in os.listdir(IMAGES) if n.endswith(".img"))


def dump(exe, outp):
    token = secrets.token_hex(32)

    def req(method, path, body=None, ctype="application/json"):
        data = None if body is None else (body if isinstance(body, bytes) else json.dumps(body).encode())
        r = urllib.request.Request(BASE + path, data=data, method=method)
        r.add_header("Authorization", "Bearer " + token)
        if data is not None:
            r.add_header("Content-Type", ctype)
        with urllib.request.urlopen(r, timeout=30) as resp:
            return resp.read()

    scratch = tempfile.mkdtemp(prefix="sr-relimg-")
    os.makedirs(scratch + "/xdg/dosbox-automation"); os.makedirs(scratch + "/run")
    with open(scratch + "/xdg/dosbox-automation/dosbox-automation.conf", "w") as f:
        f.write(f"[webserver]\nwebserver_enabled = true\nmount_allowed_bases = {DATA}\n"
                f"[autoexec]\nMOUNT C -RO \"{DATA}\"\nC:\n{exe}\n")
    deps = DOSBOX + "/build/release-linux/_deps"
    libs = ":".join(sorted(os.path.join(deps, d) for d in os.listdir(deps) if d.endswith("-build")))
    env = dict(os.environ, XDG_CONFIG_HOME=scratch + "/xdg", LD_LIBRARY_PATH=libs,
               SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy", DOSBOX_API_TOKEN=token)
    proc = subprocess.Popen([BIN, "--set", "cpu_cycles=30000"], env=env, cwd=scratch + "/run",
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                req("GET", "/dosbox/info"); break
            except Exception:
                time.sleep(0.1)
        req("POST", "/script/load?name=w", b"while true do\n  dosbox.wait_frames(60)\nend\n", "text/plain")
        req("POST", "/script/start", {})
        while json.loads(req("GET", "/script/status"))["frame"] < frames:
            time.sleep(0.1)
        mem = bytearray()
        for off in range(0, 0xA0000, 0x10000):
            mem += req("GET", f"/memory/{off}/{min(0x10000, 0xA0000 - off)}")
        with open(outp, "wb") as f:
            f.write(mem)
        return bytes(mem)
    finally:
        proc.terminate(); proc.wait(timeout=10); subprocess.run(["rm", "-rf", scratch])


def compare(name, mem):
    img = open(f"{IMAGES}/{name}.img", "rb").read()
    rel = [int(x) for x in open(f"{IMAGES}/{name}.rel").read().split()]
    bad = set()
    for r in rel:
        bad.update((r, r + 1))
    base = -1
    for k in range(0, len(img) - 64, 61):
        c = img[k:k + 64]
        if len(set(c)) < 12 or any(x in bad for x in range(k, k + 64)):
            continue
        i = mem.find(c)
        if i >= 0 and mem.find(c, i + 1) < 0 and (i - k) % 16 == 0:
            base = i - k; break
    if base < 0:
        return f"{name:<10} not found in memory"
    fix = bytearray(img)
    for r in rel:
        v = (fix[r] | fix[r + 1] << 8) + base // 16
        fix[r] = v & 255; fix[r + 1] = (v >> 8) & 255
    n = min(len(fix), len(mem) - base)
    diff = sum(1 for k in range(n) if fix[k] != mem[base + k])
    return f"{name:<10} load {base:06x} image {len(img):7d} compared {n:7d} differ {diff:6d}"


os.makedirs(IMAGES + "/mem", exist_ok=True)
for name in names:
    print(compare(name, dump(name + ".EXE", f"{IMAGES}/mem/{name}.bin")), flush=True)
