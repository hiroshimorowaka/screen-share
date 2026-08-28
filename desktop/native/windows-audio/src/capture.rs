use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use napi::bindgen_prelude::Buffer;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use wasapi::{initialize_mta, AudioClient, Direction, SampleType, StreamMode, WaveFormat};

use crate::process_identity::list_active_audio_processes;

const SAMPLE_RATE: usize = 48_000;
const CHANNELS: usize = 2;
// Matches the Linux implementation's own `scanAndLink` cadence — frequent
// enough that a process starting to make sound mid-share is picked up
// "within about a second" (this plan's own requirement), infrequent enough
// not to matter for CPU cost.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
// Matches Task 1's proven `MediaStreamTrackGenerator` chunk size.
const MIX_TICK: Duration = Duration::from_millis(20);
const MIX_TICK_FRAMES: usize = SAMPLE_RATE / 50;

/// Decides whether one resolved-by-name process's audio belongs in the
/// mix. Self-exclusion always wins — this app's own playback (e.g.
/// watching a share, including one's own, on the same machine) must never
/// be swept back into a mix this app is producing. Mirrors
/// `shouldIncludeFor` in the Linux implementation's `loopback-session.ts`.
pub fn should_include(
    mode: &str,
    target_name: &Option<String>,
    excluded_names: &[String],
    own_exe_name: &str,
    candidate_name: &str,
) -> bool {
    if candidate_name == own_exe_name {
        return false;
    }
    match mode {
        "window" => target_name.as_deref() == Some(candidate_name),
        _ => !excluded_names.iter().any(|n| n == candidate_name),
    }
}

/// This module's own executable name, resolved once — the same role
/// `OWN_BINARY_NAME` plays on Linux.
fn own_exe_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

type SharedBuffers = Arc<Mutex<HashMap<u32, Arc<Mutex<VecDeque<f32>>>>>>;

fn sleep_while_running(stop: &AtomicBool, total: Duration) {
    const STEP: Duration = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    while waited < total && !stop.load(Ordering::SeqCst) {
        let remaining = total - waited;
        std::thread::sleep(if remaining < STEP { remaining } else { STEP });
        waited += STEP;
    }
}

/// One running per-process WASAPI loopback capture: a dedicated thread
/// pulling frames as fast as the device signals they're ready into a
/// shared buffer, which the mixer thread drains on its own fixed cadence.
/// A per-process thread is needed (rather than one thread polling every
/// capture) because each capture's readiness is signalled by its own
/// WASAPI event handle.
struct CaptureHandle {
    stop: Arc<AtomicBool>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    thread: Option<JoinHandle<()>>,
}

impl CaptureHandle {
    fn spawn(process_id: u32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let buffer = Arc::new(Mutex::new(VecDeque::new()));
        let thread_stop = stop.clone();
        let thread_buffer = buffer.clone();
        let thread = std::thread::spawn(move || {
            if let Err(err) = run_capture(process_id, &thread_buffer, &thread_stop) {
                eprintln!("windows-audio: capture for pid {process_id} stopped: {err}");
            }
        });
        Self {
            stop,
            buffer,
            thread: Some(thread),
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_capture(
    process_id: u32,
    buffer: &Mutex<VecDeque<f32>>,
    stop: &AtomicBool,
) -> Result<(), wasapi::WasapiError> {
    initialize_mta().ok()?;

    let desired_format = WaveFormat::new(32, 32, &SampleType::Float, SAMPLE_RATE, CHANNELS, None);
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 0,
    };

    // `include_tree: true` — see the plan's "Verification risk" note on
    // Task 2: Chromium's Audio Service subprocess isn't reliably a
    // descendant of a given window's own process, so this only widens
    // what WASAPI itself watches for this one target PID. Which apps end
    // up captured at all is still decided entirely by `should_include`
    // matching resolved executable names, never by this process tree.
    let mut audio_client = AudioClient::new_application_loopback_client(process_id, true)?;
    audio_client.initialize_client(&desired_format, &Direction::Capture, &mode)?;
    let h_event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;

    let mut raw = VecDeque::new();
    audio_client.start_stream()?;

    while !stop.load(Ordering::SeqCst) {
        // A short timeout (rather than an unbounded wait) so this thread
        // still notices `stop` promptly even if the target process falls
        // silent — matches `stop()`'s expectation that every capture
        // thread actually exits, not lingers holding the device open.
        if h_event.wait_for_event(200).is_err() {
            continue;
        }
        let new_frames = capture_client.get_next_packet_size()?.unwrap_or(0);
        if new_frames == 0 {
            continue;
        }
        capture_client.read_from_device_to_deque(&mut raw)?;
        let mut samples = buffer.lock().unwrap();
        while raw.len() >= 4 {
            let bytes = [
                raw.pop_front().unwrap(),
                raw.pop_front().unwrap(),
                raw.pop_front().unwrap(),
                raw.pop_front().unwrap(),
            ];
            samples.push_back(f32::from_le_bytes(bytes));
        }
    }

    let _ = audio_client.stop_stream();
    Ok(())
}

fn run_poll_loop(
    mode: String,
    target_name: Option<String>,
    excluded_names: Vec<String>,
    own_name: String,
    shared_buffers: SharedBuffers,
    stop: Arc<AtomicBool>,
) {
    let mut captures: HashMap<u32, CaptureHandle> = HashMap::new();

    while !stop.load(Ordering::SeqCst) {
        if let Ok(processes) = list_active_audio_processes() {
            let mut active_pids = HashSet::new();
            for process in &processes {
                if !should_include(
                    &mode,
                    &target_name,
                    &excluded_names,
                    &own_name,
                    &process.exe_name,
                ) {
                    continue;
                }
                active_pids.insert(process.pid);
                captures.entry(process.pid).or_insert_with(|| {
                    let handle = CaptureHandle::spawn(process.pid);
                    shared_buffers
                        .lock()
                        .unwrap()
                        .insert(process.pid, handle.buffer.clone());
                    handle
                });
            }
            // A process whose session went away or stopped being active
            // gets its capture torn down here — `CaptureHandle::drop`
            // signals its thread to stop and joins it before this
            // returns.
            captures.retain(|pid, _| {
                let keep = active_pids.contains(pid);
                if !keep {
                    shared_buffers.lock().unwrap().remove(pid);
                }
                keep
            });
        }

        sleep_while_running(&stop, POLL_INTERVAL);
    }

    captures.clear();
    shared_buffers.lock().unwrap().clear();
}

fn run_mixer(
    shared_buffers: SharedBuffers,
    on_chunk: ThreadsafeFunction<Buffer>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::SeqCst) {
        let mut mixed = vec![0f32; MIX_TICK_FRAMES * CHANNELS];
        {
            let buffers = shared_buffers.lock().unwrap();
            for buffer in buffers.values() {
                let mut samples = buffer.lock().unwrap();
                for slot in mixed.iter_mut() {
                    let Some(sample) = samples.pop_front() else {
                        break;
                    };
                    *slot = (*slot + sample).clamp(-1.0, 1.0);
                }
            }
        }

        let mut bytes = Vec::with_capacity(mixed.len() * 4);
        for sample in &mixed {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        on_chunk.call(
            Ok(Buffer::from(bytes)),
            ThreadsafeFunctionCallMode::NonBlocking,
        );

        sleep_while_running(&stop, MIX_TICK);
    }
}

/// Everything needed to run one audio-loopback share: the poll thread that
/// keeps the set of per-process captures in sync with WASAPI's own active
/// sessions, the mixer thread that ticks every 20ms, and every per-process
/// capture thread either of those started.
pub struct SessionHandle {
    stop: Arc<AtomicBool>,
    poll_thread: Option<JoinHandle<()>>,
    mixer_thread: Option<JoinHandle<()>>,
}

impl SessionHandle {
    pub fn start(
        mode: String,
        target_name: Option<String>,
        excluded_names: Vec<String>,
        on_chunk: ThreadsafeFunction<Buffer>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let shared_buffers: SharedBuffers = Arc::new(Mutex::new(HashMap::new()));

        let mixer_thread = {
            let stop = stop.clone();
            let shared_buffers = shared_buffers.clone();
            std::thread::spawn(move || run_mixer(shared_buffers, on_chunk, stop))
        };

        let poll_thread = {
            let stop = stop.clone();
            let own_name = own_exe_name();
            std::thread::spawn(move || {
                run_poll_loop(
                    mode,
                    target_name,
                    excluded_names,
                    own_name,
                    shared_buffers,
                    stop,
                )
            })
        };

        Self {
            stop,
            poll_thread: Some(poll_thread),
            mixer_thread: Some(mixer_thread),
        }
    }

    /// Signals both loops to exit and blocks until they (and, via
    /// `CaptureHandle::drop`, every per-PID WASAPI capture) actually have.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.poll_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.mixer_thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_include;

    const OWN: &str = "screen-share.exe";

    #[test]
    fn self_playback_is_always_excluded() {
        // Even in "window" mode targeting our own exe — a share must never
        // sweep this app's own output back into a mix it produces.
        assert!(!should_include("window", &Some(OWN.into()), &[], OWN, OWN));
        assert!(!should_include("screen", &None, &[], OWN, OWN));
    }

    #[test]
    fn window_mode_includes_only_the_target_binary() {
        assert!(should_include("window", &Some("chrome.exe".into()), &[], OWN, "chrome.exe"));
        assert!(!should_include("window", &Some("chrome.exe".into()), &[], OWN, "spotify.exe"));
        assert!(!should_include("window", &None, &[], OWN, "chrome.exe"));
    }

    #[test]
    fn screen_mode_includes_everything_except_the_excluded_binaries() {
        let excluded = ["discord.exe".to_string(), "spotify.exe".to_string()];
        assert!(should_include("screen", &None, &excluded, OWN, "chrome.exe"));
        assert!(!should_include("screen", &None, &excluded, OWN, "discord.exe"));
    }
}
