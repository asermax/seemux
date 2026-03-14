use std::cell::Cell;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, GestureClick, Label, ListBox, Orientation, SelectionMode, Widget};

/// A named, collapsible group of tabs in the sidebar.
pub struct TabGroupWidget {
    pub container: GtkBox,
    header: GtkBox,
    name_label: Label,
    toggle_label: Label,
    pub add_btn: Button,
    pub list_box: ListBox,
    collapsed: Cell<bool>,
}

impl TabGroupWidget {
    pub fn new(name: &str) -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.add_css_class("tab-group");

        // Header: [toggle_icon] [name] ... [+]
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

        container.append(&header);
        container.append(&list_box);

        let collapsed = Cell::new(false);

        // Click anywhere on the header toggles collapse (GestureClick on header)
        let list_box_toggle = list_box.clone();
        let collapsed_ref = collapsed.clone();
        let toggle_label_ref = toggle_label.clone();

        let gesture = GestureClick::new();
        gesture.set_button(1);
        gesture.connect_released(move |gesture, _n_press, _x, _y| {
            // Don't toggle if the click was on the add button (it has its own handler)
            // The gesture is on the header, so if the add button consumed the event, we won't get here
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
            name_label,
            toggle_label,
            add_btn,
            list_box,
            collapsed,
        }
    }

    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    pub fn set_name(&self, name: &str) {
        self.name_label.set_text(name);
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed.get()
    }
}
