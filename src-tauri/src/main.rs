// AeroTech Staff — native desktop shell (Tauri v2).
//
// Wraps the existing browser app: spawns the zero-dependency Node sidecar on a
// private loopback port, waits for it to listen, then opens that URL in a native
// WebView2 window. The sidecar's lifetime is bound to this process.
//
// Secrets (roadmap 2.1): BYOK API keys live in the OS keychain (never in
// localStorage). The Rust side stores/reads them via the `keyring` crate. Keys are
// injected into the sidecar's env at spawn AND can be updated live by POSTing provider
// config to the sidecar's token-guarded /api/key endpoint — so changing a key never
// restarts the sidecar (which would kill the page the user is on).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod credentials;
mod desktop_assets;
mod fresh_start;
mod hud_mode;
mod lifecycle_preferences;
mod sidecar_startup;
mod webview_recovery;
mod window_visibility;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    ipc::Channel, AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_updater::{Update, UpdaterExt};

use credentials::{
    channel_keychain_entry, credits_keychain_entry, delete_credential_honest, is_known_channel,
    keychain_entry, keychain_entry_for, keychain_pool_entry_for,
    migrate_channel_tokens_from_plaintext, migrate_credits_token_from_plaintext,
    normalize_provider, read_channel_token, read_credits_token, read_key, read_key_for,
    read_key_pool_for, read_telegram_bot_tokens, restore_credential, rollback_error,
    KEYCHAIN_PROVIDERS, SIDECAR_CHANNEL_TOKEN_ENVS, SIDECAR_PROVIDER_KEY_ENVS,
};
use lifecycle_preferences::{
    load as load_lifecycle_preferences, save_verified as save_lifecycle_preferences,
    LifecyclePreferences,
};

/// Shared runtime state: the fixed sidecar port, the per-launch IPC token (shared
/// only with the sidecar), the project root, and the live child.
struct AppState {
    port: u16,
    app_version: String,
    ipc_token: String,
    api_token: String,
    root: PathBuf,
    workspaces: PathBuf,
    startup_log: Option<PathBuf>,
    sidecar: Mutex<Option<Child>>,
    keep_awake: Mutex<KeepAwakeState>,
    lifecycle_preferences_path: PathBuf,
    lifecycle_preferences: Mutex<LifecyclePreferences>,
    close_exit_pending: AtomicBool,
    startup_reveal: window_visibility::StartupReveal,
    // Pauses the crash guardian while an explicit restart/reset owns the child lifecycle.
    recovery_in_progress: AtomicBool,
    // Flipped true the instant the app starts exiting, so the guardian thread stops
    // respawning the sidecar during an intentional quit.
    shutting_down: AtomicBool,
    // Crash-loop memory for the guardian (see spawn_guardian): consecutive unexpected sidecar exits,
    // the backoff in force, and the HALTED verdict the frontend reads via starnet_sidecar_status.
    guardian: Mutex<GuardianStatus>,
    // WebView2 crash recovery (webview_recovery.rs): true while a dead main window is being
    // destroyed and rebuilt, so the momentary zero-window state is not taken as an app exit.
    webview_rebuilding: AtomicBool,
    webview_recovery: webview_recovery::RecoveryBudget,
    // Set once setup has built the main window. Before that a second launch must not treat the
    // missing window as a dead instance (see webview_recovery::second_launch_action).
    main_window_built: AtomicBool,
}

/// What the guardian knows about the sidecar's exit history. Serialized verbatim to the frontend
/// (STATION DATA UNREACHABLE screen) so a halted crash loop is shown as such, never as a generic
/// "not answering".
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuardianStatus {
    /// true once the guardian gave up respawning (cap reached). Cleared by a user restart.
    halted: bool,
    /// unexpected exits inside the current crash window (reset once a child stays up).
    consecutive_crashes: u32,
    /// the most recent child exit code (None = killed by signal / unknown).
    last_exit_code: Option<i32>,
    /// wall-clock ms of that exit.
    last_exit_at_ms: Option<u64>,
    /// backoff currently in force before the next respawn, in ms (None = no delay pending).
    next_respawn_in_ms: Option<u64>,
    /// one human line: why the guardian is holding off / halted.
    reason: Option<String>,
}

/// After this many unexpected exits inside GUARDIAN_CRASH_WINDOW the guardian stops respawning.
const GUARDIAN_MAX_CONSECUTIVE_CRASHES: u32 = 6;
const GUARDIAN_CRASH_WINDOW: Duration = Duration::from_secs(10 * 60);
const GUARDIAN_MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Exit codes that mean the sidecar CHOSE to stop and expects a clean respawn — never a crash:
/// 0 = graceful shutdown (SIGTERM / gracefulShutdown), 75 = recovery / START FRESH restart request
/// (sidecar/index.js handleWorkspaceRecovery + handleWorkspaceStartFresh exit 75 after the ack).
fn sidecar_exit_is_intentional(code: Option<i32>) -> bool {
    matches!(code, Some(0) | Some(75))
}

/// Exponential backoff before the n-th consecutive crash respawn: 1s, 2s, 4s, 8s, 16s, 30s (cap).
fn guardian_backoff(consecutive_crashes: u32) -> Duration {
    let n = consecutive_crashes.max(1) - 1;
    let secs = 1u64
        .checked_shl(n.min(10))
        .unwrap_or(GUARDIAN_MAX_BACKOFF.as_secs());
    Duration::from_secs(secs).min(GUARDIAN_MAX_BACKOFF)
}

#[cfg(test)]
mod guardian_cap_tests {
    use super::*;

    #[test]
    fn backoff_doubles_from_one_second_and_caps_at_thirty() {
        assert_eq!(guardian_backoff(0), Duration::from_secs(1));
        assert_eq!(guardian_backoff(1), Duration::from_secs(1));
        assert_eq!(guardian_backoff(2), Duration::from_secs(2));
        assert_eq!(guardian_backoff(3), Duration::from_secs(4));
        assert_eq!(guardian_backoff(5), Duration::from_secs(16));
        assert_eq!(guardian_backoff(6), GUARDIAN_MAX_BACKOFF);
        assert_eq!(guardian_backoff(40), GUARDIAN_MAX_BACKOFF);
        assert_eq!(guardian_backoff(u32::MAX), GUARDIAN_MAX_BACKOFF);
    }

    #[test]
    fn sidecar_ready_timeout_outlasts_a_slow_boot_under_memory_pressure() {
        // 2026-09-23: healthy boots under memory pressure took 30 s+; the old 25 s timeout killed them.
        assert!(SIDECAR_READY_TIMEOUT >= Duration::from_secs(60));
    }

    #[test]
    fn intentional_exits_never_count_as_crashes() {
        assert!(sidecar_exit_is_intentional(Some(0)));
        assert!(sidecar_exit_is_intentional(Some(75)));
        assert!(!sidecar_exit_is_intentional(Some(1)));
        assert!(!sidecar_exit_is_intentional(Some(73)));
        assert!(!sidecar_exit_is_intentional(Some(74)));
        assert!(!sidecar_exit_is_intentional(None));
    }

    #[test]
    fn cap_is_reached_on_the_sixth_crash_inside_the_window() {
        let mut count = 0u32;
        let mut halted = false;
        for _ in 0..GUARDIAN_MAX_CONSECUTIVE_CRASHES {
            count = count.saturating_add(1);
            if count >= GUARDIAN_MAX_CONSECUTIVE_CRASHES {
                halted = true;
            }
        }
        assert!(halted);
        assert_eq!(count, 6);
        assert!(GUARDIAN_CRASH_WINDOW >= Duration::from_secs(60));
    }

    #[test]
    fn guardian_status_serializes_camel_case_for_the_frontend() {
        let g = GuardianStatus {
            halted: true,
            consecutive_crashes: 6,
            last_exit_code: Some(73),
            last_exit_at_ms: Some(1),
            next_respawn_in_ms: None,
            reason: Some("crash-loop".to_string()),
        };
        let json = serde_json::to_string(&g).expect("serializes");
        assert!(json.contains("\"halted\":true"));
        assert!(json.contains("\"consecutiveCrashes\":6"));
        assert!(json.contains("\"lastExitCode\":73"));
        assert!(json.contains("\"reason\":\"crash-loop\""));
    }
}

/// Serializes user-driven recovery commands and keeps the guardian paused until every return path
/// (including errors) has finished. Tauri commands may run concurrently, so a plain load/store can
/// let Restart and Start Fresh kill/spawn/move the same station at the same time.
struct RecoveryOperation<'a> {
    flag: &'a AtomicBool,
}

impl Drop for RecoveryOperation<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

fn begin_recovery(state: &AppState) -> Result<RecoveryOperation<'_>, String> {
    state
        .recovery_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "another station recovery is already running".to_string())?;
    Ok(RecoveryOperation {
        flag: &state.recovery_in_progress,
    })
}

#[cfg(unix)]
fn terminate_sidecar_child(child: &mut Child) {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    const SIGTERM: i32 = 15;
    let _ = unsafe { kill(child.id() as i32, SIGTERM) };
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate_sidecar_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

impl AppState {
    /// Kill the child sidecar on intentional shutdown. HONESTY NOTE: this only covers the
    /// graceful paths — the ExitRequested run-event and `Drop for AppState`. A hard kill of
    /// the shell (`taskkill /F`, crash, task-manager End Task, power loss) runs NEITHER, and
    /// there is no in-process hook that can — which is exactly how orphan sidecars happen.
    /// The reliable other half is `reap_orphan_sidecars`, which runs at the NEXT boot before
    /// spawning and terminates any process still running from our own bundled node runtime.
    fn kill_sidecar(&self) {
        if let Ok(mut guard) = self.sidecar.lock() {
            if let Some(mut child) = guard.take() {
                terminate_sidecar_child(&mut child);
            }
        }
    }
}

struct PendingUpdate(Mutex<Option<Update>>);

/// Lane 4D: the parsed result of a GET /api/lifecycle/armed poll — the sidecar's truthful account of whether
/// any background work (armed routines, connected channels, an armed night-shift) requires the process to keep
/// running after the window closes. `reasons` are short human strings the tray shows verbatim.
struct LifecycleArmed {
    armed: bool,
    reasons: Vec<String>,
}

/// Handles to the mutable tray menu items so the background poll thread can keep the tray honest (the status
/// line + tooltip must reflect REAL armed state, never a stale or optimistic claim).
struct TrayHandles {
    status: tauri::menu::MenuItem<tauri::Wry>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutostartStatus {
    desktop: bool,
    enabled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeepAwakeStatus {
    desktop: bool,
    supported: bool,
    enabled: bool,
    message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderKeyStatus {
    provider: String,
    configured: bool,
    alternate_count: usize,
}

#[cfg(windows)]
struct KeepAwakeHandle {
    handle: windows_sys::Win32::Foundation::HANDLE,
    _reason: Vec<u16>,
}

#[cfg(windows)]
unsafe impl Send for KeepAwakeHandle {}

#[cfg(windows)]
impl KeepAwakeHandle {
    fn create() -> Result<Self, String> {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Power::{
            PowerCreateRequest, PowerRequestSystemRequired, PowerSetRequest,
        };
        use windows_sys::Win32::System::Threading::{
            POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
        };

        let mut reason: Vec<u16> =
            "AeroTech Staff scheduled tasks are allowed to run while the app is open"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
        let context = REASON_CONTEXT {
            Version: 0,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: reason.as_mut_ptr(),
            },
        };
        let handle = unsafe { PowerCreateRequest(&context) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            let code = unsafe { GetLastError() };
            return Err(format!(
                "PowerCreateRequest failed with Windows error {code}"
            ));
        }
        if unsafe { PowerSetRequest(handle, PowerRequestSystemRequired) } == 0 {
            let code = unsafe { GetLastError() };
            unsafe {
                CloseHandle(handle);
            }
            return Err(format!("PowerSetRequest failed with Windows error {code}"));
        }
        Ok(Self {
            handle,
            _reason: reason,
        })
    }
}

#[cfg(windows)]
impl Drop for KeepAwakeHandle {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Power::{PowerClearRequest, PowerRequestSystemRequired};

        unsafe {
            let _ = PowerClearRequest(self.handle, PowerRequestSystemRequired);
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
struct KeepAwakeState {
    request: Option<KeepAwakeHandle>,
}

#[cfg(windows)]
impl KeepAwakeState {
    fn new() -> Self {
        Self { request: None }
    }

    fn status(&self) -> KeepAwakeStatus {
        KeepAwakeStatus {
            desktop: true,
            supported: true,
            enabled: self.request.is_some(),
            message: None,
        }
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<KeepAwakeStatus, String> {
        if enabled && self.request.is_none() {
            self.request = Some(KeepAwakeHandle::create()?);
        } else if !enabled {
            self.request = None;
        }
        Ok(self.status())
    }
}

#[cfg(not(windows))]
struct KeepAwakeState;

#[cfg(not(windows))]
impl KeepAwakeState {
    fn new() -> Self {
        Self
    }

    fn status(&self) -> KeepAwakeStatus {
        KeepAwakeStatus {
            desktop: true,
            supported: false,
            enabled: false,
            message: Some(
                "Keep Computer Awake is currently supported on Windows desktop builds.".to_string(),
            ),
        }
    }

    fn set_enabled(&mut self, _enabled: bool) -> Result<KeepAwakeStatus, String> {
        Ok(self.status())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStatus {
    desktop: bool,
    current_version: String,
    target: Option<String>,
    pending: Option<UpdateMetadata>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheck {
    available: bool,
    checked_at: u64,
    update: Option<UpdateMetadata>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateMetadata {
    version: String,
    current_version: String,
    date: Option<String>,
    body: Option<String>,
    target: String,
    critical: bool,
}

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
enum UpdateInstallEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
    },
    Finished,
    Installing,
}

fn update_metadata(update: &Update) -> UpdateMetadata {
    UpdateMetadata {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        date: update.date.map(|d| d.to_string()),
        body: update.body.clone(),
        target: update.target.clone(),
        critical: false,
    }
}

/// The sidecar is considered "ready" once it responds to GET /api/health with 200.
/// Under memory pressure this can take 30+ seconds; the timeout must outlast that.
const SIDECAR_READY_TIMEOUT: Duration = Duration::from_secs(90);

/// How long the close-path/tray lifecycle probe waits for a response before classifying Ambiguous.
const LIFECYCLE_PROBE_TIMEOUT: Duration = Duration::from_millis(800);

/// WebView2 cache directories that become stale across builds and MUST be purged on upgrade.
#[cfg(windows)]
const WEBVIEW2_STALE_CACHE_DIRS: &[&str] = &[
    "Code Cache",
    "GPUCache",
    "blob_storage",
    "Cache",
    "Service Worker",
];

/// Returns the path to the bundled Node runtime (the sidecar binary).
/// In development this falls back to `node` on PATH.
fn node_binary(root: &Path) -> PathBuf {
    #[cfg(windows)]
    let bundled = root.join("node.exe");
    #[cfg(not(windows))]
    let bundled = root.join("node");

    if bundled.exists() {
        bundled
    } else {
        // Dev fallback: the Node on PATH (never reaped — see is_reapable_node_path).
        PathBuf::from("node")
    }
}

/// Returns true for paths that are clearly our bundled runtime (absolute, contains our install dir).
/// Used by reap_orphan_sidecars to avoid killing unrelated Node processes.
fn is_reapable_node_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("StarNet") || s.contains("starnet")
}

/// Reap any orphan sidecars from previous launches. Called once at boot before spawning.
/// Returns the count of processes terminated.
#[cfg(windows)]
fn reap_orphan_sidecars(node_path: &Path, log: &Option<PathBuf>) -> usize {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    let mut reaped = 0usize;
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return reaped;
    }

    let mut entry = std::mem::MaybeUninit::<windows_sys::Win32::System::Diagnostics::ToolHelp::PROCESSENTRY32W>::uninit();
    unsafe {
        let ptr = entry.as_mut_ptr();
        (*ptr).dwSize = std::mem::size_of::<windows_sys::Win32::System::Diagnostics::ToolHelp::PROCESSENTRY32W>() as u32;
    }

    let mut first = true;
    loop {
        let ok = if first {
            first = false;
            unsafe { Process32FirstW(snapshot, entry.as_mut_ptr()) }
        } else {
            unsafe { Process32NextW(snapshot, entry.as_mut_ptr()) }
        };
        if ok == 0 {
            break;
        }
        let entry = unsafe { entry.assume_init_ref() };
        let exe_len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(260);
        let exe = String::from_utf16_lossy(&entry.szExeFile[..exe_len]);
        if !exe.eq_ignore_ascii_case("node.exe") {
            continue;
        }
        let pid = entry.th32ProcessID;
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            continue;
        }
        // Get the full image path to confirm it's our bundled runtime.
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let got_path = unsafe {
            windows_sys::Win32::System::Threading::QueryFullProcessImageNameW(
                handle,
                0,
                buf.as_mut_ptr(),
                &mut len,
            )
        };
        if got_path != 0 && len > 0 {
            let image_path = String::from_utf16_lossy(&buf[..len as usize]);
            if is_reapable_node_path(Path::new(&image_path)) {
                unsafe {
                    let _ = TerminateProcess(handle, 1);
                }
                reaped += 1;
                if let Some(log) = log {
                    let _ = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(log)
                        .and_then(|mut f| {
                            writeln!(
                                f,
                                "[reap] pid={pid} image={image_path}"
                            )
                        });
                }
            }
        }
        unsafe {
            let _ = CloseHandle(handle);
        }
    }
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    reaped
}

#[cfg(not(windows))]
fn reap_orphan_sidecars(_node_path: &Path, _log: &Option<PathBuf>) -> usize {
    0
}

/// Returns true if two paths point to the same file (case-insensitive on Windows).
fn same_path(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

/// Spawns the Node sidecar and waits for it to listen on the given port.
/// Injects the IPC token and API token into the sidecar's environment.
fn spawn_sidecar(
    state: &AppState,
    node_path: &Path,
    app_handle: &AppHandle,
) -> Result<Child, String> {
    let mut cmd = Command::new(node_path);
    cmd.arg("sidecar/index.js")
        .env("STARNET_PORT", state.port.to_string())
        .env("SKYNET_PORT", state.port.to_string())
        .env("STARNET_API_TOKEN", &state.api_token)
        .env("SKYNET_API_TOKEN", &state.api_token)
        .env("__STARNET_CUSTOM_CHROME__", "1")
        .current_dir(&state.root);

    // Inject provider API keys from the OS keychain.
    for provider in KEYCHAIN_PROVIDERS.iter() {
        if let Ok(key) = read_key(provider) {
            if let Some(env_name) = SIDECAR_PROVIDER_KEY_ENVS.get(provider) {
                cmd.env(env_name, key);
            }
        }
    }

    // Inject channel tokens.
    for channel in ["telegram", "discord", "slack"] {
        if let Ok(token) = read_channel_token(channel) {
            if let Some(env_name) = SIDECAR_CHANNEL_TOKEN_ENVS.get(channel) {
                cmd.env(env_name, token);
            }
        }
    }

    // Spawn and return.
    let child = cmd.spawn().map_err(|e| format!("Failed to spawn sidecar: {e}"))?;
    Ok(child)
}

/// Polls the sidecar's /api/health endpoint until it responds or the timeout expires.
fn wait_for_sidecar_ready(port: u16, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let url = format!("http://127.0.0.1:{port}/api/health");
    while Instant::now() < deadline {
        if let Ok(resp) = ureq::get(&url).timeout(Duration::from_millis(500)).call() {
            if resp.status() == 200 {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Sidecar failed to become ready within timeout".to_string())
}

/// The main Tauri setup function.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let _ = (app, argv, cwd);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .manage(PendingUpdate(Mutex::new(None)))
        .setup(|app| {
            let handle = app.handle().clone();
            let _ = setup_app(&handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_app(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let root = state.root.clone();
    let node_path = node_binary(&root);

    // Reap orphans from previous runs.
    let reaped = reap_orphan_sidecars(&node_path, &state.startup_log);
    if reaped > 0 {
        log::info!("Reaped {reaped} orphan sidecar(s)");
    }

    // Spawn the sidecar.
    let child = spawn_sidecar(&state, &node_path, app)?;
    *state.sidecar.lock().unwrap() = Some(child);

    // Wait for it to be ready.
    wait_for_sidecar_ready(state.port, SIDECAR_READY_TIMEOUT)?;

    // Build the main window.
    let _window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(format!("http://127.0.0.1:{}", state.port).parse().unwrap()))
        .title("AeroTech Staff")
        .inner_size(1280.0, 800.0)
        .center()
        .build()
        .map_err(|e| format!("Failed to build window: {e}"))?;

    Ok(())
}
