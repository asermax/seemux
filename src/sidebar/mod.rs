pub mod tab_group;
pub mod tab_row;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self, Box as GtkBox, Button, ListBox, Orientation, ScrolledWindow, SelectionMode};

use crate::session::{Session, DEFAULT_GROUP};
use tab_group::TabGroupWidget;
use tab_row::TabRow;

#[allow(dead_code)]
pub struct Sidebar {
    pub container: GtkBox,
    scroll: ScrolledWindow,
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

    selecting: Rc<Cell<bool>>,
}

#[allow(dead_code)]
struct GroupEntry {
    id: String,
    name: String,
}

#[allow(dead_code)]
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
        default_add_btn.add_css_class("new-tab-btn");

        // New Group button
        let new_group_btn = Button::new();
        new_group_btn.set_label("+ New Group");
        new_group_btn.add_css_class("new-group-btn");

        content.append(&default_list);
        content.append(&default_add_btn);
        content.append(&new_group_btn);

        scroll.set_child(Some(&content));
        container.append(&scroll);

        Self {
            container,
            scroll,
            content,
            default_list,
            default_add_btn,
            groups: Rc::new(RefCell::new(Vec::new())),
            group_widgets: Rc::new(RefCell::new(HashMap::new())),
            new_group_btn,
            rows: Rc::new(RefCell::new(HashMap::new())),
            selecting: Rc::new(Cell::new(false)),
        }
    }

    fn list_for_group(&self, group_id: &str) -> Option<ListBox> {
        if group_id == DEFAULT_GROUP {
            return Some(self.default_list.clone());
        }

        self.group_widgets.borrow().get(group_id).map(|gw| gw.list_box.clone())
    }

    pub fn add_tab(&self, session: &Session) {
        let tab_row = TabRow::new(&session.id, &session.title);

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

    pub fn wire_rename<F: Fn(String, String) + Clone + 'static>(&self, session_id: &str, f: F) {
        if let Some((row, _)) = self.rows.borrow().get(session_id) {
            let id = session_id.to_string();
            row.connect_rename(move |new_title| f(id.clone(), new_title));
        }
    }

    pub fn connect_tab_selected<F: Fn(&str) + Clone + 'static>(&self, f: F) {
        // Wire click-to-select on each tab row instead of ListBox row-selected
        // This is handled per-row via wire_tab_click
        let _ = f;
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

    pub fn connect_group_new_tab<F: Fn(String) + Clone + 'static>(&self, group_id: &str, f: F) {
        if let Some(gw) = self.group_widgets.borrow().get(group_id) {
            let gid = group_id.to_string();
            gw.add_btn.connect_clicked(move |_| f(gid.clone()));
        }
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
                *gid = DEFAULT_GROUP.to_string();
            }
        }
        drop(rows);

        // Remove the group widget
        if let Some(gw) = self.group_widgets.borrow_mut().remove(group_id) {
            self.content.remove(gw.widget());
        }
        self.groups.borrow_mut().retain(|g| g.id != group_id);
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
