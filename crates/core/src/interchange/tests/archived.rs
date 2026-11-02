//! A deck's `archived_at` in the file (ADR 0068): always written, optional
//! when read, kept by a new deck, and never changed on a deck the database
//! has.

use super::*;

/// "Maths" (archived, one card) and "Physics" (not), with the stamp.
fn env_with_decks() -> (Env, String, String, i64) {
    let mut env = Env::new();
    let maths = env.store.create_deck("Maths").unwrap().id;
    let physics = env.store.create_deck("Physics").unwrap().id;
    env.store.add_card(&maths, "2 + 2", "4").unwrap();
    env.store.add_card(&physics, "c", "light").unwrap();
    let stamp = env.store.archive_deck(&maths).unwrap().archived_at.unwrap();
    (env, maths, physics, stamp)
}

fn archived_at(store: &Store, id: &str) -> Option<i64> {
    store.deck(id).unwrap().archived_at
}

/// Each deck record's `archived_at`, by deck id, as the file has it.
fn flags_in(json: &Value) -> Vec<(String, Value)> {
    json["decks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["id"].as_str().unwrap().to_owned(),
                d["archived_at"].clone(),
            )
        })
        .collect()
}

#[test]
fn the_flag_is_written_on_every_deck_and_round_trips() {
    let (source, maths, physics, stamp) = env_with_decks();
    let bytes = source.export();
    let json = json_of(&bytes);
    let mut flags = flags_in(&json);
    flags.sort_by(|a, b| a.0.cmp(&b.0));
    let mut expected = vec![
        (maths.clone(), json!(stamp)),
        (physics.clone(), json!(null)),
    ];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(flags, expected);
    // Written after `updated_at`, before `deleted_at`.
    let text = String::from_utf8(read_cards_json(&bytes, &Limits::DEFAULT).unwrap()).unwrap();
    let at = |key: &str| text.find(key).unwrap();
    assert!(at("\"updated_at\"") < at("\"archived_at\""));
    assert!(at("\"archived_at\"") < at("\"deleted_at\""));

    let mut target = Env::new();
    target.import(&bytes).unwrap();
    assert_eq!(archived_at(&target.store, &maths), Some(stamp));
    assert_eq!(archived_at(&target.store, &physics), None);
    assert_eq!(target.export(), bytes);
    assert_eq!(target.store.archived_decks().unwrap().len(), 1);
}

#[test]
fn a_file_without_the_field_or_with_null_reads_as_not_archived() {
    let (source, maths, physics, _) = env_with_decks();
    let base = json_of(&source.export());
    for remove in [true, false] {
        let mut value = base.clone();
        for deck in value["decks"].as_array_mut().unwrap() {
            let deck = deck.as_object_mut().unwrap();
            if remove {
                deck.remove("archived_at").unwrap();
            } else {
                deck.insert("archived_at".to_owned(), json!(null));
            }
        }
        let mut target = Env::new();
        target.import(&archive_of(&value)).unwrap();
        assert_eq!(archived_at(&target.store, &maths), None, "{remove}");
        assert_eq!(archived_at(&target.store, &physics), None);
        assert_eq!(
            target.store.list_decks(T_EXPORT, T_EXPORT).unwrap().len(),
            2
        );
    }
}

#[test]
fn a_new_deck_keeps_the_files_value() {
    let (source, maths, physics, _) = env_with_decks();
    let mut value = json_of(&source.export());
    for deck in value["decks"].as_array_mut().unwrap() {
        deck["archived_at"] = json!(5);
    }
    let mut target = Env::new();
    target.import(&archive_of(&value)).unwrap();
    assert_eq!(archived_at(&target.store, &maths), Some(5));
    assert_eq!(archived_at(&target.store, &physics), Some(5));
    assert_eq!(target.store.list_decks(T_EXPORT, T_EXPORT).unwrap(), []);
}

/// Anything but `null` or a whole number of milliseconds from 1970 to
/// 9999 is refused as the field of its row (ADR 0068), and nothing is
/// written.
#[test]
fn every_other_value_is_refused_as_the_field() {
    let (source, ..) = env_with_decks();
    let base = json_of(&source.export());
    let cases = [
        (json!("1792067696789"), FieldProblem::NotATimestamp),
        (json!(""), FieldProblem::NotATimestamp),
        (json!(1.5), FieldProblem::NotATimestamp),
        (json!(1.0), FieldProblem::NotATimestamp),
        (json!(true), FieldProblem::NotATimestamp),
        (json!(false), FieldProblem::NotATimestamp),
        (json!({}), FieldProblem::NotATimestamp),
        (json!({"at": 5}), FieldProblem::NotATimestamp),
        (json!([]), FieldProblem::NotATimestamp),
        (json!([5]), FieldProblem::NotATimestamp),
        (json!(-1), FieldProblem::TimestampOutOfRange),
        (json!(i64::MIN), FieldProblem::TimestampOutOfRange),
        (
            json!(253_402_300_800_000_i64),
            FieldProblem::TimestampOutOfRange,
        ),
        (json!(u64::MAX), FieldProblem::TimestampOutOfRange),
    ];
    for (bad, reason) in cases {
        for index in [0, 1] {
            let mut value = base.clone();
            value["decks"][index]["archived_at"] = bad.clone();
            let mut target = Env::new();
            let problem = refused(target.plan(&archive_of(&value)));
            assert_eq!(
                problem,
                ImportProblem::InvalidField {
                    table: "decks",
                    index,
                    field: "archived_at",
                    reason,
                },
                "{bad}"
            );
            assert_eq!(
                problem.to_string(),
                format!("decks[{index}].archived_at: {reason}")
            );
            assert!(target.import(&archive_of(&value)).is_err());
            assert_eq!(target.store.decks_with_archived().unwrap(), []);
        }
    }
    // The edges of the range are accepted.
    for good in [json!(0), json!(253_402_300_799_999_i64)] {
        let mut value = base.clone();
        value["decks"][0]["archived_at"] = good.clone();
        assert!(Env::new().plan(&archive_of(&value)).is_ok(), "{good}");
    }
}

/// A deck the database has that differs only in `archived_at` is already
/// present, whichever way the file has it: the database keeps its own
/// flag, as an import never changes how a live deck is studied.
#[test]
fn a_deck_that_differs_only_in_the_flag_is_already_present() {
    let (mut env, maths, physics, stamp) = env_with_decks();
    let archived = env.export();
    env.store.unarchive_deck(&maths).unwrap();
    let physics_stamp = env.store.archive_deck(&physics).unwrap().archived_at;
    let swapped = env.export();
    assert_ne!(archived, swapped);
    let mut later = json_of(&archived);
    for deck in later["decks"].as_array_mut().unwrap() {
        deck["archived_at"] = json!(stamp + 7);
    }
    for file in [&archived, &swapped, &archive_of(&later)] {
        let plan = env.plan(file).unwrap();
        let s = plan.summary();
        assert_eq!((s.decks.new, s.decks.present, s.decks.revived), (0, 2, 0));
        assert_eq!(s.conflicts, []);
        assert!(s.is_noop(), "{s:?}");
        env.import(file).unwrap();
        assert_eq!(archived_at(&env.store, &maths), None);
        assert_eq!(archived_at(&env.store, &physics), physics_stamp);
    }
    assert_eq!(env.export(), swapped);
}

/// A deck deleted here and live in the file is revived with the
/// database's own flag.
#[test]
fn a_revived_deck_keeps_the_databases_flag() {
    let (mut env, maths, physics, stamp) = env_with_decks();
    let mut value = json_of(&env.export());
    for deck in value["decks"].as_array_mut().unwrap() {
        deck["archived_at"] = if deck["id"] == json!(maths) {
            json!(null)
        } else {
            json!(stamp)
        };
    }
    env.store.delete_deck(&maths).unwrap();
    env.store.delete_deck(&physics).unwrap();
    let plan = env.plan(&archive_of(&value)).unwrap();
    assert_eq!(plan.summary().decks.revived, 2);
    env.import(&archive_of(&value)).unwrap();
    assert_eq!(archived_at(&env.store, &maths), Some(stamp));
    assert_eq!(archived_at(&env.store, &physics), None);
    assert_eq!(env.store.card_count(&maths).unwrap(), 1);
}

/// An archived deck keeps its name taken against an import too.
#[test]
fn a_name_an_archived_deck_has_is_a_conflict() {
    let (source, ..) = env_with_decks();
    let mut target = Env::new();
    let own = target.store.create_deck("maths").unwrap().id;
    target.store.archive_deck(&own).unwrap();
    let plan = target.plan(&source.export()).unwrap();
    assert!(matches!(
        plan.summary().conflicts.as_slice(),
        [Conflict::DeckNameTaken { existing_id, .. }] if *existing_id == own
    ));
}
