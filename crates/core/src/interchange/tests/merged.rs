//! Export and import after Merge Into… (ADR 0066).

use super::*;

/// The sample, plus a live "Chemistry" deck with one card, and "Maths"
/// (live, with a deleted card and reviews) merged into it. Returns the env
/// and the merge.
fn merged_env() -> (Env, crate::storage::Merged) {
    let (mut env, maths, _) = sample_env();
    let chemistry = env.store.create_deck("Chemistry").unwrap().id;
    env.store.add_card(&chemistry, "H₂O ?", "water").unwrap();
    let merged = env.store.merge_deck(&maths, &chemistry).unwrap();
    assert!(merged.cards.len() > merged.live, "a deleted card moved too");
    (env, merged)
}

#[test]
fn an_export_after_a_merge_round_trips() {
    let (source, merged) = merged_env();
    let bytes = source.export();
    let json = json_of(&bytes);
    // The merged deck is in the file, deleted, and every card it had is
    // in the target.
    let decks = json["decks"].as_array().unwrap();
    let deck = decks.iter().find(|d| d["id"] == merged.source).unwrap();
    assert_eq!(deck["deleted_at"], json!(merged.stamp));
    for card in json["cards"].as_array().unwrap() {
        assert_ne!(card["deck_id"], json!(merged.source));
    }

    let mut target = Env::new();
    target.import(&bytes).unwrap();
    assert_eq!(target.export(), bytes);
    assert_eq!(schedule_table(&target.store), schedule_table(&source.store));
    // And importing it again changes nothing.
    let plan = target.plan(&bytes).unwrap();
    assert_eq!(plan.summary().conflicts, []);
    target.import(&bytes).unwrap();
    assert_eq!(target.export(), bytes);
}

#[test]
fn an_export_after_the_undo_equals_the_one_before_the_merge() {
    let (mut env, maths, _) = sample_env();
    let chemistry = env.store.create_deck("Chemistry").unwrap().id;
    let before = env.export();
    let merged = env.store.merge_deck(&maths, &chemistry).unwrap();
    env.store.unmerge_deck(&merged).unwrap();
    assert_eq!(env.export(), before);
}

/// A file from before the merge has the moved cards in their old deck:
/// like a card moved since the export, each is a conflict, and the whole
/// import is refused (SPEC §4.1), so it never undoes the merge by itself.
#[test]
fn a_file_from_before_the_merge_conflicts_on_the_moved_cards() {
    let (mut env, maths, _) = sample_env();
    let chemistry = env.store.create_deck("Chemistry").unwrap().id;
    let old = env.export();
    let merged = env.store.merge_deck(&maths, &chemistry).unwrap();
    let plan = env.plan(&old).unwrap();
    let mut expected: Vec<Conflict> = merged
        .cards
        .iter()
        .map(|id| Conflict::CardDiffers(id.clone()))
        .collect();
    let mut found = plan.summary().conflicts.clone();
    expected.sort_by_key(|c| format!("{c:?}"));
    found.sort_by_key(|c| format!("{c:?}"));
    assert_eq!(found, expected);
    let before = export(&env.store);
    assert!(matches!(
        env.import(&old),
        Err(Error::ImportConflicts(n)) if n == merged.cards.len()
    ));
    assert_eq!(export(&env.store), before);
}
