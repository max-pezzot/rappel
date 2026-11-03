//! Practice (ADR 0070): what it goes over, in which order, and that it
//! leaves the database exactly as it was.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::Connection;

use crate::Error;
use crate::domain::Grade;
use crate::scheduling::{PRACTICE_LIMIT, Practice, STEP_MS, StudyScope};
use crate::storage::{DB_FILE_NAME, Store};

use super::query_only;
use crate::test_support::{DAY_MS, T0, answer, dump, with_suffix};

/// Practice time, 40 days after the first cards were learned.
const NOW: i64 = T0 + 40 * DAY_MS;

/// Learns `card` on day `day` after `T0`: the later, the better it is
/// remembered at `NOW` (the same history, less time since).
fn learn(store: &mut Store, card: &str, day: i64) {
    let at = T0 + day * DAY_MS;
    answer(store, card, Grade::Pass, at);
    answer(store, card, Grade::Pass, at + STEP_MS);
}

fn card(store: &mut Store, deck: &str, front: &str, tags: &[&str]) -> String {
    let tags: Vec<String> = tags.iter().map(|&t| t.to_owned()).collect();
    store
        .add_card_with_tags_at(deck, front, "back", &tags, T0)
        .unwrap()
        .card
        .id
}

fn tag_id(store: &Store, name: &str) -> String {
    store
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.name == name)
        .unwrap()
        .id
}

fn fronts(store: &Store, ids: &[String]) -> Vec<String> {
    ids.iter()
        .map(|id| store.card(id).unwrap().front_md)
        .collect()
}

/// A deck "Spanish" with cards learned on days 0, 10 and 20, two new
/// cards, one suspended and one deleted; a deck "Gone", archived, with a
/// card tagged "verbs"; "verbs" also on two of Spanish's cards.
fn fixture(store: &mut Store) -> (String, String) {
    let spanish = store.create_deck_at("Spanish", T0).unwrap().id;
    let gone = store.create_deck_at("Gone", T0).unwrap().id;
    let late = card(store, &spanish, "late", &["verbs"]);
    let early = card(store, &spanish, "early", &[]);
    let middle = card(store, &spanish, "middle", &["verbs"]);
    card(store, &spanish, "new 1", &[]);
    card(store, &spanish, "new 2", &["verbs"]);
    let suspended = card(store, &spanish, "suspended", &["verbs"]);
    let deleted = card(store, &spanish, "deleted", &["verbs"]);
    let archived = card(store, &gone, "archived", &["verbs"]);
    learn(store, &early, 0);
    learn(store, &middle, 10);
    learn(store, &late, 20);
    learn(store, &suspended, 0);
    learn(store, &deleted, 0);
    learn(store, &archived, 0);
    store.set_suspended(&[suspended], true).unwrap();
    store.delete_card_at(&deleted, T0).unwrap();
    store.archive_deck_at(&gone, T0).unwrap();
    (spanish, gone)
}

#[test]
fn the_least_remembered_come_first_then_new_cards_and_nothing_left_out_is_offered() {
    let mut store = Store::in_memory();
    let (spanish, _) = fixture(&mut store);
    let queue = store
        .practice_queue(&StudyScope::deck(&spanish), NOW)
        .unwrap();
    assert_eq!(
        fronts(&store, &queue),
        ["early", "middle", "late", "new 1", "new 2"]
    );
    // A tag's practice: its live cards in decks that are not archived.
    let verbs = tag_id(&store, "verbs");
    let queue = store.practice_queue(&StudyScope::tag(verbs), NOW).unwrap();
    assert_eq!(fronts(&store, &queue), ["middle", "late", "new 2"]);
}

#[test]
fn the_order_follows_retrievability_as_card_information_computes_it() {
    let mut store = Store::in_memory();
    let (spanish, _) = fixture(&mut store);
    let queue = store
        .practice_queue(&StudyScope::deck(&spanish), NOW)
        .unwrap();
    let recall: Vec<f32> = queue
        .iter()
        .filter_map(|id| store.card_info(id, NOW).unwrap().retrievability)
        .collect();
    assert_eq!(recall.len(), 3);
    assert!(recall.windows(2).all(|w| w[0] <= w[1]), "{recall:?}");
}

#[test]
fn cards_that_are_not_due_are_practised_too() {
    let mut store = Store::in_memory();
    let deck = store.create_deck_at("D", T0).unwrap().id;
    let a = card(&mut store, &deck, "a", &[]);
    learn(&mut store, &a, 0);
    // A day later the card is far from due; it is still offered.
    let at = T0 + DAY_MS;
    let input = store
        .study_input(&StudyScope::deck(&deck), at, at - 3_600_000)
        .unwrap();
    assert!(input.is_empty());
    assert_eq!(
        store.practice_queue(&StudyScope::deck(&deck), at).unwrap(),
        [a]
    );
}

#[test]
fn a_practice_is_capped() {
    let mut store = Store::in_memory();
    let deck = store.create_deck_at("Big", T0).unwrap().id;
    for i in 0..PRACTICE_LIMIT + 5 {
        card(&mut store, &deck, &format!("c{i}"), &[]);
    }
    let queue = store.practice_queue(&StudyScope::deck(&deck), NOW).unwrap();
    assert_eq!(queue.len(), PRACTICE_LIMIT);
    // The oldest new cards.
    assert_eq!(fronts(&store, &queue[..1]), ["c0"]);
}

#[test]
fn empty_archived_deleted_and_unknown_scopes() {
    let mut store = Store::in_memory();
    let (_, gone) = fixture(&mut store);
    let empty = store.create_deck_at("Empty", T0).unwrap().id;
    assert_eq!(
        store
            .practice_queue(&StudyScope::deck(&empty), NOW)
            .unwrap(),
        Vec::<String>::new()
    );
    assert!(matches!(
        store.practice_queue(&StudyScope::deck(&gone), NOW),
        Err(Error::DeckArchived(_))
    ));
    store.delete_deck(&empty).unwrap();
    assert!(matches!(
        store.practice_queue(&StudyScope::deck(&empty), NOW),
        Err(Error::DeckNotFound(_))
    ));
    assert!(matches!(
        store.practice_queue(&StudyScope::tag("not-a-tag"), NOW),
        Err(Error::TagNotFound(_))
    ));
    // A tag only on a deleted card gives nothing.
    let deck = store.create_deck_at("Other", T0).unwrap().id;
    let only = card(&mut store, &deck, "only", &["lonely"]);
    store.delete_card_at(&only, T0).unwrap();
    let lonely = tag_id(&store, "lonely");
    assert_eq!(
        store.practice_queue(&StudyScope::tag(lonely), NOW).unwrap(),
        Vec::<String>::new()
    );
}

/// The state of a database on disk that a write would change: the bytes
/// and modification times of the main file and of its write-ahead log
/// (in WAL mode a commit lands in the log first, so the main file alone
/// could miss it; the `-shm` index is left out, as readers write their
/// read marks there and it holds no data), and a full logical dump.
/// A file's path, and its bytes and modification time if it exists.
type FileState = (PathBuf, Option<(Vec<u8>, SystemTime)>);

#[derive(Debug, PartialEq)]
struct Snapshot {
    files: Vec<FileState>,
    dump: Vec<(String, Vec<String>)>,
}

fn snapshot(store: &Store, db: &Path) -> Snapshot {
    let files = [db.to_path_buf(), with_suffix(db, "-wal")]
        .into_iter()
        .map(|path| {
            let content = std::fs::read(&path)
                .ok()
                .map(|bytes| (bytes, std::fs::metadata(&path).unwrap().modified().unwrap()));
            (path, content)
        })
        .collect();
    Snapshot {
        files,
        dump: dump(store),
    }
}

/// Another connection's `PRAGMA data_version`: it changes whenever any
/// other connection commits a change to the file.
fn data_version(observer: &Connection) -> i64 {
    observer
        .query_row("PRAGMA data_version", [], |r| r.get(0))
        .unwrap()
}

/// Practises `scope` as the review page does: read the queue, then for
/// each card shown read it and answer it from `script` (`Some(grade)`, or
/// `None` for an Undo), until the practice is over. Returns the answers
/// given and the undos made.
fn practise(store: &Store, scope: &StudyScope, script: &[Option<Grade>]) -> (usize, usize) {
    let mut practice = Practice::new(store.practice_queue(scope, NOW).unwrap());
    let (mut answers, mut undos) = (0, 0);
    let mut step = 0;
    while let Some(id) = practice.next().map(str::to_owned) {
        assert!(step < 1_000, "the practice does not end");
        store.card(&id).unwrap();
        // Past the script: Pass, so the practice ends.
        match script.get(step).copied().unwrap_or(Some(Grade::Pass)) {
            Some(grade) => {
                practice.graded(&id, grade, Duration::from_secs(2));
                answers += 1;
            }
            None => {
                if practice.undo_last().is_some() {
                    undos += 1;
                }
            }
        }
        step += 1;
    }
    // An Undo from the summary, as the page allows.
    if practice.undo_last().is_some() {
        undos += 1;
    }
    (answers, undos)
}

/// The test the brief asks for: a database with a history, practised in
/// full (deck and tag, with fails, passes and undos), is byte-identical
/// afterwards, in its main file and its log, and no connection committed
/// anything.
#[test]
fn a_practice_leaves_the_database_byte_identical() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join(DB_FILE_NAME);
    let mut store = Store::open(tmp.path()).unwrap();
    let (spanish, _) = fixture(&mut store);
    // Some undo and reset history too.
    let extra = card(&mut store, &spanish, "extra", &["verbs"]);
    learn(&mut store, &extra, 5);
    let failed = answer(&mut store, &extra, Grade::Fail, T0 + 30 * DAY_MS);
    store
        .undo(&extra, &failed.id, T0 + 30 * DAY_MS + 1_000)
        .unwrap();
    store.reset_card(&extra, T0 + 31 * DAY_MS).unwrap();
    let observer = Connection::open(&db).unwrap();
    let before = snapshot(&store, &db);
    let version = data_version(&observer);
    let changes = store.conn.total_changes();
    assert!(before.files.iter().all(|(_, content)| content.is_some()));

    let (fail, pass) = (Some(Grade::Fail), Some(Grade::Pass));
    let script = [fail, pass, None, fail, fail, None, None, pass, fail];
    let (answers, undos) = practise(&store, &StudyScope::deck(&spanish), &script);
    assert!(answers > script.len() && undos >= 3, "{answers} {undos}");
    let verbs = StudyScope::tag(tag_id(&store, "verbs"));
    let (answers, _) = practise(&store, &verbs, &script);
    assert!(answers > 0);

    assert_eq!(store.conn.total_changes(), changes);
    assert_eq!(data_version(&observer), version);
    assert_eq!(snapshot(&store, &db), before);

    // The checks see a write: one real answer changes all of them.
    answer(&mut store, &extra, Grade::Pass, NOW);
    assert_ne!(store.conn.total_changes(), changes);
    assert_ne!(data_version(&observer), version);
    let after = snapshot(&store, &db);
    assert_ne!(after.dump, before.dump);
    assert_ne!(after.files, before.files);
}

/// A practice dropped part-way (the page closed, the app quit) leaves
/// nothing behind: the files are as they were while the store is open,
/// and the database's content after it is closed and opened again.
#[test]
fn a_practice_dropped_part_way_leaves_nothing_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join(DB_FILE_NAME);
    let mut store = Store::open(tmp.path()).unwrap();
    let (spanish, _) = fixture(&mut store);
    let before = snapshot(&store, &db);
    let changes = store.conn.total_changes();
    {
        let mut practice = Practice::new(
            store
                .practice_queue(&StudyScope::deck(&spanish), NOW)
                .unwrap(),
        );
        for grade in [Grade::Fail, Grade::Pass] {
            let id = practice.next().unwrap().to_owned();
            practice.graded(&id, grade, Duration::from_secs(1));
        }
        assert!(practice.remaining() > 0);
    }
    assert_eq!(snapshot(&store, &db), before);
    assert_eq!(store.conn.total_changes(), changes);
    drop(store);
    let store = Store::open(tmp.path()).unwrap();
    assert_eq!(dump(&store), before.dump);
}

fn is_query_only(store: &Store) -> bool {
    store
        .conn
        .query_row("PRAGMA query_only", [], |r| r.get(0))
        .unwrap()
}

/// While a practice reads, SQLite refuses any write (`SQLITE_READONLY`):
/// a write slipped into the read fails and changes nothing. The setting
/// is cleared afterwards, also when the read fails, so the app's writes
/// still work.
#[test]
fn a_write_inside_a_practices_read_is_refused() {
    let mut store = Store::in_memory();
    let (spanish, gone) = fixture(&mut store);
    let before = dump(&store);
    let refused = query_only(&store.conn, |conn| {
        conn.execute("UPDATE decks SET name = 'changed'", [])?;
        Ok(())
    });
    match refused {
        Err(Error::Sqlite(rusqlite::Error::SqliteFailure(e, _))) => {
            assert_eq!(e.code, rusqlite::ErrorCode::ReadOnly);
        }
        other => panic!("the write was not refused: {other:?}"),
    }
    assert_eq!(dump(&store), before);
    assert!(!is_query_only(&store));
    // Cleared after the store's own calls, whether they succeed or fail.
    store
        .practice_queue(&StudyScope::deck(&spanish), NOW)
        .unwrap();
    assert!(!is_query_only(&store));
    assert!(store.practice_queue(&StudyScope::deck(&gone), NOW).is_err());
    assert!(store.has_practisable(&StudyScope::deck(&gone)).is_err());
    assert!(!is_query_only(&store));
    // A write after it works.
    store.create_deck_at("After", T0).unwrap();
}

/// A read that panics still leaves the connection writable: the guard
/// turns `query_only` off as the panic unwinds.
#[test]
fn a_panicking_practice_read_leaves_the_store_writable() {
    let mut store = Store::in_memory();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        query_only(&store.conn, |_| -> crate::Result<()> {
            panic!("a read went wrong");
        })
    }));
    assert!(panicked.is_err());
    assert!(!is_query_only(&store));
    store.create_deck_at("After", T0).unwrap();
}

/// `has_practisable` says whether `practice_queue` has a card, for every
/// kind of scope.
#[test]
fn has_practisable_agrees_with_the_queue() {
    let mut store = Store::in_memory();
    let (spanish, gone) = fixture(&mut store);
    let empty = store.create_deck_at("Empty", T0).unwrap().id;
    let only_suspended = store.create_deck_at("Suspended", T0).unwrap().id;
    let s = card(&mut store, &only_suspended, "s", &["lonely"]);
    store.set_suspended(&[s], true).unwrap();
    let verbs = tag_id(&store, "verbs");
    let lonely = tag_id(&store, "lonely");
    for scope in [
        StudyScope::deck(&spanish),
        StudyScope::deck(&empty),
        StudyScope::deck(&only_suspended),
        StudyScope::tag(verbs),
        StudyScope::tag(lonely),
    ] {
        let queue = store.practice_queue(&scope, NOW).unwrap();
        assert_eq!(
            store.has_practisable(&scope).unwrap(),
            !queue.is_empty(),
            "{scope:?}"
        );
    }
    assert!(matches!(
        store.has_practisable(&StudyScope::deck(&gone)),
        Err(Error::DeckArchived(_))
    ));
    assert!(matches!(
        store.has_practisable(&StudyScope::tag("not-a-tag")),
        Err(Error::TagNotFound(_))
    ));
}
