use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{EventControllerKey, Orientation, Overlay, gdk::Key, glib};

use crate::session::manager::SessionManager;

/// Generic centered entry-form overlay. Both "New Group" and "Rename Group" are thin wrappers.
fn show_entry_overlay<F: Fn(String) + 'static>(
    overlay: &Overlay,
    manager: &Rc<RefCell<SessionManager>>,
    label_text: &str,
    prefill: Option<&str>,
    placeholder: Option<&str>,
    button_label: &str,
    on_submit: F,
) {
    use gtk4::{Box as GtkBox, Button, Entry, Label};

    let card = GtkBox::new(Orientation::Vertical, 12);
    card.add_css_class("overlay-card");
    card.set_halign(gtk4::Align::Center);
    card.set_valign(gtk4::Align::Center);
    card.set_margin_top(16);
    card.set_margin_bottom(16);
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_width_request(300);

    let label = Label::new(Some(label_text));
    label.set_xalign(0.0);

    let entry = Entry::new();

    if let Some(text) = prefill {
        entry.set_text(text);
    }

    if let Some(text) = placeholder {
        entry.set_placeholder_text(Some(text));
    }

    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let submit_btn = Button::with_label(button_label);
    submit_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&submit_btn);

    card.append(&label);
    card.append(&entry);
    card.append(&btn_box);

    overlay.add_overlay(&card);

    let overlay_cancel = overlay.clone();
    let card_cancel = card.clone();
    let mgr_cancel = manager.clone();
    cancel_btn.connect_clicked(move |_| {
        overlay_cancel.remove_overlay(&card_cancel);
        super::refocus_terminal(&mgr_cancel);
    });

    let submit = {
        let overlay = overlay.clone();
        let card = card.clone();
        let entry = entry.clone();
        let on_submit = Rc::new(on_submit);
        let mgr = manager.clone();

        move || {
            let name = entry.text().to_string();

            if !name.is_empty() {
                on_submit(name);
            }

            overlay.remove_overlay(&card);
            super::refocus_terminal(&mgr);
        }
    };

    let submit_click = submit.clone();
    submit_btn.connect_clicked(move |_| submit_click());

    entry.connect_activate(move |_| submit());

    // Handle Escape to dismiss
    let key_controller = EventControllerKey::new();
    let overlay_esc = overlay.clone();
    let card_esc = card.clone();
    let mgr_esc = manager.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == Key::Escape {
            overlay_esc.remove_overlay(&card_esc);
            super::refocus_terminal(&mgr_esc);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    entry.add_controller(key_controller);

    entry.grab_focus();

    if prefill.is_some() {
        entry.select_region(0, -1);
    }
}

pub(crate) fn show_new_group_overlay<F: Fn(String) + 'static>(
    overlay: &Overlay,
    manager: &Rc<RefCell<SessionManager>>,
    on_create: F,
) {
    show_entry_overlay(
        overlay,
        manager,
        "Group name:",
        None,
        Some("Enter group name"),
        "Create",
        on_create,
    );
}

pub(crate) fn show_rename_group_overlay<F: Fn(String) + 'static>(
    overlay: &Overlay,
    manager: &Rc<RefCell<SessionManager>>,
    current_name: &str,
    on_rename: F,
) {
    show_entry_overlay(
        overlay,
        manager,
        "Rename group:",
        Some(current_name),
        None,
        "Rename",
        on_rename,
    );
}

/// Show a centered confirmation dialog as an overlay child.
pub(crate) fn show_confirm_overlay<F: Fn() + 'static>(
    overlay: &Overlay,
    manager: &Rc<RefCell<SessionManager>>,
    title: &str,
    detail: &str,
    on_confirm: F,
) {
    use gtk4::{Box as GtkBox, Button, Label};

    let card = GtkBox::new(Orientation::Vertical, 12);
    card.add_css_class("overlay-card");
    card.set_halign(gtk4::Align::Center);
    card.set_valign(gtk4::Align::Center);
    card.set_margin_top(16);
    card.set_margin_bottom(16);
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_width_request(300);

    let title_label = Label::new(Some(title));
    title_label.add_css_class("title-3");

    let detail_label = Label::new(Some(detail));
    detail_label.set_wrap(true);
    detail_label.set_xalign(0.0);

    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let confirm_btn = Button::with_label("Delete");
    confirm_btn.add_css_class("destructive-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&confirm_btn);

    card.append(&title_label);
    card.append(&detail_label);
    card.append(&btn_box);

    overlay.add_overlay(&card);

    let overlay_cancel = overlay.clone();
    let card_cancel = card.clone();
    let mgr_cancel = manager.clone();
    cancel_btn.connect_clicked(move |_| {
        overlay_cancel.remove_overlay(&card_cancel);
        super::refocus_terminal(&mgr_cancel);
    });

    let overlay_confirm = overlay.clone();
    let card_confirm = card.clone();
    let mgr_confirm = manager.clone();
    confirm_btn.connect_clicked(move |_| {
        on_confirm();
        overlay_confirm.remove_overlay(&card_confirm);
        super::refocus_terminal(&mgr_confirm);
    });
}
