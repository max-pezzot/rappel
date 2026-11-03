# 71. Phase 2's leftovers: narrow headers, Add Tags, big imports and exports

- Status: Accepted
- Date: 2026-11-03
- Refers to: SPEC §4.1, §4.6; `docs/briefs/phase-3a.md` (slice 7); `docs/briefs/phase-2-report.md` ("Known limits from the PRs"); ADR 0058 (toasts and breakpoints at 360 px), ADR 0064 (small polish), ADR 0065 (shared selection mode), ADR 0068 (archive), ADR 0070 (practice)

## Context

Phase 2 listed what it left: All Cards' filters cut in French, headers cut at 360 px when minimise and maximise are shown, Add Tags turning text into a pill when the window shrinks, a 256 MiB file refused only after 21 s on a worker with no Cancel, and exports above 2 GiB of media that cannot be imported again. This ADR records the decisions that fixed them, one section per item.

## 1. Filters and headers at 360 px

**Filters.** The three (four, with archived decks) drop-downs of All Cards are in an `AdwWrapBox` (libadwaita 1.7, the version Cargo.lock builds against has it; `justify: fill` with `justify-last-line`): each is as wide as its text, and a line breaks where they do not fit side by side. French at 360 px takes three lines ("Tous les paquets" / "Toutes les étiquettes" and "Tous les états" / "Archivés masqués"), English two. Nothing is cut except a single entry wider than the line (a long deck name), which `ui::dropdown::shrinkable` still ends in "…" and whose tests stay. Two equal columns or a popover were rejected: the first still cuts "Toutes les étiquettes", the second hides the filters behind a tap. The `FilterLayout` of ADR 0068 is removed.

**Deck page.** Select Cards leaves the header under 400 sp for the bottom bar, next to Study (its slot, a `GtkBox`, is what the breakpoint hides: selection mode shows and hides the button itself, so a setter on the button would be overwritten). `ui::selection::Parts::narrow_select_button` takes the keyboard when selection mode ends and the header's button is not showing. The subtitle keeps one count on a phone, the due cards or, with none, the new ones (`study_subtitle(input, compact)`), the full line being the title's tooltip.

**Selection header.** Under a 400 px window it shows the bare count ("30") as its title, the full "30 sélectionnées" as a tooltip (`selection_title_tight`). It follows the window's width through `ui::breakpoint::follow_window_width` (the surface's `layout` signal), not a breakpoint: the page already has two, and `AdwBreakpointBin` applies only the last matching one, so a third would repeat their setters; and an `AdwBin` has a layout manager, so its `size_allocate` is never called.

**Review page.** Under 400 px Edit Card leaves the header for the Card Menu (which has "Edit Card" at all widths now), so the title and the subtitle ("30 restantes") fit beside Back, Undo, the menu and the window buttons (`edit_button_shown`).
