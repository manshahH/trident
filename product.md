# Trident, product specification

Version 1.0. Owner: Manshah Hussain. Platform: Windows 10 (1809+) and Windows 11, x64.

## 1. One paragraph

Trident is a small always-on-top desktop companion for people who lose the thread of what they were doing. It lives as a floating pill on top of every window. When you are working on something you declare it as a thread, and the pill shows that thread's name and elapsed time without you opening anything. When you drift into another app for long enough that it stops being a quick look, Trident notices by itself, logs what pulled you away, and changes colour. When you come back, the pill tells you what you were doing and what stole you. Clicking it opens a small panel with three tabs that feed each other: Threads, Dump, Tasks.

## 2. Who it is for

Primary: ADHD knowledge workers and developers on Windows who work with many apps open, get pulled away constantly, and lose the reconstruction time on return. They are comfortable granting a desktop app permission to see which app is in the foreground.

Secondary: anyone who context switches heavily (support engineers, ops, freelancers juggling clients).

Explicitly not for: mobile-first users, team collaboration, anyone wanting a full project manager.

## 3. The problem, stated precisely

Existing tools help you decide what to do. None of them help you return to what you were already doing. The gap between an interruption and a resumption is unowned by software. Trident owns exactly that gap.

Three symptoms it targets:

1. The user cannot recall what they were mid-way through after an interruption.
2. The user does not notice that they have drifted until many minutes have passed.
3. New thoughts arriving mid-task force a choice between losing the thought and losing the task. Trident removes that choice by making capture cost one keystroke.

## 4. Core principles, binding on all future decisions

1. **The collapsed state must always be informative.** The pill shows the live thread name and timer without a click. If a change would make the collapsed pill a bare icon, reject it.
2. **The app never steals focus.** Not on start, not on drift, not on nudge. The orb window is non-activating at the OS level. Nothing about this app may interrupt typing.
3. **Detection over self-reporting.** Any state the OS can observe (which app is foreground, whether the user is idle) is observed, never asked for. The user should never have to tell Trident they got distracted.
4. **No guilt mechanics.** No streaks, no red overdue badges, no scoring, no "you failed" language. Drift is displayed as information, in neutral wording, at low visual weight.
5. **Local only.** No account, no server, no telemetry in v1. All data in a local SQLite file the user can open, export or delete.
6. **Weight is a feature.** Idle RAM target under 80MB, installer under 15MB, CPU under 0.5% at idle. These are acceptance criteria, not aspirations.
7. **Two accent colours, no more.** Green for a live thread, coral for drift. Nothing else in the UI carries colour.

## 5. Surfaces

### 5.1 Orb (the pill)

A frameless, transparent, always-on-top, non-activating window.

States:

| State | Appearance | Trigger |
| --- | --- | --- |
| Idle | 40px circle, grey dot | No active thread |
| Running | Pill, green dot, thread title, elapsed timer counting up | Thread active, user in a focus-set app |
| Drifted | Pill, coral dot, thread title, "paused Nm" | User has dwelt in a non focus-set app past the drift threshold |
| Away | Pill, grey dot, thread title, "away" | System idle or locked, thread auto-paused |
| Hidden | Not rendered | Fullscreen app in foreground, or user hid it via tray |

Behaviours:
- Draggable by the whole surface. Magnetically docks to the nearest screen edge on release.
- Remembers position per monitor. On monitor removal, moves to primary and clamps inside the work area.
- Fades to 40% opacity after 8 seconds without pointer proximity. Returns to full opacity on hover.
- Left click toggles the panel. Right click opens a small context menu (start thread, quick dump, hide for an hour, settings, quit).
- No animation except on state change. No pulsing, no breathing, no idle motion, ever.

### 5.2 Panel

A separate focusable frameless window, 360 by 480 logical pixels, anchored to the orb's docked edge so it never crosses the user's work. Opens on orb click or global hotkey. Closes on Escape, on click outside, or on a second hotkey press.

Three tabs, in this order, because this order encodes the priority: **Threads**, **Dump**, **Tasks**.

**Threads tab.** Shows the active thread: title, elapsed time, and the user's own free-text "where I was" note (editable inline, autosaved). Below it, a list headed "Pulled away by", each row being one logged interruption: app name, duration, and status. Interruptions that ended are struck through. One that is still open is shown in normal weight. Row actions: mark as part of this thread (adds the app to the focus set and deletes the row), send to dump (turns it into a dump entry so it is not lost), dismiss. Footer actions: Resume, Close thread. Below the active thread, a collapsed history of today's closed threads.

If there is no active thread, the tab shows a single text input: "What are you working on?" plus, underneath, up to three suggestions taken from the top of the task list.

**Dump tab.** A single always-focused text box at the top, then a reverse chronological list of captured lines. Entering text and pressing Enter saves and clears. Each line has two actions: promote to task, archive. Nothing is ever auto-deleted. Nothing is auto-sorted, auto-categorised or sent to an AI in v1.

**Tasks tab.** A flat, manually ordered list. Add, toggle done, reorder by drag, delete. Each task has one extra action that no other task app has: **start thread**, which creates an active thread from that task in one click and closes the panel. Completed tasks fall to a collapsed "done today" group.

### 5.3 Quick capture bar

A third window, 420 by 44, summoned by a global hotkey from anywhere. Single line. Enter saves to the dump and closes. Escape closes without saving. This is the fastest path in the app and must stay under 150ms from keypress to caret.

### 5.4 Tray icon

Right click menu: start or stop thread, open panel, quick dump, pause detection for 1 hour, settings, open data folder, check for updates, quit. Left click toggles the panel.

### 5.5 Settings window

A normal decorated window, opened from the tray. Sections: General (start with Windows, theme, hide over fullscreen), Detection (drift threshold, grace period, idle threshold, ignored apps list, record window titles on or off), Hotkeys (three rebindable bindings with conflict reporting), Data (open folder, export JSON, delete all data).

## 6. The three tabs feeding each other

These flows are the reason the three tabs exist together rather than as three apps. Each must be reachable in one action.

1. Dump line to task: promote.
2. Task to thread: start thread.
3. Thread to dump: when an interruption is logged, one click sends it to the dump so the distracting thought is preserved rather than followed.
4. Quick capture to dump: global hotkey, from inside any app, without leaving the current thread.
5. Thread close to dump: on closing a thread the user may leave a one-line handoff note, which is stored on the thread and shown if a thread with the same title is started later.

## 7. Drift detection, product-level rules

Trident watches which application is in the foreground. It does not read window contents, keystrokes or clipboard.

- Each thread carries a **focus set** of applications considered on-task. It is seeded with whatever app was in the foreground when the thread started, plus any app the user marks as part of the thread.
- Switching into an app outside the focus set starts a candidate interruption, but nothing is shown or recorded until the user has stayed there past the **grace period** (default 20 seconds). Quick lookups are invisible by design.
- If the dwell passes the **drift threshold** (default 90 seconds), the interruption is recorded and the orb turns coral. The thread's timer pauses.
- Returning to any focus-set app closes the interruption, records its duration, and resumes the timer.
- If the system is idle past the **idle threshold** (default 5 minutes) or the workstation is locked, the thread pauses and the elapsed time is rewound to the last observed input. This is "away", not drift, and is never displayed as a distraction.
- The user can add any app to a global ignore list, in which case it never triggers drift for any thread.
- Recording of window titles is off by default. When off, only the application name is stored.

## 8. Onboarding

Four screens maximum, skippable, under 45 seconds:
1. What it is, in one sentence, with the orb visible on screen already.
2. Ask for the one permission-shaped thing: explain that Trident sees which app is in the foreground, that nothing leaves the machine, and offer the window title toggle.
3. Set the three hotkeys (offer defaults, one click to accept).
4. Start your first thread. The app does not proceed until a thread exists, because a user who never starts a thread has not used the product.

No account, no email, no paywall in the first session.

## 9. Success criteria for v1

The only metric that matters is retention of the orb on screen.

- Primary: percentage of installers with the app running on day 14. Target 25% for a first release to a warm audience.
- Secondary: median threads started per active day (target 3+), median interruptions logged per thread (target 1+, this proves the detection is firing), and percentage of interruptions that are corrected via "part of this thread" (target under 20%, above that means the detection is too noisy).
- Anti-metric: if users are opening the panel more than about 15 times a day, the collapsed state is not informative enough and the design has failed.

Instrumentation for these is local only in v1. The app writes a local counters file the user may voluntarily export and send.

## 10. Non-goals for v1

Explicitly out of scope, and any agent proposing them should be refused:

- AI features of any kind, including sorting the dump, generating tasks or summarising a day.
- Cloud sync, accounts, sharing, teams.
- macOS or Linux builds.
- Calendar or third party integrations.
- Pomodoro timers, streaks, gamification, XP, badges.
- Website or content blocking.
- Nested tasks, projects, tags, due dates, recurrence.
- Mobile companion.

## 11. Monetisation, planned but not built in v1

v1 ships free and unlicensed. The architecture must not preclude a later licence check: keep all data local, keep a settings table, and do not scatter feature checks. Expected shape later is a one-time purchase with a free tier limited by thread history length, not by core function. Pricing research puts comparable tools between $4 and $12 per month, and a perpetual licence around $30 to $60 is more appropriate for a local-only utility.

## 12. Glossary

- **Thread**: a declared unit of work in progress, with a title, an optional note, a focus set, and an elapsed timer.
- **Interruption**: a recorded period spent in an app outside the active thread's focus set, past the grace period.
- **Focus set**: the set of applications considered on-task for a given thread.
- **Drift**: the state entered when an interruption passes the drift threshold.
- **Away**: paused because the user is idle or the workstation is locked, distinct from drift.
- **Dump**: the unordered capture list.
- **Orb**: the always-on-top pill window.
- **Panel**: the three-tab window.
