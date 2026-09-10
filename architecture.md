# Trident, architecture

Companion to `product.md`. Read that first. This document defines structure and contracts. Concrete code-level instructions live in `implementation-details.md`.

## 1. Technology decisions, with the reasoning

| Layer | Choice | Why, and what was rejected |
| --- | --- | --- |
| Shell | Tauri 2.x | Rust core plus the system WebView2. Release binary in the low tens of MB, idle RSS well under the 80MB budget. Electron rejected: a 150MB+ floating dot contradicts the product's core claim. Native WinUI 3 rejected: far weaker agent-authoring ergonomics and slower iteration. |
| UI | React 19 + TypeScript (strict) + Vite | Largest, best-understood surface for agent authoring. |
| Styling | Tailwind CSS v4 | Design tokens expressed once as CSS variables, consumed as utilities. |
| Client state | Zustand | Small, no boilerplate, testable outside React. Redux rejected as overweight. |
| Storage | SQLite via `rusqlite` (bundled feature) | Relational queries over thread history are the point. `tauri-plugin-sql` rejected: it puts SQL in the frontend, which breaks the rule that the frontend owns no domain logic. |
| Win32 access | `windows` crate | Direct, typed bindings. |
| IPC types | `tauri-specta` + `specta` | Rust types generate the TypeScript client. Prevents the two sides drifting. |
| Errors | `thiserror` in core, serialisable `AppError` at the boundary | |
| Logging | `tracing` + `tracing-appender` daily rolling file | |
| Locks | `parking_lot` | No lock poisoning, which removes a whole class of unwrap. |
| IDs | UUID v7 (`uuid` crate, `v7` feature) | Time-sortable, no coordination. |
| Window effects | `window-vibrancy` | Mica or acrylic on Windows 11, graceful fallback to solid on Windows 10. |

Pin exact versions in the lockfiles. Do not add a dependency not listed in `implementation-details.md` section 2 without an explicit decision recorded in this file.

## 2. Process and window model

One process. Four WebView windows, all served from the same Vite bundle, routed by window label.

```
trident.exe
├── window "orb"      frameless, transparent, always-on-top, NON-ACTIVATING, skip-taskbar
├── window "panel"    frameless, transparent, focusable, hides on blur
├── window "capture"  frameless, transparent, focusable, hides on blur or Escape
├── window "settings" decorated, normal, created lazily on first open
└── tray icon
```

**The single most important structural decision in this app**: the orb and the panel are separate windows because their window styles are contradictory. The orb must carry `WS_EX_NOACTIVATE` so it can never take keyboard focus from the app the user is typing in. The panel must be able to take focus so its text inputs work. One window cannot be both, and toggling the extended style at runtime is unreliable while a WebView2 is hosted. Any proposal to merge them must be rejected.

The renderer decides which UI to mount by reading `getCurrentWindow().label` at boot. Each window mounts exactly one root component and shares only the generated IPC client and the token stylesheet.

### 2.1 Threads (OS threads, not product threads)

- Main thread: Tauri event loop, all window and tray operations.
- Watcher task: a dedicated OS thread (not a tokio task) running a fixed 1000ms loop doing the Win32 foreground and idle queries, because these calls are synchronous and must not sit on the async runtime.
- Tokio runtime: command handlers, database writes.

The watcher never touches windows directly. It emits domain events to the session machine, which emits state changes, which the main thread renders. This layering keeps every Win32 concern out of the domain logic.

## 3. Layering

Dependencies point downward only. A layer may never import from a layer above it.

```
┌───────────────────────────────────────────────┐
│ renderer (React)                              │  no domain logic, no SQL, no timers of record
├───────────────────────────────────────────────┤
│ commands/  events/                            │  thin: parse, call, map error, serialise
├───────────────────────────────────────────────┤
│ session/   (the state machine)                │  PURE. no Win32, no SQL, no clock, no Tauri
├───────────────────────────────────────────────┤
│ domain/    (thread, dump, task, settings)     │  business rules over repositories
├───────────────────────────────────────────────┤
│ db/        repositories, migrations           │
│ platform/  trait + win32 impl + mock impl     │
└───────────────────────────────────────────────┘
```

`session/` being pure is what makes this testable. It receives ticks and events as values, including the current time as a parameter, and returns a new state plus a list of effects. It never calls the OS or the database. Every drift, grace, idle and resume rule is exercised in unit tests with a fake clock and a scripted sequence of foreground apps.

## 4. Rust module tree

```
src-tauri/src/
├── main.rs                  builder, plugin registration, window creation, tray, panic hook
├── state.rs                 AppState: Arc<Db>, Arc<RwLock<Session>>, Arc<Settings>, handles
├── error.rs                 AppError, Result alias, serialisation for the boundary
├── logging.rs               tracing setup, rolling file, panic hook wiring
├── config.rs                paths, defaults, first-run bootstrap
│
├── platform/
│   ├── mod.rs               trait Platform (foreground_app, idle_millis, is_locked,
│   │                        is_fullscreen_foreground, set_no_activate, work_area, monitors)
│   ├── win32.rs             the only file containing `unsafe`
│   └── mock.rs              scriptable fake used by every test and by non-Windows dev builds
│
├── watcher.rs               1000ms loop, calls Platform, feeds Session, detects clock jumps
│
├── session/
│   ├── mod.rs               Session struct, step(), handle(), pure
│   ├── state.rs             SessionState enum and transition table
│   ├── effects.rs           Effect enum returned by the machine
│   └── tests/               table-driven transition tests
│
├── domain/
│   ├── thread.rs            start, pause, resume, close, elapsed computation, focus set
│   ├── interruption.rs      open, close, promote, dismiss, mark as on-task
│   ├── dump.rs
│   ├── task.rs
│   └── settings.rs          typed settings with defaults and validation
│
├── db/
│   ├── mod.rs               connection setup, pragmas, transaction helper
│   ├── migrations.rs        ordered, forward-only, each with an up() and a test
│   ├── models.rs            row structs
│   └── repo/                thread_repo, interruption_repo, dump_repo, task_repo, settings_repo
│
├── commands/
│   ├── mod.rs               registration, specta collection
│   ├── threads.rs  dumps.rs  tasks.rs  settings.rs  system.rs
│
├── windows/
│   ├── orb.rs               creation, no-activate application, docking, opacity
│   ├── panel.rs             creation, anchoring to orb edge, blur handling
│   ├── capture.rs
│   └── positioning.rs       monitor maths, work area clamping, DPI handling
│
├── hotkeys.rs               registration, conflict detection, rebinding
├── tray.rs
└── events.rs                event name constants and payload structs
```

## 5. Frontend tree

```
src/
├── main.tsx                 reads window label, mounts the right root
├── ipc/
│   ├── bindings.ts          GENERATED by tauri-specta. never edit by hand.
│   ├── client.ts            thin wrapper adding error mapping and dev logging
│   └── mock.ts              in-memory implementation of the same interface, for tests and Storybook
├── stores/
│   ├── sessionStore.ts      mirrors backend session state, subscribes to events
│   ├── dumpStore.ts  taskStore.ts  settingsStore.ts
├── windows/
│   ├── orb/OrbRoot.tsx      + OrbPill, OrbDot, useElapsed
│   ├── panel/PanelRoot.tsx  + ThreadsTab, DumpTab, TasksTab
│   ├── capture/CaptureRoot.tsx
│   └── settings/SettingsRoot.tsx
├── components/              Button, Row, TextField, Tabs, EmptyState
├── lib/
│   ├── time.ts              elapsed formatting, tabular digits
│   ├── tokens.css           the design tokens, single source
│   └── errors.tsx           ErrorBoundary + toast surface
└── test/                    vitest setup, IPC mock wiring
```

**The frontend never computes a domain decision.** It does not decide when drift starts, it does not decide whether a thread is paused, it does not persist anything. It renders state and dispatches commands.

**The one exception, deliberately made**: the ticking timer. The backend does not emit a per-second tick, because a 1Hz IPC message with three visible windows is wasteful and drifts under load. Instead the backend sends, on state change only, `{ state, started_at, accumulated_ms, last_resumed_at }`, and the renderer derives the displayed elapsed value locally on a `setInterval` that only runs while that window is visible. Correctness stays in Rust, animation stays in JS.

## 6. Data model

SQLite, WAL mode, foreign keys on. All timestamps are integer Unix milliseconds in UTC. All durations are integer milliseconds.

```sql
CREATE TABLE threads (
  id                TEXT PRIMARY KEY,
  title             TEXT NOT NULL CHECK (length(trim(title)) > 0),
  note              TEXT NOT NULL DEFAULT '',
  handoff           TEXT NOT NULL DEFAULT '',
  state             TEXT NOT NULL CHECK (state IN ('active','paused','away','closed')),
  created_at        INTEGER NOT NULL,
  started_at        INTEGER NOT NULL,
  closed_at         INTEGER,
  accumulated_ms    INTEGER NOT NULL DEFAULT 0,
  last_resumed_at   INTEGER,
  source_task_id    TEXT REFERENCES tasks(id) ON DELETE SET NULL
);

-- At most one thread may be non-closed at any time. Enforced by the database,
-- not by application code, because this is the invariant everything else assumes.
CREATE UNIQUE INDEX idx_threads_single_open
  ON threads ((1)) WHERE state != 'closed';

CREATE INDEX idx_threads_started ON threads (started_at DESC);

CREATE TABLE focus_set (
  thread_id     TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
  process_key   TEXT NOT NULL,          -- lowercased executable file name, e.g. "code.exe"
  added_at      INTEGER NOT NULL,
  source        TEXT NOT NULL CHECK (source IN ('seed','user')),
  PRIMARY KEY (thread_id, process_key)
);

CREATE TABLE interruptions (
  id            TEXT PRIMARY KEY,
  thread_id     TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
  process_key   TEXT NOT NULL,
  app_name      TEXT NOT NULL,          -- friendly name from version info, falls back to process_key
  window_title  TEXT,                   -- NULL unless the user enabled title recording
  started_at    INTEGER NOT NULL,
  ended_at      INTEGER,
  duration_ms   INTEGER,
  status        TEXT NOT NULL CHECK (status IN ('open','scratched','dismissed','promoted')),
  dump_id       TEXT REFERENCES dumps(id) ON DELETE SET NULL
);

CREATE INDEX idx_interruptions_thread ON interruptions (thread_id, started_at DESC);

CREATE TABLE dumps (
  id            TEXT PRIMARY KEY,
  body          TEXT NOT NULL CHECK (length(trim(body)) > 0),
  origin        TEXT NOT NULL CHECK (origin IN ('capture','panel','interruption','handoff')),
  created_at    INTEGER NOT NULL,
  archived_at   INTEGER,
  task_id       TEXT REFERENCES tasks(id) ON DELETE SET NULL
);

CREATE INDEX idx_dumps_active ON dumps (created_at DESC) WHERE archived_at IS NULL;

CREATE TABLE tasks (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL CHECK (length(trim(title)) > 0),
  notes         TEXT NOT NULL DEFAULT '',
  created_at    INTEGER NOT NULL,
  done_at       INTEGER,
  sort_key      REAL NOT NULL,          -- fractional index, see implementation-details 7.3
  dump_id       TEXT REFERENCES dumps(id) ON DELETE SET NULL
);

CREATE INDEX idx_tasks_order ON tasks (done_at, sort_key);

CREATE TABLE app_ignores (
  process_key   TEXT PRIMARY KEY,
  app_name      TEXT NOT NULL,
  added_at      INTEGER NOT NULL
);

CREATE TABLE settings (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL           -- JSON encoded
);

CREATE TABLE schema_migrations (
  version       INTEGER PRIMARY KEY,
  applied_at    INTEGER NOT NULL
);
```

Notes that matter:

- `process_key` is the lowercased executable file name, not the full path, so a user moving an install does not break their focus set. The full path is never stored, which also removes a small privacy surface.
- Elapsed time is never stored as a running total that gets incremented. It is `accumulated_ms + (now - last_resumed_at)` when running, and `accumulated_ms` otherwise. This makes crashes and clock jumps non-corrupting.
- Deleting a thread cascades to its focus set and interruptions. Deleting a dump or task only nulls the references, so history survives.

## 7. The session state machine

`session/` is pure. Its whole contract:

```rust
pub enum Input {
    Tick { now: Instant, wall: i64 },
    Foreground { process_key: String, app_name: String, title: Option<String> },
    NoForeground,                 // lock screen, desktop switch, secure desktop
    IdleFor { millis: u64 },
    ClockJump { by_ms: i64 },     // wall clock moved without matching monotonic movement
    Command(SessionCommand),      // start, pause, resume, close, mark_on_task, ignore_app
}

pub enum SessionState {
    Idle,
    Running   { thread_id: Uuid, since: Instant },
    Candidate { thread_id: Uuid, process_key: String, since: Instant },  // outside focus set, within grace
    Drifting  { thread_id: Uuid, interruption_id: Uuid, since: Instant },
    Away      { thread_id: Uuid, since: Instant, reason: AwayReason },
}

pub enum Effect {
    PersistThreadPause { thread_id: Uuid, at: i64, rewind_ms: u64 },
    PersistThreadResume { thread_id: Uuid, at: i64 },
    OpenInterruption { .. },
    CloseInterruption { id: Uuid, at: i64 },
    EmitState(SessionSnapshot),
    SetOrbAppearance(OrbAppearance),
    Log(LogLine),
}

impl Session {
    pub fn handle(&mut self, input: Input, cfg: &DetectionConfig) -> Vec<Effect>;
}
```

Transition table, which the tests mirror one to one:

| From | Input | Condition | To | Effects |
| --- | --- | --- | --- | --- |
| Idle | Command(Start) | | Running | PersistThreadResume, EmitState |
| Running | Foreground(p) | p in focus set or ignore list | Running | none |
| Running | Foreground(p) | p not in focus set | Candidate | none, nothing visible yet |
| Candidate | Foreground(p') | p' in focus set | Running | none, candidate discarded silently |
| Candidate | Foreground(p') | p' also outside, different app | Candidate(p') | restart the grace clock |
| Candidate | Tick | dwell < grace | Candidate | none |
| Candidate | Tick | grace <= dwell < drift | Candidate | none |
| Candidate | Tick | dwell >= drift | Drifting | OpenInterruption backdated to the switch instant, PersistThreadPause with rewind, SetOrbAppearance(Drifted), EmitState |
| Drifting | Foreground(p) | p in focus set | Running | CloseInterruption, PersistThreadResume, SetOrbAppearance(Running), EmitState |
| Drifting | Foreground(p') | p' outside focus set | Drifting | extend the same interruption if the same app, otherwise close it and open a new one |
| Running / Candidate / Drifting | IdleFor(x) | x >= idle threshold | Away(Idle) | close any open interruption, PersistThreadPause with rewind to last input, EmitState |
| Running / Candidate / Drifting | NoForeground | sustained 2 ticks | Away(Locked) | as above |
| Away | Foreground(p) or IdleFor(< threshold) | | Running or Candidate, by focus set | PersistThreadResume, EmitState |
| any | ClockJump(by) | abs(by) > 5000ms | same state | recompute wall timestamps from monotonic, Log, never alter accumulated_ms |
| any with thread | Command(Close) | | Idle | close open interruption, PersistThreadPause, close thread, EmitState |

**Backdating rule.** When a candidate matures into drift at t, the interruption's `started_at` is the moment of the app switch, not t, and the thread's paused time is rewound to that same moment. Otherwise every drift would silently credit the user with the grace and threshold period as focused work. This rule is why grace and threshold can be tuned freely without distorting the data.

## 8. IPC contract

All commands return `Result<T, AppError>`. All are generated into `src/ipc/bindings.ts` by `tauri-specta`. Names are snake_case in Rust and camelCase in TypeScript.

**Threads**
- `thread_start(title: String, note: Option<String>, source_task_id: Option<Uuid>) -> ThreadDto`
- `thread_active() -> Option<ThreadDto>`
- `thread_set_note(id, note) -> ()`
- `thread_pause(id) -> ThreadDto`
- `thread_resume(id) -> ThreadDto`
- `thread_close(id, handoff: Option<String>) -> ()`
- `thread_history(from: i64, to: i64) -> Vec<ThreadDto>`
- `thread_suggestions() -> Vec<TaskDto>` (top three open tasks)

**Interruptions**
- `interruption_list(thread_id) -> Vec<InterruptionDto>`
- `interruption_mark_on_task(id) -> ()` (adds to focus set, deletes the row)
- `interruption_to_dump(id) -> DumpDto`
- `interruption_dismiss(id) -> ()`
- `interruption_ignore_app(id) -> ()` (adds to the global ignore list)

**Dumps**
- `dump_add(body, origin) -> DumpDto`
- `dump_list(include_archived: bool, limit: u32, before: Option<i64>) -> Vec<DumpDto>`
- `dump_archive(id) -> ()`
- `dump_delete(id) -> ()`
- `dump_to_task(id) -> TaskDto`

**Tasks**
- `task_add(title) -> TaskDto`
- `task_list(include_done: bool) -> Vec<TaskDto>`
- `task_set_done(id, done: bool) -> TaskDto`
- `task_reorder(id, before: Option<Uuid>, after: Option<Uuid>) -> TaskDto`
- `task_delete(id) -> ()`
- `task_update(id, title, notes) -> TaskDto`

**System**
- `settings_all() -> SettingsDto`
- `settings_set(patch: SettingsPatch) -> SettingsDto`
- `hotkeys_rebind(action, accelerator) -> HotkeyResult`
- `orb_begin_drag() -> ()` and `orb_dropped(x, y) -> DockDto`
- `panel_toggle() -> ()`, `panel_close() -> ()`
- `capture_close() -> ()`
- `detection_pause_for(minutes: u32) -> ()`
- `data_export() -> PathBuf`
- `data_wipe(confirmation: String) -> ()`
- `health() -> HealthDto` (db ok, watcher last tick age, platform errors, version)

**Events, Rust to renderer**
- `session:state` payload `SessionSnapshot { state, thread, elapsed_base, drift }` (emitted only on change)
- `session:interruption` payload `InterruptionDto` (opened or closed)
- `dump:added`, `task:changed`, `settings:changed`
- `system:error` payload `{ code, message, recoverable }` for surfacing non-fatal failures
- `orb:appearance` payload `OrbAppearance`

Every event has exactly one emitting site in Rust. Grep for the constant in `events.rs` to find it.

## 9. Error model

```rust
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(tag = "kind", content = "detail")]
pub enum AppError {
    #[error("database error: {0}")] Db(String),
    #[error("migration failed at version {version}: {message}")] Migration { version: i64, message: String },
    #[error("not found: {entity} {id}")] NotFound { entity: String, id: String },
    #[error("invalid state: {0}")] InvalidState(String),
    #[error("validation: {field}: {message}")] Validation { field: String, message: String },
    #[error("platform call failed: {api} ({code})")] Platform { api: String, code: i32 },
    #[error("hotkey unavailable: {0}")] HotkeyConflict(String),
    #[error("window {0} not found")] WindowMissing(String),
    #[error("io: {0}")] Io(String),
    #[error("internal: {0}")] Internal(String),
}
```

Rules:
1. No `unwrap`, `expect` or `panic!` anywhere outside `#[cfg(test)]` and `main.rs` startup. Enforced by clippy at deny level.
2. Every command returns `Result`. Every command logs its error before returning it.
3. The watcher can never terminate the app. A failed platform call increments a counter, logs at warn, and the loop continues. Five consecutive failures of the same API downgrade detection to disabled and emit `system:error` with `recoverable: true`, so the user is told that drift detection stopped rather than silently getting nothing.
4. A database failure at startup is the only fatal error, and it fails loudly with a dialog naming the database path, then exits.
5. A panic anywhere is caught by a custom panic hook that writes the backtrace to the log file and shows a dialog with the log path, rather than vanishing the tray icon with no trace.
6. The renderer wraps each window root in an `ErrorBoundary`. Command errors surface as a small non-blocking toast inside the panel and are never thrown into the void.

## 10. Threading and concurrency invariants

- The database is accessed through a single `Connection` behind a `parking_lot::Mutex`, with `busy_timeout` set. Concurrency here is trivially low, and a pool would add failure modes for no benefit.
- The session machine sits behind a `parking_lot::RwLock`. The watcher takes the write lock for the duration of one `handle()` call, which is microseconds and allocation-light.
- Effects returned by the machine are executed by the caller, outside the lock. Never execute an effect while holding the session lock, or a database stall will freeze the watcher.
- All window operations happen on the main thread via `app_handle.run_on_main_thread`.

## 11. Configuration and file locations

```
%APPDATA%\Trident\
├── trident.db          SQLite, plus -wal and -shm
├── settings.json       written by tauri-plugin-store, mirrors the settings table for fast boot
├── logs\trident.YYYY-MM-DD.log     rolling, keep 7
└── exports\            user-triggered JSON exports
```

Defaults, all overridable in settings:

| Key | Default | Bounds |
| --- | --- | --- |
| `grace_ms` | 20000 | 5000 to 120000 |
| `drift_ms` | 90000 | 30000 to 900000, must be > grace |
| `idle_ms` | 300000 | 60000 to 3600000 |
| `poll_ms` | 1000 | fixed in v1 |
| `record_window_titles` | false | |
| `hide_over_fullscreen` | true | |
| `orb_dim_after_ms` | 8000 | |
| `theme` | "dark" | dark, light, system |
| `start_with_windows` | false | |
| `hotkey.capture` | Ctrl+Alt+D | |
| `hotkey.panel` | Ctrl+Alt+T | |
| `hotkey.thread` | Ctrl+Alt+S | |

Settings are validated on write. An out-of-bounds value is rejected with `AppError::Validation`, never clamped silently.

## 12. Performance budget, enforced in review

| Metric | Budget |
| --- | --- |
| Idle private working set, all windows created | under 80MB |
| Idle CPU, averaged over 60s | under 0.5% of one core |
| Installer size | under 15MB |
| Cold start to orb visible | under 1200ms |
| Hotkey press to capture caret ready | under 150ms |
| Panel open to painted | under 200ms |
| One watcher tick | under 3ms |

If a change breaks a budget, the change is wrong, not the budget.

## 13. Build and release

- Dev: `npm run tauri dev`.
- Release: `npm run tauri build`, producing an NSIS installer and a portable exe.
- Bundle identifier `dev.manshah.trident`. Product name `Trident`.
- Updater is wired but disabled in v1 (no endpoint). The plugin stays registered so that turning it on later needs no restructuring.
- Code signing is a v1.1 item. Until then the installer documents the SmartScreen warning.

## 14. Security and privacy posture

- No network calls in v1. The CSP in `tauri.conf.json` denies everything except `self`.
- `withGlobalTauri` is false. The renderer reaches Rust only through generated bindings.
- Window titles are not read unless the setting is on. The Win32 call is not made at all when it is off, so nothing sensitive ever enters memory.
- Clipboard, keystrokes and window contents are never accessed. There is no code path that could.
- `data_wipe` deletes the database and the log directory and restarts the app clean.

## 15. Extension points, deliberately left open

Do not build these, but do not close them off either.

1. `Platform` is a trait, so a macOS implementation later is an added file, not a refactor.
2. Every `Effect` flows through one executor, so adding a sync or telemetry sink later is one match arm.
3. The settings table is generic key-value, so a licence key later needs no migration of shape.
4. `session/` is pure, so replacing the heuristic drift rule with a learned one later touches one module.
