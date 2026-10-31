//! The binary as a process: what a command line dumps, what it refuses and with which exit
//! code, and the run's audio (the `--audio-dump` file, a machine with no sound card, the
//! settings screen's wait). The anchor table, the numbering rules and the device's underrun
//! policy are unit-tested in `src/shell/` and `src/audio.rs`.

mod common;

use common::{differing, run, Dump, Scratch};

/// `2` is the command line's own verdict and `1` a run that failed once it started, the two
/// codes `xtask compare` uses too.
const USAGE: i32 = 2;
const FAILED: i32 = 1;
const OK: i32 = 0;

fn manifest(dir: &Scratch) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
#[should_panic(expected = "a short read is not a comparison")]
fn a_short_read_is_not_a_comparison() {
    differing(&[0; 6], &[0; 3], 3);
}

#[test]
fn headless_writes_two_frames_and_a_manifest() {
    let dir = Scratch::new("frames");
    let out = run(&["--headless", "--part", "testcard", "--frames", "2", "--dump-dir", dir.arg()]);
    assert_eq!(out.status.code(), Some(OK), "{}", stderr(&out));
    let manifest = manifest(&dir);
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["frames"].as_array().unwrap().len(), 2);
    assert_eq!(std::fs::read(dir.join("testcard_000000.idx")).unwrap().len(), 320 * 200);
    assert_eq!(std::fs::read(dir.join("testcard_000000.rgb")).unwrap().len(), 320 * 200 * 3);
}

#[test]
fn a_sparse_dump_writes_exactly_the_indices_it_was_asked_for() {
    let dir = Scratch::new("sparse");
    let out = run(&["--headless", "--part", "testcard", "--dump-frames", "3", "--dump-dir", dir.arg()]);
    assert_eq!(out.status.code(), Some(OK), "{}", stderr(&out));
    let manifest = manifest(&dir);
    assert_eq!(manifest["frames"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["frames"][0]["index"], 3);
    assert_eq!(manifest["frames"][0]["idx"], "testcard_000003.idx");
    assert!(dir.join("testcard_000003.rgb").is_file());
    assert!(!dir.join("testcard_000002.idx").exists(), "the run dumped more than it was asked");
}

#[test]
fn a_dump_manifest_says_which_numbering_its_indices_are_in() {
    // A run that began at no measured landmark numbers its own frames, and says so.
    let dump = Dump::new(&["--part", "testcard"], &[0]);
    assert_eq!(dump.manifest["index_origin"], "part");
    assert_eq!(dump.manifest["anchor"], serde_json::Value::Null);
    assert_eq!(dump.manifest["version"], 1);
}

#[test]
fn a_run_whose_entry_point_has_no_music_says_so_in_its_manifest() {
    // `music_for_part` is `None` for the test card: no player, no music in the manifest.
    let dump = Dump::new(&["--part", "testcard"], &[2]);
    assert_eq!(dump.manifest["music"], serde_json::Value::Null);
    assert_eq!(dump.frame(2).music_position, None);
}

#[test]
fn a_command_line_that_cannot_produce_a_dump_is_refused_before_it_runs() {
    let dir = Scratch::new("refusals");
    let dir = dir.arg();
    let cases: &[&[&str]] = &[
        // An unknown block, and a rejected one.
        &["--headless", "--block", "7", "--dump-dir", dir, "--dump-frames", "1"],
        &["--headless", "--block", "0", "--dump-dir", dir, "--dump-frames", "1"],
        // Two ways of bounding the run at once.
        &["--headless", "--dump-dir", dir, "--frames", "2", "--dump-frames", "3"],
        &["--headless", "--dump-dir", dir, "--run-seconds", "1", "--dump-frames", "3"],
        // A dump with no headless run to write it, and a headless run with nowhere to write.
        &["--dump-dir", dir, "--dump-frames", "3"],
        &["--headless", "--dump-frames", "3"],
        // A list with a hole in it.
        &["--headless", "--dump-dir", dir, "--dump-frames", "430,,470"],
        // An index before the block's anchor (404), which the run walks away from.
        &["--headless", "--block", "5", "--dump-dir", dir, "--dump-frames", "100"],
    ];
    for case in cases {
        let out = run(case);
        assert_eq!(out.status.code(), Some(USAGE), "{case:?}: {}", stderr(&out));
    }
}

#[test]
fn a_part_anchored_at_a_boot_index_refuses_a_dump_before_it() {
    // PAM's first drawn frame is the reference's 7710 (`reference-indices.md` §6.1), so 7709
    // is refused by the same `first_unreachable` rule a block's anchor enforces.
    let dir = Scratch::new("pam-anchor");
    let out = run(&["--headless", "--part", "pam", "--dump-frames", "7709", "--dump-dir", dir.arg()]);
    assert_eq!(out.status.code(), Some(USAGE), "{}", stderr(&out));
    assert!(stderr(&out).contains("7709"), "the refusal names the index: {}", stderr(&out));
}

#[test]
fn the_vault_menu_refuses_a_headless_run() {
    // The menu is the window's; a headless run names its part with `--part`.
    let out = run(&["--vault", "--headless"]);
    assert_eq!(out.status.code(), Some(USAGE), "{}", stderr(&out));
    let message = stderr(&out);
    assert!(message.contains("the vault menu needs a window; name a part with --part"), "{message}");
    assert!(message.contains("--vault"), "the usage does not list --vault: {message}");
}

// --- audio -----------------------------------------------------------------------------------

#[test]
fn a_headless_run_plays_the_music_with_and_without_a_device() {
    // `--headless` opens no device, so `--no-audio` changes nothing: the frames byte for byte,
    // and the music's published position at every index.
    let keyframes = [430, 500, 900, 1180];
    let with = Dump::new(&["--block", "5"], &keyframes);
    let without = Dump::new(&["--block", "5", "--no-audio"], &keyframes);
    assert_eq!(with.manifest["music"], without.manifest["music"]);
    for index in keyframes {
        let (a, b) = (with.frame(index), without.frame(index));
        assert_eq!(a.music_position, b.music_position, "{index}: the music");
        assert!(a.rgb == b.rgb && a.idx == b.idx, "{index} differs between a run with and without audio");
    }
}

#[test]
fn the_audio_dump_is_the_ticks_samples_at_the_mixers_own_rate() {
    // 16-bit stereo PCM at **20000 frames a second**: the mixer makes 40000 i16 a second
    // (`MixRate::Hz40000`) and two channels make a frame (`music.md` §10.2).
    let dir = Scratch::new("wav");
    let wav = dir.join("music.wav");
    let out = run(&[
        "--headless", "--block", "5", "--frames", "30", "--dump-dir", dir.arg(),
        "--audio-dump", wav.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(OK), "{}", stderr(&out));

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

    // One poll a tick: 30 ticks from 404 are 26 polls from the clock's 408, each
    // 40000 / 70.086 = 570.7 samples.
    let samples = word(40) / 2;
    assert!((26 * 570..=26 * 572).contains(&samples), "26 polls are about 14,838 samples, not {samples}");
    let data: Vec<i16> =
        bytes[44..].chunks_exact(2).map(|pair| i16::from_le_bytes([pair[0], pair[1]])).collect();
    assert!(data.iter().filter(|sample| **sample != 0).count() > data.len() / 2, "mostly silence");
    assert!(data.iter().any(|sample| sample.abs() > 4096), "its peak is a signal, not dither");
    assert_eq!(manifest(&dir)["music"]["module"], "MUSIC0");
}

#[test]
fn a_run_with_no_music_refuses_to_dump_audio() {
    // A WAV of the test card would be a header that reads as a recording of silence.
    let dir = Scratch::new("no-music-wav");
    let wav = dir.join("nothing.wav");
    let out = run(&[
        "--headless", "--part", "testcard", "--frames", "1", "--dump-dir", dir.arg(),
        "--audio-dump", wav.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(USAGE), "{}", stderr(&out));
    assert!(stderr(&out).contains("--audio-dump needs a run with music"), "{}", stderr(&out));
    assert!(!wav.exists(), "and nothing was written");
}

#[test]
fn a_dump_that_cannot_be_written_fails_the_run() {
    let dir = Scratch::new("unwritable");
    let wav = dir.join("no/such/directory/music.wav");
    let out = run(&[
        "--headless", "--block", "5", "--frames", "1", "--dump-dir", dir.arg(),
        "--audio-dump", wav.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(FAILED), "{}", stderr(&out));
    let message = stderr(&out);
    assert!(message.contains("music.wav") || message.contains("No such file"), "{message}");
}

/// A windowed run under SDL's dummy video driver, with `audio_driver` for its sound.
fn windowed(audio_driver: &str, args: &[&str]) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(common::root())
        .env("SDL_VIDEODRIVER", "dummy")
        .env("SDL_RENDER_DRIVER", "software")
        .env("SDL_AUDIODRIVER", audio_driver)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn a_machine_with_no_audio_device_says_so_and_keeps_running() {
    // `SDL_AUDIODRIVER` names a driver that is not built in, so `sdl.audio()` fails: the run
    // must say so and keep drawing. `--no-setup` skips the settings screen, which opens the
    // card only after Enter (D44).
    let mut child = windowed("sr-not-a-driver", &["--block", "5", "--no-setup"]);
    // Read the lines on a thread, so the wait below is bounded whatever the child does.
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
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    assert!(
        said_so,
        "the run never said the machine has no audio device; it printed {heard:?} and exited \
         with {:?}, stderr: {}",
        output.status.code(),
        stderr(&output)
    );
}

#[test]
fn a_demo_run_waits_on_the_settings_screen_and_opens_no_card_until_enter() {
    // With no key, a windowed demo run is still on the settings screen after three seconds and
    // has looked for no sound card (D44).
    let mut child = windowed("dummy", &[]);
    std::thread::sleep(std::time::Duration::from_secs(3));
    let alive = child.try_wait().unwrap().is_none();
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        alive,
        "the run ended instead of waiting on the settings screen: {:?}, stdout: {stdout}, \
         stderr: {}",
        output.status.code(),
        stderr(&output)
    );
    assert!(
        !stdout.contains("audio at") && !stdout.contains("no audio device"),
        "a sound card was looked for before Enter: {stdout}"
    );
}
