# 64. Small polish: key hints in tooltips, Ctrl+A, Move to Deck…, tables

- Status: Accepted
- Date: 2026-11-02
- Amends: ADR 0045 (the key hints under the review page's buttons are gone)
- Refers to: SPEC §4.5, §4.6; `docs/briefs/phase-3a.md` (slice 1); ADR 0036 (All Cards), ADR 0037 (suspend during review), ADR 0058 (phone and touch)

## Context

Four small items of the phase 3a brief: #105, #106, #102 and the table width of PR #32.

## Decision

### Key hints leave the review page (#105)

ADR 0045 put a dimmed caption ("Space", "1", "2") under each answer button. The maintainer finds them cluttered. They are gone, with the breakpoint setters that hid them on short windows. Each button (`reveal_button`, `fail_button`, `pass_button`) says its key in its tooltip instead: "Show Answer (Space)", "Fail (1)", "Pass (2)". The format is one translatable string with named placeholders, `{name} ({key})`, filled by the pure `key_tooltip`; the key's text is how GTK labels the first trigger of the page's `KEYS` table bound to that key (`gtk_accelerator_get_label`, "Space" in the user's language), so the tooltip cannot name a key the page does not bind. The accessible descriptions ("Press 2 or Space") stay, and the Keyboard Shortcuts dialog is unchanged. ADR 0045 said the tooltips were dropped because the key was on screen; now that it is not, they come back. A touch user sees no tooltip, and needs none: grading is by the buttons.

The store screenshots in `data/screenshots/` still show the hints. They are retaken with the release.
