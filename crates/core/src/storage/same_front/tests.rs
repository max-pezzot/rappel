use super::*;
use crate::test_support::T0;

/// A store with the decks "Spanish" and "French".
fn two_decks() -> (Store, String, String) {
    let mut store = Store::in_memory();
    let spanish = store.create_deck_at("Spanish", T0).unwrap().id;
    let french = store.create_deck_at("French", T0).unwrap().id;
    (store, spanish, french)
}

fn add(store: &mut Store, deck: &str, front: &str) -> String {
    store.add_card_at(deck, front, "back", T0).unwrap().id
}

/// The ids of the cards found for `front`, in their order.
fn found(store: &Store, front: &str, except: Option<&str>) -> Vec<String> {
    store
        .cards_with_front(front, except)
        .unwrap()
        .into_iter()
        .map(|s| s.card_id)
        .collect()
}

#[test]
fn the_key_is_the_folded_plain_text_on_one_line() {
    assert_eq!(front_key("  Hola,\n\n  **MUNDO**  "), "hola, mundo");
    assert_eq!(front_key("Économie\t é"), "economie e");
    assert_eq!(front_key("$x^2$ and `code`"), "x^2 and code");
    assert_eq!(
        front_key("[link](https://example.org) ![alt](media:a.png)"),
        "link alt"
    );
    assert_eq!(front_key(""), "");
    assert_eq!(front_key(" \n\t "), "");
    // Punctuation stays.
    assert_ne!(front_key("Hola!"), front_key("Hola"));
}

#[test]
fn the_phrase_is_quoted_and_never_syntax() {
    assert_eq!(
        phrase_query("hola mundo").as_deref(),
        Some("front : \"hola mundo\"")
    );
    assert_eq!(
        phrase_query("say \"hi\"").as_deref(),
        Some("front : \"say \"\"hi\"\"\"")
    );
    // Leading punctuation holds no token and is left out.
    assert_eq!(phrase_query("¿qué?").as_deref(), Some("front : \"qué?\""));
    assert_eq!(phrase_query("a\0b").as_deref(), Some("front : \"a b\""));
    // No letter or digit: nothing to look up.
    assert_eq!(phrase_query(""), None);
    assert_eq!(phrase_query("+ - * ?"), None);
}

#[test]
fn a_long_key_is_cut_and_ends_in_a_prefix() {
    let key = "é".repeat(MAX_PHRASE_BYTES);
    let query = phrase_query(&key).unwrap();
    // Cut between two characters, at most the budget.
    let inner = query
        .strip_prefix("front : \"")
        .and_then(|q| q.strip_suffix("\"*"))
        .unwrap();
    assert_eq!(inner, "é".repeat(MAX_PHRASE_BYTES / 2));
    // Padding before the first letter is free.
    let padded = format!("{}{}", "-".repeat(10_000), "word");
    assert_eq!(phrase_query(&padded).as_deref(), Some("front : \"word\""));
}

#[test]
fn case_accents_and_whitespace_do_not_count() {
    let (mut store, spanish, _) = two_decks();
    let id = add(&mut store, &spanish, "El  Niño\n");
    for typed in [
        "el niño",
        "EL NINO",
        "  el\tnino  ",
        "el\n\nniño",
        "Él Ñiño",
    ] {
        assert_eq!(found(&store, typed, None), [id.as_str()], "{typed:?}");
    }
    for typed in ["el niños", "niño", "el", "el niño!", "elnino"] {
        assert_eq!(
            found(&store, typed, None),
            Vec::<String>::new(),
            "{typed:?}"
        );
    }
}

#[test]
fn markdown_and_math_are_compared_as_their_text() {
    let (mut store, spanish, _) = two_decks();
    let bold = add(&mut store, &spanish, "**el** _gato_");
    let math = add(&mut store, &spanish, "$x^2$");
    assert_eq!(found(&store, "el gato", None), [bold.as_str()]);
    assert_eq!(found(&store, "*el* **gato**", None), [bold]);
    assert_eq!(found(&store, "x^2", None), [math.as_str()]);
    assert_eq!(found(&store, "$$x^2$$", None), [math]);
}

#[test]
fn deleted_cards_and_decks_are_left_out() {
    let (mut store, spanish, french) = two_decks();
    let gone = add(&mut store, &spanish, "hola");
    let kept = add(&mut store, &french, "hola");
    store.delete_card_at(&gone, T0 + 1).unwrap();
    assert_eq!(found(&store, "hola", None), [kept]);
    store.delete_deck_at(&french, T0 + 2).unwrap();
    assert_eq!(found(&store, "hola", None), Vec::<String>::new());
}

#[test]
fn archived_decks_are_searched_flagged_and_listed_last() {
    let (mut store, spanish, french) = two_decks();
    let in_spanish = add(&mut store, &spanish, "hola");
    let in_french = add(&mut store, &french, "hola");
    store.archive_deck_at(&spanish, T0 + 1).unwrap();
    let same = store.cards_with_front("Hola", None).unwrap();
    assert_eq!(
        same,
        [
            SameFront {
                card_id: in_french,
                deck_id: french,
                deck_name: "French".to_owned(),
                archived: false,
            },
            SameFront {
                card_id: in_spanish,
                deck_id: spanish,
                deck_name: "Spanish".to_owned(),
                archived: true,
            },
        ]
    );
}

#[test]
fn the_newest_comes_first_and_the_edited_card_is_left_out() {
    let (mut store, spanish, _) = two_decks();
    let first = add(&mut store, &spanish, "hola");
    let second = add(&mut store, &spanish, "hola");
    assert_eq!(found(&store, "hola", None), [second.clone(), first.clone()]);
    assert_eq!(found(&store, "hola", Some(&second)), [first.as_str()]);
    assert_eq!(found(&store, "hola", Some(&first)), [second]);
    // An id that is no card leaves nothing out.
    assert_eq!(found(&store, "hola", Some("nope")).len(), 2);
}

#[test]
fn an_edit_moves_the_card_out_of_and_into_the_matches() {
    let (mut store, spanish, _) = two_decks();
    let id = add(&mut store, &spanish, "hola");
    store.edit_card_at(&id, "adiós", "back", T0 + 1).unwrap();
    assert_eq!(found(&store, "hola", None), Vec::<String>::new());
    assert_eq!(found(&store, "adios", None), [id]);
}

#[test]
fn a_phrase_inside_a_longer_front_is_not_the_same_front() {
    let (mut store, spanish, _) = two_decks();
    add(&mut store, &spanish, "el gato negro");
    add(&mut store, &spanish, "un gato");
    add(&mut store, &spanish, "gatos");
    let exact = add(&mut store, &spanish, "gato");
    assert_eq!(found(&store, "gato", None), [exact]);
    assert_eq!(found(&store, "el gato", None), Vec::<String>::new());
}

#[test]
fn fts_syntax_in_a_front_is_only_text() {
    let (mut store, spanish, _) = two_decks();
    let fronts = [
        "say \"hi\"",
        "a* b",
        "x - y",
        "NEAR(a b)",
        "a OR b",
        "front: back",
        "(a) AND NOT (b)",
        "^start",
        "\"",
        "\"\"\"",
        "col:umn \"un closed",
    ];
    let ids: Vec<String> = fronts
        .iter()
        .map(|f| add(&mut store, &spanish, f))
        .collect();
    for (front, id) in fronts.iter().zip(&ids) {
        let same = found(&store, front, None);
        if front.contains(char::is_alphanumeric) {
            assert_eq!(same, [id.as_str()], "{front:?}");
        } else {
            // No token to look up: no note, never an error.
            assert_eq!(same, Vec::<String>::new(), "{front:?}");
        }
    }
    // The operators are words, not syntax: "a OR b" finds no "a".
    assert_eq!(found(&store, "a", None), Vec::<String>::new());
}

#[test]
fn a_front_without_a_token_finds_nothing() {
    let (mut store, spanish, _) = two_decks();
    for front in ["?", "+ - *", "$+$", "…", "\u{301}"] {
        add(&mut store, &spanish, front);
        assert_eq!(
            found(&store, front, None),
            Vec::<String>::new(),
            "{front:?}"
        );
    }
    assert_eq!(found(&store, "", None), Vec::<String>::new());
    assert_eq!(found(&store, "  \n ", None), Vec::<String>::new());
}

#[test]
fn odd_characters_never_fail_the_query() {
    let (mut store, spanish, _) = two_decks();
    // Characters Rust calls alphanumeric that FTS5 may read as no token,
    // private use joined to a word, a lone combining mark.
    for front in ["ⓐ", "\u{10ffff}", "card\u{10ffff}", "Ⅻ", "x\u{301}y"] {
        let id = add(&mut store, &spanish, front);
        let same = store.cards_with_front(front, None).unwrap();
        assert!(
            same.is_empty() || same.iter().map(|s| &s.card_id).eq([&id]),
            "{front:?}: {same:?}"
        );
    }
    // A card never holds NUL, but the editor's text may: it is a
    // character no front has, never an error.
    add(&mut store, &spanish, "a b");
    for typed in ["a\0b", "a \0", "\0a b\0", "\0"] {
        assert_eq!(
            found(&store, typed, None),
            Vec::<String>::new(),
            "{typed:?}"
        );
    }
}

#[test]
fn long_fronts_are_compared_in_full() {
    let (mut store, spanish, _) = two_decks();
    let words = "palabra ".repeat(200);
    let long = add(&mut store, &spanish, &words);
    let longer = add(&mut store, &spanish, &format!("{words} más"));
    let cut_word = format!("{}{}", "x".repeat(MAX_PHRASE_BYTES - 2), "yyyy");
    let cut = add(&mut store, &spanish, &cut_word);
    assert_eq!(found(&store, &words.to_uppercase(), None), [long]);
    assert_eq!(found(&store, &format!("{words}   MÁS"), None), [longer]);
    // The phrase cuts this word in two; its prefix still finds it.
    assert_eq!(found(&store, &cut_word, None), [cut]);
    assert_eq!(
        found(&store, &"x".repeat(MAX_PHRASE_BYTES + 2), None),
        Vec::<String>::new()
    );
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]

    /// Any text typed is a valid lookup.
    #[test]
    fn any_input_is_a_valid_lookup(text in proptest::prelude::any::<String>()) {
        let (mut store, spanish, _) = two_decks();
        add(&mut store, &spanish, "alpha beta");
        store.cards_with_front(&text, None).unwrap();
    }

    /// The query finds exactly the cards whose key is the typed front's,
    /// as comparing every card's key would: the phrase never misses one.
    #[test]
    fn the_lookup_is_comparing_every_key(
        fronts in proptest::collection::vec("[aAéÉ b*_$!\n-]{0,8}", 1..12),
        typed in "[aAéÉ b*_$!\n-]{0,8}",
    ) {
        let (mut store, spanish, _) = two_decks();
        let mut expected = Vec::new();
        for front in &fronts {
            if front.trim().is_empty() {
                continue;
            }
            let id = add(&mut store, &spanish, front);
            let key = front_key(&typed);
            if front_key(front) == key && key.contains(char::is_alphanumeric) {
                expected.push(id);
            }
        }
        expected.reverse();
        proptest::prop_assert_eq!(found(&store, &typed, None), expected);
    }
}
