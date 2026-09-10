# Trident, implementation details

Read `product.md` and `architecture.md` first. This document is the build manual. Everything here is prescriptive.

## 1. Toolchain installation, from a clean Windows machine

Run these in an elevated PowerShell. Verify each before moving on. Do not skip verification: a missing C++ toolchain fails 40 minutes later with an opaque linker error.

```powershell
# 1. Visual Studio C++ build tools. Required by the MSVC Rust toolchain.
winget install --id Microsoft.VisualStudio.2022.BuildTools -e `
  --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# 2. Rust
winget install --id Rustlang.Rustup -e
# restart the shell so PATH updates, then:
rustup default stable-x86_64-pc-windows-msvc
rustup component add clippy rustfmt

# 3. Node LTS
winget install --id OpenJS.NodeJS.LTS -e

# 4. WebView2 runtime. Preinstalled on Windows 11 and most Windows 10.
#    Check first, install only if missing:
Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue
winget install --id Microsoft.EdgeWebView2Runtime -e   # only if the above returned nothing

# 5. Optional, for E2E only
cargo install tauri-driver --locked
```

Verification, all must succeed:

```powershell
rustc --version          # expect 1.8x or newer, host x86_64-pc-windows-msvc
cargo --version
node --version           # expect v20 or v22
npm --version
cargo clippy --version
```

If `rustc` reports a `gnu` host triple, the wrong toolchain is default. Fix with `rustup default stable-x86_64-pc-windows-msvc` before writing any code.

## 2. Project bootstrap and exact dependencies

```powershell
npm create tauri-app@latest trident -- --template react-ts --manager npm
cd trident
npm install
```

Then bring the manifests to exactly this shape.

`package.json` dependencies:

```
dependencies:      react ^19, react-dom ^19, zustand ^5, @tauri-apps/api ^2,
                   @tauri-apps/plugin-global-shortcut ^2, @tauri-apps/plugin-autostart ^2,
                   @tauri-apps/plugin-store ^2, @tauri-apps/plugin-dialog ^2,
                   @tauri-apps/plugin-opener ^2, clsx ^2
devDependencies:   @tauri-apps/cli ^2, typescript ^5.6, vite ^6, @vitejs/plugin-react ^4,
                   tailwindcss ^4, @tailwindcss/vite ^4, vitest ^2, jsdom ^25,
                   @testing-library/react ^16, @testing-library/user-event ^14,
                   @types/react ^19, @types/react-dom ^19, eslint ^9, prettier ^3
```

`src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "devtools"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-autostart = "2"
tauri-plugin-store = "2"
tauri-plugin-dialog = "2"
tauri-plugin-single-instance = "2"
tauri-plugin-opener = "2"
specta = { version = "2.0.0-rc", features = ["derive"] }
specta-typescript = "0.0.7"
tauri-specta = { version = "2.0.0-rc", features = ["derive", "typescript"] }
rusqlite = { version = "0.32", features = ["bundled", "uuid", "backup"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
uuid = { version = "1", features = ["v7", "serde"] }
parking_lot = "0.12"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
window-vibrancy = "0.5"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_System_Threading",
  "Win32_System_ProcessStatus",
  "Win32_Graphics_Gdi",
  "Win32_Storage_FileSystem",
  "Win32_System_SystemInformation",
] }

[dev-dependencies]
tempfile = "3"
proptest = "1"

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

Do not add dependencies beyond this list without recording the decision in `architecture.md` section 1.

## 3. tauri.conf.json, the parts that matter

```jsonc
{
  "productName": "Trident",
  "identifier": "dev.manshah.trident",
  "app": {
    "withGlobalTauri": false,
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
    },
    "windows": [
      {
        "label": "orb",
        "url": "index.html",
        "width": 240, "height": 44,
        "decorations": false, "transparent": true, "resizable": false,
        "alwaysOnTop": true, "skipTaskbar": true, "shadow": false,
        "focus": false, "visible": false, "center": false
      },
      {
        "label": "panel",
        "url": "index.html",
        "width": 360, "height": 480,
        "decorations": false, "transparent": true, "resizable": false,
        "alwaysOnTop": true, "skipTaskbar": true, "shadow": true,
        "focus": false, "visible": false
      },
      {
        "label": "capture",
        "url": "index.html",
        "width": 420, "height": 44,
        "decorations": false, "transparent": true, "resizable": false,
        "alwaysOnTop": true, "skipTaskbar": true, "shadow": true,
        "focus": false, "visible": false
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "windows": { "nsis": { "installMode": "currentUser" } }
  }
}
```

The settings window is not declared here. It is created at runtime with `WebviewWindowBuilder` the first time it is opened, so its memory is not paid for by users who never open it.

All three declared windows start `visible: false`. The orb is shown only after `set_no_activate` has been applied, otherwise there is a visible frame in which it can steal focus.

## 4. The Win32 layer

This is the only place `unsafe` appears. Every function returns `Result<_, AppError>` and every failure names its API.

### 4.1 Making the orb non-activating

Call this once, after the window exists, before showing it.

```rust
// platform/win32.rs
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

pub fn set_no_activate(hwnd: HWND) -> Result<(), AppError> {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if current == 0 { return Err(AppError::platform("GetWindowLongPtrW")); }
        let updated = current
            | WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_TOPMOST.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated);
    }
    Ok(())
}
```

Get the `HWND` from `window.hwnd()?`. Apply it to the `orb` window only.

Three follow-on rules, all of which are easy to break by accident:
- Never call `set_focus()` on the orb, from Rust or from JS.
- The orb's HTML must contain no focusable elements. No `<button>`, no `<input>`, no `tabindex`. Use `<div role="button">` with pointer handlers, and set `user-select: none` on the root.
- If the orb ever needs to accept a keystroke, that is a signal to open the panel instead.

### 4.2 Foreground application

```rust
pub fn foreground_app(read_title: bool) -> Result<Option<ForegroundApp>, AppError>
```

Sequence, with every failure branch handled:

1. `GetForegroundWindow()`. If it returns a null HWND, return `Ok(None)`. This happens on the lock screen, during a secure desktop prompt (UAC), and briefly during desktop switches. It is a normal condition, not an error.
2. `GetWindowThreadProcessId(hwnd, &mut pid)`. If pid is 0, return `Ok(None)`.
3. **UWP unwrapping.** If the resolved executable is `ApplicationFrameHost.exe`, the real app is a child window with a different pid. Call `EnumChildWindows` and take the first child whose pid differs from the frame host pid. If none is found, fall back to the frame host and mark the record as unresolved. Without this, every Store app (Mail, Photos, some Terminal configurations) reports as the same process and the focus set becomes meaningless.
4. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)`. On `ERROR_ACCESS_DENIED`, which happens for elevated processes when Trident is not elevated, return a `ForegroundApp` with `process_key: "unknown"` and `app_name: "Elevated application"` rather than an error. Treat `unknown` as always on-task, so an elevated tool never produces a false drift.
5. `QueryFullProcessImageNameW` into a 260-wide buffer, then take the file name only, lowercased, as `process_key`.
6. Friendly name: read the `FileDescription` from the executable's version info. On failure, fall back to the file stem with the first letter capitalised. Cache this in a `HashMap<String, String>` keyed by process_key, because version info reads are relatively expensive and the answer never changes within a session.
7. Title, only if `read_title` is true: `GetWindowTextLengthW` then `GetWindowTextW`. Truncate to 200 chars.

Always close the process handle. Use a small RAII guard so an early return cannot leak it.

### 4.3 Idle detection

```rust
pub fn idle_millis() -> Result<u64, AppError>
```

`GetLastInputInfo` returns `dwTime` as a `u32` tick count since boot, which **wraps every 49.7 days**. `GetTickCount64` does not wrap. Compute as:

```rust
let now = GetTickCount64();
let last = lii.dwTime as u64;
// reconstruct the 64-bit value of a 32-bit counter
let last64 = (now & !0xFFFF_FFFFu64) | last;
let last64 = if last64 > now { last64 - 0x1_0000_0000 } else { last64 };
Ok(now.saturating_sub(last64))
```

Never subtract the raw `u32` from a `u64` tick count. On an uptime past 49.7 days that produces an idle time of several weeks and every thread silently goes Away.

Note that `GetLastInputInfo` reports zero idle time while the workstation is locked in some configurations, so lock detection must not rely on idle alone. See 4.5.

### 4.4 Fullscreen detection

Compare the foreground window's rect against the rect of the monitor it is on:

1. `GetWindowRect(hwnd)`.
2. `MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)` then `GetMonitorInfoW` for `rcMonitor`.
3. Fullscreen if the window rect covers the monitor rect within 2 pixels on every edge **and** the window is not the desktop (`GetDesktopWindow`) and not the shell (`GetShellWindow`).

When fullscreen is detected and `hide_over_fullscreen` is on, hide the orb. Do not merely lower it: exclusive fullscreen games will flicker and some capture software will glitch. Re-show on the first non-fullscreen tick.

### 4.5 Lock and session state

Two independent signals, because neither alone is reliable:
- `GetForegroundWindow()` returning null for two consecutive ticks.
- The foreground process being `lockapp.exe` or `logonui.exe`.

Either signal enters `Away { reason: Locked }`. Both clearing returns to Running.

### 4.6 Sleep, hibernate and clock jumps

Do not register for power broadcast messages. Detect it structurally instead, which covers sleep, hibernation, VM suspension and NTP corrections with one mechanism:

Each tick records both `Instant::now()` and `SystemTime::now()`. Compare the monotonic delta with the wall delta since the previous tick.

- If the monotonic delta is much larger than `poll_ms` (say above 5 seconds), the process was suspended. Emit `Input::IdleFor(monotonic_delta)` so the machine goes Away and rewinds correctly, rather than crediting the sleep as focused work.
- If the wall delta and monotonic delta disagree by more than 5 seconds, the wall clock moved. Emit `Input::ClockJump`. Durations are always computed from `Instant`, so nothing is corrupted; the only action is to log it and re-derive display timestamps.

### 4.7 Monitors, DPI and orb docking

- Store the orb position as `{ monitor_name, physical_x, physical_y, edge }`, not logical coordinates, because logical coordinates are meaningless across mixed-DPI monitors.
- On startup and on `WindowEvent::Moved`, resolve the target monitor by name. If that monitor is gone, fall back to the primary monitor and clamp.
- Clamp into the **work area**, not the monitor rect, so the orb never lands under the taskbar. `SystemParametersInfoW(SPI_GETWORKAREA)` for the primary; `GetMonitorInfoW` `rcWork` for others.
- Docking: on drag end, compute the distance to each of the four work-area edges, snap to the nearest if within 64 physical pixels, and store the edge so the panel knows which side to anchor on.
- Handle `ScaleFactorChanged` by re-resolving the position rather than trusting the previous one.

## 5. Window behaviour details

### 5.1 Panel open, close and blur

- Open: position it relative to the orb's docked edge with an 8px gap, clamp inside the work area, `show()`, then `set_focus()`.
- Close on blur: listen for `WindowEvent::Focused(false)`, but **debounce by 120ms and cancel the hide if focus returns**. Without the debounce, clicking a dropdown or dragging inside the panel closes it. This is the single most common bug in this class of app.
- Escape closes. Handle it in JS with a `keydown` listener on `window`, and call `panel_close()`.
- Clicking the orb while the panel is open closes the panel. Track panel visibility in Rust, not in the orb's JS, because the orb never receives focus and its state can go stale.

### 5.2 Capture bar

- Summoned by hotkey from any app. Position it centred horizontally, 20% down the **active** monitor (the one with the foreground window), not the primary.
- The 150ms budget means the window is created at startup and only shown and hidden, never created on demand.
- On show: `show()`, `set_focus()`, then in JS focus the input and select all. On Escape or blur: clear the input, hide.
- On submit: call `dump_add`, then hide immediately without waiting for the response. If the call fails, re-show with the text restored and a small error line. Losing a captured thought is the worst failure this app can have, so the write path also appends the raw line to `logs\capture-fallback.log` before the database write is attempted.

### 5.3 Transparency and effects

Apply `window-vibrancy`'s acrylic to the panel and capture windows only. On Windows 10 builds where acrylic is unavailable, catch the error and set a solid `#16171A` background instead. Never leave a transparent window with no backdrop, or text renders over arbitrary desktop content.

The orb does not use acrylic. It is a small solid pill; acrylic on a 40px element costs more than it gives.

## 6. Hotkeys

Register through `tauri-plugin-global-shortcut` at startup.

- If registration fails because the accelerator is taken by another app, do not retry, do not crash. Record it in a `Vec<HotkeyProblem>` in state, emit `system:error` with `recoverable: true`, and show the conflict in Settings with a rebind affordance.
- Rebinding unregisters the old accelerator before registering the new one, and rolls back to the old one if the new registration fails.
- Validate accelerators before attempting registration: reject single keys, reject anything without at least one modifier, reject `Ctrl+Alt+Del` and the Windows key combinations reserved by the shell.
- Unregister all on exit.

## 7. Domain implementation notes

### 7.1 Elapsed time

Never store a running counter. The only correct computation:

```rust
pub fn elapsed_ms(t: &Thread, now_wall: i64) -> i64 {
    match t.state {
        Active => t.accumulated_ms + (now_wall - t.last_resumed_at.unwrap_or(now_wall)).max(0),
        _ => t.accumulated_ms,
    }
}
```

On pause, fold the delta into `accumulated_ms` and null `last_resumed_at` in a single transaction. On a crash mid-thread, the next startup finds a thread in `active` with a stale `last_resumed_at`; recovery folds in the time up to the last watcher heartbeat (persisted every 30 seconds in the settings table as `last_heartbeat_ms`), not up to now, so a machine left off overnight does not report a 14 hour focus session.

### 7.2 The single-open-thread invariant

Enforced by the partial unique index in the schema. `thread_start` runs inside a transaction that first closes any open thread. If the index still fires, the error surfaces as `AppError::InvalidState` rather than being silently swallowed, because it means two writers raced and that is a bug worth seeing.

### 7.3 Task ordering

Fractional indexing on a `REAL sort_key`. Insert at top uses `min - 1.0`, at bottom `max + 1.0`, between two neighbours the midpoint. When the gap between neighbours falls below `1e-6`, renormalise the whole list to integers in one transaction. Do not use integer positions with a shift-everything update; it makes every reorder an O(n) write.

### 7.4 Migrations

Forward-only, numbered, each an `up(&Connection) -> Result<()>`. Run inside a transaction, record the version, and refuse to start if the database version is higher than the binary knows about (a user who downgraded). Every migration gets a test that runs it against a database built by the previous migration and asserts the resulting schema and a data-preservation case.

## 8. Design tokens, single source

Put these in `src/lib/tokens.css` and reference them everywhere. No hex codes anywhere else in the codebase.

```css
:root {
  --surface:        #16171A;
  --surface-alpha:  0.82;
  --border:         rgba(255,255,255,0.08);
  --border-strong:  rgba(255,255,255,0.12);
  --text-primary:   #ECECEE;
  --text-secondary: #8A8A93;
  --text-muted:     #55565C;
  --accent-live:    #6FD3A8;
  --accent-drift:   #E2795F;
  --radius-pill:    999px;
  --radius-panel:   16px;
  --radius-control: 8px;
  --dur:            180ms;
  --ease:           cubic-bezier(0.22, 1, 0.36, 1);
  --font: "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
}
```

Rules: font sizes are 12, 13 and 15 only. Font weights are 400 and 500 only. The timer uses `font-variant-numeric: tabular-nums`. Transitions apply only to `opacity`, `background-color`, `border-color` and `transform`, only on state change, with no animation at rest. Orb opacity drops to 0.4 after `orb_dim_after_ms` and returns on `pointerenter`.

## 9. Edge cases, each with a required behaviour

This list is a test checklist. Every row must have a test or an explicit manual QA entry.

**Detection**
1. Alt-tab to another app and back within the grace period: nothing recorded, nothing shown.
2. Rapid switching between two apps, both outside the focus set: the grace clock restarts on each switch, so no interruption is opened until one of them is dwelt in past the threshold.
3. Drift into an app, then into a second app, both outside the focus set: the first interruption closes, a second opens. Never one merged record.
4. Drift, then the machine locks: the interruption closes at the lock instant, state becomes Away, not Drifting.
5. Foreground process is elevated and cannot be opened: treated as on-task, never a drift.
6. Foreground is a UWP app: reports the real app, not `ApplicationFrameHost.exe`.
7. Foreground returns null (UAC prompt): no state change on a single tick, Away after two.
8. Uptime past 49.7 days: idle detection still correct.
9. System sleeps for 8 hours mid-thread: on wake the thread is Away, elapsed unchanged, no 8 hour interruption row.
10. NTP moves the clock back 30 minutes mid-thread: elapsed does not go negative, no duplicate rows, one log line.
11. The user marks a drifted app as on-task: the interruption row is deleted, the app joins the focus set, state returns to Running immediately.
12. An app is on the global ignore list: never opens a candidate for any thread.
13. Detection paused for one hour: watcher still ticks for idle and lock, but never opens candidates.

**Windows and input**
14. Orb never takes focus: type continuously in Notepad while clicking and dragging the orb, no character is lost.
15. Panel stays open while interacting with its own controls, closes on genuine outside click.
16. Second instance launched: `tauri-plugin-single-instance` focuses the existing panel, no second tray icon.
17. Monitor unplugged while the orb is docked to it: orb reappears on the primary, inside the work area.
18. Display scale changed from 100% to 150%: orb size and position remain correct.
19. Taskbar moved to the top or auto-hidden: orb clamps to the work area, never underneath it.
20. Fullscreen game launched: orb hides, then returns on exit.
21. Hotkey already taken by another app: startup succeeds, conflict is visible in Settings.

**Data**
22. Process killed while a thread is active: on restart, the thread is recovered and elapsed is folded to the last heartbeat.
23. Database file is read-only or locked by an external tool: startup fails with a dialog naming the path, not a silent tray-less process.
24. Database file corrupt: a `PRAGMA integrity_check` at startup fails, the file is renamed to `trident.corrupt.<ts>.db`, a fresh database is created, and the user is told where the old one went.
25. Dump body of 50,000 characters pasted: accepted, list rendering stays smooth via truncation at display time.
26. Emoji and RTL text in a thread title: stored and rendered correctly, and the orb pill truncates with an ellipsis rather than growing unboundedly.
27. Thread title of one space: rejected by the CHECK constraint and by frontend validation with an inline message.
28. 10,000 dump rows: list is windowed, `dump_list` is paginated by `before` cursor, panel open still under 200ms.
29. Delete-all-data while a thread is active: thread stops, windows reset to Idle, no dangling state.

**Lifecycle**
30. Windows shuts down while running: no data loss, because every mutation is committed at the time it happens, never batched.
31. Start-with-Windows enabled: the app starts minimised to tray with the orb visible and no window flash.
32. First run with no database: migrations run, onboarding shows, no error toast.

## 10. Testing strategy

### 10.1 Rust unit tests, the core of the suite

`session/` is pure, so its tests are the highest value in the project. Write them as a table of scripted inputs.

```rust
#[test]
fn candidate_matures_into_drift_and_backdates() {
    let mut s = Session::new_running(thread_id, t0);
    let cfg = DetectionConfig { grace_ms: 20_000, drift_ms: 90_000, ..Default::default() };
    let effects = drive(&mut s, &cfg, &[
        at(0,      Foreground("code.exe")),
        at(10_000, Foreground("chrome.exe")),
        at(11_000, Tick),
        at(30_000, Tick),   // past grace, still nothing visible
        at(100_001, Tick),  // past drift
    ]);
    assert_matches!(s.state(), SessionState::Drifting { .. });
    let opened = effects.iter().find_map(as_open_interruption).unwrap();
    assert_eq!(opened.started_at_ms, 10_000, "must backdate to the switch, not the maturation");
    assert_eq!(pause_rewind_ms(&effects), 90_001);
}
```

Required cases, at minimum one test each: every row of the transition table in `architecture.md` section 7, plus every Detection row in section 9 above.

Add a proptest asserting an invariant that must hold for any random sequence of foreground changes and ticks: **the sum of thread elapsed time and all interruption durations never exceeds the wall time since the thread started.** This one property catches almost every double-counting bug.

### 10.2 Database tests

In-memory SQLite via `rusqlite::Connection::open_in_memory`. Test: each migration up, the partial unique index actually rejecting a second open thread, cascade behaviour on thread delete, fractional index renormalisation, and the corrupt-database recovery path using a deliberately truncated file in a `tempfile::TempDir`.

### 10.3 Platform tests

`platform::mock::MockPlatform` is a scriptable queue of foreground apps and idle values with a controllable clock. Every test above uses it. The real `win32.rs` gets only smoke tests behind `#[cfg(windows)]`, asserting that calls return without error on the test runner's own window, because its correctness is not unit-testable and belongs in manual QA.

### 10.4 Frontend tests

Vitest plus testing-library, with `src/ipc/mock.ts` swapped in for `bindings.ts`. Cover: elapsed formatting across hour boundaries and zero, the orb rendering the correct appearance for each session state, tab switching preserving unsent input in the dump box, promote flows calling the right command exactly once, and the ErrorBoundary rendering a readable message rather than a blank window.

### 10.5 Integration and manual QA

`tauri::test::mock_builder` for command-level tests that exercise commands against a temporary database.

E2E via `tauri-driver` plus `msedgedriver` is optional and flaky on transparent frameless windows. Do not block the build on it.

A manual QA checklist file lives at `docs/qa-checklist.md` and covers every Windows and input row in section 9, since none of those can be automated reliably. It must be run before every release.

### 10.6 Gates

Every commit must pass:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
cargo test
npm run lint
npm run typecheck
npm run test
```

## 11. Logging

`tracing` with a daily rolling file appender in `%APPDATA%\Trident\logs`, keeping seven days. Default level `info`, overridable by the `TRIDENT_LOG` environment variable.

What is always logged: startup with version and database path, every migration, every session state transition (one line, with from, to, reason), every platform call failure, every command error, hotkey registration results, and shutdown.

What is never logged: window titles (even when title recording is enabled), dump bodies, task titles, thread notes. Log identifiers instead. A log file the user might send for support must never contain their content.

## 12. Milestones, in build order

Each milestone must be independently runnable and independently testable. Do not begin one before the previous is green on all gates.

**M0, skeleton.** Project boots, four windows configured, tray icon, logging, panic hook, database created and migrated, health command returning real values. No features.

**M1, the orb does not steal focus.** Orb window with `WS_EX_NOACTIVATE`, rendering a static pill. Verification is manual and non-negotiable: hold a key in Notepad, click and drag the orb, lose no keystroke. Docking, work-area clamping, and multi-monitor handling land here.

**M2, the pure session machine.** `session/`, `platform::mock`, the full transition table, and the entire unit test suite including the proptest. No UI wiring at all. This is the milestone where the product is actually decided, and it is finished when the tests are, not when something is visible on screen.

**M3, the watcher and real Win32.** `platform::win32`, the 1000ms loop, sleep and clock-jump detection, idle and lock handling. The orb now changes appearance for real.

**M4, persistence and threads.** Thread and interruption repositories, commands, crash recovery, the Threads tab.

**M5, dump and capture.** Dump repository and commands, the capture window, global hotkeys, the fallback log. The 150ms budget is measured here, not assumed.

**M6, tasks and the three flows.** Task repository, ordering, and the five cross-tab flows from `product.md` section 6.

**M7, settings, onboarding, polish.** Settings window, validation, ignore list, theme, autostart, onboarding, empty states.

**M8, hardening and release.** Full edge case pass against section 9, performance budgets measured against section 12 of `architecture.md`, manual QA checklist, NSIS installer, portable build.

## 13. Definition of done for any change

1. Compiles with zero warnings under the clippy flags in 10.6.
2. Has tests for the new behaviour, including at least one failure path.
3. Adds no dependency not listed in section 2 of this document.
4. Contains no `unwrap`, `expect` or `panic!` outside tests.
5. Contains no hex colour, font size or duration literal outside `tokens.css`.
6. Does not move domain logic into the renderer.
7. Does not make the orb focusable.
8. Does not regress any performance budget.
9. Updates the relevant document if it changes a contract.
