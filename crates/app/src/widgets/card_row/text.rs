//! What a card row of the deck page says to a screen reader, and which of
//! its menu items it offers. Pure, so it is unit-tested (ADR 0042).

use rappel_core::domain::Tag;

use crate::i18n::{fill, gettext};
use crate::ui::rows::menu_text;
use crate::ui::text::{MAX_NAMED, isolate, one_line};
use crate::widgets::tag_pills::text::spoken;

/// The front as a screen reader names the card, in the row's label and its
/// menu button's ("Actions for …"): its preview, or "empty front" when it
/// has no text (e.g. a side that is only an empty formula).
pub fn spoken_front(front: &str) -> String {
    if front.is_empty() {
        gettext("empty front")
    } else {
        front.to_owned()
    }
}

/// The accessible label of a card row: the front, the back, every tag
/// (the ones folded into "+N" too) and whether the card is suspended, so a
/// screen reader says what the row shows: "2 + 2, back: 4, tags: maths,
/// easy, suspended". The previews are already one plain line each.
pub fn row_label(front: &str, back: &str, tags: &[String], suspended: bool) -> String {
    let mut parts = vec![spoken_front(front)];
    if !back.is_empty() {
        // Translators: a card's back for screen readers ("back: 4").
        parts.push(fill(&gettext("back: {back}"), &[("back", back)]));
    }
    parts.extend(spoken(tags));
    if suspended {
        parts.push(gettext("suspended"));
    }
    parts.join(&crate::ui::text::list_separator())
}

/// The row actions that depend on the card, each with whether it is
/// enabled: the one of Suspend and Unsuspend that changes something, and
/// Reset Progress unless the card is already new, where a reset would only
/// add a log entry that changes nothing.
pub fn row_actions(suspended: bool, is_new: bool) -> [(&'static str, bool); 3] {
    [
        ("row.suspend", !suspended),
        ("row.unsuspend", suspended),
        ("row.reset", !is_new),
    ]
}

/// "Study Cards Tagged “verbs”": the entry that starts a session over the
/// cards carrying a tag (ADR 0069), in a row's menu and on All Cards. For
/// the eye: the name is isolated (`ui::text::isolate`).
pub fn study_tag_label(name: &str) -> String {
    // Translators: starts a study session over the cards that carry a tag,
    // in every deck; {tag} is the tag's name.
    fill(
        &gettext("Study Cards Tagged “{tag}”"),
        &[("tag", &isolate(&one_line(name)))],
    )
}

/// [`study_tag_label`] for a screen reader: the name as it is.
pub fn study_tag_spoken(name: &str) -> String {
    // Translators: starts a study session over the cards that carry a tag,
    // in every deck; {tag} is the tag's name.
    fill(
        &gettext("Study Cards Tagged “{tag}”"),
        &[("tag", &one_line(name))],
    )
}

/// How many tags a row's menu lists as entries of their own; with more,
/// they go in a submenu.
pub const TAG_ITEMS: usize = 3;

/// A row menu's entries for studying the card's tags: each a menu label
/// (mnemonics escaped, `ui::rows::menu_text`) and the tag's id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagMenu {
    /// No tag: nothing.
    None,
    /// Up to [`TAG_ITEMS`] tags: "Study Cards Tagged “verbs”" each.
    Items(Vec<(String, String)>),
    /// More: a submenu "Study Cards Tagged" listing the names, at most
    /// [`MAX_NAMED`] (a card can have thousands, ADR 0050; the others are
    /// in All Cards' tag filter).
    Submenu(String, Vec<(String, String)>),
}

/// The [`TagMenu`] of a card with `tags` (in the order they are shown).
pub fn tag_menu(tags: &[Tag]) -> TagMenu {
    if tags.is_empty() {
        TagMenu::None
    } else if tags.len() <= TAG_ITEMS {
        TagMenu::Items(
            tags.iter()
                .map(|t| (menu_text(&study_tag_label(&t.name)), t.id.clone()))
                .collect(),
        )
    } else {
        TagMenu::Submenu(
            // Translators: a submenu listing a card's tags; choosing one
            // studies the cards that carry it, in every deck.
            gettext("Study Cards _Tagged"),
            tags.iter()
                .take(MAX_NAMED)
                .map(|t| (menu_text(&isolate(&one_line(&t.name))), t.id.clone()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|&n| n.to_owned()).collect()
    }

    #[test]
    fn a_row_is_read_as_it_shows() {
        assert_eq!(row_label("2 + 2", "", &[], false), "2 + 2");
        assert_eq!(row_label("2 + 2", "4", &[], false), "2 + 2, back: 4");
        assert_eq!(
            row_label("2 + 2", "4", &names(&["maths", "easy"]), false),
            "2 + 2, back: 4, tags: maths, easy"
        );
        assert_eq!(
            row_label("2 + 2", "", &names(&["maths"]), true),
            "2 + 2, tags: maths, suspended"
        );
        // A front with no text (e.g. only an empty formula) still says
        // something.
        assert_eq!(row_label("", "4", &[], false), "empty front, back: 4");
    }

    #[test]
    fn every_tag_is_read_even_past_the_pills_shown() {
        let tags = names(&["a", "b", "c", "d", "e"]);
        assert_eq!(
            row_label("Front", "", &tags, false),
            "Front, tags: a, b, c, d, e"
        );
    }

    fn tag(id: &str, name: &str) -> Tag {
        Tag {
            id: id.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn the_study_entry_names_the_tag_on_one_line() {
        assert_eq!(
            study_tag_label("verbs"),
            "Study Cards Tagged “\u{2068}verbs\u{2069}”"
        );
        assert_eq!(
            study_tag_spoken("irregular\nverbs"),
            "Study Cards Tagged “irregular verbs”"
        );
        // A placeholder in the name stays as it is.
        assert_eq!(study_tag_spoken("{tag}"), "Study Cards Tagged “{tag}”");
    }

    #[test]
    fn a_few_tags_are_entries_and_more_a_submenu() {
        assert_eq!(tag_menu(&[]), TagMenu::None);
        let few = [tag("1", "verbs"), tag("2", "my_tag")];
        assert_eq!(
            tag_menu(&few),
            TagMenu::Items(vec![
                (
                    "Study Cards Tagged “\u{2068}verbs\u{2069}”".to_owned(),
                    "1".to_owned()
                ),
                // An underscore is not a mnemonic.
                (
                    "Study Cards Tagged “\u{2068}my__tag\u{2069}”".to_owned(),
                    "2".to_owned()
                ),
            ])
        );
        let three: Vec<Tag> = (0..TAG_ITEMS).map(|i| tag(&i.to_string(), "t")).collect();
        assert!(matches!(tag_menu(&three), TagMenu::Items(items) if items.len() == TAG_ITEMS));
        let many: Vec<Tag> = (0..1_000)
            .map(|i| tag(&i.to_string(), &format!("t_{i}")))
            .collect();
        let TagMenu::Submenu(label, items) = tag_menu(&many) else {
            panic!("a submenu");
        };
        assert_eq!(label, "Study Cards _Tagged");
        assert_eq!(items.len(), MAX_NAMED);
        assert_eq!(
            items[0],
            ("\u{2068}t__0\u{2069}".to_owned(), "0".to_owned())
        );
    }

    #[test]
    fn a_row_offers_only_what_changes_the_card() {
        assert_eq!(
            row_actions(false, false),
            [
                ("row.suspend", true),
                ("row.unsuspend", false),
                ("row.reset", true)
            ]
        );
        assert_eq!(
            row_actions(true, true),
            [
                ("row.suspend", false),
                ("row.unsuspend", true),
                ("row.reset", false)
            ]
        );
    }
}
