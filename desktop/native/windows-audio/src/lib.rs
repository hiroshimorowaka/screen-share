#![deny(clippy::all)]

mod capture;
mod process_identity;

use napi::bindgen_prelude::Buffer;
use napi::threadsafe_function::ThreadsafeFunction;
use napi_derive::napi;

/// One process WASAPI currently reports as having an active audio
/// session, resolved down to its executable's file name — the same
/// identity Linux matches audio streams on via
/// `application.process.binary`.
#[napi(object)]
pub struct AudioProcessInfo {
    pub pid: u32,
    pub exe_name: String,
}

/// Resolves a `hwnd` — the numeric part of the `window:<hwnd>:0` source id
/// Chromium's `desktopCapturer` reports — to the process that owns that
/// window.
#[napi]
pub fn get_pid_for_window(hwnd: i64) -> Option<u32> {
    process_identity::get_pid_for_window(hwnd)
}

/// Resolves any live PID to its executable's file name — works for any
/// process, not just ones with an active WASAPI session, so the caller
/// can turn a shared window's owning PID into the name `should_include`
/// needs even when that window's own process never itself holds the
/// audio session (see `process_identity::resolve_exe_name`).
#[napi]
pub fn get_exe_name_for_pid(pid: u32) -> Option<String> {
    process_identity::resolve_exe_name(pid)
}

/// Every process WASAPI currently reports as having an active
/// render-audio session, one entry per distinct resolved executable name.
#[napi]
pub fn list_active_audio_processes() -> napi::Result<Vec<AudioProcessInfo>> {
    let processes = process_identity::list_active_audio_processes()?;
    Ok(processes
        .into_iter()
        .map(|p| AudioProcessInfo {
            pid: p.pid,
            exe_name: p.exe_name,
        })
        .collect())
}

/// A running audio-loopback share: continuously mixes together every
/// currently-active WASAPI session whose resolved executable name passes
/// `mode`/`target_name`/`excluded_names`, delivering interleaved f32
/// stereo PCM chunks to `on_chunk` roughly every 20ms until `stop()` is
/// called.
#[napi]
pub struct WindowsAudioSession {
    inner: Option<capture::SessionHandle>,
}

#[napi]
impl WindowsAudioSession {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: None }
    }

    /// `mode` is `"window"` (only `target_name`'s audio) or `"screen"`
    /// (everything except `excluded_names`) — mirrors `AudioShareTarget`'s
    /// two variants on the TypeScript side. Calling `start` while already
    /// started is a no-op error rather than silently replacing the
    /// running session, since the caller (Task 4's TS wrapper) owns
    /// exactly one `WindowsAudioSession` per share.
    #[napi]
    pub fn start(
        &mut self,
        mode: String,
        target_name: Option<String>,
        excluded_names: Vec<String>,
        on_chunk: ThreadsafeFunction<Buffer>,
    ) -> napi::Result<()> {
        if self.inner.is_some() {
            return Err(napi::Error::from_reason(
                "windows-audio session already started",
            ));
        }
        self.inner = Some(capture::SessionHandle::start(
            mode,
            target_name,
            excluded_names,
            on_chunk,
        ));
        Ok(())
    }

    /// Stops every per-process capture, the mixer, and the poll loop, and
    /// blocks until all of their threads have actually exited. A no-op if
    /// `start` was never called or `stop` already was.
    #[napi]
    pub fn stop(&mut self) {
        if let Some(session) = self.inner.take() {
            session.stop();
        }
    }
}

impl Default for WindowsAudioSession {
    fn default() -> Self {
        Self::new()
    }
}
