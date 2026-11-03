//! The toast that stays while a worker reads or writes a file, with a
//! Cancel button, and Escape as the same Cancel from the keyboard (ADR 0063
//! for the Anki import, ADR 0071 for the plan of a `.rappel` file).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use gtk::glib;

use crate::i18n::gettext;
use crate::window::RappelWindow;

/// A toast that stays while work runs, with a Cancel button that sets
/// `cancel`. Dismissed by [`Progress::finish`]. The title fits beside its
/// button at 360 px (the texts' tests).
fn toast(title: &str, cancel: &Arc<AtomicBool>) -> adw::Toast {
    let toast = adw::Toast::builder()
        .title(title)
        .use_markup(false)
        .timeout(0)
        .button_label(gettext("_Cancel"))
        // Shown at once, even over a toast that stays until dismissed
        // (`ADW_TOAST_PRIORITY_HIGH` pushes the shown toast into the queue,
        // Adw-1.gir).
        .priority(adw::ToastPriority::High)
        .build();
    let cancel = Arc::clone(cancel);
    // `button-clicked` is emitted after the button is pressed (Adw-1.gir);
    // `adw_toast_dismiss` does nothing on a toast already dismissed.
    toast.connect_button_clicked(move |toast| {
        cancel.store(true, Ordering::Relaxed);
        toast.dismiss();
    });
    toast
}

/// A controller in which Escape sets `cancel` and calls `then`.
fn escape_cancels(cancel: &Arc<AtomicBool>, then: impl Fn() + 'static) -> gtk::ShortcutController {
    let keys = gtk::ShortcutController::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let flag = Arc::clone(cancel);
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("Escape"),
        Some(gtk::CallbackAction::new(move |_, _| {
            flag.store(true, Ordering::Relaxed);
            then();
            glib::Propagation::Stop
        })),
    ));
    keys
}

/// A running job's toast and its Escape key.
pub(super) struct Progress {
    toast: adw::Toast,
    keys: gtk::ShortcutController,
    window: glib::WeakRef<RappelWindow>,
}

impl Progress {
    /// Shows the toast `title` over `window` and makes Escape cancel as
    /// the button does. The toast's button is reachable by Tab, but a
    /// toast takes no focus of its own, so Escape works wherever the
    /// focus is: the controller is in the capture phase of the window
    /// (`GtkShortcutController`, `GtkPropagationPhase`), ahead of the
    /// widget that has the focus, and the job has no dialog of its own that
    /// would want the key.
    pub(super) fn start(window: &RappelWindow, title: &str, cancel: &Arc<AtomicBool>) -> Self {
        let toast = toast(title, cancel);
        window.add_toast(toast.clone());
        let shown = toast.clone();
        let keys = escape_cancels(cancel, move || shown.dismiss());
        window.add_controller(keys.clone());
        Self {
            toast,
            keys,
            window: window.downgrade(),
        }
    }

    /// Takes the toast and the key away, when the job has ended.
    pub(super) fn finish(self) {
        self.toast.dismiss();
        if let Some(window) = self.window.upgrade() {
            window.remove_controller(&self.keys);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn escape_cancels_in_the_capture_phase_and_the_button_cancels_too() {
        crate::testing::with_widgets(|| {
            let cancel = Arc::new(AtomicBool::new(false));
            let called = Rc::new(Cell::new(false));
            let seen = Rc::clone(&called);
            let keys = escape_cancels(&cancel, move || seen.set(true));
            assert_eq!(keys.propagation_phase(), gtk::PropagationPhase::Capture);
            let shortcut = keys
                .item(0)
                .and_downcast::<gtk::Shortcut>()
                .expect("the controller has the Escape shortcut");
            assert_eq!(
                shortcut.trigger().map(|t| t.to_str().to_string()),
                Some("Escape".to_owned())
            );
            let window = gtk::Window::new();
            assert!(!cancel.load(Ordering::Relaxed));
            let action = shortcut.action().expect("the shortcut has an action");
            assert!(action.activate(gtk::ShortcutActionFlags::empty(), &window, None));
            assert!(cancel.load(Ordering::Relaxed) && called.get());

            // The button of the toast does the same.
            let cancel = Arc::new(AtomicBool::new(false));
            let toast = toast("Checking the file…", &cancel);
            assert_eq!(toast.button_label().as_deref(), Some("_Cancel"));
            toast.emit_by_name::<()>("button-clicked", &[]);
            assert!(cancel.load(Ordering::Relaxed));
        });
    }
}
