# 69. Study by tag

- Status: Accepted
- Date: 2026-11-03
- Refers to: SPEC §4.5, §4.6; `docs/briefs/phase-3a.md` (slice 5); ADR 0014 (the session), ADR 0027 (daily limits per deck), ADR 0028 (counted reviews), ADR 0029 (tags), ADR 0031 (tags on the deck page), ADR 0035 (suspension), ADR 0039 (the remembered deck), ADR 0068 (archived decks)

## Context

Tags cut across decks ("verbs" in Spanish and in Italian). The brief asks for "Study Cards Tagged …" from a tag on the deck page and from All Cards' tag filter: a normal session (same FSRS, written to the log) over the due and new cards that carry the tag in every deck that is not archived, with the daily limits *shared with the decks*. No saved filters, no custom study screens. The questions were what "shared" means when the limits are per deck (ADR 0027), how the session builder learns about a scope, and where the entry goes on each page.

## Decision

**A scope.** `scheduling::StudyScope` is `Deck(id)` or `Tag(id)`. `Store::study_input` and `Store::next_study_change` take it. The deck path is the SQL it was, unchanged; the existing tests pin it.

**The limits stay per deck, and a tag session spends each deck's own.** For each live deck that is not archived and has a live card with the tag, its allowance left today is computed exactly as for its own session (`max_new_per_day` and `max_reviews_per_day` less what that deck did today, from the counted reviews of ADR 0028). The deck gives the tag session, of its cards with the tag, its most overdue due reviews up to its review allowance and its oldest new cards up to its new allowance; step cards are never limited (a failed card always comes back). The parts are merged with the deck session's rules: reviews and steps by `(due_at, seq)`, new cards by `seq` (global creation order), and `Session` interleaves new cards among reviews as for a deck. Because the log is per card, a card studied by tag counts toward its deck's "done today" with no new bookkeeping: studying the deck afterwards sees a smaller allowance, and the other way round. A deck therefore never gives a tag session more than its own session would (the tag's cards are a subset, cut by the same allowance), and a tag on every card of one deck gives exactly that deck's session (tested).

Rejected: one shared allowance for the tag (the limits of one deck, or a sum), which would let a tag session exceed a deck's day, or starve one deck to feed another, and would need a new notion of "done today" per tag.

**Pure allocation.** `scheduling::scope::allocate` does the cutting and merging over plain data: one `DeckAllowance` per deck in scope and the `Candidate` cards (id, deck, `seq`, suspended, deleted, stage). It drops suspended and deleted cards and any card whose deck has no allowance (an archived or deleted deck, which the caller leaves out), so the rules are tested without a database: limits reached, a deck with zero allowance, ties across decks, fifty decks, archived, suspended and deleted cards, a tag on no card, and two property tests (a tag session is the union of what each deck gives for the tag alone, never more than the deck's own session; a tag on every card of one deck is that deck). Storage reads the candidates in one query and the allowances with the deck path's own `today_counts_in`, and a property test over random databases checks the result against `study_input` of each deck.

**Archived decks are left out by the queries** (ADR 0068): every tag-scope query shares one condition, `d.deleted_at IS NULL AND d.archived_at IS NULL`, and the tests archive and unarchive a deck. `next_study_change` of a tag is the deck rule over the tag's live, unsuspended cards in those decks. An unknown tag id is `TagNotFound`; a tag on no live card (tags are never deleted, ADR 0029) gives an empty session. `new_today` and `reviews_today` of a tag's input are the sums over the decks that gave a card. Reviews are written by `record_review` as always: no new event kind, no migration. A tag session over 5,000 cards is held to the deck session's time bound (`perf.rs`).

**Entry points.**
- **Deck page.** Tags show as pills inside each card row (ADR 0031), which is activatable as a whole; making pills focusable inside it would add a Tab stop per tag per row. The row's ⋮ menu (keyboard reachable, already labelled "Actions for …") gets a section after Edit and Card Information: "Study Cards Tagged “verbs”" per tag, or, past three tags, a submenu "Study Cards Tagged" listing at most 20 names (a card can have thousands, ADR 0050; the rest are in All Cards' filter). The row is shared, so All Cards' rows have the same entries. A tag name in a menu has its underscores doubled (`ui::rows::menu_text`): a `GMenuModel` label reads `_` as a mnemonic (tested on the opened menu).
- **All Cards.** While the tag filter names a tag, a **Study** button (`media-playback-start-symbolic`, before Select Cards in the header) has the tooltip "Study Cards Tagged “verbs”" and the same accessible label. The header has no menu to put it in, and the filter bar has no room at 360 px.

**The session.** `ReviewPage::for_tag` runs a normal session; Undo, Edit, Suspend, Delete and Move to Deck work as in a deck's (Move to Deck leaves the session even though the card may still have the tag: simple, and what it does in a deck's). The title is "Tagged “verbs”" where a deck's session shows the deck's name; the subtitle still counts what is left. The editor opens on the card's own deck, so Ctrl+Shift+Enter adds the next card there. The summary's button says "Back to Deck" when started from a deck page and "Back to All Cards" from All Cards; Back always returns to where the session began. With nothing to study at the start, the summary page says **"Nothing to Study"** and "No cards tagged “verbs” are left to study now." (a deck's session started empty says the same for the deck), instead of "Session Complete"; slice 6 adds Practice there. A tag session is no deck's: `ReviewPage::deck_id` is `None`, so the window never remembers it as the last deck (a deck page under it is remembered, as before), and archiving a deck does not leave it.

**After the session.** The deck list, a deck page and All Cards read their counts and rows again when shown (All Cards notes that it started a session). A deck page only shows saves of its own deck's cards: a tag session edits cards of other decks too.

## Consequences

- A tag session can offer fewer cards than "due" suggests: each deck's limits apply, and a deck at its limit gives only its step cards.
- Studying by tag and by deck on the same day share each deck's limits, whichever comes first.
- There is no count beside the entries; with nothing to study the session says so at once.
- The deck page has no "study a tag" entry of its own outside a card's menu; to study a tag no card of the deck shows, use All Cards' filter.
