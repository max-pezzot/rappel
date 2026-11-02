//! `SelectionHeader`: the header of selection mode, shared by All Cards and
//! a deck's page (ADR 0065). Select All at the start, how many cards are
//! chosen as the title, Cancel at the end (HIG, "Selection mode"). Layout in
//! `data/selection_header.blp`; its buttons activate the page's
//! `selection.*` actions, and `ui::selection` shows it in place of the
//! page's own header and sets its title.

use adw::subclass::prelude::*;
use gtk::glib;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SelectionHeader)]
    #[template(resource = "/io/github/rsmirke/Rappel/selection_header.ui")]
    pub struct SelectionHeader {
        #[template_child]
        pub title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub select_all_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub select_all_content: TemplateChild<adw::ButtonContent>,
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        /// A narrow window: Select All keeps only its icon.
        #[property(get, set)]
        pub compact: Cell<bool>,
        /// Select All's word, as the layout gives it (translated).
        pub select_all_word: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SelectionHeader {
        const NAME: &'static str = "RappelSelectionHeader";
        type Type = super::SelectionHeader;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SelectionHeader {
        fn constructed(&self) {
            self.parent_constructed();
            self.select_all_word
                .replace(self.select_all_content.label().to_string());
            let header = self.obj();
            header.connect_compact_notify(|header| {
                let imp = header.imp();
                let word = if header.compact() {
                    String::new()
                } else {
                    imp.select_all_word.borrow().clone()
                };
                imp.select_all_content.set_label(&word);
            });
        }
    }

    impl WidgetImpl for SelectionHeader {
        fn root(&self) {
            // Roots the children first (`gtk_widget_real_root`), so the
            // `AdwButtonContent` has named its button by then.
            self.parent_root();
            super::super::selection_bar::name_by_own_label(&self.select_all_button);
        }
    }

    impl BinImpl for SelectionHeader {}
}

glib::wrapper! {
    pub struct SelectionHeader(ObjectSubclass<imp::SelectionHeader>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SelectionHeader {
    /// Sets the title ("3 Selected").
    pub fn set_title(&self, title: &str) {
        self.imp().title.set_title(title);
    }

    /// The title shown.
    pub fn title(&self) -> String {
        self.imp().title.title().to_string()
    }

    /// Select All.
    pub fn select_all_button(&self) -> gtk::Button {
        self.imp().select_all_button.get()
    }

    /// Cancel.
    pub fn cancel_button(&self) -> gtk::Button {
        self.imp().cancel_button.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_header_keeps_only_the_icon_of_select_all() {
        crate::testing::with_widgets(|| {
            let header: SelectionHeader = glib::Object::new();
            let content = &header.imp().select_all_content;
            assert_eq!(content.label(), "Select _All");
            header.set_compact(true);
            assert_eq!(content.label(), "");
            header.set_compact(false);
            assert_eq!(content.label(), "Select _All");
            header.set_title("2 Selected");
            assert_eq!(header.title(), "2 Selected");
        });
    }
}
