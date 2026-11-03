//! What a practice goes over (ADR 0070): read only.

use rusqlite::{Connection, params};

use super::Store;
use super::decks::ensure_active_deck;
use super::review::{TAGGED_FROM, TAGGED_IN_SCOPE, ensure_tag};
use super::schedule::to_f32;
use crate::error::Result;
use crate::scheduling::StudyScope;
use crate::scheduling::practice::{PracticeCandidate, practice_order};
use crate::stats::retrievability;

impl Store {
    /// The cards a practice of `scope` shows at `now` (UTC milliseconds),
    /// in order: the live cards of the scope that are not suspended, in
    /// decks that are neither deleted nor archived, the reviewed ones by
    /// their retrievability now (lowest first; from the cached stability,
    /// difficulty and last review, as Card Information computes it), then
    /// the new ones oldest first, ties by id, at most
    /// [`PRACTICE_LIMIT`](crate::scheduling::PRACTICE_LIMIT). Every card is
    /// offered, due or not: practice ignores the schedule and the daily
    /// limits, since it records nothing.
    ///
    /// Takes `&self` and reads in one read transaction that commits
    /// nothing: a practice cannot write.
    ///
    /// # Errors
    /// [`Error::DeckNotFound`](crate::Error::DeckNotFound),
    /// [`Error::DeckArchived`](crate::Error::DeckArchived),
    /// [`Error::TagNotFound`](crate::Error::TagNotFound), SQLite errors.
    pub fn practice_queue(&self, scope: &StudyScope, now: i64) -> Result<Vec<String>> {
        let tx = self.conn.unchecked_transaction()?;
        let candidates = match scope {
            StudyScope::Deck(deck_id) => {
                ensure_active_deck(&tx, deck_id)?;
                candidates_in(&tx, "cards c", "c.deck_id = ?1", deck_id, now)?
            }
            StudyScope::Tag(tag_id) => {
                ensure_tag(&tx, tag_id)?;
                candidates_in(&tx, TAGGED_FROM, TAGGED_IN_SCOPE, tag_id, now)?
            }
        };
        // Read only: ending it commits nothing.
        tx.finish()?;
        Ok(practice_order(candidates))
    }
}

/// The live, unsuspended cards of `from` (whose cards table is `c`) that
/// `scope` (a condition on `?1` = `id`) selects, with their retrievability
/// at `now`.
fn candidates_in(
    conn: &Connection,
    from: &str,
    scope: &str,
    id: &str,
    now: i64,
) -> Result<Vec<PracticeCandidate>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT c.id, c.seq, s.stability, s.difficulty, s.last_review_at
         FROM {from} LEFT JOIN card_schedule s ON s.card_id = c.id
         WHERE {scope} AND c.deleted_at IS NULL AND c.suspended = 0"
    ))?;
    let rows =
        stmt.query_map(params![id], |r| {
            let memory: (Option<f64>, Option<f64>, Option<i64>) = (r.get(2)?, r.get(3)?, r.get(4)?);
            Ok(PracticeCandidate {
                card_id: r.get(0)?,
                seq: r.get(1)?,
                // A card without a schedule row is new: it has no memory.
                retrievability: match memory {
                    (Some(stability), Some(difficulty), Some(last_review_at)) => Some(
                        retrievability(to_f32(stability), to_f32(difficulty), last_review_at, now),
                    ),
                    _ => None,
                },
            })
        })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests;
