use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::gio;
use gtk4::gdk;
use gtk4::glib;
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
    pub fn new(id: &str, name: &str) -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.add_css_class("tab-group");
        container.set_widget_name(id);

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

    pub fn is_collapsed(&self) -> bool {
        self.collapsed.get()
    }

    /// Expand the group if it's collapsed.
    pub fn expand(&self) {
        if self.collapsed.get() {
            self.collapsed.set(false);
            self.list_box.set_visible(true);
            self.toggle_label.set_text("\u{25bc}"); // ▼
        }
    }

    pub fn setup_drag_source(&self, dragging_group_id: Rc<RefCell<String>>) {
        let drag_source = gtk4::DragSource::new();
        drag_source.set_actions(gdk::DragAction::MOVE);

        let container = self.container.clone();
        let dragging_prepare = dragging_group_id.clone();
        drag_source.connect_prepare(move |_source, _x, _y| {
            let group_id = container.widget_name().to_string();
            *dragging_prepare.borrow_mut() = group_id.clone();
            Some(gdk::ContentProvider::for_value(&group_id.to_variant().to_value()))
        });

        let container = self.container.clone();
        drag_source.connect_drag_begin(move |source, _drag| {
            let paintable = gtk4::WidgetPaintable::new(Some(&container));
            source.set_icon(Some(&paintable), 0, 0);
            container.add_css_class("dragging");
        });

        let container = self.container.clone();
        let dragging_end = dragging_group_id.clone();
        drag_source.connect_drag_end(move |_source, _drag, _delete_data| {
            container.remove_css_class("dragging");
            dragging_end.borrow_mut().clear();
        });

        self.header.add_controller(drag_source);
    }

    /// Set up a drop target for group reordering.  Uses `Variant` content type
    /// so it never conflicts with tab DropTargets (which use `GString`).
    /// The `on_drop` callback receives `(dragged_group_id, this_group_id)`.
    pub fn setup_drop_target<F: Fn(String, String) + 'static>(
        &self,
        dragging_group_id: Rc<RefCell<String>>,
        on_drop: F,
    ) {
        let drop_target = gtk4::DropTarget::new(glib::Variant::static_type(), gdk::DragAction::MOVE);

        let container = self.container.clone();
        let dragging_motion = dragging_group_id.clone();
        drop_target.connect_motion(move |_target, _x, _y| {
            let dragged = dragging_motion.borrow();

            if dragged.is_empty() {
                return gdk::DragAction::MOVE;
            }

            let target_id = container.widget_name();

            // Don't show indicator on self
            if dragged.as_str() == target_id.as_str() {
                return gdk::DragAction::MOVE;
            }

            // Don't show indicator on the group directly above the dragged group —
            // dropping after it would leave the dragged group in the same position.
            let is_above_dragged = container.next_sibling()
                .is_some_and(|next| next.widget_name().as_str() == dragged.as_str());

            if is_above_dragged {
                return gdk::DragAction::MOVE;
            }

            container.add_css_class("drop-after-group");
            gdk::DragAction::MOVE
        });

        let container = self.container.clone();
        drop_target.connect_leave(move |_target| {
            container.remove_css_class("drop-after-group");
        });

        let container = self.container.clone();
        let on_drop = Rc::new(on_drop);
        drop_target.connect_drop(move |_target, value, _x, _y| {
            container.remove_css_class("drop-after-group");

            let Ok(variant) = value.get::<glib::Variant>() else { return false };
            let Some(group_id) = variant.get::<String>() else { return false };
            let target_id = container.widget_name().to_string();

            if group_id == target_id {
                return false;
            }

            let is_above_dragged = container.next_sibling()
                .is_some_and(|next| next.widget_name().as_str() == group_id.as_str());

            if is_above_dragged {
                return false;
            }

            on_drop(group_id, target_id);
            true
        });

        self.container.add_controller(drop_target);
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
