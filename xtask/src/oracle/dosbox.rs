//! `dosbox-automation` as the reference oracle, over its REST API.
//!
//! Every shape here was measured against the running emulator and is recorded
//! in `docs/decisions.md` D14, with literal response bytes committed under
//! `xtask/tests/fixtures/`. Two traps that cost hours during that probe and
//! are encoded below: the raw frame header is **15 bytes, not 16**, and the
//! pixels are `[blue, green, red, 0]`.

use std::collections::VecDeque;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;

use super::{sleep_frames, sleep_until_close, CapturedFrame, Oracle, OracleError, DEFAULT_WAIT};
use crate::reference::BootWait;

/// Where the emulator is mounted by default on this machine (D10).
const DEFAULT_SR_DOSBOX: &str = "/home/ew/workspace/reality/dosbox-automation";
/// Where the emulator's API listens unless its own config says otherwise; this
/// is the address `SR_DOSBOX` builds from a stock checkout.
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8386";
/// How long `spawn` waits for the API to answer before giving up.
const API_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the emulated DOS takes to reach its shell prompt on this machine.
const SHELL_TIMEOUT: Duration = Duration::from_secs(60);
/// Keys within one sequence are placed this far apart, in rendered frames
/// (200 ms at the dos rate): long enough for the guest to read each one.
const KEY_SPACING_FRAMES: u64 = 14;
/// How many frames before the requested one the exact-grab loop starts: enough to outlast a
/// host stall of tens of milliseconds. D17 was measured with it.
const GRAB_WINDOW_FRAMES: u64 = 25;
/// How long the exact-grab loop keeps trying before it refuses.
const GRAB_DEADLINE: Duration = Duration::from_secs(10);

const PORT_SEQUENCER_INDEX: u16 = 0x3C4;
const PORT_SEQUENCER_DATA: u16 = 0x3C5;
const PORT_GC_INDEX: u16 = 0x3CE;
const PORT_GC_DATA: u16 = 0x3CF;
const PORT_DAC_READ_INDEX: u16 = 0x3C7;
const PORT_DAC_DATA: u16 = 0x3C9;
const PORT_CRTC_INDEX: u16 = 0x3D4;
const PORT_CRTC_DATA: u16 = 0x3D5;
/// Graphics controller register 4, read map select (D14).
const GC_READ_MAP_SELECT: u16 = 4;
/// The CRTC's offset register: the number of bytes in a scanline divided by
/// the mode's address unit (see `super::row_stride_bytes`).
const CRTC_OFFSET: u16 = 0x13;
/// Sequencer register 4, memory mode: bit 3 set means chain-4, i.e. the
/// framebuffer is one linear image rather than four planes (D14).
const SEQUENCER_MEMORY_MODE: u16 = 4;
const VGA_SEGMENT: u16 = 0xA000;

const PIXEL_FORMAT_INDEXED8: u8 = 8;
const PIXEL_FORMAT_BGR24: u8 = 24;
const PIXEL_FORMAT_BGRX32: u8 = 32;

/// The 15-byte header `/api/v1/video/frame?format=raw` puts in front of its
/// pixels: `u32 width, u32 height, i32 pitch, u8 pixel_format, u16
/// palette_count`, little-endian (D14).
pub const RAW_HEADER_LEN: usize = 15;

/// `GET /api/v1/dosbox/info`.
#[derive(Debug, Clone, Deserialize)]
pub struct DosboxInfo {
    pub version: String,
    pub features: Features,
}

/// The capabilities capture depends on (D11): pinning cycles, injected input,
/// live memory, and the port API for the DAC and the VGA registers.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Features {
    #[serde(default)]
    pub cpu_control: bool,
    #[serde(default)]
    pub input: bool,
    #[serde(default)]
    pub memory: bool,
    #[serde(default)]
    pub port_io: bool,
}

impl DosboxInfo {
    pub fn from_json(raw: &str) -> Result<DosboxInfo, OracleError> {
        serde_json::from_str(raw)
            .map_err(|e| OracleError::Decode(format!("dosbox/info is not the documented shape: {e}")))
    }
}

/// The header of a raw frame, and where its pixels start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawFrameHeader {
    pub width: u32,
    pub height: u32,
    pub pitch: i32,
    pub pixel_format: u8,
    pub palette_count: u16,
    pub pixel_offset: usize,
}

impl RawFrameHeader {
    pub fn parse(raw: &[u8]) -> Result<RawFrameHeader, OracleError> {
        if raw.len() < RAW_HEADER_LEN {
            return Err(OracleError::Decode(format!(
                "a raw frame starts with a {RAW_HEADER_LEN}-byte header, got {} bytes",
                raw.len()
            )));
        }
        let u32_at = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        let palette_count = u16::from_le_bytes(raw[13..15].try_into().unwrap());
        let header = RawFrameHeader {
            width: u32_at(0),
            height: u32_at(4),
            pitch: i32::from_le_bytes(raw[8..12].try_into().unwrap()),
            pixel_format: raw[12],
            palette_count,
            pixel_offset: RAW_HEADER_LEN + palette_count as usize * 3,
        };
        if header.width == 0 || header.height == 0 || header.pitch == 0 {
            return Err(OracleError::Decode(format!(
                "raw frame header declares {}x{} pitch {}",
                header.width, header.height, header.pitch
            )));
        }
        if header.width > u16::MAX as u32 || header.height > u16::MAX as u32 {
            return Err(OracleError::Decode(format!(
                "raw frame header declares {}x{}, larger than a frame can be",
                header.width, header.height
            )));
        }
        Ok(header)
    }

    /// Bytes of pixel data the header promises.
    pub fn pixel_bytes(&self) -> usize {
        self.pitch.unsigned_abs() as usize * self.height as usize
    }
}

/// Decode a `format=raw` frame: strip the header, check the length the header
/// promises, and turn the pixels into top-down RGB triples. The oracle's other
/// formats are lossy (jpeg) or carry no indices, so capture always asks for
/// raw (D14).
pub fn decode_frame(raw: &[u8]) -> Result<CapturedFrame, OracleError> {
    let header = RawFrameHeader::parse(raw)?;
    let promised = header
        .pixel_offset
        .checked_add(header.pixel_bytes())
        .ok_or_else(|| OracleError::Decode("raw frame header overflows".into()))?;
    if promised != raw.len() {
        return Err(OracleError::Decode(format!(
            "raw frame header promises {promised} bytes, got {}",
            raw.len()
        )));
    }

    let width = header.width as usize;
    let height = header.height as usize;
    let stride = header.pitch.unsigned_abs() as usize;
    let bytes_per_pixel = match header.pixel_format {
        PIXEL_FORMAT_BGRX32 => 4,
        PIXEL_FORMAT_BGR24 => 3,
        PIXEL_FORMAT_INDEXED8 => {
            return Err(OracleError::Decode(
                "the frame is paletted (Indexed8), a shape /video/frame never returned in any \
                 mode the demo uses; its palette payload is unobserved, so it is refused rather \
                 than guessed at"
                    .into(),
            ))
        }
        other => {
            return Err(OracleError::Decode(format!(
                "unknown pixel format {other}"
            )))
        }
    };
    if stride < width * bytes_per_pixel {
        return Err(OracleError::Decode(format!(
            "raw frame pitch {stride} is less than one row of {width} pixels"
        )));
    }

    let pixels = &raw[header.pixel_offset..];
    let mut rgb = Vec::with_capacity(width * height * 3);
    for row in pixels.chunks_exact(stride).take(height) {
        for pixel in row[..width * bytes_per_pixel].chunks_exact(bytes_per_pixel) {
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
    }
    Ok(CapturedFrame {
        width: header.width as u16,
        height: header.height as u16,
        indexed: None,
        palette: None,
        rgb,
    })
}

/// Map a non-2xx answer to a legible error rather than a panic: 401 is the
/// token contract, 503 means there is no frame yet, anything else carries the
/// emulator's own message.
pub fn map_http_error(status: u16, body: &str) -> OracleError {
    match status {
        401 => OracleError::Unauthorized,
        503 => OracleError::NotBooted,
        _ => OracleError::Http {
            status,
            body: body.chars().take(400).collect(),
        },
    }
}

/// A 64 hex character token: the emulator rejects any other length and then
/// silently generates its own, which its log masks (D11).
pub fn generate_token() -> Result<String, OracleError> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| OracleError::Transport(format!("clock is before the epoch: {e}")))?
        .as_nanos() as u64
        ^ (std::process::id() as u64) << 32;
    // SplitMix64: enough to make four unpredictable-looking 16-character
    // groups. The token only has to keep other local processes out of an
    // emulator that listens on loopback.
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut token = String::with_capacity(64);
    for _ in 0..4 {
        token.push_str(&format!("{:016x}", next()));
    }
    Ok(token)
}

/// What the oracle asks the emulator for: HTTP in a capture, a script in the tests, so the
/// frame-identity logic is tested on the code that ships (D17).
trait EmulatorApi {
    fn get_bytes(&self, path: &str) -> Result<Vec<u8>, OracleError>;
    fn get_json(&self, path: &str) -> Result<serde_json::Value, OracleError>;
    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError>;
    fn put_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError>;
    /// `POST` a `text/plain` body, which is all `/script/load` accepts.
    fn post_text(&self, path: &str, body: &str) -> Result<Vec<u8>, OracleError>;
    /// Whether the API answers at all: any status, even an unauthenticated 401, means it is up.
    fn probe(&self) -> Result<(), OracleError>;
}

/// The emulator's HTTP API, with the token it was given.
struct HttpApi {
    agent: ureq::Agent,
    token: String,
    base_url: String,
}

impl HttpApi {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// A request to `path` carrying the token.
    fn authed(&self, method: &str, path: &str) -> ureq::Request {
        self.agent
            .request(method, &self.url(path))
            .set("Authorization", &format!("Bearer {}", self.token))
    }

    fn transport_error(&self, path: &str, error: ureq::Error) -> OracleError {
        match error {
            ureq::Error::Status(status, response) => {
                map_http_error(status, &response.into_string().unwrap_or_default())
            }
            ureq::Error::Transport(transport) => OracleError::Transport(format!(
                "cannot reach the emulator at {} for {path}: {transport}",
                self.base_url
            )),
        }
    }
}

impl EmulatorApi for HttpApi {
    fn get_bytes(&self, path: &str) -> Result<Vec<u8>, OracleError> {
        let response = self
            .authed("GET", path)
            .call()
            .map_err(|e| self.transport_error(path, e))?;
        read_body(response)
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, OracleError> {
        let body = self.get_bytes(path)?;
        let text = String::from_utf8(body)
            .map_err(|e| OracleError::Decode(format!("{path} did not answer UTF-8: {e}")))?;
        serde_json::from_str(&text)
            .map_err(|e| OracleError::Decode(format!("{path} is not the documented shape: {e}")))
    }

    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError> {
        let response = self
            .authed("POST", path)
            .send_json(body.clone())
            .map_err(|e| self.transport_error(path, e))?;
        read_body(response)
    }

    fn put_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError> {
        let response = self
            .authed("PUT", path)
            .send_json(body.clone())
            .map_err(|e| self.transport_error(path, e))?;
        read_body(response)
    }

    fn post_text(&self, path: &str, body: &str) -> Result<Vec<u8>, OracleError> {
        let response = self
            .authed("POST", path)
            .set("Content-Type", "text/plain")
            .send_string(body)
            .map_err(|e| self.transport_error(path, e))?;
        read_body(response)
    }

    fn probe(&self) -> Result<(), OracleError> {
        match self.agent.get(&self.url("/api/v1/dosbox/info")).call() {
            Ok(_) | Err(ureq::Error::Status(_, _)) => Ok(()),
            Err(ureq::Error::Transport(transport)) => Err(OracleError::Transport(format!(
                "cannot reach the emulator at {}: {transport}",
                self.base_url
            ))),
        }
    }
}

pub struct DosboxOracle {
    child: Option<Child>,
    /// The emulator in a capture, a script in the tests.
    api: Box<dyn EmulatorApi + Send>,
    scratch: PathBuf,
    /// The guest runs its own boot out of `[autoexec]` (D17), so `boot` only waits for
    /// `boot_wait`.
    autoexec: bool,
    /// [`BootWait::Setup`] until [`Oracle::set_boot_wait`] says otherwise.
    boot_wait: BootWait,
    /// The frames [`Oracle::wait_for_frame`] has taken and not yet handed over, oldest first.
    pending: VecDeque<CapturedFrame>,
}

impl DosboxOracle {
    /// Launch the emulator headless and wait for its API.
    ///
    /// `allowed_base`, the directory the capture mounts, has to be in the primary config's
    /// allow-list, which `--set` cannot fill (D14), so `XDG_CONFIG_HOME` points at a scratch
    /// primary config. `autoexec` are the lines that config gives the guest's shell (D17);
    /// empty means the typed recipe.
    pub fn spawn(
        cycles: u32,
        allowed_base: &Path,
        autoexec: &[String],
    ) -> Result<DosboxOracle, OracleError> {
        // The guest resolves a relative host path against the emulator's working directory.
        let allowed_base = absolute(allowed_base)?;
        let repo = env::var("SR_DOSBOX").unwrap_or_else(|_| DEFAULT_SR_DOSBOX.to_string());
        let binary = Path::new(&repo).join("build/release-linux/dosbox-automation");
        if !binary.is_file() {
            return Err(OracleError::Transport(format!(
                "no emulator at {}; set SR_DOSBOX to the dosbox-automation checkout",
                binary.display()
            )));
        }
        let libraries = dependency_search_path(Path::new(&repo))?;

        let base_url = DEFAULT_BASE_URL.to_string();
        if let Some(occupant) = listener_on(&base_url) {
            return Err(OracleError::Transport(format!(
                "{base_url} is already answering ({occupant}); shut the other emulator down first, \
                 because a capture must not talk to an emulator it did not start"
            )));
        }

        let scratch = env::temp_dir().join(format!("secondreality-xtask-{}", std::process::id()));
        let config_dir = scratch.join("xdg/dosbox-automation");
        std::fs::create_dir_all(&config_dir)
            .and_then(|_| std::fs::create_dir_all(scratch.join("run")))
            .map_err(|e| OracleError::Transport(format!("cannot prepare {}: {e}", scratch.display())))?;
        let config_path = config_dir.join("dosbox-automation.conf");
        std::fs::write(&config_path, primary_config(&allowed_base, autoexec)).map_err(|e| {
            OracleError::Transport(format!("cannot write {}: {e}", config_path.display()))
        })?;

        let token = generate_token()?;
        let log_path = scratch.join("emulator.log");
        let log = File::create(&log_path)
            .map_err(|e| OracleError::Transport(format!("cannot write {}: {e}", log_path.display())))?;
        let child = emulator_command(&binary, &scratch, cycles, &libraries, &token)
            .stdout(Stdio::from(log.try_clone().map_err(|e| {
                OracleError::Transport(format!("cannot redirect the emulator log: {e}"))
            })?))
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| OracleError::Transport(format!("cannot start {}: {e}", binary.display())))?;

        let mut oracle = DosboxOracle {
            child: Some(child),
            api: Box::new(HttpApi {
                agent: ureq::AgentBuilder::new()
                    .timeout(Duration::from_secs(15))
                    .build(),
                token,
                base_url,
            }),
            scratch,
            autoexec: !autoexec.is_empty(),
            boot_wait: BootWait::Setup,
            pending: VecDeque::new(),
        };
        match oracle.wait_for_api(&log_path) {
            Ok(()) => {}
            Err(e) => {
                oracle.kill();
                return Err(e);
            }
        }
        match oracle.check_token().and_then(|_| oracle.start_frame_watcher()) {
            Ok(()) => Ok(oracle),
            Err(e) => {
                oracle.kill();
                Err(e)
            }
        }
    }

    /// Fail with a legible 401 if the emulator did not take the token.
    fn check_token(&self) -> Result<(), OracleError> {
        self.api.get_bytes("/api/v1/dosbox/info").map(|_| ())
    }

    fn wait_for_api(&mut self, log_path: &Path) -> Result<(), OracleError> {
        let deadline = std::time::Instant::now() + API_STARTUP_TIMEOUT;
        loop {
            if let Some(child) = self.child.as_mut() {
                if let Some(status) = child.try_wait().map_err(|e| {
                    OracleError::Transport(format!("cannot watch the emulator process: {e}"))
                })? {
                    return Err(OracleError::Transport(format!(
                        "the emulator exited with {status} before its API came up.\n{}",
                        log_tail(log_path)
                    )));
                }
            }
            if self.api.probe().is_ok() {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(OracleError::Transport(format!(
                    "the emulator did not answer on {DEFAULT_BASE_URL} within {}s.\n{}",
                    API_STARTUP_TIMEOUT.as_secs(),
                    log_tail(log_path)
                )));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// A resident script is the only way to read the frame counter: it is not
    /// on `/video/frame` or `/status` (D14).
    fn start_frame_watcher(&self) -> Result<(), OracleError> {
        const WATCHER: &str = "while true do\n  dosbox.wait_frames(60)\nend\n";
        // /script/load only accepts text/plain or application/lua, and is
        // rate limited to one call per two seconds (D14).
        let status = self
            .api
            .post_text("/api/v1/script/load?name=xtask-frame-watch", WATCHER)?;
        if !String::from_utf8_lossy(&status).contains("loaded") {
            return Err(OracleError::Decode(format!(
                "the emulator did not load the frame-watching script: {}",
                String::from_utf8_lossy(&status)
            )));
        }
        self.api.post_json("/api/v1/script/start", &json!({}))?;
        Ok(())
    }

    fn type_line(&self, line: &str) -> Result<(), OracleError> {
        self.api.post_json(
            "/api/v1/input/type",
            &json!({ "text": format!("{line}\n"), "cps": 250 }),
        )?;
        Ok(())
    }

    fn port_write(&self, port: u16, value: u8) -> Result<(), OracleError> {
        let answer = self
            .api
            .put_json("/api/v1/io/port", &json!({ "port": port, "value": value }))?;
        let answer: serde_json::Value = serde_json::from_slice(&answer)
            .map_err(|e| OracleError::Decode(format!("io/port write is not JSON: {e}")))?;
        if answer["status"].as_str() != Some("ok") {
            return Err(OracleError::Decode(format!(
                "writing port {port:x} was not acknowledged: {answer}"
            )));
        }
        Ok(())
    }

    fn port_read(&self, port: u16) -> Result<u16, OracleError> {
        let answer = self.api.get_json(&format!("/api/v1/io/port?port={port}"))?;
        answer["value"]
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .ok_or_else(|| {
                OracleError::Decode(format!("reading port {port:x} returned no value: {answer}"))
            })
    }

    fn chain4_enabled(&self) -> Result<bool, OracleError> {
        self.port_write(PORT_SEQUENCER_INDEX, SEQUENCER_MEMORY_MODE as u8)?;
        Ok(self.port_read(PORT_SEQUENCER_DATA)? & 0x08 != 0)
    }

    /// The DAC as the guest holds it: 256 entries of 6-bit components, read
    /// through 0x3C7/0x3C9. Each triple needs its own select, because every
    /// read advances the DAC's internal index (D14).
    fn read_palette(&self) -> Result<[[u8; 3]; 256], OracleError> {
        let mut palette = [[0u8; 3]; 256];
        for (entry, components) in palette.iter_mut().enumerate() {
            self.port_write(PORT_DAC_READ_INDEX, entry as u8)?;
            for component in components.iter_mut() {
                let value = self.port_read(PORT_DAC_DATA)?;
                *component = u8::try_from(value).ok().filter(|v| *v <= 0x3F).ok_or_else(
                    || {
                        OracleError::Decode(format!(
                            "DAC entry {entry} answered {value}, which is not a six-bit component"
                        ))
                    },
                )?;
            }
        }
        Ok(palette)
    }

    /// The CRTC's offset register (3d4h index 13h) as the port answers it.
    fn crtc_offset_register(&self) -> Result<u16, OracleError> {
        self.port_write(PORT_CRTC_INDEX, CRTC_OFFSET as u8)?;
        self.port_read(PORT_CRTC_DATA)
    }

    /// What kind of screen is up, from the mode's colour depth and the sequencer's chain-4 bit.
    fn screen_layout(&self, color_depth: &str) -> Result<super::ScreenLayout, OracleError> {
        Ok(super::ScreenLayout::of(
            color_depth,
            self.chain4_enabled()?,
        ))
    }

    /// Indices for the frame on screen now: a linear read in chain-4, otherwise one read per
    /// plane with the read map select set (D14), assembled at the screen's own row stride. The
    /// read starts at each plane's offset 0, so a screen whose rows run past one plane (ALKU's,
    /// 176 × 400 = 70,400 bytes) is refused by [`super::plan_indexed_read`] and stays RGB-only.
    fn capture_indexed(
        &self,
        width: u16,
        height: u16,
        color_depth: &str,
    ) -> Result<(Vec<u8>, [[u8; 3]; 256]), OracleError> {
        let layout = self.screen_layout(color_depth)?;
        let plan = super::plan_indexed_read(
            width,
            height,
            self.crtc_offset_register()?,
            layout,
        )
        .map_err(OracleError::ScreenUnreadable)?;
        let stride = plan.stride;
        let indexed = if layout == super::ScreenLayout::Chained256 {
            let mut memory = self.read_memory(VGA_SEGMENT, 0, plan.plane_bytes)?;
            if stride != width as usize {
                // The rows are not packed at the screen's width: gather them.
                let mut gathered = vec![0u8; width as usize * height as usize];
                for y in 0..height as usize {
                    let from = y * stride;
                    let to = y * width as usize;
                    gathered[to..to + width as usize]
                        .copy_from_slice(&memory[from..from + width as usize]);
                }
                memory = gathered;
            }
            memory
        } else {
            let mut planes = Vec::with_capacity(4);
            for plane in 0..4u8 {
                self.port_write(PORT_GC_INDEX, GC_READ_MAP_SELECT as u8)?;
                self.port_write(PORT_GC_DATA, plane)?;
                planes.push(self.read_memory(VGA_SEGMENT, 0, plan.plane_bytes)?);
            }
            super::assemble_planes(&planes, width, height, stride)?
        };
        if indexed.len() != width as usize * height as usize {
            return Err(OracleError::Decode(format!(
                "the framebuffer answered {} bytes for a {width}x{height} screen",
                indexed.len()
            )));
        }
        let palette = self.read_palette()?;
        Ok((indexed, palette))
    }

    /// The guest's screen as raw bytes, with the counter reading that identifies it, or `None`
    /// when a frame boundary fell inside the grab (D17).
    ///
    /// The emulator publishes a frame's pixels and then counts it, on one thread
    /// (`gui/render/render.cpp:446`, then `gui/sdl_gui.cpp:1205`), so a reading that is the same
    /// on both sides of the grab names the picture. A video-mode stop advances the counter
    /// without publishing (`GFX_Stop`, `gui/sdl_gui.cpp:396-402`; `halt_render`,
    /// `gui/render/render.cpp:349-353`); a stop that changes geometry is refused as
    /// `GeometryChanged`. The bytes stay undecoded: decoding inside the loop would slow it to
    /// one grab per two or three frames.
    fn grab_identified(&mut self) -> Result<Option<(u64, Vec<u8>)>, OracleError> {
        let before = self.frame_counter()?;
        let raw = self.api.get_bytes("/api/v1/video/frame?format=raw")?;
        let after = self.frame_counter()?;
        Ok((before == after).then_some((before, raw)))
    }

    /// Take the frame the counter names `index`, or refuse.
    fn grab_at(&mut self, index: u64) -> Result<CapturedFrame, OracleError> {
        let deadline = std::time::Instant::now() + GRAB_DEADLINE;
        let raw = grab_exactly(index, deadline, || self.grab_identified())?;
        decode_frame(&raw)
    }

    /// Take the next frame that can be identified, whatever the counter calls it.
    fn grab_any(&mut self) -> Result<CapturedFrame, OracleError> {
        let deadline = std::time::Instant::now() + GRAB_DEADLINE;
        loop {
            if let Some((_, raw)) = self.grab_identified()? {
                return decode_frame(&raw);
            }
            if std::time::Instant::now() >= deadline {
                return Err(grab_timeout(None));
            }
        }
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Wait for the frame the counter names exactly. `grab` returns the identifying reading and
/// the frame, or `None` for a grab that straddled a boundary; the loop ends on the frame asked
/// for, on a late one (refused, never relabelled), or on the deadline (D17).
fn grab_exactly<T>(
    index: u64,
    deadline: std::time::Instant,
    mut grab: impl FnMut() -> Result<Option<(u64, T)>, OracleError>,
) -> Result<T, OracleError> {
    loop {
        if let Some((at, frame)) = grab()? {
            match at.cmp(&index) {
                std::cmp::Ordering::Equal => return Ok(frame),
                std::cmp::Ordering::Greater => {
                    return Err(OracleError::FrameMissed {
                        index,
                        observed: at,
                    })
                }
                std::cmp::Ordering::Less => {}
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(grab_timeout(Some(index)));
        }
    }
}

fn grab_timeout(index: Option<u64>) -> OracleError {
    let what = match index {
        Some(index) => format!("frame {index}"),
        None => "a frame".to_owned(),
    };
    OracleError::Transport(format!(
        "the emulator never held {what} still enough to be taken exactly (no frame the counter \
         named on both sides of the grab within {}s); the host is not keeping up with it",
        GRAB_DEADLINE.as_secs()
    ))
}

impl Oracle for DosboxOracle {
    fn describe(&self) -> Result<String, OracleError> {
        let info = DosboxInfo::from_json(
            &String::from_utf8(self.api.get_bytes("/api/v1/dosbox/info")?)
                .map_err(|e| OracleError::Decode(format!("dosbox/info is not UTF-8: {e}")))?,
        )?;
        let features = &info.features;
        if !(features.cpu_control && features.input && features.memory && features.port_io) {
            return Err(OracleError::Decode(format!(
                "this emulator cannot be a capture oracle: it lacks the features capture needs \
                 (cpu_control={}, input={}, memory={}, port_io={})",
                features.cpu_control, features.input, features.memory, features.port_io
            )));
        }
        Ok(format!("dosbox-automation {}", info.version))
    }

    fn boot(&mut self, exe: &Path, cwd: &Path, args: &[String]) -> Result<(), OracleError> {
        if self.autoexec {
            // The guest's shell runs the recipe, `AUTOTYPE` included; this waits for the anchor.
            // The setup screen also catches a mount the policy refused (D17).
            return match self.boot_wait {
                BootWait::Setup => {
                    self.wait_for_text(crate::reference::SETUP_SCREEN_TEXT, SHELL_TIMEOUT)
                }
                BootWait::Graphics => self.wait_for_graphics(SHELL_TIMEOUT),
            };
        }
        let cwd = absolute(cwd)?;
        // A refused mount shows up as screen text, not as an HTTP error.
        self.wait_for_text(":\\>", SHELL_TIMEOUT)?;
        self.type_line(&format!("MOUNT C -RO {}", cwd.display()))?;
        self.wait_for_text("Mounted read-only", DEFAULT_WAIT)?;
        self.type_line("C:")?;
        self.wait_for_text("C:\\>", DEFAULT_WAIT)?;

        let line = super::guest_command_line(exe, args).ok_or_else(|| {
            OracleError::Transport(format!("{} has no file name to run", exe.display()))
        })?;
        self.type_line(&line)
    }

    fn set_cycles(&mut self, cycles: u32) -> Result<(), OracleError> {
        if cycles == 0 {
            return Err(OracleError::UnpinnedCycles);
        }
        let answer = self
            .api
            .put_json("/api/v1/cpu/cycles", &json!({ "cycles": cycles }))?;
        let answer: serde_json::Value = serde_json::from_slice(&answer)
            .map_err(|e| OracleError::Decode(format!("cpu/cycles is not JSON: {e}")))?;
        // The PUT sets both real and protected mode, and reports what it took.
        if answer["status"].as_str() != Some("ok") || answer["cycles"].as_u64() != Some(cycles as u64)
        {
            return Err(OracleError::Decode(format!(
                "the emulator did not pin {cycles} cycles/ms: {answer}"
            )));
        }
        Ok(())
    }

    fn set_boot_wait(&mut self, wait: BootWait) -> Result<(), OracleError> {
        self.boot_wait = wait;
        Ok(())
    }

    fn step_frames(&mut self, frames: u32) -> Result<(), OracleError> {
        if frames == 0 {
            return Ok(());
        }
        let target = self.frame_counter()? + frames as u64;
        loop {
            let now = self.frame_counter()?;
            if now >= target {
                return Ok(());
            }
            sleep_until_close(target - now);
        }
    }

    fn read_memory(&self, segment: u16, offset: u32, len: usize) -> Result<Vec<u8>, OracleError> {
        self.api
            .get_bytes(&format!("/api/v1/memory/{segment}/{offset}/{len}"))
    }

    fn wait_for_frame(&mut self, index: u64) -> Result<(), OracleError> {
        // The grab loop below polls; it must not do that for the length of a skip.
        loop {
            let now = self.frame_counter()?;
            if now + GRAB_WINDOW_FRAMES >= index {
                break;
            }
            sleep_frames(index - now, GRAB_WINDOW_FRAMES);
        }
        let frame = self.grab_at(index)?;
        self.pending.push_back(frame);
        Ok(())
    }

    fn frame(&mut self) -> Result<CapturedFrame, OracleError> {
        // The frame the wait pinned (D17). The indexed read below takes many frames, in Mode X
        // some eight hundred requests, so it can only be checked against the picture, not the
        // counter.
        let mut frame = match self.pending.pop_front() {
            Some(frame) => frame,
            None => self.grab_any()?,
        };
        let info = self.api.get_json("/api/v1/video/frame/info")?;
        if info["video_mode"]["is_graphics_mode"].as_bool() != Some(true) {
            // A text screen has no palette indices, only glyph codes.
            return Ok(frame);
        }
        // A depth the emulator did not name is refused, not assumed 256-colour.
        let color_depth = info["video_mode"]["color_depth"]
            .as_str()
            .unwrap_or_default();
        match self.capture_indexed(frame.width, frame.height, color_depth) {
            Ok((indexed, palette)) => {
                frame.indexed = Some(indexed);
                frame.palette = Some(palette);
                if !frame.indexed_matches_rgb() {
                    eprintln!(
                        "warning: indexed capture does not reproduce the RGB frame \
                         (the screen moved while it was read); keeping RGB only"
                    );
                    frame.indexed = None;
                    frame.palette = None;
                }
            }
            Err(e) => {
                eprintln!("warning: indexed capture failed ({e}); keeping RGB only");
            }
        }
        Ok(frame)
    }

    fn send_keys(&mut self, script: &str) -> Result<(), OracleError> {
        let names: Vec<&str> = script.split_whitespace().collect();
        if names.is_empty() {
            return Ok(());
        }
        // Frame-scheduled: the events land on rendered frames relative to the call.
        let events: Vec<serde_json::Value> = names
            .iter()
            .enumerate()
            .map(|(i, name)| json!({ "key": name, "frame": 1 + i as u64 * KEY_SPACING_FRAMES }))
            .collect();
        self.api
            .post_json("/api/v1/input/sequence", &json!({ "events": events }))?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), OracleError> {
        if let Some(mut child) = self.child.take() {
            let _ = self.api.post_json(
                "/api/v1/dosbox/shutdown",
                &serde_json::json!({}),
            );
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while std::time::Instant::now() < deadline {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                    Err(_) => break,
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.scratch);
        Ok(())
    }

    fn wait_for_text(&mut self, needle: &str, timeout: Duration) -> Result<(), OracleError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let screen = self.api.get_json("/api/v1/video/text")?;
            if screen["text"]
                .as_str()
                .is_some_and(|text| text.contains(needle))
            {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(OracleError::Timeout(format!(
                    "waited {}s for {needle:?}; the bottom of the screen says: {}",
                    timeout.as_secs(),
                    screen_tail(&screen)
                )));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn wait_for_graphics(&mut self, timeout: Duration) -> Result<(), OracleError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let info = self.api.get_json("/api/v1/video/frame/info")?;
            if info["video_mode"]["is_graphics_mode"].as_bool() == Some(true) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                let screen = self.api.get_json("/api/v1/video/text")?;
                return Err(OracleError::Timeout(format!(
                    "the screen is still text after {}s; the bottom of it says: {}",
                    timeout.as_secs(),
                    screen_tail(&screen)
                )));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn frame_counter(&mut self) -> Result<u64, OracleError> {
        let status = self.api.get_json("/api/v1/script/status")?;
        if status["state"].as_str() == Some("error") {
            return Err(OracleError::Decode(format!(
                "the frame-watching script failed: {}",
                status["error"]
            )));
        }
        status["frame"].as_u64().ok_or_else(|| {
            OracleError::Decode(format!("script/status reports no frame: {status}"))
        })
    }
}

/// The last four non-blank lines of a `/video/text` answer, where a command and its answer
/// are (a refused mount says why there, not in the banner).
fn screen_tail(screen: &serde_json::Value) -> String {
    let lines: Vec<&str> = screen["text"]
        .as_str()
        .unwrap_or("<not a text screen>")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines[lines.len().saturating_sub(4)..].join(" / ")
}

impl Drop for DosboxOracle {
    fn drop(&mut self) {
        // Never leave the emulator running, not even when a capture failed.
        let _ = self.shutdown();
    }
}

/// The emulator process, as capture starts it: headless, silent, on the pinned cycles, with
/// the token its API answers to.
///
/// A real audio device paces the mixer by the host's clock and the demo then drifts against
/// the frame counter between runs, so a silent run is what makes a moving screen repeat (D17).
/// The silence comes from `SDL_AUDIODRIVER=dummy`: the emulator's setting is `[mixer] nosound`
/// (`src/audio/mixer.cpp`), so the `mixer_nosound=true` passed here does not take effect.
/// Correcting it needs D17's determinism re-measured first (CLAUDE.md's open items).
fn emulator_command(
    binary: &Path,
    scratch: &Path,
    cycles: u32,
    libraries: &str,
    token: &str,
) -> Command {
    let mut command = Command::new(binary);
    command
        // `cpu_cycles`, not `cycles`: `cycles=` forces legacy mode, where the PUT is a 409 (D14).
        .arg("--set")
        .arg(format!("cpu_cycles={cycles}"))
        .arg("--set")
        .arg("mixer_nosound=true")
        .env("XDG_CONFIG_HOME", scratch.join("xdg"))
        .env("LD_LIBRARY_PATH", libraries)
        .env("SDL_VIDEODRIVER", "dummy")
        .env("SDL_AUDIODRIVER", "dummy")
        .env("DOSBOX_API_TOKEN", token)
        .current_dir(scratch.join("run"))
        .stdin(Stdio::null());
    command
}

/// The scratch primary config: the mount allow-list, only ever read from this file (D14), and
/// under the autoexec recipe the `[autoexec]` section the shell runs (D17). Both name the same
/// directory.
fn primary_config(allowed_base: &Path, autoexec: &[String]) -> String {
    let mut config = format!(
        "# Written by xtask reference capture; deleted when the capture ends.\n\
         [webserver]\n\
         webserver_enabled = true\n\
         mount_allowed_bases = {}\n",
        allowed_base.display()
    );
    if !autoexec.is_empty() {
        config.push_str("[autoexec]\n");
        for line in autoexec {
            config.push_str(line);
            config.push('\n');
        }
    }
    config
}

/// A path the emulator and this process agree on: absolute, symlinks resolved.
fn absolute(path: &Path) -> Result<PathBuf, OracleError> {
    path.canonicalize()
        .map_err(|e| OracleError::Transport(format!("cannot resolve {}: {e}", path.display())))
}

fn read_body(response: ureq::Response) -> Result<Vec<u8>, OracleError> {
    let mut body = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| OracleError::Transport(format!("reading the response body failed: {e}")))?;
    Ok(body)
}

/// The colon-joined `_deps/*-build` directories the emulator binary needs on
/// `LD_LIBRARY_PATH`: it has no rpath of its own (D10).
fn dependency_search_path(repo: &Path) -> Result<String, OracleError> {
    let deps = repo.join("build/release-linux/_deps");
    let mut paths = Vec::new();
    let entries = std::fs::read_dir(&deps).map_err(|e| {
        OracleError::Transport(format!(
            "cannot read {} for the emulator's libraries: {e}",
            deps.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| OracleError::Transport(format!("{e}")))?;
        let path = entry.path();
        if path.is_dir() && path.to_string_lossy().ends_with("-build") {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(OracleError::Transport(format!(
            "no built dependencies under {}; the emulator cannot start without them",
            deps.display()
        )));
    }
    Ok(paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(":"))
}

/// Something already answering on the emulator's port, which would be captured instead of the
/// emulator this tool starts.
fn listener_on(base_url: &str) -> Option<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(500))
        .build();
    let answer = agent.get(&format!("{base_url}/api/v1/dosbox/info")).call();
    match answer {
        Ok(response) => Some(format!("HTTP {}", response.status())),
        Err(ureq::Error::Status(status, _)) => Some(format!("HTTP {status}")),
        Err(ureq::Error::Transport(_)) => None,
    }
}

fn log_tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let tail: Vec<&str> = text.lines().rev().take(5).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    format!("emulator log ({}):\n{}", path.display(), tail.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use crate::oracle::fixtures::{DAC_BIN, FRAME_BIN};

    const INFO_JSON: &str = include_str!("../../tests/fixtures/info.json");
    const DAC_PORT_JSON: &str = include_str!("../../tests/fixtures/dac_port.json");

    #[test]
    fn parses_an_info_response() {
        let info = DosboxInfo::from_json(INFO_JSON).unwrap();
        assert_eq!(info.version, "0.85.1 (6d100)");
        assert!(info.features.cpu_control);
        assert!(info.features.memory);
        assert!(info.features.port_io);
        assert!(info.features.input);
    }

    #[test]
    fn a_frame_response_yields_rgb_of_the_declared_geometry() {
        let raw = FRAME_BIN;
        let f = decode_frame(raw).unwrap();
        assert_eq!((f.width, f.height), (320, 200));
        assert_eq!(f.rgb.len(), 320 * 200 * 3);
        assert!(f.indexed.is_none());

        let header = RawFrameHeader::parse(raw).unwrap();
        assert_eq!(header.pitch, 1280);
        assert_eq!(header.pixel_format, PIXEL_FORMAT_BGRX32);
        assert_eq!(header.palette_count, 0);
        // 15 bytes of header, not 16.
        assert_eq!(raw.len(), RAW_HEADER_LEN + 200 * 1280);
    }

    #[test]
    fn a_truncated_or_lying_frame_is_refused_rather_than_decoded() {
        assert!(matches!(
            decode_frame(&FRAME_BIN[..RAW_HEADER_LEN - 1]),
            Err(OracleError::Decode(_))
        ));
        assert!(matches!(
            decode_frame(&FRAME_BIN[..FRAME_BIN.len() - 1]),
            Err(OracleError::Decode(_))
        ));
        assert!(matches!(
            decode_frame(&[]),
            Err(OracleError::Decode(_))
        ));
    }

    #[test]
    fn an_error_response_is_reported_not_panicked_on() {
        assert!(matches!(
            map_http_error(401, r#"{"error":"unauthorized"}"#),
            OracleError::Unauthorized
        ));
        assert!(matches!(
            map_http_error(500, "boom"),
            OracleError::Http { status: 500, .. }
        ));
        assert!(matches!(map_http_error(409, "x"), OracleError::Http { status: 409, .. }));
        assert!(matches!(map_http_error(503, "{}"), OracleError::NotBooted));
    }

    #[test]
    fn the_emulator_is_started_headless_silent_and_on_pinned_cycles() {
        // The recipe D17 was measured with.
        let command = emulator_command(
            Path::new("/emulator/dosbox-automation"),
            Path::new("/scratch"),
            30000,
            "/deps/sdl3-build",
            "token",
        );
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec!["--set", "cpu_cycles=30000", "--set", "mixer_nosound=true"]
        );
        let env = |name: &str| {
            command
                .get_envs()
                .find(|(key, _)| *key == name)
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
        };
        assert_eq!(env("SDL_VIDEODRIVER").as_deref(), Some("dummy"));
        assert_eq!(env("SDL_AUDIODRIVER").as_deref(), Some("dummy"));
        assert_eq!(env("LD_LIBRARY_PATH").as_deref(), Some("/deps/sdl3-build"));
        assert_eq!(env("DOSBOX_API_TOKEN").as_deref(), Some("token"));
        assert_eq!(
            command.get_current_dir().map(Path::to_path_buf),
            Some(PathBuf::from("/scratch/run"))
        );
    }

    /// A scripted emulator, driven through the oracle's own calls. Counters and pictures are
    /// separate queues, so a grab can straddle a boundary as the real one does (D17).
    struct ScriptedEmulator {
        counters: RefCell<VecDeque<u64>>,
        pictures: RefCell<VecDeque<u64>>,
    }

    impl ScriptedEmulator {
        fn new(counters: Vec<u64>, pictures: Vec<u64>) -> ScriptedEmulator {
            ScriptedEmulator {
                counters: RefCell::new(counters.into()),
                pictures: RefCell::new(pictures.into()),
            }
        }

        /// A raw frame whose first pixel's red byte is `picture` (`frame.rgb[0]` reads it).
        fn raw_frame(picture: u64) -> Vec<u8> {
            let mut raw = Vec::with_capacity(RAW_HEADER_LEN + 200 * 1280);
            raw.extend_from_slice(&320u32.to_le_bytes());
            raw.extend_from_slice(&200u32.to_le_bytes());
            raw.extend_from_slice(&1280i32.to_le_bytes());
            raw.push(PIXEL_FORMAT_BGRX32);
            raw.extend_from_slice(&0u16.to_le_bytes());
            raw.extend_from_slice(&[0, 0, picture as u8]); // BGRX: red last
            raw.resize(RAW_HEADER_LEN + 200 * 1280, 0);
            raw
        }
    }

    impl EmulatorApi for ScriptedEmulator {
        fn get_bytes(&self, path: &str) -> Result<Vec<u8>, OracleError> {
            if path.starts_with("/api/v1/video/frame") {
                let picture = self
                    .pictures
                    .borrow_mut()
                    .pop_front()
                    .ok_or_else(|| OracleError::Decode("the script ran out of pictures".into()))?;
                return Ok(Self::raw_frame(picture));
            }
            Err(OracleError::Transport(format!("the script does not answer {path}")))
        }

        fn get_json(&self, path: &str) -> Result<serde_json::Value, OracleError> {
            match path {
                "/api/v1/script/status" => {
                    let frame = self
                        .counters
                        .borrow_mut()
                        .pop_front()
                        .ok_or_else(|| OracleError::Decode("the script ran out of counters".into()))?;
                    Ok(json!({ "frame": frame, "state": "yielded" }))
                }
                // A text screen, so `frame()` skips the indexed read.
                "/api/v1/video/frame/info" => {
                    Ok(json!({ "video_mode": { "is_graphics_mode": false } }))
                }
                "/api/v1/video/text" => Ok(json!({ "text": "" })),
                _ => Err(OracleError::Transport(format!("the script does not answer {path}"))),
            }
        }

        fn post_json(&self, path: &str, _body: &serde_json::Value) -> Result<Vec<u8>, OracleError> {
            Err(OracleError::Transport(format!("the script does not answer {path}")))
        }

        fn put_json(&self, path: &str, _body: &serde_json::Value) -> Result<Vec<u8>, OracleError> {
            Err(OracleError::Transport(format!("the script does not answer {path}")))
        }

        fn post_text(&self, path: &str, _body: &str) -> Result<Vec<u8>, OracleError> {
            Err(OracleError::Transport(format!("the script does not answer {path}")))
        }

        fn probe(&self) -> Result<(), OracleError> {
            Ok(())
        }
    }

    /// An oracle talking to a scripted emulator, with no process behind it.
    fn scripted_oracle(script: ScriptedEmulator) -> DosboxOracle {
        DosboxOracle {
            child: None,
            api: Box::new(script),
            scratch: std::env::temp_dir().join("xtask-scripted-oracle"),
            autoexec: false,
            boot_wait: BootWait::Setup,
            pending: VecDeque::new(),
        }
    }

    #[test]
    fn a_grab_that_straddles_a_frame_boundary_is_refused_not_written() {
        // A clean grab of 9, then counter 10 with picture 11 (a straddle), then a clean 11:
        // the wait refuses, naming both, rather than file 11 under 10.
        let script = ScriptedEmulator::new(vec![9, 9, 9, 10, 11, 11, 11], vec![9, 11, 11]);
        let mut oracle = scripted_oracle(script);
        let error = oracle.wait_for_frame(10).unwrap_err();
        assert!(matches!(
            error,
            OracleError::FrameMissed {
                index: 10,
                observed: 11
            }
        ));
    }

    #[test]
    fn the_frame_written_under_an_index_is_the_one_the_wait_pinned() {
        // The wait keeps picture 10; a `frame` that grabbed for itself would return 11.
        let script = ScriptedEmulator::new(vec![9, 9, 9, 10, 10, 11], vec![9, 10, 11]);
        let mut oracle = scripted_oracle(script);
        oracle.wait_for_frame(10).unwrap();
        let frame = oracle.frame().unwrap();
        assert_eq!(frame.rgb[0], 10, "the frame for index ten, not a later grab");
    }

    #[test]
    fn a_sleep_never_runs_longer_than_a_second() {
        // An hour of frames sleeps for at most a second a call (D17).
        let start = std::time::Instant::now();
        sleep_frames(60 * 60 * 70, 0);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the sleep was not capped: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_frame_is_taken_only_when_the_counter_names_it() {
        // Straddled grabs (None) and early frames are passed over (D17).
        let script = vec![
            None,
            Some((6, "six")),
            None,
            Some((7, "seven")),
            Some((8, "eight")),
        ];
        let mut script = script.into_iter();
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        let frame = grab_exactly(8, deadline, || Ok(script.next().unwrap_or(None))).unwrap();
        assert_eq!(frame, "eight");
    }

    #[test]
    fn a_frame_that_arrives_late_is_refused_rather_than_relabelled() {
        let mut script = vec![Some((7, "seven")), Some((9, "nine"))].into_iter();
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        let error = grab_exactly(8, deadline, || Ok(script.next().unwrap_or(None))).unwrap_err();
        assert!(matches!(
            error,
            OracleError::FrameMissed {
                index: 8,
                observed: 9
            }
        ));
    }

    #[test]
    fn a_grab_that_never_identifies_its_frame_gives_up_loudly() {
        let deadline = std::time::Instant::now();
        let error = grab_exactly(8, deadline, || Ok(None::<(u64, &str)>)).unwrap_err();
        assert!(
            matches!(error, OracleError::Transport(ref message) if message.contains("frame 8")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn the_scratch_config_carries_the_guest_recipe_only_when_there_is_one() {
        // The typed recipe's config is the allow-list alone.
        let typed = primary_config(Path::new("/reference/original"), &[]);
        assert!(typed.contains("mount_allowed_bases = /reference/original"));
        assert!(!typed.contains("[autoexec]"));

        // The autoexec recipe's lines follow, in order.
        let lines = vec![
            "MOUNT C -RO /reference/original".to_owned(),
            "C:".to_owned(),
            "AUTOTYPE -w 5 left enter".to_owned(),
            "SECOND.EXE 2".to_owned(),
        ];
        let autoexec = primary_config(Path::new("/reference/original"), &lines);
        assert!(autoexec.ends_with(
            "[autoexec]\n\
             MOUNT C -RO /reference/original\n\
             C:\n\
             AUTOTYPE -w 5 left enter\n\
             SECOND.EXE 2\n"
        ));
    }

    #[test]
    fn the_token_is_exactly_sixty_four_hex_characters() {
        let t = generate_token().unwrap();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(t, generate_token().unwrap());
    }

    /// The recorded port session (`tests/fixtures/dac_port.json`): its reads are the path the
    /// oracle builds, and the triple read after selecting DAC entry 1 is entry 1 of the
    /// recorded palette, because reads advance and the select resets.
    #[test]
    fn the_port_reads_match_the_recorded_session() {
        let recorded: serde_json::Value = serde_json::from_str(DAC_PORT_JSON).unwrap();
        let calls = recorded["calls"].as_array().unwrap();
        let reads: Vec<&str> = calls
            .iter()
            .filter(|call| call["request"]["method"] == "GET")
            .map(|call| call["request"]["path"].as_str().unwrap())
            .collect();
        assert!(reads.len() >= 3, "the fixture backs the read protocol");
        for path in reads {
            let port: u16 = path.rsplit('=').next().unwrap().parse().unwrap();
            assert_eq!(path, format!("/io/port?port={port}"));
        }
        let values: Vec<u8> = calls
            .iter()
            .filter(|c| c["request"]["path"] == "/io/port?port=969")
            .filter_map(|c| c["response"]["value"].as_u64().map(|v| v as u8))
            .collect();
        assert_eq!(&values[..3], &DAC_BIN[3..6]);
    }
}
