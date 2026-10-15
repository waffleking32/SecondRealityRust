//! The run's music through the real binary: what is dumped beside the frames, and what a
//! machine with no sound card still plays.
//!
//! **The device itself is not here.** A test cannot open a sound card (there may be none,
//! and opening one is not a test's act), so what this file checks is what a card is *not*
//! needed for: that a run's music is the same run with and without a device, that
//! `--audio-dump` writes the mixer's own samples, and that a run with no music refuses to
//! pretend otherwise. The device's underrun policy — silence, never a stall or a resample —
//! is unit-tested against the callback itself in `src/audio.rs`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The keyframes ENDLOGO's reference holds under `--arg 5` (`reference-indices.md` §19.2):
/// the same four `bins/secondreality/tests/endlogo.rs` compares, and the frames a `--block 5`
/// run reaches ENDLOGO's gate inside.
const KEYFRAMES: &str = "430,500,900,1180";

const OK: i32 = 0;
const USAGE: i32 = 2;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run the binary the way the repository's commands do, from the repository root — where
/// `assets/` is, and where the music's modules are read from.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(args)
        .output()
        .unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sr-audio-it-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn manifest(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap()
}

/// Every dumped frame's two planes, by file name.
fn frames_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.ends_with(".idx") || name.ends_with(".rgb")
        })
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn a_headless_run_plays_the_music_with_and_without_a_device() {
    // **The phase's own test that the run is the demo.** `--headless` opens no device, so
    // `--no-audio` must change *nothing* about it: the frames, byte for byte, and the
    // music's published position at every index. Without this the phase's comparisons would
    // be measuring a demo that is not the one that plays.
    let with_device = scratch("with");
    let without = scratch("without");
    let both: &[&[&str]] = &[
        &[
            "--headless",
            "--block",
            "5",
            "--dump-frames",
            KEYFRAMES,
            "--dump-dir",
            with_device.to_str().unwrap(),
        ],
        &[
            "--headless",
            "--block",
            "5",
            "--dump-frames",
            KEYFRAMES,
            "--dump-dir",
            without.to_str().unwrap(),
            "--no-audio",
        ],
    ];
    for args in both {
        let out = run(args);
        assert_eq!(out.status.code(), Some(OK), "{}", String::from_utf8_lossy(&out.stderr));
    }

    let a = frames_of(&with_device);
    let b = frames_of(&without);
    assert_eq!(a.len(), 8, "four keyframes, two planes each");
    assert_eq!(
        a.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        b.iter().map(|(name, _)| name).collect::<Vec<_>>()
    );
    for ((name, left), (_, right)) in a.iter().zip(&b) {
        assert_eq!(left, right, "{name} differs between a run with and without audio");
    }

    // And the two runs' music is at the same place at every index, which is the fact the
    // byte equality rests on: the position the parts read is the *tick's*, and a device —
    // or its absence — never moves it.
    let a = manifest(&with_device);
    let b = manifest(&without);
    assert_eq!(a["music"], b["music"]);
    assert_eq!(a["music"]["module"], "MUSIC0");
    assert_eq!(a["music"]["start_order"], 25);
    assert_eq!(a["music"]["start_frame"], 408);
    let positions: Vec<Option<String>> = a["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| frame["music_position"].as_str().map(str::to_string))
        .collect();
    assert_eq!(
        positions,
        vec![
            Some("25:1".to_string()),
            Some("25:9".to_string()),
            Some("25:55".to_string()),
            Some("26:53".to_string()),
        ],
        "the four keyframes' positions, as `reference/music/second5` has them"
    );
    assert_eq!(
        a["frames"][0]["music_position"], b["frames"][0]["music_position"],
        "and they are equal between the two runs"
    );

    let _ = std::fs::remove_dir_all(&with_device);
    let _ = std::fs::remove_dir_all(&without);
}

#[test]
fn the_audio_dump_is_the_ticks_samples_at_the_mixers_own_rate() {
    // The dump is the artifact a person listens to, so its header has to be the mixer's
    // own: 16-bit signed PCM, stereo, and **20000 frames a second** — the mixer's samples
    // are 40000 i16 a second (`MixRate::Hz40000`) and two channels make a frame, so the
    // frame rate a WAV header carries is half that (`music.md` §10.2's "the frame rate is
    // 20000 a second in stereo").
    let dir = scratch("dump");
    let wav = dir.join("music.wav");
    let out = run(&[
        "--headless",
        "--block",
        "5",
        "--frames",
        "30",
        "--dump-dir",
        dir.to_str().unwrap(),
        "--audio-dump",
        wav.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(OK), "{}", String::from_utf8_lossy(&out.stderr));

    let bytes = std::fs::read(&wav).unwrap();
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let half = |at: usize| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(word(4), (bytes.len() - 8) as u32, "the RIFF size is the file's");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(word(16), 16);
    assert_eq!(half(20), 1, "PCM");
    assert_eq!(half(22), 2, "the mixer's channels");
    assert_eq!(word(24), 20_000, "the mixer's frame rate");
    assert_eq!(half(34), 16, "16-bit samples");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(word(40), (bytes.len() - 44) as u32, "the data chunk is the file");
    assert_eq!(bytes.len() % 4, 44 % 4, "a whole number of stereo frames");

    // **The samples are the ticks'.)** A `--block 5` run begins at 404 and the music's own
    // clock starts at 408, so 30 ticks (404…433) are 26 polls — one a tick from 408 — and a
    // poll is `40000/70.086` = 570.7 samples. The count is asserted in that bracket rather
    // than against a number remembered from elsewhere: the point is that it is *one poll a
    // tick*, not a rate the dump invented.
    let samples = word(40) / 2;
    assert!(
        (26 * 570..=26 * 572).contains(&samples),
        "26 polls of 570.7 samples is about 14,838, and the dump holds {samples}"
    );
    let samples_of = |from: usize| -> Vec<i16> {
        bytes[from..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    };
    let data = samples_of(44);
    assert_eq!(data.len() as u32, samples);
    assert!(
        data.iter().filter(|sample| **sample != 0).count() > data.len() / 2,
        "the dump is mostly silence, which is not a recording of the music"
    );
    assert!(
        data.iter().any(|sample| sample.abs() > 4096),
        "and its peak is a signal, not dither"
    );

    // The frames were dumped too, and the manifest carries the same run's music.
    assert_eq!(manifest(&dir)["music"]["module"], "MUSIC0");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_run_with_no_music_refuses_to_dump_audio() {
    // The test card is the port's own part and not the demo's: a run of it installs no
    // player, so a WAV would be a header and nothing else — a file that reads as a
    // recording which captured silence. The command line says so instead.
    let dir = scratch("no-music");
    let out = run(&[
        "--headless",
        "--part",
        "testcard",
        "--frames",
        "1",
        "--dump-dir",
        dir.to_str().unwrap(),
        "--audio-dump",
        dir.join("nothing.wav").to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(USAGE), "{}", String::from_utf8_lossy(&out.stderr));
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(message.contains("--audio-dump needs a run with music"), "{message}");
    assert!(!dir.join("nothing.wav").exists(), "and nothing was written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dump_that_cannot_be_written_fails_the_run() {
    // Not a silent shorter recording: the file cannot be opened, and the run says so with
    // an exit code a script can branch on rather than drawing frames beside a dump that is
    // not there.
    let dir = scratch("unwritable");
    let out = run(&[
        "--headless",
        "--block",
        "5",
        "--frames",
        "1",
        "--dump-dir",
        dir.to_str().unwrap(),
        "--audio-dump",
        dir.join("no/such/directory/music.wav").to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(message.contains("music.wav") || message.contains("No such file"), "{message}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_machine_with_no_audio_device_says_so_and_keeps_running() {
    // **The fallback, as behaviour.** SDL's audio subsystem cannot start here — the
    // `SDL_AUDIODRIVER` names a driver that is not built in — so `sdl.audio()` fails and the
    // run must print what happened and keep drawing rather than dying: the demo has to run
    // on a machine with no card, and the picture is not the card's.
    //
    // A window on a machine with no display, so SDL's own dummy video driver: the run's
    // *pixels* are not what this test is about, only that it starts, says the line and is
    // still alive to be killed when it does.
    let mut child = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .env("SDL_VIDEODRIVER", "dummy")
        .env("SDL_RENDER_DRIVER", "software")
        .env("SDL_AUDIODRIVER", "sr-not-a-driver")
        .args(["--block", "5"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    // Read the child's lines on a thread, so waiting for the one line cannot hang the test:
    // the loop below is bounded by its own deadline whatever the child does.
    let stdout = child.stdout.take().unwrap();
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut said_so = false;
    let mut heard: Vec<String> = Vec::new();
    while !said_so && std::time::Instant::now() < deadline {
        match lines.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok(line) => {
                said_so = line.contains("no audio device");
                heard.push(line);
            }
            // Nothing yet; the child may still be starting.
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            // Its stdout is closed: it either died or never opened a window.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    assert!(
        said_so,
        "the run never said the machine has no audio device; it printed {heard:?} and \
         exited with {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
