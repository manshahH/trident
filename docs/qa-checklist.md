# Manual QA checklist

## M1 — orb focus and placement

Automated positioning tests cover the clamp, exact docking boundary, one-pixel-inside boundary, one-pixel-outside boundary, and missing-monitor fallback. The following require a Windows desktop session:

- [ ] **Section 9, row 14:** In Notepad, hold a letter key while clicking and dragging the orb. No character is lost and the orb never becomes the foreground keyboard target.
- [ ] **Section 9, row 17:** Dock the orb to a secondary monitor, disconnect that monitor, and restart Trident. The orb appears inside the primary monitor's work area.
- [ ] **Section 9, row 18:** Change display scale from 100% to 150%. The orb remains correctly sized and positioned.
- [ ] **Section 9, row 19:** Move or auto-hide the taskbar. The orb remains inside the usable work area rather than underneath it.

## Deferred Windows and input rows

- Rows 15 and 16 require panel and single-instance behaviour.
- Row 20 requires fullscreen detection in M3.
- Row 21 requires global hotkeys and Settings support in later milestones.
