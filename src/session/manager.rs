use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::Stack;
use vte4::prelude::*;

use crate::config::{Config, SavedSession, SessionState};
use crate::session::{Session, SessionStatus};
use crate::sidebar::Sidebar;
use crate::terminal::{Direction, VteTerminal, SplitView};

pub struct SessionManager {
    sessions: Vec<Session>,
    split_views: HashMap<String, SplitView>,
    active_id: Option<String>,
    stack: Stack,
    sidebar: Rc<Sidebar>,
    config: Rc<RefCell<Config>>,
    on_empty: Option<Box<dyn Fn()>>,
    /// Shared CWD tracking — updated by terminal CWD signal, read at save time
    session_cwds: Rc<RefCell<HashMap<String, String>>>,
    socket_path: PathBuf,
}

impl SessionManager {
    pub fn new(
        stack: Stack,
        sidebar: Rc<Sidebar>,
        socket_path: PathBuf,
        config: Rc<RefCell<Config>>,
    ) -> Rc<RefCell<Self>> {
        let manager = Rc::new(RefCell::new(Self {
            sessions: Vec::new(),
            split_views: HashMap::new(),
            active_id: None,
            stack,
            sidebar,
            config,
            on_empty: None,
            session_cwds: Rc::new(RefCell::new(HashMap::new())),
            socket_path,
        }));

        manager
    }

    pub fn set_on_empty<F: Fn() + 'static>(&mut self, f: F) {
        self.on_empty = Some(Box::new(f));
    }

    pub fn create_session(&mut self, title: Option<&str>, cwd: Option<&str>) -> String {
        self.create_session_in_group(title, cwd, crate::session::DEFAULT_GROUP)
    }

    pub fn create_session_in_group(&mut self, title: Option<&str>, cwd: Option<&str>, group_id: &str) -> String {
        let index = self.sessions.len() + 1;
        let mut session = Session::new(title.unwrap_or(&format!("Tab {index}")).to_string());
        session.group_id = group_id.to_string();
        session.cwd = cwd.map(|s| s.to_string());
        let id = session.id.clone();

        let env_vars = self.build_env_vars(&id);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        let pane_id = uuid::Uuid::new_v4().to_string();
        let terminal = VteTerminal::new_with_config(&self.config.borrow());
        let vte_term = terminal.terminal().clone();

        if self.stack.is_realized() {
            terminal.spawn_shell(cwd, &env_refs);
        }

        // Wire title and CWD signals
        self.wire_vte_signals(&vte_term, &id, &pane_id);

        let split_view = SplitView::new(terminal, pane_id);
        let widget = split_view.build_widget();
        self.stack.add_named(&widget, Some(&id));
        self.sidebar.add_tab(&session);

        self.split_views.insert(id.clone(), split_view);
        self.sessions.push(session);
        self.switch_to(&id);

        id
    }

    /// Wire title and CWD signals on a vte4::Terminal.
    fn wire_vte_signals(&self, vte_term: &vte4::Terminal, session_id: &str, pane_id: &str) {
        let sidebar = self.sidebar.clone();
        let sid = session_id.to_string();
        vte_term.connect_window_title_changed(move |term: &vte4::Terminal| {
            if let Some(title) = term.window_title() {
                sidebar.update_title(&sid, &title);
            }
        });

        let sidebar = self.sidebar.clone();
        let sid = session_id.to_string();
        let pid = pane_id.to_string();
        let last_cwd: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let cwds = self.session_cwds.clone();
        vte_term.connect_current_directory_uri_changed(move |term: &vte4::Terminal| {
            let path = term.current_directory_uri()
                .map(|uri| {
                    let s = uri.to_string();
                    s.strip_prefix("file://").unwrap_or(&s).to_string()
                });

            let Some(cwd) = path else {
                sidebar.update_branch(&sid, None);
                return;
            };

            if last_cwd.borrow().as_deref() == Some(cwd.as_str()) {
                return;
            }
            *last_cwd.borrow_mut() = Some(cwd.clone());
            cwds.borrow_mut().insert(pid.clone(), cwd.clone());

            let sidebar = sidebar.clone();
            let sid = sid.clone();
            crate::git::detect_branch_async(&cwd, move |branch| {
                sidebar.update_branch(&sid, branch.as_deref());
            });
        });
    }

    pub fn destroy_session(&mut self, session_id: &str) {
        if let Some(child) = self.stack.child_by_name(session_id) {
            self.stack.remove(&child);
        }

        self.sidebar.remove_tab(session_id);
        self.split_views.remove(session_id);
        self.sessions.retain(|s| s.id != session_id);

        if self.sessions.is_empty() {
            if let Some(ref on_empty) = self.on_empty {
                on_empty();
            }
        } else if self.active_id.as_deref() == Some(session_id) {
            let next_id = self.sessions.last().map(|s| s.id.clone());

            if let Some(next_id) = next_id {
                self.switch_to(&next_id);
            }
        }
    }

    pub fn close_others(&mut self, keep_id: &str) {
        let to_remove: Vec<String> = self.sessions.iter()
            .filter(|s| s.id != keep_id)
            .map(|s| s.id.clone())
            .collect();

        for id in to_remove {
            self.destroy_session(&id);
        }
    }

    pub fn update_session_status(&mut self, session_id: &str, status: SessionStatus) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            session.status = status.clone();
            self.sidebar.update_status(session_id, &status);
        }
    }

    pub fn set_claude_pid(&mut self, session_id: &str, pid: Option<u32>) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            session.claude_pid = pid;
        }
    }

    pub fn spawn_deferred(&self) {
        for session in &self.sessions {
            self.spawn_restored_panes(&session.id);
        }
    }

    pub fn switch_to(&mut self, session_id: &str) {
        self.active_id = Some(session_id.to_string());
        self.stack.set_visible_child_name(session_id);
        self.sidebar.set_active(session_id);

        if let Some(sv) = self.split_views.get(session_id) {
            if let Some(term) = sv.focused_terminal() {
                term.grab_focus();
            }
        }
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    pub fn active_terminal_vte(&self) -> Option<vte4::Terminal> {
        self.active_id.as_deref()
            .and_then(|id| self.split_views.get(id))
            .and_then(|sv| sv.focused_terminal())
    }

    /// Navigate between split panes in the active session.
    pub fn navigate_pane(&mut self, direction: Direction) {
        let Some(active_id) = self.active_id.clone() else { return };
        let Some(sv) = self.split_views.get(&active_id) else { return };

        if let Some(term) = sv.navigate(direction) {
            term.grab_focus();
        }
    }

    /// Update the focused pane for a session (called by focus tracking).
    fn update_focused_pane(&self, session_id: &str, pane_id: &str) {
        if let Some(sv) = self.split_views.get(session_id) {
            sv.set_focused_pane_id(pane_id);
        }
    }

    /// Split the focused pane in the active session.
    /// Static method — needs Rc<RefCell<Self>> to wire child-exited on the new pane.
    pub fn split_active_pane(self_ref: &Rc<RefCell<Self>>, orientation: gtk4::Orientation) -> bool {
        let mgr = self_ref.borrow();
        let Some(active_id) = mgr.active_id.clone() else { return false };
        let Some(sv) = mgr.split_views.get(&active_id) else { return false };

        let config = mgr.config.borrow();
        let (new_pane_id, new_vte) = sv.split(orientation, &config);
        drop(config);

        // Spawn shell in the new pane
        let env_vars = mgr.build_env_vars(&active_id);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        sv.spawn_pane(&new_pane_id, None, &env_refs);

        // Rebuild widget tree in the stack
        sv.rebuild_in_stack(&mgr.stack, &active_id);

        // Wire title/CWD signals on the new terminal
        mgr.wire_vte_signals(&new_vte, &active_id, &new_pane_id);

        // Focus the new pane
        let term = new_vte.clone();
        glib::idle_add_local_once(move || { term.grab_focus(); });

        drop(mgr);

        // Wire child-exited and focus tracking (needs Rc<RefCell<Self>>)
        Self::wire_pane_child_exited(self_ref, &active_id, &new_pane_id, &new_vte);
        Self::wire_pane_focus(self_ref, &active_id, &new_pane_id, &new_vte);

        true
    }

    /// Close the focused pane in the active session. Returns true if the session should be destroyed.
    pub fn close_active_pane(&mut self) -> bool {
        let Some(active_id) = self.active_id.clone() else { return false };
        let Some(sv) = self.split_views.get(&active_id) else { return false };

        if sv.close_focused_pane() {
            return true; // Last pane — caller should destroy the session
        }

        // Rebuild widget tree in the stack
        sv.rebuild_in_stack(&self.stack, &active_id);

        // Focus the remaining terminal
        if let Some(term) = sv.focused_terminal() {
            let term = term.clone();
            glib::idle_add_local_once(move || { term.grab_focus(); });
        }

        false
    }

    pub fn switch_to_index(&mut self, index: usize) {
        if let Some(session) = self.sessions.get(index) {
            let id = session.id.clone();
            self.switch_to(&id);
        }
    }

    pub fn switch_next(&mut self) {
        let Some(active_id) = &self.active_id else { return };
        let Some(pos) = self.sessions.iter().position(|s| &s.id == active_id) else { return };

        let next = (pos + 1) % self.sessions.len();
        let id = self.sessions[next].id.clone();
        self.switch_to(&id);
    }

    pub fn switch_prev(&mut self) {
        let Some(active_id) = &self.active_id else { return };
        let Some(pos) = self.sessions.iter().position(|s| &s.id == active_id) else { return };

        let prev = if pos == 0 { self.sessions.len() - 1 } else { pos - 1 };
        let id = self.sessions[prev].id.clone();
        self.switch_to(&id);
    }

    pub fn switch_next_group(&mut self) {
        let Some(active_id) = &self.active_id else { return };
        let Some(current) = self.sessions.iter().find(|s| &s.id == active_id) else { return };

        let current_group = &current.group_id;

        // Collect unique group IDs in session order
        let mut group_order: Vec<&str> = Vec::new();
        for s in &self.sessions {
            if !group_order.contains(&s.group_id.as_str()) {
                group_order.push(&s.group_id);
            }
        }

        if let Some(gpos) = group_order.iter().position(|g| *g == current_group) {
            let next_gpos = (gpos + 1) % group_order.len();
            let target_group = group_order[next_gpos];

            if let Some(first) = self.sessions.iter().find(|s| s.group_id == target_group) {
                let id = first.id.clone();
                self.switch_to(&id);
            }
        }
    }

    pub fn switch_prev_group(&mut self) {
        let Some(active_id) = &self.active_id else { return };
        let Some(current) = self.sessions.iter().find(|s| &s.id == active_id) else { return };

        let current_group = &current.group_id;

        let mut group_order: Vec<&str> = Vec::new();
        for s in &self.sessions {
            if !group_order.contains(&s.group_id.as_str()) {
                group_order.push(&s.group_id);
            }
        }

        if let Some(gpos) = group_order.iter().position(|g| *g == current_group) {
            let prev_gpos = if gpos == 0 { group_order.len() - 1 } else { gpos - 1 };
            let target_group = group_order[prev_gpos];

            if let Some(first) = self.sessions.iter().find(|s| s.group_id == target_group) {
                let id = first.id.clone();
                self.switch_to(&id);
            }
        }
    }

    pub fn move_session_to_group(&mut self, session_id: &str, new_group_id: &str) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            session.group_id = new_group_id.to_string();
        }
    }

    pub fn sessions_with_claude_pid(&self) -> Vec<(String, u32)> {
        self.sessions.iter()
            .filter_map(|s| s.claude_pid.map(|pid| (s.id.clone(), pid)))
            .collect()
    }

    /// Close a specific pane in a specific session (used by child-exited handler).
    fn close_pane(&mut self, session_id: &str, pane_id: &str) {
        let should_destroy = {
            let Some(sv) = self.split_views.get(session_id) else { return };
            if !sv.has_pane(pane_id) { return; }

            sv.set_focused_pane_id(pane_id);
            sv.close_focused_pane()
        };

        if should_destroy {
            self.destroy_session(session_id);
            return;
        }

        // Rebuild widget tree in the stack
        if let Some(sv) = self.split_views.get(session_id) {
            sv.rebuild_in_stack(&self.stack, session_id);
        }

        // Only grab focus if this is the active session
        if self.active_id.as_deref() == Some(session_id) {
            if let Some(sv) = self.split_views.get(session_id) {
                if let Some(term) = sv.focused_terminal() {
                    let term = term.clone();
                    glib::idle_add_local_once(move || { term.grab_focus(); });
                }
            }
        }
    }

    /// Wire child-exited on a single terminal pane.
    fn wire_pane_child_exited(
        self_ref: &Rc<RefCell<Self>>,
        session_id: &str,
        pane_id: &str,
        vte_term: &vte4::Terminal,
    ) {
        let mgr = self_ref.clone();
        let sid = session_id.to_string();
        let pid = pane_id.to_string();

        vte_term.connect_child_exited(move |_term, _status| {
            let mgr = mgr.clone();
            let sid = sid.clone();
            let pid = pid.clone();

            glib::idle_add_local_once(move || {
                mgr.borrow_mut().close_pane(&sid, &pid);
            });
        });
    }

    /// Wire child-exited on ALL terminals in a session's split tree.
    pub fn wire_child_exited(self_ref: &Rc<RefCell<Self>>, session_id: &str) {
        let terminals = {
            let borrow = self_ref.borrow();
            let Some(sv) = borrow.split_views.get(session_id) else { return };
            sv.collect_vte_terminals()
        };

        for (pane_id, vte_term) in terminals {
            Self::wire_pane_child_exited(self_ref, session_id, &pane_id, &vte_term);
        }
    }

    /// Wire focus tracking on a single terminal pane.
    fn wire_pane_focus(
        self_ref: &Rc<RefCell<Self>>,
        session_id: &str,
        pane_id: &str,
        vte_term: &vte4::Terminal,
    ) {
        let mgr = Rc::downgrade(self_ref);
        let sid = session_id.to_string();
        let pid = pane_id.to_string();

        let focus_controller = gtk4::EventControllerFocus::new();
        focus_controller.connect_enter(move |_| {
            let Some(mgr) = mgr.upgrade() else { return };

            if let Ok(m) = mgr.try_borrow() {
                m.update_focused_pane(&sid, &pid);
            }
        });

        vte_term.upcast_ref::<gtk4::Widget>().add_controller(focus_controller);
    }

    /// Wire focus tracking on ALL terminals in a session's split tree.
    pub fn wire_focus_tracking(self_ref: &Rc<RefCell<Self>>, session_id: &str) {
        let terminals = {
            let borrow = self_ref.borrow();
            let Some(sv) = borrow.split_views.get(session_id) else { return };
            sv.collect_vte_terminals()
        };

        for (pane_id, vte_term) in terminals {
            Self::wire_pane_focus(self_ref, session_id, &pane_id, &vte_term);
        }
    }

    /// Save current session state for restoration on next launch.
    pub fn save_state(&self) {
        let groups = self.sidebar.group_ids().iter().map(|(id, name)| {
            crate::config::SavedGroup { id: id.clone(), name: name.clone() }
        }).collect();

        let cwds = self.session_cwds.borrow();

        let active_session_index = self.active_id.as_ref()
            .and_then(|id| self.sessions.iter().position(|s| &s.id == id));

        let state = SessionState {
            sessions: self.sessions.iter().map(|s| {
                let split_tree = self.split_views.get(&s.id)
                    .map(|sv| sv.to_saved(&cwds))
                    .unwrap_or_else(|| crate::config::SavedSplitNode::Leaf {
                        cwd: cwds.get(&s.id).cloned().or_else(|| s.cwd.clone()),
                    });

                SavedSession {
                    title: s.title.clone(),
                    split_tree,
                    group_id: s.group_id.clone(),
                }
            }).collect(),
            groups,
            active_session_index,
        };

        state.save();
    }

    /// Restore a session from a saved split tree.
    pub fn restore_session_with_splits(
        &mut self,
        title: &str,
        group_id: &str,
        split_tree: &crate::config::SavedSplitNode,
    ) -> String {
        let mut session = crate::session::Session::new(title.to_string());
        session.group_id = group_id.to_string();
        let id = session.id.clone();

        let config = self.config.borrow();
        let (split_view, panes) = SplitView::from_saved(split_tree, &config);
        drop(config);

        // Wire signals for all panes
        for (pane_id, vte_term) in split_view.collect_vte_terminals() {
            self.wire_vte_signals(&vte_term, &id, &pane_id);
        }

        let widget = split_view.build_widget();
        self.stack.add_named(&widget, Some(&id));
        self.sidebar.add_tab(&session);

        self.split_views.insert(id.clone(), split_view);
        self.sessions.push(session);
        self.switch_to(&id);

        // Store pane CWDs for deferred spawning
        for (pane_id, cwd) in &panes {
            if let Some(c) = cwd {
                self.session_cwds.borrow_mut().insert(pane_id.clone(), c.clone());
            }
        }

        id
    }

    /// Spawn deferred shells for a restored session with per-pane CWDs.
    pub fn spawn_restored_panes(&self, session_id: &str) {
        let Some(sv) = self.split_views.get(session_id) else { return };
        let env_vars = self.build_env_vars(session_id);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let cwds = self.session_cwds.borrow();

        for (pane_id, _) in sv.collect_vte_terminals() {
            let cwd = cwds.get(&pane_id).map(|s| s.as_str());
            sv.spawn_pane(&pane_id, cwd, &env_refs);
        }
    }

    fn build_env_vars(&self, session_id: &str) -> Vec<(String, String)> {
        vec![
            ("SEEMUX_SOCKET".to_string(), self.socket_path.to_string_lossy().to_string()),
            ("SEEMUX_SESSION_ID".to_string(), session_id.to_string()),
        ]
    }
}
