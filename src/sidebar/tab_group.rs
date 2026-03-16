use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::gio;
use gtk4::{Box as GtkBox, Button, GestureClick, Label, ListBox, Orientation, PopoverMenu, SelectionMode, Widget};

/// A named, collapsible group of tabs in the sidebar.
pub struct TabGroupWidget {
    pub container: GtkBox,
    header: GtkBox,
    pub add_btn: Button,
    pub list_box: ListBox,
    collapsed: Rc<Cell<bool>>,
    toggle_label: Label,
}

impl TabGroupWidget {
    pub fn new(name: &str) -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.add_css_class("tab-group");

        let header = GtkBox::new(Orientation::Horizontal, 4);
        header.add_css_class("tab-group-header");

        let toggle_label = Label::new(Some("\u{25bc}")); // ▼
        toggle_label.add_css_class("group-toggle");

        let name_label = Label::new(Some(name));
        name_label.add_css_class("group-name");
        name_label.set_hexpand(true);
        name_label.set_xalign(0.0);
        name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let add_btn = Button::with_label("+");
        add_btn.add_css_class("group-add-btn");

        header.append(&toggle_label);
        header.append(&name_label);
        header.append(&add_btn);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.set_size_request(-1, 36);

        let placeholder = Label::new(Some("No tabs yet"));
        placeholder.add_css_class("dim-label");
        placeholder.set_margin_top(8);
        placeholder.set_margin_bottom(8);
        list_box.set_placeholder(Some(&placeholder));

        container.append(&header);
        container.append(&list_box);

        let collapsed = Rc::new(Cell::new(false));

        // Left-click on header toggles collapse
        let list_box_toggle = list_box.clone();
        let toggle_label_ref = toggle_label.clone();
        let collapsed_ref = collapsed.clone();

        let gesture = GestureClick::new();
        gesture.set_button(1);
        gesture.connect_released(move |gesture, _n_press, _x, _y| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);

            let new_state = !collapsed_ref.get();
            collapsed_ref.set(new_state);
            list_box_toggle.set_visible(!new_state);
            toggle_label_ref.set_text(if new_state { "\u{25b6}" } else { "\u{25bc}" });
        });

        header.add_controller(gesture);

        Self {
            container,
            header,
            add_btn,
            list_box,
            collapsed,
            toggle_label,
        }
    }

    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    pub fn header_widget(&self) -> &GtkBox {
        &self.header
    }

    /// Expand the group if it's collapsed.
    pub fn expand(&self) {
        if self.collapsed.get() {
            self.collapsed.set(false);
            self.list_box.set_visible(true);
            self.toggle_label.set_text("\u{25bc}"); // ▼
        }
    }

    /// Add right-click context menu with "Delete Group" action.
    pub fn setup_context_menu(&self, group_id: &str) {
        let menu = gio::Menu::new();
        menu.append(Some("Delete Group"), Some(&format!("win.group-delete('{group_id}')")));

        let popover = PopoverMenu::from_model(Some(&menu));
        popover.set_parent(&self.header);
        popover.set_has_arrow(false);

        let gesture = GestureClick::new();
        gesture.set_button(3);
        gesture.connect_released(move |gesture, _n_press, x, y| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        });

        self.header.add_controller(gesture);
    }
}
