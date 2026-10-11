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
/// How many frames before the requested one the exact-grab loop starts.
///
/// That loop identifies every frame it takes with a counter reading on both
/// sides of the grab, so it begins early rather than at the requested frame.
/// The window has to outlast a host stall: a grab costs a millisecond or two
/// when the host is keeping up, but one scheduled-out round trip of tens of
/// milliseconds can jump several frames, and a window narrower than that
/// refuses captures that would otherwise have landed exactly. Twenty-five
/// frames is what the measurement in D17 used, and it costs a few hundred
/// grabs — all small, and cheap next to the indexed reads a capture makes.
const GRAB_WINDOW_FRAMES: u64 = 25;
/// How long the exact-grab loop keeps trying before it gives up. A frame that
/// cannot be identified must end as a refusal, never as a guess at which one
/// it was.
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

/// The `PUT /io/port` body for writing `value` to `port`, as recorded in
/// `tests/fixtures/dac_port.json`.
pub fn port_write_body(port: u16, value: u8) -> serde_json::Value {
    json!({ "port": port, "value": value })
}

/// The `GET /io/port` path for reading `port`, as recorded in
/// `tests/fixtures/dac_port.json` (the fixture records the path below the
/// `/api/v1` root).
pub fn port_read_path(port: u16) -> String {
    format!("/api/v1/io/port?port={port}")
}

/// What the oracle asks the emulator for. The capture's calls go to the
/// emulator over HTTP; the tests answer them from a script, so the identity
/// logic runs on the code that ships (D17).
trait EmulatorApi {
    fn get_bytes(&self, path: &str) -> Result<Vec<u8>, OracleError>;
    fn get_json(&self, path: &str) -> Result<serde_json::Value, OracleError>;
    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError>;
    fn put_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError>;
    /// `POST` a `text/plain` body, which is all `/script/load` accepts.
    fn post_text(&self, path: &str, body: &str) -> Result<Vec<u8>, OracleError>;
    /// Whether the API has answered at all: any status counts as up (the 401
    /// an unauthenticated probe gets means the API is listening), and only a
    /// transport error means it is not.
    fn probe(&self) -> Result<(), OracleError>;
}

/// The emulator's HTTP API, with the token it was given.
struct HttpApi {
    agent: ureq::Agent,
    token: String,
    base_url: String,
}

impl HttpApi {
    /// The emulator API, with the token the emulator was given.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
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
            .agent
            .get(&self.url(path))
            .set("Authorization", &format!("Bearer {}", self.token))
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
            .agent
            .post(&self.url(path))
            .set("Authorization", &format!("Bearer {}", self.token))
            .send_json(body.clone())
            .map_err(|e| self.transport_error(path, e))?;
        read_body(response)
    }

    fn put_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError> {
        let response = self
            .agent
            .put(&self.url(path))
            .set("Authorization", &format!("Bearer {}", self.token))
            .send_json(body.clone())
            .map_err(|e| self.transport_error(path, e))?;
        read_body(response)
    }

    fn post_text(&self, path: &str, body: &str) -> Result<Vec<u8>, OracleError> {
        let response = self
            .agent
            .post(&self.url(path))
            .set("Authorization", &format!("Bearer {}", self.token))
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
    /// Where the HTTP calls go: the emulator in a capture, a script in the
    /// tests. Everything the oracle asks for goes through it, so a test drives
    /// the same `wait_for_frame`/`frame` path a capture does rather than a
    /// stand-in for it (D17).
    api: Box<dyn EmulatorApi + Send>,
    scratch: PathBuf,
    /// The guest ran its own boot recipe out of `[autoexec]`
    /// (`BootRecipe::Autoexec`), so `boot` waits for the setup screen instead
    /// of typing one (D17).
    autoexec: bool,
    /// The frames [`Oracle::wait_for_frame`] has taken and not yet handed
    /// over, oldest first.
    ///
    /// A queue rather than a slot: a capture with neighbours to take waits for
    /// them before it reads the indices of the frame before, and a single slot
    /// would drop the earlier frame on the floor.
    pending: VecDeque<CapturedFrame>,
}

impl DosboxOracle {
    /// Launch the emulator headless and wait for its API.
    ///
    /// `allowed_base` is the directory the capture will mount, and must be
    /// named in the *primary* config: the mount allow-list is only read from
    /// there, and `--set` cannot fill it (D14). The spawn therefore points
    /// `XDG_CONFIG_HOME` at a scratch directory holding a primary config, and
    /// does not pass `--noprimaryconf`.
    ///
    /// `autoexec` are the lines that same config gives the guest's shell to
    /// run at boot (`BootRecipe::Autoexec`, D17); empty means the typed
    /// recipe, where the host drives the boot over the API instead.
    pub fn spawn(
        cycles: u32,
        allowed_base: &Path,
        autoexec: &[String],
    ) -> Result<DosboxOracle, OracleError> {
        // The mount policy compares the guest's MOUNT argument with the
        // allow-list, and the guest resolves a relative host path against the
        // emulator's own working directory, not ours. Absolutise here so the
        // two always name the same directory.
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

    /// Fail with a legible 401 rather than a puzzle if the emulator did not
    /// take the token this process generated for it.
    fn check_token(&self) -> Result<(), OracleError> {
        self.get_bytes("/api/v1/dosbox/info").map(|_| ())
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, OracleError> {
        self.api.get_json(path)
    }

    fn get_bytes(&self, path: &str) -> Result<Vec<u8>, OracleError> {
        self.api.get_bytes(path)
    }

    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError> {
        self.api.post_json(path, body)
    }

    fn put_json(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, OracleError> {
        self.api.put_json(path, body)
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
            // Any answer with a status — 200, or the 401 an unauthenticated
            // probe gets — means the API is up. A transport error means it is
            // not listening yet.
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
        self.post_json("/api/v1/script/start", &json!({}))?;
        Ok(())
    }

    fn type_line(&self, line: &str) -> Result<(), OracleError> {
        self.post_json(
            "/api/v1/input/type",
            &json!({ "text": format!("{line}\n"), "cps": 250 }),
        )?;
        Ok(())
    }

    fn read_memory(&self, segment: u16, offset: u32, len: usize) -> Result<Vec<u8>, OracleError> {
        self.get_bytes(&format!(
            "/api/v1/memory/{segment}/{offset}/{len}"
        ))
    }

    fn port_write(&self, port: u16, value: u8) -> Result<(), OracleError> {
        let answer = self.put_json("/api/v1/io/port", &port_write_body(port, value))?;
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
        let answer = self.get_json(&port_read_path(port))?;
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

    /// What kind of screen is up: the emulator's own name for the mode's
    /// colour depth, and the sequencer's chain-4 bit. The decision that turns
    /// the two into a read is [`super::plan_indexed_read`], which is pure and
    /// tested; this only asks.
    fn screen_layout(&self, color_depth: &str) -> Result<super::ScreenLayout, OracleError> {
        Ok(super::ScreenLayout::of(
            color_depth,
            self.chain4_enabled()?,
        ))
    }

    /// Indices for the frame on screen now: a linear read when the sequencer
    /// is in chain-4, otherwise one memory read per plane with the graphics
    /// controller's read map select set (D14), assembled at the screen's own
    /// row stride.
    ///
    /// **A screen the read cannot describe is refused rather than guessed
    /// at.** The read starts at offset 0 of each plane because that is where
    /// the demo's screens begin; when `stride * height` runs past what one
    /// plane holds, the guest's own display wraps and this read cannot say
    /// what it shows — ALKU is that case (176 × 400 = 70,400 > 65,536), and
    /// its frames come back RGB-only. So does a screen whose pixels are not
    /// one index a byte. A missing `.idx` is honest; a sheared one is a wrong
    /// artefact in the reference tree.
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

    /// The guest's screen as raw bytes, with the counter reading that
    /// identifies it — or `None` when it cannot be identified.
    ///
    /// The emulator publishes a frame's pixels and counts it immediately
    /// afterwards, in that order and on one thread (`gui/render/render.cpp:446`
    /// then `gui/sdl_gui.cpp:1205`), so a counter reading that is the same on
    /// both sides of the grab can only have seen the frame that reading names:
    /// no frame was published in between, or the reading would have moved. The
    /// exception is narrow and loud: a video-mode stop or reset advances the
    /// counter without publishing (`GFX_Stop`, `gui/sdl_gui.cpp:396-402`;
    /// `halt_render`, `gui/render/render.cpp:349-353`), so one counter value
    /// per stop can see the old picture — and a stop that changes geometry is
    /// refused by the capture's `GeometryChanged` check rather than written.
    /// When it did move, a frame boundary fell inside the grab and the picture
    /// belongs to one side or the other — `None` says so instead of guessing
    /// (D17).
    ///
    /// The bytes are returned undecoded on purpose. Decoding costs several
    /// milliseconds in a debug build, and anywhere inside or between the grab
    /// loop's iterations it would drop the loop's sampling rate to one grab
    /// per two or three frames — which is how the frame asked for used to slip
    /// between two grabs and get refused.
    fn grab_identified(&mut self) -> Result<Option<(u64, Vec<u8>)>, OracleError> {
        let before = self.frame_counter()?;
        let raw = self.get_bytes("/api/v1/video/frame?format=raw")?;
        let after = self.frame_counter()?;
        Ok((before == after).then_some((before, raw)))
    }

    /// Take the frame the counter names `index`, or refuse.
    fn grab_at(&mut self, index: u64) -> Result<CapturedFrame, OracleError> {
        let deadline = std::time::Instant::now() + GRAB_DEADLINE;
        let raw = grab_exactly(index, deadline, || self.grab_identified())?;
        decode_frame(&raw)
    }

    /// Take the next frame that can be identified, whatever the counter calls
    /// it.
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

/// Wait for the frame the counter names exactly, over an abstract grab.
///
/// `grab` returns the counter reading that identifies the frame it took and
/// the frame itself, or `None` for a grab that straddled a frame boundary.
/// The counter only moves forward one frame at a time, so the loop ends on the
/// frame that was asked for, on a frame that arrived too late (a refusal, not
/// a relabelling), or on the deadline.
///
/// Split out from the HTTP path so the three endings can be tested without an
/// emulator; the REST side around it is a thin shell (D17).
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

/// What the grab loop says when it runs out of time: a host slow enough to
/// miss every frame's window is a failure to report, not a frame to invent.
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
            &String::from_utf8(self.get_bytes("/api/v1/dosbox/info")?)
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
            // The guest's shell already ran the whole recipe out of
            // `[autoexec]`, `AUTOTYPE` included, so there is nothing to type
            // and no key to send: this waits for the screen every route has to
            // pass through, which also catches a mount the policy refused
            // before the capture waits on frames that would never come (D17).
            return self.wait_for_text(crate::reference::SETUP_SCREEN_TEXT, SHELL_TIMEOUT);
        }
        let cwd = absolute(cwd)?;
        // The mount policy is enforced by the guest MOUNT command, so a
        // refused base shows up as screen text, not as an HTTP error.
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
        let answer = self.put_json("/api/v1/cpu/cycles", &json!({ "cycles": cycles }))?;
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

    fn wait_for_frame(&mut self, index: u64) -> Result<(), OracleError> {
        // Sleep off everything but the last few frames: the grab loop below
        // polls, and it must not do that for the length of a skip.
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
        // The frame the wait pinned, when there is one: writing any other
        // frame under the index that was asked for is the failure this whole
        // dance exists to prevent (D17).
        //
        // The grab inside comes first and fast: everything below it — the
        // frame info, and in Mode X some eight hundred requests for planes and
        // palette — takes long enough that the emulator is many frames further
        // on by the time it is done, so a staleness check after it would
        // measure nothing.
        let mut frame = match self.pending.pop_front() {
            Some(frame) => frame,
            None => self.grab_any()?,
        };
        let info = self.get_json("/api/v1/video/frame/info")?;
        if info["video_mode"]["is_graphics_mode"].as_bool() != Some(true) {
            // A text screen has no palette indices, only glyph codes.
            return Ok(frame);
        }
        // The emulator's own name for the mode's colour depth. A depth it did
        // not name is not one this read claims to describe, and is refused
        // rather than assumed to be 256-colour.
        let color_depth = info["video_mode"]["color_depth"]
            .as_str()
            .unwrap_or_default();
        match self.capture_indexed(frame.width, frame.height, color_depth) {
            Ok((indexed, palette)) => {
                frame.indexed = Some(indexed);
                frame.palette = Some(palette);
                if !frame.indexed_matches_rgb() {
                    // The screen moved while its planes and palette were being
                    // read a request at a time. The RGB frame is still the
                    // frame that was asked for; the indices are not.
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
        // Frame-scheduled dispatch: the emulator normalises the frames around
        // the first timestamped event, so events land on exact rendered frames
        // relative to the call, instead of drifting with the host clock.
        let events: Vec<serde_json::Value> = names
            .iter()
            .enumerate()
            .map(|(i, name)| json!({ "key": name, "frame": 1 + i as u64 * KEY_SPACING_FRAMES }))
            .collect();
        self.post_json("/api/v1/input/sequence", &json!({ "events": events }))?;
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
            let screen = self.get_json("/api/v1/video/text")?;
            if screen["text"]
                .as_str()
                .is_some_and(|text| text.contains(needle))
            {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                // The last lines are where the command and its answer are; a
                // refused mount says why there, not in the banner.
                let lines: Vec<&str> = screen["text"]
                    .as_str()
                    .unwrap_or("<not a text screen>")
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .collect();
                let shown = lines[lines.len().saturating_sub(4)..].join(" / ");
                return Err(OracleError::Timeout(format!(
                    "waited {}s for {needle:?}; the bottom of the screen says: {shown}",
                    timeout.as_secs()
                )));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn wait_for_graphics(&mut self, timeout: Duration) -> Result<(), OracleError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let info = self.get_json("/api/v1/video/frame/info")?;
            if info["video_mode"]["is_graphics_mode"].as_bool() == Some(true) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                let screen = self.get_json("/api/v1/video/text")?;
                let lines: Vec<&str> = screen["text"]
                    .as_str()
                    .unwrap_or("<not a text screen>")
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .collect();
                let shown = lines[lines.len().saturating_sub(4)..].join(" / ");
                return Err(OracleError::Timeout(format!(
                    "the screen is still text after {}s; the bottom of it says: {shown}",
                    timeout.as_secs()
                )));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn frame_counter(&mut self) -> Result<u64, OracleError> {
        let status = self.get_json("/api/v1/script/status")?;
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

impl Drop for DosboxOracle {
    fn drop(&mut self) {
        // Never leave the emulator running, not even when a capture failed.
        let _ = self.shutdown();
    }
}

/// The emulator process, as capture starts it: headless, silent, on the pinned
/// cycles, with the token its API answers to.
///
/// Its output goes nowhere on purpose. A real audio device paces the mixer by
/// the host's audio clock, and the demo's music and animation then drift
/// against the frame counter between runs — a *silent* run is what makes a
/// moving screen repeat, and no boot recipe can undo the drift (D17). SDL's
/// dummy driver alone still let two runs land a frame or two apart over twenty
/// seconds; the emulator's own `nosound` mode leaves the emulated clock in
/// charge. Nothing captures audio, so nothing is lost.
fn emulator_command(
    binary: &Path,
    scratch: &Path,
    cycles: u32,
    libraries: &str,
    token: &str,
) -> Command {
    let mut command = Command::new(binary);
    command
        // `cpu_cycles`, not `cycles`: `cycles=` at startup forces legacy
        // mode, where the runtime PUT is refused with 409 (D14).
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

/// The primary config the scratch `XDG_CONFIG_HOME` holds: the mount
/// allow-list, which is only ever read from this file (D14), and — when the
/// capture boots through the guest (`BootRecipe::Autoexec`) — the `[autoexec]`
/// section the shell runs at boot (D17).
///
/// The allow-list and the `MOUNT` in the autoexec lines name the same
/// directory, so the mount the recipe performs is the one the policy allows.
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

/// Something already answering on the emulator's port would be captured
/// instead of the emulator this tool starts, which would be a silent lie.
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

    const INFO_JSON: &str = include_str!("../../tests/fixtures/info.json");
    const FRAME_BIN: &[u8] = include_bytes!("../../tests/fixtures/frame.bin");
    const MODEX_FRAME_BIN: &[u8] = include_bytes!("../../tests/fixtures/modex_title_frame.bin");
    const DAC_BIN: &[u8] = include_bytes!("../../tests/fixtures/dac.bin");
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
        // 15 bytes of header, not 16: one byte of shift costs every pixel.
        assert_eq!(raw.len(), RAW_HEADER_LEN + 200 * 1280);
    }

    #[test]
    fn the_title_screen_fixture_is_a_genuine_four_hundred_line_frame() {
        let f = decode_frame(MODEX_FRAME_BIN).unwrap();
        assert_eq!((f.width, f.height), (320, 400));
        assert_eq!(f.rgb.len(), 320 * 400 * 3);
        assert_eq!(MODEX_FRAME_BIN.len(), RAW_HEADER_LEN + 400 * 1280);
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
        // The two settings and the two dummy drivers are the recipe D17 was
        // measured with: a host video device, a host audio device, or an
        // unpinned cycle count each make captures of a moving screen differ
        // between runs.
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

    /// A scripted emulator, driven through the oracle's own calls.
    ///
    /// The counters and the pictures are separate queues on purpose: the
    /// emulator publishes a picture and *then* counts it, so a grab that
    /// straddles a frame boundary sees a counter from one side and a picture
    /// from the other, and that ordering is what the identity rule rests on
    /// (D17).
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

        /// The picture numbered `picture`, as a raw frame whose first pixel
        /// carries the number in its red byte — the frame comes back as
        /// `[red, green, blue]`, so `frame.rgb[0]` reads it back.
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
                // A text screen: `frame()` keeps the frame it was given and
                // skips the indexed path, which is not what these tests are
                // about.
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
            pending: VecDeque::new(),
        }
    }

    #[test]
    fn a_grab_that_straddles_a_frame_boundary_is_refused_not_written() {
        // The counter reads 9 twice (a clean grab of picture 9), then 10 while
        // the picture has already moved to 11: that grab straddled a boundary,
        // and picture 11 would be filed under index 10 by a grab that read the
        // counter only once. The wait must end in a refusal that names both
        // frames — never in a frame written under the wrong index.
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
        // Two clean grabs, 9 then 10: the wait keeps picture 10 and the
        // following `frame` hands it over. A `frame` that grabbed for itself
        // would return picture 11, which is the next thing the script has.
        let script = ScriptedEmulator::new(vec![9, 9, 9, 10, 10, 11], vec![9, 10, 11]);
        let mut oracle = scripted_oracle(script);
        oracle.wait_for_frame(10).unwrap();
        let frame = oracle.frame().unwrap();
        assert_eq!(frame.rgb[0], 10, "the frame for index ten, not a later grab");
    }

    #[test]
    fn a_sleep_never_runs_longer_than_a_second() {
        // An hour of frames must not sleep for an hour: one call is capped at
        // a second and the caller re-reads the counter in between. The
        // uncapped version overshot by three frames over a minute-long skip,
        // and that overshoot was the old capture's 0-3 frame lateness (D17).
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
        // Grabs that straddled a frame boundary (None) are thrown away, an
        // early frame is passed over, and the frame the counter names is the
        // one that comes back. This loop is what stands between the capture
        // and a frame written under an index it does not hold (D17).
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
        // The old wait accepted anything up to three frames past the index and
        // wrote it under the index anyway; that is the lie this refuses.
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
        // A stalled host must end as an error naming the frame, never as a
        // guess at which frame the picture belonged to.
        let deadline = std::time::Instant::now();
        let error = grab_exactly(8, deadline, || Ok(None::<(u64, &str)>)).unwrap_err();
        assert!(
            matches!(error, OracleError::Transport(ref message) if message.contains("frame 8")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn the_scratch_config_carries_the_guest_recipe_only_when_there_is_one() {
        // The typed recipe's config is what Phase 0 wrote: the allow-list and
        // nothing else, because the host drives that boot over the API.
        let typed = primary_config(Path::new("/reference/original"), &[]);
        assert!(typed.contains("mount_allowed_bases = /reference/original"));
        assert!(!typed.contains("[autoexec]"));

        // The autoexec recipe's lines go into the same file, in order: the
        // mount policy is only read from the primary config (D14), and the
        // section the shell runs has to be readable there too (D17).
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
        // Two spawns must not share a token.
        assert_ne!(t, generate_token().unwrap());
    }

    #[test]
    fn the_port_requests_match_the_recorded_session() {
        let recorded: serde_json::Value = serde_json::from_str(DAC_PORT_JSON).unwrap();
        let calls = recorded["calls"].as_array().unwrap();
        let mut writes = 0;
        let mut reads = 0;
        for call in calls {
            let request = &call["request"];
            let path = request["path"].as_str().unwrap();
            match request["method"].as_str().unwrap() {
                "PUT" => {
                    assert_eq!(path, "/io/port");
                    let port = request["port"].as_u64().unwrap() as u16;
                    let value = request["value"].as_u64().unwrap() as u8;
                    // The body the client builds is the one that was recorded.
                    assert_eq!(
                        port_write_body(port, value),
                        json!({ "port": port, "value": value })
                    );
                    writes += 1;
                }
                "GET" => {
                    let port: u16 = path
                        .rsplit('=')
                        .next()
                        .unwrap()
                        .parse()
                        .expect("recorded GET names its port");
                    assert_eq!(port_read_path(port), format!("/api/v1{path}"));
                    reads += 1;
                }
                other => panic!("unexpected method {other}"),
            }
        }
        assert!(writes >= 2 && reads >= 3, "the fixture backs the read protocol");

        // The session's first triple, read after selecting DAC entry 1, is
        // entry 1 of the recorded palette: reads advance, the select resets.
        let values: Vec<u8> = calls
            .iter()
            .filter(|c| c["request"]["path"] == "/io/port?port=969")
            .filter_map(|c| c["response"]["value"].as_u64().map(|v| v as u8))
            .collect();
        assert_eq!(&values[..3], &DAC_BIN[3..6]);
    }
}
