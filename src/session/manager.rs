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
use crate::terminal::{VteTerminal, SplitView};
use crate::terminal::split_view::SplitNode;

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
    bin_dir: PathBuf,
    hook_script_path: PathBuf,
}

impl SessionManager {
    pub fn new(
        stack: Stack,
        sidebar: Rc<Sidebar>,
        socket_path: PathBuf,
        bin_dir: PathBuf,
        config: Rc<RefCell<Config>>,
    ) -> Rc<RefCell<Self>> {
        let hook_script_path = bin_dir.join("seemux-hook.sh");

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
            bin_dir,
            hook_script_path,
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

        if self.stack.is_realized() {
            terminal.spawn_shell(cwd, &env_refs);
        }

        // Wire title and CWD signals
        self.wire_terminal_signals(&terminal, &id, &pane_id);

        let split_view = SplitView::new(terminal, pane_id);
        let widget = split_view.build_widget();
        self.stack.add_named(&widget, Some(&id));
        self.sidebar.add_tab(&session);

        self.split_views.insert(id.clone(), split_view);
        self.sessions.push(session);
        self.switch_to(&id);

        id
    }

    fn wire_terminal_signals(&self, terminal: &VteTerminal, session_id: &str, pane_id: &str) {
        let sidebar = self.sidebar.clone();
        let sid = session_id.to_string();
        terminal.connect_title_changed(move |new_title| {
            sidebar.update_title(&sid, new_title);
        });

        let sidebar = self.sidebar.clone();
        let sid = session_id.to_string();
        let pid = pane_id.to_string();
        let last_cwd: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let cwds = self.session_cwds.clone();
        terminal.connect_cwd_changed(move |path| {
            let Some(cwd) = path else {
                sidebar.update_branch(&sid, None);
                return;
            };

            if last_cwd.borrow().as_deref() == Some(&cwd) {
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
        // Find the existing widget in the stack by name, not by rebuilding
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

    /// Split the focused pane in the active session. Returns true if split succeeded.
    pub fn split_active_pane(&mut self, orientation: gtk4::Orientation) -> bool {
        let Some(active_id) = self.active_id.clone() else { return false };
        let Some(sv) = self.split_views.get(&active_id) else { return false };

        let config = self.config.borrow();
        let new_pane_id = sv.split(orientation, &config);
        drop(config);

        // Spawn shell in the new pane
        let env_vars = self.build_env_vars(&active_id);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        sv.spawn_pane(&new_pane_id, None, &env_refs);

        // Rebuild: remove old tree from stack, unparent terminals, build new tree
        if let Some(old) = self.stack.child_by_name(&active_id) {
            self.stack.remove(&old);
        }
        sv.root.borrow().unparent_all();

        let new_widget = sv.build_widget();
        self.stack.add_named(&new_widget, Some(&active_id));
        self.stack.set_visible_child_name(&active_id);

        // Focus the new pane
        sv.set_focused_pane_id(&new_pane_id);
        if let Some(term) = sv.focused_terminal() {
            let term = term.clone();
            glib::idle_add_local_once(move || { term.grab_focus(); });
        }

        true
    }

    /// Close the focused pane in the active session. Returns true if the session should be destroyed.
    pub fn close_active_pane(&mut self) -> bool {
        let Some(active_id) = self.active_id.clone() else { return false };
        let Some(sv) = self.split_views.get(&active_id) else { return false };

        if sv.pane_count() <= 1 {
            return true; // Last pane — caller should destroy the session
        }

        // Remove old widget tree from stack BEFORE modifying the split tree
        if let Some(old) = self.stack.child_by_name(&active_id) {
            self.stack.remove(&old);
        }

        let should_destroy = sv.close_focused_pane();

        if should_destroy {
            return true;
        }

        // Unparent surviving terminals, then rebuild
        sv.root.borrow().unparent_all();

        let new_widget = sv.build_widget();
        self.stack.add_named(&new_widget, Some(&active_id));
        self.stack.set_visible_child_name(&active_id);

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

    pub fn sessions_with_claude_pid(&self) -> Vec<(String, u32)> {
        self.sessions.iter()
            .filter_map(|s| s.claude_pid.map(|pid| (s.id.clone(), pid)))
            .collect()
    }

    pub fn wire_child_exited(self_ref: &Rc<RefCell<Self>>, session_id: &str) {
        let borrow = self_ref.borrow();
        let Some(sv) = borrow.split_views.get(session_id) else { return };

        // Wire child-exited on ALL terminals in the split tree, not just the focused one
        for (pane_id, vte_term) in sv.collect_vte_terminals() {
            let mgr = self_ref.clone();
            let sid = session_id.to_string();
            let pid = pane_id.clone();

            vte_term.connect_child_exited(move |_term, _status| {
                let mgr = mgr.clone();
                let sid = sid.clone();
                let pid = pid.clone();

                glib::idle_add_local_once(move || {
                    let mut m = mgr.borrow_mut();

                    // Check if this session has multiple panes
                    if let Some(sv) = m.split_views.get(&sid) {
                        if sv.pane_count() > 1 {
                            // Set focus to the exited pane, then close it
                            sv.set_focused_pane_id(&pid);
                            drop(m);
                            mgr.borrow_mut().close_active_pane();
                            return;
                        }
                    }

                    // Single pane — destroy the whole session
                    m.destroy_session(&sid);
                });
            });
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

        // Wire signals for all panes via vte4::Terminal (GObject ref, no borrow issue)
        for (pane_id, vte_term) in split_view.collect_vte_terminals() {
            let sidebar = self.sidebar.clone();
            let sid = id.clone();
            vte_term.connect_window_title_changed(move |term: &vte4::Terminal| {
                if let Some(title) = term.window_title() {
                    sidebar.update_title(&sid, &title);
                }
            });

            let sidebar = self.sidebar.clone();
            let sid = id.clone();
            let pid = pane_id.clone();
            let last_cwd: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
            let cwds = self.session_cwds.clone();
            vte_term.connect_current_directory_uri_changed(move |term: &vte4::Terminal| {
                let path: Option<String> = term.current_directory_uri()
                    .and_then(|uri| {
                        let s = uri.to_string();
                        Some(s.strip_prefix("file://").unwrap_or(&s).to_string())
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

        // Spawn each pane with its own CWD
        Self::spawn_panes_recursive(&sv.root.borrow(), &cwds, &env_refs);
    }

    fn spawn_panes_recursive(
        node: &SplitNode,
        cwds: &std::collections::HashMap<String, String>,
        env_vars: &[(&str, &str)],
    ) {
        match node {
            SplitNode::Leaf { id, terminal } => {
                if terminal.needs_spawn() {
                    let cwd = cwds.get(id).map(|s| s.as_str());
                    terminal.spawn_shell(cwd, env_vars);
                }
            }
            SplitNode::Split { first, second, .. } => {
                Self::spawn_panes_recursive(first, cwds, env_vars);
                Self::spawn_panes_recursive(second, cwds, env_vars);
            }
        }
    }

    fn build_env_vars(&self, session_id: &str) -> Vec<(String, String)> {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let bin_dir_str = self.bin_dir.to_string_lossy().to_string();

        vec![
            ("SEEMUX_SOCKET".to_string(), self.socket_path.to_string_lossy().to_string()),
            ("SEEMUX_SESSION_ID".to_string(), session_id.to_string()),
            ("SEEMUX_HOOK_SCRIPT".to_string(), self.hook_script_path.to_string_lossy().to_string()),
            ("SEEMUX_BIN_DIR".to_string(), bin_dir_str.clone()),
            ("PATH".to_string(), format!("{bin_dir_str}:{current_path}")),
        ]
    }
}
