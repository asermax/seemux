pub mod tab_group;
pub mod tab_row;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self, Box as GtkBox, Button, Label, ListBox, Orientation, ScrolledWindow, SelectionMode};
use gtk4::gdk;
use gtk4::glib;

use crate::session::{Session, DEFAULT_GROUP};
use tab_group::TabGroupWidget;
use tab_row::TabRow;

pub struct Sidebar {
    pub container: GtkBox,
    content: GtkBox,

    // Default group (no header, just a ListBox at the top)
    default_list: ListBox,
    default_add_btn: Button,

    // Named groups
    groups: Rc<RefCell<Vec<GroupEntry>>>,
    group_widgets: Rc<RefCell<HashMap<String, TabGroupWidget>>>,
    new_group_btn: Button,

    // All tab rows indexed by session ID
    rows: Rc<RefCell<HashMap<String, (TabRow, String)>>>, // session_id -> (row, group_id)

    // Shared state: session ID of the currently dragged tab (empty when not dragging)
    dragging_id: Rc<RefCell<String>>,

    // Shared state: group ID of the currently dragged group (empty when not dragging)
    dragging_group_id: Rc<RefCell<String>>,

    // "Groups" section header — stored so it can be used as a drop target for position 0
    groups_header: Label,

    // Callback when a tab is moved/reordered via drag-and-drop
    on_tab_moved: Rc<RefCell<Option<Box<dyn Fn(String, String, i32)>>>>, // (session_id, group_id, position)
}

struct GroupEntry {
    id: String,
    name: String,
}

impl Sidebar {
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.add_css_class("sidebar");
        container.set_width_request(120);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);

        let content = GtkBox::new(Orientation::Vertical, 0);

        // Default group: just a ListBox + add button
        let default_list = ListBox::new();
        default_list.set_selection_mode(SelectionMode::None);
        default_list.set_size_request(-1, 36);

        let placeholder = Label::new(Some("No tabs yet"));
        placeholder.add_css_class("dim-label");
        placeholder.set_margin_top(8);
        placeholder.set_margin_bottom(8);
        default_list.set_placeholder(Some(&placeholder));

        let default_add_btn = Button::new();
        default_add_btn.set_label("+ Add tab");
        default_add_btn.add_css_class("sidebar-action-btn");

        // New Group button
        let new_group_btn = Button::new();
        new_group_btn.set_label("+ New Group");
        new_group_btn.add_css_class("sidebar-action-btn");

        // Section titles — also serve as drop targets for inserting at position 0
        let sessions_header = Label::new(Some("Sessions"));
        sessions_header.add_css_class("sidebar-section-title");
        sessions_header.set_xalign(0.0);

        let groups_header = Label::new(Some("Groups"));
        groups_header.add_css_class("sidebar-section-title");
        groups_header.set_xalign(0.0);

        content.append(&sessions_header);
        content.append(&default_list);
        content.append(&default_add_btn);
        content.append(&groups_header);
        content.append(&new_group_btn);

        scroll.set_child(Some(&content));
        container.append(&scroll);

        let on_tab_moved: Rc<RefCell<Option<Box<dyn Fn(String, String, i32)>>>> =
            Rc::new(RefCell::new(None));

        let sidebar = Self {
            container,
            content,
            default_list: default_list.clone(),
            default_add_btn,
            groups: Rc::new(RefCell::new(Vec::new())),
            group_widgets: Rc::new(RefCell::new(HashMap::new())),
            new_group_btn,
            rows: Rc::new(RefCell::new(HashMap::new())),
            dragging_id: Rc::new(RefCell::new(String::new())),
            dragging_group_id: Rc::new(RefCell::new(String::new())),
            groups_header,
            on_tab_moved,
        };

        sidebar.setup_header_drop_target(&sessions_header, DEFAULT_GROUP);
        sidebar.setup_list_fallback_drop_target(&default_list, DEFAULT_GROUP);
        sidebar.setup_groups_header_drop_target();
        sidebar
    }

    pub fn set_on_tab_moved<F: Fn(String, String, i32) + 'static>(&self, f: F) {
        *self.on_tab_moved.borrow_mut() = Some(Box::new(f));
    }

    fn list_for_group(&self, group_id: &str) -> Option<ListBox> {
        if group_id == DEFAULT_GROUP {
            return Some(self.default_list.clone());
        }

        self.group_widgets.borrow().get(group_id).map(|gw| gw.list_box.clone())
    }

    /// Drop target on the group header — inserts at the first position in the group.
    /// Also serves as the drop zone for collapsed or empty groups.
    fn setup_header_drop_target(&self, widget: &impl IsA<gtk4::Widget>, group_id: &str) {
        let drop_target = gtk4::DropTarget::new(glib::GString::static_type(), gdk::DragAction::MOVE);

        let group_widgets_enter = self.group_widgets.clone();
        let default_list_enter = self.default_list.clone();
        let target_group_enter = group_id.to_string();
        let dragging_enter = self.dragging_id.clone();

        drop_target.connect_enter(move |_target, _x, _y| {
            let dragged = dragging_enter.borrow();

            let list = if target_group_enter == DEFAULT_GROUP {
                Some(default_list_enter.clone())
            } else {
                group_widgets_enter.borrow().get(&target_group_enter).map(|gw| gw.list_box.clone())
            };

            if let Some(list) = list {
                // Find the first non-dragged row to show the indicator on
                let mut idx = 0;

                while let Some(row) = list.row_at_index(idx) {
                    if let Some(child) = row.child() {
                        if child.widget_name().as_str() != dragged.as_str() {
                            child.add_css_class("drop-before");
                            break;
                        }
                    }
                    idx += 1;
                }
            }

            gdk::DragAction::MOVE
        });

        let group_widgets_leave = self.group_widgets.clone();
        let default_list_leave = self.default_list.clone();
        let target_group_leave = group_id.to_string();

        drop_target.connect_leave(move |_target| {
            let list = if target_group_leave == DEFAULT_GROUP {
                Some(default_list_leave.clone())
            } else {
                group_widgets_leave.borrow().get(&target_group_leave).map(|gw| gw.list_box.clone())
            };

            if let Some(list) = list {
                let mut idx = 0;

                while let Some(row) = list.row_at_index(idx) {
                    if let Some(child) = row.child() {
                        child.remove_css_class("drop-before");
                    }
                    idx += 1;
                }
            }
        });

        let rows = self.rows.clone();
        let group_widgets = self.group_widgets.clone();
        let default_list = self.default_list.clone();
        let target_group = group_id.to_string();
        let on_moved = self.on_tab_moved.clone();

        drop_target.connect_drop(move |_target, value, _x, _y| {
            let Ok(session_id) = value.get::<glib::GString>() else { return false };
            let session_id = session_id.to_string();

            let mut rows_ref = rows.borrow_mut();
            let Some((row, current_group)) = rows_ref.get_mut(&session_id) else { return false };

            // Remove from old ListBox
            if let Some(parent) = row.widget().parent() {
                let old_list = if *current_group == DEFAULT_GROUP {
                    Some(default_list.clone())
                } else {
                    group_widgets.borrow().get(current_group.as_str()).map(|gw| gw.list_box.clone())
                };

                if let Some(list) = old_list {
                    list.remove(&parent);
                }
            }

            // Insert at first position in target ListBox
            let new_list = if target_group == DEFAULT_GROUP {
                Some(default_list.clone())
            } else {
                group_widgets.borrow().get(target_group.as_str()).map(|gw| gw.list_box.clone())
            };

            if let Some(list) = new_list {
                list.insert(row.widget(), 0);
            }

            // Expand target group if collapsed
            if target_group != DEFAULT_GROUP {
                if let Some(gw) = group_widgets.borrow().get(target_group.as_str()) {
                    gw.expand();
                }
            }

            *current_group = target_group.clone();
            drop(rows_ref);

            if let Some(ref callback) = *on_moved.borrow() {
                callback(session_id, target_group.clone(), 0);
            }

            true
        });

        widget.as_ref().add_controller(drop_target);
    }

    /// Fallback drop target on a ListBox — catches drops below all rows (appends to end).
    /// Shows a highlight when the list is empty so the drop zone is visible.
    fn setup_list_fallback_drop_target(&self, list: &ListBox, group_id: &str) {
        let drop_target = gtk4::DropTarget::new(glib::GString::static_type(), gdk::DragAction::MOVE);

        let list_ref = list.clone();
        drop_target.connect_enter(move |_target, _x, _y| {
            if list_ref.row_at_index(0).is_none() {
                list_ref.add_css_class("drop-target-highlight");
            }
            gdk::DragAction::MOVE
        });

        let list_ref = list.clone();
        drop_target.connect_leave(move |_target| {
            list_ref.remove_css_class("drop-target-highlight");
        });

        let rows = self.rows.clone();
        let group_widgets = self.group_widgets.clone();
        let default_list = self.default_list.clone();
        let target_group = group_id.to_string();
        let on_moved = self.on_tab_moved.clone();

        drop_target.connect_drop(move |_target, value, _x, _y| {
            let Ok(session_id) = value.get::<glib::GString>() else { return false };
            let session_id = session_id.to_string();

            let mut rows_ref = rows.borrow_mut();
            let Some((row, current_group)) = rows_ref.get_mut(&session_id) else { return false };

            // Remove from old ListBox
            if let Some(parent) = row.widget().parent() {
                let old_list = if *current_group == DEFAULT_GROUP {
                    Some(default_list.clone())
                } else {
                    group_widgets.borrow().get(current_group.as_str()).map(|gw| gw.list_box.clone())
                };

                if let Some(list) = old_list {
                    list.remove(&parent);
                }
            }

            // Append to target ListBox
            let new_list = if target_group == DEFAULT_GROUP {
                Some(default_list.clone())
            } else {
                group_widgets.borrow().get(target_group.as_str()).map(|gw| gw.list_box.clone())
            };

            if let Some(list) = new_list {
                list.append(row.widget());
            }

            // Expand target group if collapsed
            if target_group != DEFAULT_GROUP {
                if let Some(gw) = group_widgets.borrow().get(target_group.as_str()) {
                    gw.expand();
                }
            }

            *current_group = target_group.clone();
            drop(rows_ref);

            if let Some(ref callback) = *on_moved.borrow() {
                callback(session_id, target_group.clone(), -1);
            }

            true
        });

        list.add_controller(drop_target);
    }

    /// Drop target on the "Groups" section header — moves dragged group to first position.
    fn setup_groups_header_drop_target(&self) {
        let drop_target = gtk4::DropTarget::new(glib::Variant::static_type(), gdk::DragAction::MOVE);

        let dragging_enter = self.dragging_group_id.clone();
        let group_widgets_enter = self.group_widgets.clone();
        let groups_enter = self.groups.clone();

        drop_target.connect_enter(move |_target, _x, _y| {
            let dragged = dragging_enter.borrow();

            if dragged.is_empty() {
                return gdk::DragAction::MOVE;
            }

            // Show drop-before on the first non-dragged group
            for entry in groups_enter.borrow().iter() {
                if entry.id != *dragged {
                    if let Some(gw) = group_widgets_enter.borrow().get(&entry.id) {
                        gw.container.add_css_class("drop-before-group");
                        break;
                    }
                }
            }

            gdk::DragAction::MOVE
        });

        let group_widgets_leave = self.group_widgets.clone();

        drop_target.connect_leave(move |_target| {
            for gw in group_widgets_leave.borrow().values() {
                gw.container.remove_css_class("drop-before-group");
            }
        });

        let content = self.content.clone();
        let groups_header = self.groups_header.clone();
        let groups = self.groups.clone();
        let group_widgets = self.group_widgets.clone();

        drop_target.connect_drop(move |_target, value, _x, _y| {
            let Ok(variant) = value.get::<glib::Variant>() else { return false };
            let Some(group_id) = variant.get::<String>() else { return false };

            // Already first group — no-op
            let groups_ref = groups.borrow();
            if groups_ref.first().is_some_and(|g| g.id == group_id) {
                return false;
            }
            drop(groups_ref);

            // Reorder GTK widget: move after groups_header
            if let Some(gw) = group_widgets.borrow().get(&group_id) {
                content.reorder_child_after(gw.widget(), Some(groups_header.upcast_ref::<gtk4::Widget>()));
            }

            // Reorder groups Vec: move to index 0
            let mut groups_ref = groups.borrow_mut();
            if let Some(idx) = groups_ref.iter().position(|g| g.id == group_id) {
                let entry = groups_ref.remove(idx);
                groups_ref.insert(0, entry);
            }

            true
        });

        self.groups_header.add_controller(drop_target);
    }

    /// Per-group drop target — determines exact insertion position among groups.
    fn setup_group_drop_target(&self, group_id: &str) {
        let content = self.content.clone();
        let groups = self.groups.clone();
        let group_widgets = self.group_widgets.clone();

        let gw = self.group_widgets.borrow();
        let Some(target_widget) = gw.get(group_id) else { return };

        target_widget.setup_drop_target(self.dragging_group_id.clone(), move |dragged_id, target_id| {
            // Reorder GTK widget: move dragged after target
            let widgets = group_widgets.borrow();
            let Some(dragged_widget) = widgets.get(&dragged_id) else { return };
            let Some(target_widget) = widgets.get(&target_id) else { return };

            content.reorder_child_after(dragged_widget.widget(), Some(target_widget.widget()));
            drop(widgets);

            // Reorder groups Vec
            let mut groups_ref = groups.borrow_mut();
            let Some(dragged_idx) = groups_ref.iter().position(|g| g.id == dragged_id) else { return };
            let entry = groups_ref.remove(dragged_idx);

            let target_idx = groups_ref.iter().position(|g| g.id == target_id)
                .map(|i| i + 1)
                .unwrap_or(groups_ref.len());

            groups_ref.insert(target_idx, entry);
        });
    }

    /// Per-row drop target — determines exact insertion position within a group.
    fn setup_row_drop_target(&self, tab_row: &TabRow) {
        let rows = self.rows.clone();
        let group_widgets = self.group_widgets.clone();
        let default_list = self.default_list.clone();
        let on_moved = self.on_tab_moved.clone();

        tab_row.setup_drop_target(self.dragging_id.clone(), move |dragged_id, target_id| {
            let mut rows_ref = rows.borrow_mut();

            let Some((_, target_group_str)) = rows_ref.get(&target_id) else { return };
            let target_group = target_group_str.clone();

            // Find the target ListBox and the target row's index
            let target_list = if target_group == DEFAULT_GROUP {
                default_list.clone()
            } else {
                let Some(list) = group_widgets.borrow().get(&target_group).map(|gw| gw.list_box.clone())
                    else { return };
                list
            };

            let Some((target_row, _)) = rows_ref.get(&target_id) else { return };
            let Some(target_index) = row_index_in_list(target_row.widget(), &target_list) else { return };

            let Some((dragged_row, current_group)) = rows_ref.get_mut(&dragged_id) else { return };
            let current_group_str = current_group.clone();

            let same_list = current_group_str == target_group;

            // Find dragged row's current index (needed for same-list adjustment)
            let dragged_index = if same_list {
                row_index_in_list(dragged_row.widget(), &target_list)
            } else {
                None
            };

            // Remove from old ListBox
            if let Some(parent) = dragged_row.widget().parent() {
                let old_list = if current_group_str == DEFAULT_GROUP {
                    Some(default_list.clone())
                } else {
                    group_widgets.borrow().get(current_group_str.as_str()).map(|gw| gw.list_box.clone())
                };

                if let Some(list) = old_list {
                    list.remove(&parent);
                }
            }

            // Always insert after the target row
            let mut insert_index = target_index + 1;

            // Adjust for same-list removal shifting indices down
            if same_list {
                if let Some(di) = dragged_index {
                    if di < insert_index {
                        insert_index -= 1;
                    }
                }
            }

            // Insert at computed position
            target_list.insert(dragged_row.widget(), insert_index);

            // Expand target group if collapsed
            if target_group != DEFAULT_GROUP {
                if let Some(gw) = group_widgets.borrow().get(target_group.as_str()) {
                    gw.expand();
                }
            }

            *current_group = target_group.clone();
            drop(rows_ref);

            if let Some(ref callback) = *on_moved.borrow() {
                callback(dragged_id, target_group, insert_index);
            }
        });
    }

    pub fn add_tab(&self, session: &Session, after: Option<&str>) {
        let tab_row = TabRow::new(&session.id, &session.title);
        tab_row.setup_drag_source(self.dragging_id.clone());
        self.setup_row_drop_target(&tab_row);

        let list = self.list_for_group(&session.group_id)
            .unwrap_or_else(|| self.default_list.clone());

        // Insert after the specified session if it's in the same group, otherwise append
        let inserted = after
            .and_then(|id| self.rows.borrow().get(id).map(|(r, g)| (r.widget().clone(), g.clone())))
            .filter(|(_, group)| *group == session.group_id)
            .and_then(|(widget, _)| row_index_in_list(&widget, &list))
            .map(|index| list.insert(tab_row.widget(), index + 1));

        if inserted.is_none() {
            list.append(tab_row.widget());
        }

        self.rows.borrow_mut().insert(
            session.id.clone(),
            (tab_row, session.group_id.clone()),
        );
    }

    pub fn remove_tab(&self, session_id: &str) {
        if let Some((row, group_id)) = self.rows.borrow_mut().remove(session_id) {
            if let Some(parent) = row.widget().parent() {
                if let Some(list) = self.list_for_group(&group_id) {
                    list.remove(&parent);
                }
            }
        }
    }

    pub fn set_active(&self, session_id: &str) {
        let rows = self.rows.borrow();

        for (id, (row, _)) in rows.iter() {
            row.set_active(id == session_id);
        }
    }

    pub fn update_title(&self, session_id: &str, title: &str) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.set_title(title);
        }
    }

    pub fn update_cwd(&self, session_id: &str, folder_name: &str, display_path: &str) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.update_cwd(folder_name, display_path);
        }
    }

    pub fn update_badge(&self, session_id: &str, count: u32) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.set_badge_count(count);
        }
    }

    pub fn update_status(&self, session_id: &str, status: &crate::session::SessionStatus, label_override: Option<&str>) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.set_status(status, label_override);
        }
    }

    pub fn update_branch(&self, session_id: &str, branch: Option<&str>) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.set_branch(branch);
        }
    }

    pub fn update_notification_preview(&self, session_id: &str, text: Option<&str>) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.set_notification_preview(text);
        }
    }

    pub fn setup_context_menu(&self, session_id: &str) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.setup_context_menu(session_id);
        }
    }

    pub fn show_tab_indices(&self) {
        let visible = self.ordered_visible_session_ids();
        let rows = self.rows.borrow();

        // Hide all first to clear stale indices on collapsed-group tabs
        for (row, _) in rows.values() {
            row.set_index_visible(None);
        }

        for (i, id) in visible.iter().enumerate() {
            if let Some((row, _)) = rows.get(id) {
                row.set_index_visible(if i < 9 { Some((i + 1) as u32) } else { None });
            }
        }
    }

    pub fn hide_tab_indices(&self) {
        for (row, _) in self.rows.borrow().values() {
            row.set_index_visible(None);
        }
    }

    pub fn wire_tab_click<F: Fn(String) + Clone + 'static>(&self, session_id: &str, f: F) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            let id = session_id.to_string();
            let gesture = gtk4::GestureClick::new();
            gesture.set_button(1);
            let f = f.clone();
            gesture.connect_released(move |gesture, n_press, _, _| {
                if n_press == 1 {
                    gesture.set_state(gtk4::EventSequenceState::Claimed);
                    f(id.clone());
                }
            });
            row.widget().add_controller(gesture);
        }
    }

    pub fn connect_new_tab<F: Fn() + Clone + 'static>(&self, f: F) {
        self.default_add_btn.connect_clicked({
            let f = f.clone();
            move |_| f()
        });
    }

    pub fn connect_new_group<F: Fn() + 'static>(&self, f: F) {
        self.new_group_btn.connect_clicked(move |_| f());
    }

    pub fn add_group(&self, id: &str, name: &str) {
        let group_widget = TabGroupWidget::new(id, name);
        group_widget.setup_context_menu(id);
        group_widget.setup_drag_source(self.dragging_group_id.clone());

        // Per-row drop targets handle positioning; header handles collapsed/empty groups
        self.setup_list_fallback_drop_target(&group_widget.list_box, id);
        self.setup_header_drop_target(group_widget.header_widget(), id);

        // Insert before the new_group_btn
        self.content.remove(&self.new_group_btn);
        self.content.append(group_widget.widget());
        self.content.append(&self.new_group_btn);

        self.groups.borrow_mut().push(GroupEntry {
            id: id.to_string(),
            name: name.to_string(),
        });
        self.group_widgets.borrow_mut().insert(id.to_string(), group_widget);

        // Group drop target must be set up after inserting into group_widgets
        self.setup_group_drop_target(id);
    }

    pub fn remove_group(&self, group_id: &str) {
        // Move tabs from this group back to default
        let mut rows = self.rows.borrow_mut();
        let session_ids: Vec<String> = rows.iter()
            .filter(|(_, (_, gid))| gid == group_id)
            .map(|(sid, _)| sid.clone())
            .collect();

        for sid in &session_ids {
            if let Some((row, gid)) = rows.get_mut(sid) {
                if let Some(parent) = row.widget().parent() {
                    if let Some(list) = self.list_for_group(gid) {
                        list.remove(&parent);
                    }
                }
                self.default_list.append(row.widget());
                *gid = crate::session::DEFAULT_GROUP.to_string();
            }
        }
        drop(rows);

        // Remove the group widget
        if let Some(gw) = self.group_widgets.borrow_mut().remove(group_id) {
            self.content.remove(gw.widget());
        }
        self.groups.borrow_mut().retain(|g| g.id != group_id);
    }

    /// Count tabs belonging to a specific group.
    pub fn tab_count_in_group(&self, group_id: &str) -> usize {
        self.rows.borrow().values()
            .filter(|(_, gid)| gid == group_id)
            .count()
    }

    pub fn expand_group(&self, group_id: &str) {
        if let Some(gw) = self.group_widgets.borrow().get(group_id) {
            gw.expand();
        }
    }

    pub fn collapse_group(&self, group_id: &str) {
        if let Some(gw) = self.group_widgets.borrow().get(group_id) {
            gw.collapse();
        }
    }

    pub fn connect_group_new_tab<F: Fn(String) + Clone + 'static>(&self, group_id: &str, f: F) {
        if let Some(gw) = self.group_widgets.borrow().get(group_id) {
            let gid = group_id.to_string();
            gw.add_btn.connect_clicked(move |_| f(gid.clone()));
        }
    }

    pub fn wire_close_button<F: Fn(String) + Clone + 'static>(&self, session_id: &str, f: F) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            let id = session_id.to_string();
            row.connect_close(move || f(id.clone()));
        }
    }

    pub fn group_ids(&self) -> Vec<(String, String, bool)> {
        let widgets = self.group_widgets.borrow();

        self.groups.borrow().iter().map(|g| {
            let collapsed = widgets.get(&g.id).is_some_and(|gw| gw.is_collapsed());
            (g.id.clone(), g.name.clone(), collapsed)
        }).collect()
    }

    /// Return visible session IDs (skipping collapsed groups) in sidebar order.
    pub fn ordered_visible_session_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();

        collect_ids_from_list(&self.default_list, &mut ids);

        for entry in self.groups.borrow().iter() {
            if let Some(gw) = self.group_widgets.borrow().get(&entry.id) {
                if !gw.is_collapsed() {
                    collect_ids_from_list(&gw.list_box, &mut ids);
                }
            }
        }

        ids
    }

    /// Return all session IDs in visual sidebar order:
    /// default group rows first, then each named group's rows in display order.
    pub fn ordered_session_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();

        collect_ids_from_list(&self.default_list, &mut ids);

        for entry in self.groups.borrow().iter() {
            if let Some(gw) = self.group_widgets.borrow().get(&entry.id) {
                collect_ids_from_list(&gw.list_box, &mut ids);
            }
        }

        ids
    }

    /// Return group IDs in visual sidebar order: default group first, then named groups.
    pub fn ordered_group_ids(&self) -> Vec<String> {
        let mut group_ids = vec![crate::session::DEFAULT_GROUP.to_string()];

        for entry in self.groups.borrow().iter() {
            group_ids.push(entry.id.clone());
        }

        group_ids
    }

    /// Return visible group IDs (skipping collapsed groups) in sidebar order.
    pub fn ordered_visible_group_ids(&self) -> Vec<String> {
        let mut group_ids = vec![crate::session::DEFAULT_GROUP.to_string()];

        for entry in self.groups.borrow().iter() {
            if let Some(gw) = self.group_widgets.borrow().get(&entry.id) {
                if !gw.is_collapsed() {
                    group_ids.push(entry.id.clone());
                }
            }
        }

        group_ids
    }

    /// Return the first session ID in a group (visual order), or None if empty.
    pub fn first_session_id_in_group(&self, group_id: &str) -> Option<String> {
        let list = self.list_for_group(group_id)?;

        list.row_at_index(0)
            .and_then(|row| row.child())
            .map(|child| child.widget_name().to_string())
    }

    /// Return session IDs in visual order within a specific group.
    pub fn ordered_session_ids_in_group(&self, group_id: &str) -> Vec<String> {
        let Some(list) = self.list_for_group(group_id) else { return Vec::new() };
        let mut ids = Vec::new();
        collect_ids_from_list(&list, &mut ids);
        ids
    }

    /// Return the group ID that a session belongs to.
    pub fn group_id_for_session(&self, session_id: &str) -> Option<String> {
        self.rows.borrow().get(session_id).map(|(_, gid)| gid.clone())
    }
}

fn collect_ids_from_list(list: &ListBox, ids: &mut Vec<String>) {
    let mut idx = 0;

    while let Some(row) = list.row_at_index(idx) {
        if let Some(child) = row.child() {
            ids.push(child.widget_name().to_string());
        }

        idx += 1;
    }
}

/// Find the index of a tab row widget within its parent ListBox.
fn row_index_in_list(tab_widget: &gtk4::Widget, list: &ListBox) -> Option<i32> {
    let list_box_row = tab_widget.parent()?;
    let mut index = 0;
    let mut child = list.first_child();

    while let Some(c) = child {
        if c == list_box_row {
            return Some(index);
        }

        index += 1;
        child = c.next_sibling();
    }

    None
}
