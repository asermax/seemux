use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{EventControllerKey, Orientation, Overlay, gdk::Key, glib};

use crate::session::manager::SessionManager;

/// Show a centered "New Group" form as an overlay child.
pub(crate) fn show_new_group_overlay<F: Fn(String) + 'static>(
    overlay: &Overlay,
    manager: &Rc<RefCell<SessionManager>>,
    on_create: F,
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

    let label = Label::new(Some("Group name:"));
    label.set_xalign(0.0);

    let entry = Entry::new();
    entry.set_placeholder_text(Some("Enter group name"));

    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let create_btn = Button::with_label("Create");
    create_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&create_btn);

    card.append(&label);
    card.append(&entry);
    card.append(&btn_box);

    overlay.add_overlay(&card);

    let on_create = Rc::new(on_create);

    let overlay_cancel = overlay.clone();
    let card_cancel = card.clone();
    let mgr_cancel = manager.clone();
    cancel_btn.connect_clicked(move |_| {
        overlay_cancel.remove_overlay(&card_cancel);
        super::refocus_terminal(&mgr_cancel);
    });

    let overlay_create = overlay.clone();
    let card_create = card.clone();
    let entry_create = entry.clone();
    let on_create_btn = on_create.clone();
    let mgr_create = manager.clone();
    create_btn.connect_clicked(move |_| {
        let name = entry_create.text().to_string();

        if !name.is_empty() {
            on_create_btn(name);
        }

        overlay_create.remove_overlay(&card_create);
        super::refocus_terminal(&mgr_create);
    });

    let overlay_enter = overlay.clone();
    let card_enter = card.clone();
    let mgr_enter = manager.clone();
    entry.connect_activate(move |entry| {
        let name = entry.text().to_string();

        if !name.is_empty() {
            on_create(name);
        }

        overlay_enter.remove_overlay(&card_enter);
        super::refocus_terminal(&mgr_enter);
    });

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
