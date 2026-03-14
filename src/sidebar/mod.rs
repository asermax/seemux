pub mod tab_row;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{self, Box as GtkBox, Button, Label, ListBox, Orientation, SelectionMode};

use crate::session::Session;
use tab_row::TabRow;

pub struct Sidebar {
    pub container: GtkBox,
    list_box: ListBox,
    new_tab_btn: Button,
    rows: Rc<RefCell<HashMap<String, TabRow>>>,
}

impl Sidebar {
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.add_css_class("sidebar");
        container.set_width_request(200);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.set_vexpand(true);

        let new_tab_btn = Button::new();
        new_tab_btn.set_label("+ New Tab");
        new_tab_btn.add_css_class("new-tab-btn");

        container.append(&list_box);
        container.append(&new_tab_btn);

        Self {
            container,
            list_box,
            new_tab_btn,
            rows: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn add_tab(&self, session: &Session) {
        let tab_row = TabRow::new(&session.id, &session.title);
        self.list_box.append(tab_row.widget());
        self.rows.borrow_mut().insert(session.id.clone(), tab_row);
    }

    pub fn remove_tab(&self, session_id: &str) {
        if let Some(row) = self.rows.borrow_mut().remove(session_id) {
            self.list_box.remove(row.widget());
        }
    }

    pub fn set_active(&self, session_id: &str) {
        let rows = self.rows.borrow();

        for (id, row) in rows.iter() {
            if id == session_id {
                row.set_active(true);
                if let Some(gtk_row) = row.widget().parent().and_downcast::<gtk4::ListBoxRow>() {
                    self.list_box.select_row(Some(&gtk_row));
                }
            } else {
                row.set_active(false);
            }
        }
    }

    pub fn update_title(&self, session_id: &str, title: &str) {
        if let Some(row) = self.rows.borrow().get(session_id) {
            row.set_title(title);
        }
    }

    pub fn update_badge(&self, session_id: &str, count: u32) {
        if let Some(row) = self.rows.borrow().get(session_id) {
            row.set_badge_count(count);
        }
    }

    pub fn update_status(&self, session_id: &str, status: &crate::session::SessionStatus) {
        if let Some(row) = self.rows.borrow().get(session_id) {
            row.set_status(status);
        }
    }

    pub fn connect_tab_selected<F: Fn(&str) + 'static>(&self, f: F) {
        let rows = self.rows.clone();

        self.list_box.connect_row_selected(move |_, gtk_row| {
            let Some(gtk_row) = gtk_row else { return };

            let child = gtk_row.child();
            let rows = rows.borrow();

            for (id, row) in rows.iter() {
                if child.as_ref() == Some(row.widget()) {
                    f(id);
                    return;
                }
            }
        });
    }

    pub fn connect_tab_close<F: Fn(String) + Clone + 'static>(&self, f: F) {
        let rows = self.rows.clone();

        // Wire close buttons after adding rows — done lazily via this callback
        // stored for the manager to call after add_tab
        let _ = (rows, f);
    }

    pub fn connect_new_tab<F: Fn() + 'static>(&self, f: F) {
        self.new_tab_btn.connect_clicked(move |_| f());
    }

    pub fn wire_close_button<F: Fn(String) + Clone + 'static>(&self, session_id: &str, f: F) {
        if let Some(row) = self.rows.borrow().get(session_id) {
            let id = session_id.to_string();
            row.connect_close(move || f(id.clone()));
        }
    }
}
