# agents.md

Operating instructions for the coding agent building Trident. You are implementing a specification you did not write. `product.md`, `architecture.md` and `implementation-details.md` are the authority. This file tells you how to work.

## 0. Read order, every session

1. This file.
2. `implementation-details.md` section 12 to find the current milestone.
3. The sections of `architecture.md` that the current milestone touches.

Do not read the whole spec set into context for a one-file change. Do read the contract for anything you are about to modify.

## 1. Hard rules

Violating any of these means the change is wrong, regardless of whether it works.

1. **The orb window must never be able to take keyboard focus.** No `set_focus` on it, no focusable elements in its DOM, no `tabindex`, no `<input>` or `<button>`. If you think the orb needs to accept a keystroke, you have misread the design: open the panel instead.
2. **No `unwrap`, `expect` or `panic!` outside `#[cfg(test)]`.** The clippy gate denies them. `main.rs` startup may use `expect` only for the three things that make the app meaningless if absent (database open, log directory creation, main window creation), and each must show a dialog first.
3. **No domain logic in the renderer.** The frontend renders state and dispatches commands. It never decides when drift starts, whether a thread is paused, or what gets persisted. The one sanctioned exception is deriving the displayed elapsed value from the snapshot the backend sent.
4. **`session/` stays pure.** No Win32, no SQL, no `Instant::now()` inside it, no Tauri types. Time enters as a parameter. If you need the clock inside the machine, you have broken it.
5. **No new dependencies** beyond `implementation-details.md` section 2. If you believe one is needed, stop and ask, with the alternative you rejected and why.
6. **No hex colours, font sizes or durations outside `src/lib/tokens.css`.**
7. **Never edit `src/ipc/bindings.ts` by hand.** It is generated. Change the Rust type and regenerate.
8. **Never log user content.** No window titles, no dump bodies, no task titles, no thread notes. Log identifiers.
9. **No network calls.** There is no endpoint in v1 and the CSP forbids it.
10. **No features from the non-goals list** in `product.md` section 10, even if they seem like small additions. Especially not AI features, streaks or gamification.

## 2. Environment setup, do this yourself

Follow `implementation-details.md` section 1 exactly. Run the verification commands and paste their output before writing code. If `rustc` reports a `gnu` host triple, fix the toolchain before anything else. If the Visual Studio build tools install appears to succeed but linking later fails with `link.exe not found`, the C++ workload did not install; rerun with the `--override` argument shown.

Do not assume any of this is already present. Do not skip the WebView2 check because the machine is Windows 11.

## 3. The working loop

For every unit of work, in this order:

1. **State the contract.** Before writing code, write in your message which command, event, table or state transition you are implementing, and what its inputs and outputs are per the spec. If the spec is silent, see section 6.
2. **Write the test first** for anything in `session/`, `domain/` or `db/`. These layers are pure or near-pure, so a failing test is cheap and precise. For UI work, write the component and its test together.
3. **Implement the smallest thing that makes the test pass.**
4. **Run the full gate** from `implementation-details.md` section 10.6. Not a subset. Paste the result.
5. **Handle the failure paths** before declaring done. For every function you wrote, name the ways it can fail and show where each is handled. "It cannot fail" is only acceptable for pure functions over owned data.
6. **Update the docs** if you changed a contract.

Never move to the next unit with a red gate. Never commit with a red gate.

## 4. Testing standard

The gate is not "the happy path works". A change is untested until all of these exist where applicable:

- **The success case.**
- **At least one failure case per fallible call.** Database error, Win32 error, missing row, invalid input.
- **The boundary values.** Zero, one, the exact threshold, the threshold minus one millisecond, empty string, whitespace-only string, maximum length.
- **The concurrency case** where two things can happen in either order. Specifically: a drift maturing at the same tick as an idle timeout, a thread being closed while an interruption is open, a hotkey firing while the panel is animating.
- **The relevant rows from `implementation-details.md` section 9.** That list is a test checklist, not prose. When you finish a milestone, go through it and mark each row as covered by a named test or explicitly deferred to `docs/qa-checklist.md` with a reason.

Two specific things that must exist before M2 is called done:

- A table-driven test mirroring every row of the transition table in `architecture.md` section 7.
- The proptest asserting that thread elapsed time plus all interruption durations never exceeds wall time since the thread started, over randomly generated sequences of foreground switches and ticks.

When a test is hard to write, that is information about the design, not a reason to skip it. Say so and propose the restructuring.

## 5. Error handling standard

Every fallible path answers three questions in the code, not in a comment:

1. **Who finds out?** A user-visible toast, a `system:error` event, a log line, or nothing (only for expected non-errors like a null foreground window).
2. **What state is left behind?** No partial writes. Anything touching more than one table runs in a transaction.
3. **What happens next?** Retry, degrade, or stop. Watcher failures degrade: five consecutive failures of the same API disable detection and tell the user, rather than silently doing nothing. Database failures at startup stop, with a dialog naming the path.

Specific requirements you will otherwise get wrong:

- A null `GetForegroundWindow` is a normal condition, not an error. Do not log it at warn level or you will produce megabytes of noise.
- `ERROR_ACCESS_DENIED` from `OpenProcess` on an elevated app is expected. Treat that app as on-task.
- A hotkey conflict at startup must not prevent startup.
- The capture path writes to the fallback log before attempting the database write. Losing a captured thought is the worst failure this app has.

## 6. When the spec is ambiguous or wrong

Do not guess and do not silently invent. In order of preference:

1. If the answer is derivable from the principles in `product.md` section 4, derive it, state the derivation in your message, and proceed.
2. If it is a genuine gap that does not affect the architecture, choose the simplest option that respects the principles, implement it, and record the decision in a `## Decisions` section at the bottom of the relevant document.
3. If it affects a contract (data model, IPC, state machine, window model), stop and ask. Do not implement half of it while waiting.

If you believe a rule in these documents is wrong, say so explicitly with your reasoning, and wait. Do not route around it. A rule you find inconvenient is usually load-bearing for something you have not read yet.

## 7. Scope discipline

- Implement exactly the current milestone. Do not build ahead.
- Do not refactor code outside the files your task touches. If you see something wrong elsewhere, note it in your message and leave it alone.
- Do not add configuration options that nobody asked for. Every setting is a support burden and a test case.
- Do not add abstraction for a second platform, a second storage backend or a plugin system. The extension points in `architecture.md` section 15 already exist and are sufficient.
- If a task turns out to be much larger than it looked, stop and report the discovery rather than working for an hour and producing a large diff.

## 8. Commits and reporting

- One logical change per commit. Conventional commits: `feat(session): backdate interruption start to switch instant`.
- The commit body names the tests added.
- A milestone ends with a short report containing: what was built, the gate output, which rows of the section 9 edge case list are now covered, which are deferred and why, measured numbers against the performance budgets where relevant, and anything you found that contradicts the spec.

Do not claim a budget is met without measuring it. Idle RSS is read from Task Manager or `Get-Process trident | Select-Object PrivateMemorySize64` after five minutes with all windows created. Cold start and hotkey latency are measured with timestamps in the log, not estimated.

## 9. Things that will waste your time if you do not know them

Collected so you do not rediscover them the expensive way.

- Tauri v1 examples dominate the internet and do not compile against v2. The plugin names, the `WebviewWindowBuilder` path, the event API and the config schema all changed. If a snippet uses `tauri::Window` rather than `tauri::WebviewWindow`, or `app.get_window`, it is v1. Ignore it.
- Rust compile times dominate the iteration loop. Keep the Rust surface small, put logic in the pure modules where tests run fast, and avoid touching `main.rs` in a loop.
- A transparent frameless window with no background renders over arbitrary desktop content. Always set a backdrop.
- The panel closing on blur will fight you. The 120ms cancellable debounce in `implementation-details.md` section 5.1 is the fix. Do not solve it by disabling blur-to-close.
- `GetLastInputInfo` wraps at 49.7 days. The reconstruction is in section 4.3. This bug is invisible on a developer machine and universal on a user's.
- UWP apps all report as `ApplicationFrameHost.exe` without the child window unwrap in section 4.2, which quietly destroys the focus set.
- `panic = "abort"` in the release profile means a panic gives no unwind. The custom panic hook writing to the log file is the only diagnostic a user will ever have. Wire it in M0, not later.
- Windows will show a SmartScreen warning for an unsigned installer. That is expected in v1 and is not a bug to chase.

## 10. Definition of done

Repeated from `implementation-details.md` section 13 because it is the thing most often skipped:

1. Zero warnings under the clippy flags.
2. Tests for the new behaviour including at least one failure path.
3. No dependency outside the approved list.
4. No `unwrap`, `expect` or `panic!` outside tests.
5. No hex colour, font size or duration literal outside `tokens.css`.
6. No domain logic in the renderer.
7. The orb is not focusable.
8. No performance budget regressed.
9. Documents updated if a contract changed.
