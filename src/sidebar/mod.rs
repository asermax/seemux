pub mod tab_group;
pub mod tab_row;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self, Box as GtkBox, Button, ListBox, Orientation, ScrolledWindow, SelectionMode};
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

    // Callback when a tab is moved between groups via drag-and-drop
    on_tab_moved: Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>, // (session_id, new_group_id)
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

        let default_add_btn = Button::new();
        default_add_btn.set_label("+ Add tab");
        default_add_btn.add_css_class("sidebar-action-btn");

        // New Group button
        let new_group_btn = Button::new();
        new_group_btn.set_label("+ New Group");
        new_group_btn.add_css_class("sidebar-action-btn");

        content.append(&default_list);
        content.append(&default_add_btn);
        content.append(&new_group_btn);

        scroll.set_child(Some(&content));
        container.append(&scroll);

        let on_tab_moved: Rc<RefCell<Option<Box<dyn Fn(String, String)>>>> =
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
            on_tab_moved,
        };

        sidebar.setup_drop_target(&default_list, DEFAULT_GROUP);
        sidebar
    }

    pub fn set_on_tab_moved<F: Fn(String, String) + 'static>(&self, f: F) {
        *self.on_tab_moved.borrow_mut() = Some(Box::new(f));
    }

    fn list_for_group(&self, group_id: &str) -> Option<ListBox> {
        if group_id == DEFAULT_GROUP {
            return Some(self.default_list.clone());
        }

        self.group_widgets.borrow().get(group_id).map(|gw| gw.list_box.clone())
    }

    fn setup_drop_target(&self, widget: &impl IsA<gtk4::Widget>, group_id: &str) {
        let drop_target = gtk4::DropTarget::new(glib::GString::static_type(), gdk::DragAction::MOVE);

        let widget_ref = widget.as_ref().clone();
        drop_target.connect_enter(move |_target, _x, _y| {
            widget_ref.add_css_class("drop-target-highlight");
            gdk::DragAction::MOVE
        });

        let widget_ref = widget.as_ref().clone();
        drop_target.connect_leave(move |_target| {
            widget_ref.remove_css_class("drop-target-highlight");
        });

        let rows = self.rows.clone();
        let group_widgets = self.group_widgets.clone();
        let default_list = self.default_list.clone();
        let target_group = group_id.to_string();
        let on_moved = self.on_tab_moved.clone();

        drop_target.connect_drop(move |target, value, _x, _y| {
            if let Some(w) = target.widget() {
                w.remove_css_class("drop-target-highlight");
            }

            let Ok(session_id) = value.get::<glib::GString>() else { return false };
            let session_id = session_id.to_string();

            let mut rows_ref = rows.borrow_mut();
            let Some((row, current_group)) = rows_ref.get_mut(&session_id) else { return false };

            if *current_group == target_group {
                return false;
            }

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
                callback(session_id, target_group.clone());
            }

            true
        });

        widget.as_ref().add_controller(drop_target);
    }

    pub fn add_tab(&self, session: &Session) {
        let tab_row = TabRow::new(&session.id, &session.title);
        tab_row.setup_drag_source();

        if let Some(list) = self.list_for_group(&session.group_id) {
            list.append(tab_row.widget());
        } else {
            // Fallback to default
            self.default_list.append(tab_row.widget());
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

    pub fn update_badge(&self, session_id: &str, count: u32) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.set_badge_count(count);
        }
    }

    pub fn update_status(&self, session_id: &str, status: &crate::session::SessionStatus) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.set_status(status);
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

    pub fn trigger_rename<F: Fn(String) + Clone + 'static>(&self, session_id: &str, on_rename: F) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            row.start_rename(on_rename);
        }
    }

    pub fn wire_rename<F: Fn(String, String) + Clone + 'static>(&self, session_id: &str, f: F) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            let id = session_id.to_string();
            row.connect_rename(move |new_title| f(id.clone(), new_title));
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
        let group_widget = TabGroupWidget::new(name);
        group_widget.setup_context_menu(id);

        // Set up drop targets on the group's list and header
        self.setup_drop_target(&group_widget.list_box, id);
        self.setup_drop_target(group_widget.header_widget(), id);

        // Insert before the new_group_btn
        self.content.remove(&self.new_group_btn);
        self.content.append(group_widget.widget());
        self.content.append(&self.new_group_btn);

        self.groups.borrow_mut().push(GroupEntry {
            id: id.to_string(),
            name: name.to_string(),
        });
        self.group_widgets.borrow_mut().insert(id.to_string(), group_widget);
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

    pub fn group_ids(&self) -> Vec<(String, String)> {
        self.groups.borrow().iter().map(|g| (g.id.clone(), g.name.clone())).collect()
    }
}
