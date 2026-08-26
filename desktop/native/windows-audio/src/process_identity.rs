use std::ffi::c_void;
use std::path::Path;

use wasapi::{DeviceEnumerator, Direction, SessionState};
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// One process WASAPI currently reports as having an active audio session,
/// resolved down to its executable's file name — the same identity Linux
/// matches on via `application.process.binary`.
pub struct AudioProcessInfo {
    pub pid: u32,
    pub exe_name: String,
}

/// Resolves a `hwnd` (as reported by Chromium's `desktopCapturer`, e.g. the
/// numeric part of a `window:<hwnd>:0` source id) to the process that owns
/// that window.
pub fn get_pid_for_window(hwnd: i64) -> Option<u32> {
    let mut pid: u32 = 0;
    let hwnd = HWND(hwnd as isize as *mut c_void);
    let tid = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid))
    };
    (tid != 0 && pid != 0).then_some(pid)
}

/// Opens `pid` with the least-privileged access right that still permits
/// reading its image path (`PROCESS_QUERY_LIMITED_INFORMATION`) so this
/// works for other users' processes too, then resolves the process's own
/// executable file name (no directory, matching the identity Linux already
/// matches audio sessions on).
///
/// Public (unlike `list_active_audio_processes`, this works for *any*
/// live PID, not just ones with an active WASAPI session) because a
/// window's owning process is frequently not itself the process that ends
/// up holding the audio session — mirrors `resolveProcessBinary` on
/// Linux, which resolves a window's PID to a binary name the same way,
/// independently of what's currently making sound.
pub fn resolve_exe_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }
}

/// Every process WASAPI currently reports as having an active render-audio
/// session, deduplicated by resolved executable name — one representative
/// PID per app, which is all the capture engine (Task 3) needs to open one
/// loopback capture per distinct app.
pub fn list_active_audio_processes() -> napi::Result<Vec<AudioProcessInfo>> {
    let enumerator = DeviceEnumerator::new()
        .map_err(|err| napi::Error::from_reason(format!("DeviceEnumerator::new: {err}")))?;
    let devices = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|err| napi::Error::from_reason(format!("get_device_collection: {err}")))?;

    let mut seen_names = std::collections::HashSet::new();
    let mut processes = Vec::new();

    for device in &devices {
        let Ok(device) = device else { continue };
        let Ok(manager) = device.get_iaudiosessionmanager() else {
            continue;
        };
        let Ok(sessions) = manager.get_audiosessionenumerator() else {
            continue;
        };
        let Ok(count) = sessions.get_count() else {
            continue;
        };

        for i in 0..count {
            let Ok(control) = sessions.get_session(i) else {
                continue;
            };
            if control.get_state().unwrap_or(SessionState::Inactive) != SessionState::Active {
                continue;
            }
            let Ok(pid) = control.get_process_id() else {
                continue;
            };
            let Some(exe_name) = resolve_exe_name(pid) else {
                continue;
            };
            if !seen_names.insert(exe_name.clone()) {
                continue;
            }
            processes.push(AudioProcessInfo { pid, exe_name });
        }
    }

    Ok(processes)
}
