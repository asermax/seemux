use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::Stack;
use vte4::prelude::*;

use crate::config::{Config, SavedSession, SessionState};
use crate::notifications::{Notification, NotificationStore};
use crate::session::{Session, SessionStatus};
use crate::sidebar::Sidebar;
use crate::terminal::{Direction, VteTerminal, SplitView};

enum SpawnAction<'a> {
    Shell,
    Command(&'a [&'a str]),
}

/// Extract the filesystem path from a `file://[host]/path` URI.
pub(crate) fn path_from_file_uri(uri: &str) -> Option<String> {
    let without_scheme = uri.strip_prefix("file://")?;
    let slash_pos = without_scheme.find('/')?;
    Some(without_scheme[slash_pos..].to_string())
}

/// Extract the last path component (folder name) from a path.
fn folder_name(path: &str) -> &str {
    match path.rsplit('/').next() {
        Some(name) if !name.is_empty() => name,
        _ => path,
    }
}

/// Detect whether a window title indicates a git or gh command.
fn is_git_command_title(title: &str) -> bool {
    title.starts_with("git ") || title.starts_with("gh ")
}

/// Asynchronously detect the git branch and PR for a directory, updating the sidebar.
fn detect_branch_and_pr(cwd: &str, sidebar: &Rc<Sidebar>, session_id: &str) {
    let sidebar = sidebar.clone();
    let sid = session_id.to_string();

    crate::git::detect_branch_async(cwd, {
        let cwd = cwd.to_string();

        move |branch| {
            sidebar.update_branch(&sid, branch.as_deref());

            if branch.is_some() {
                let sidebar = sidebar.clone();
                let sid = sid.clone();

                crate::git::detect_pr_async(&cwd, move |pr_info| {
                    let pr = pr_info.as_ref()
                        .map(|info| (info.number.to_string(), info.url.clone()));

                    sidebar.update_pr(
                        &sid,
                        pr.as_ref().map(|(n, u)| (n.as_str(), u.as_str())),
                    );
                });
            } else {
                sidebar.update_pr(&sid, None);
            }
        }
    });
}

/// Detect shell-generated titles like `user@hostname:/path` or `user@hostname:~`.
fn is_shell_title(title: &str) -> bool {
    let Some(colon_pos) = title.find(':') else { return false };
    let before_colon = &title[..colon_pos];
    let after_colon = &title[colon_pos + 1..];

    before_colon.contains('@') && (after_colon.starts_with('/') || after_colon.starts_with('~'))
}

/// Replace the `$HOME` prefix with `~` for display.
pub(crate) fn display_path(path: &str) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();

        if let Some(rest) = path.strip_prefix(home.as_ref()) {
            return format!("~{rest}");
        }
    }

    path.to_string()
}

/// Compute the next or previous index in a circular list.
fn circular_offset(pos: usize, len: usize, forward: bool) -> usize {
    circular_offset_by(pos, len, 1, forward)
}

/// Compute an offset index in a circular list.
fn circular_offset_by(pos: usize, len: usize, offset: usize, forward: bool) -> usize {
    if forward { (pos + offset) % len } else { (pos + len - offset) % len }
}

pub struct SessionManager {
    sessions: Vec<Session>,
    split_views: HashMap<String, SplitView>,
    active_id: Option<String>,
    stack: Stack,
    sidebar: Rc<Sidebar>,
    config: Rc<RefCell<Config>>,
    notification_store: Rc<RefCell<NotificationStore>>,
    on_empty: Option<Box<dyn Fn()>>,
    /// Shared CWD tracking — updated by terminal CWD signal, read at save time
    session_cwds: Rc<RefCell<HashMap<String, String>>>,
    socket_path: PathBuf,
    /// Per-session bell debounce — tracks last bell timestamp (unix seconds)
    bell_timestamps: Rc<RefCell<HashMap<String, i64>>>,
}

impl SessionManager {
    pub fn new(
        stack: Stack,
        sidebar: Rc<Sidebar>,
        socket_path: PathBuf,
        config: Rc<RefCell<Config>>,
        notification_store: Rc<RefCell<NotificationStore>>,
    ) -> Rc<RefCell<Self>> {
        

        Rc::new(RefCell::new(Self {
            sessions: Vec::new(),
            split_views: HashMap::new(),
            active_id: None,
            stack,
            sidebar,
            config,
            notification_store,
            on_empty: None,
            session_cwds: Rc::new(RefCell::new(HashMap::new())),
            socket_path,
            bell_timestamps: Rc::new(RefCell::new(HashMap::new())),
        }))
    }

    pub fn set_on_empty<F: Fn() + 'static>(&mut self, f: F) {
        self.on_empty = Some(Box::new(f));
    }

    fn find_session_mut(&mut self, session_id: &str) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == session_id)
    }

    /// Register a fully-built session + split view into the UI and internal collections.
    fn register_session(
        &mut self,
        session: Session,
        split_view: SplitView,
        active_hint: Option<&str>,
    ) -> String {
        let id = session.id.clone();
        let widget = split_view.build_widget();

        self.stack.add_named(&widget, Some(&id));
        self.sidebar.add_tab(&session, active_hint);
        self.split_views.insert(id.clone(), split_view);
        self.sessions.push(session);
        self.switch_to(&id);

        id
    }

    pub fn create_session(&mut self, title: Option<&str>, cwd: Option<&str>) -> String {
        self.create_session_in_group(title, cwd, crate::session::DEFAULT_GROUP)
    }

    pub fn create_session_in_group(&mut self, title: Option<&str>, cwd: Option<&str>, group_id: &str) -> String {
        self.create_session_inner(title, cwd, group_id, SpawnAction::Shell)
    }

    pub fn create_session_with_command(&mut self, title: &str, cwd: Option<&str>, argv: &[&str]) -> String {
        let group_id = self.active_group_id()
            .unwrap_or(crate::session::DEFAULT_GROUP)
            .to_string();

        self.create_session_inner(Some(title), cwd, &group_id, SpawnAction::Command(argv))
    }

    pub fn create_session_with_command_in_group(
        &mut self, title: &str, cwd: Option<&str>, group_id: &str, argv: &[&str],
    ) -> String {
        self.create_session_inner(Some(title), cwd, group_id, SpawnAction::Command(argv))
    }

    fn create_session_inner(
        &mut self,
        title: Option<&str>,
        cwd: Option<&str>,
        group_id: &str,
        spawn: SpawnAction<'_>,
    ) -> String {
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
            match spawn {
                SpawnAction::Shell => terminal.spawn_shell(cwd, &env_refs),
                SpawnAction::Command(argv) => terminal.spawn_command(argv, cwd, &env_refs),
            }
        }

        self.wire_vte_signals(&vte_term, &id, &pane_id);

        let active_hint = self.active_id.as_deref().map(|s| s.to_string());
        let split_view = SplitView::new(terminal, pane_id);

        self.register_session(session, split_view, active_hint.as_deref())
    }

    /// Wire VTE signals to update tab title, subtitle, git branch, and PR.
    fn wire_vte_signals(&self, vte_term: &vte4::Terminal, session_id: &str, pane_id: &str) {
        // Shared state for git command detection across title + CWD handlers
        let last_was_git_cmd: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let pending_redetect: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));

        // Window title changes (shows running command name)
        let sidebar = self.sidebar.clone();
        let sid = session_id.to_string();
        let last_git_cmd = last_was_git_cmd.clone();
        let pending = pending_redetect.clone();

        vte_term.connect_window_title_changed(move |term: &vte4::Terminal| {
            let Some(title) = term.window_title() else { return };

            let was_git_command = last_git_cmd.replace(is_git_command_title(&title));

            if is_shell_title(&title) {
                // Shell regained control — reset title to folder name
                if let Some(cwd) = term.current_directory_uri()
                    .and_then(|uri| path_from_file_uri(&uri))
                {
                    sidebar.update_title(&sid, folder_name(&cwd));

                    // If the previous title was a git/gh command, re-detect branch + PR
                    if was_git_command {
                        if let Some(source_id) = pending.take() {
                            source_id.remove();
                        }

                        let sidebar = sidebar.clone();
                        let sid = sid.clone();

                        let source_id = glib::timeout_add_local_once(
                            std::time::Duration::from_secs(2),
                            move || {
                                detect_branch_and_pr(&cwd, &sidebar, &sid);
                            },
                        );

                        pending.set(Some(source_id));
                    }
                }
            } else {
                sidebar.update_title(&sid, &title);
            }
        });

        // CWD changes (updates folder name, subtitle path, git branch, and PR)
        let sidebar = self.sidebar.clone();
        let sid = session_id.to_string();
        let pid = pane_id.to_string();
        let last_cwd: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let cwds = self.session_cwds.clone();

        vte_term.connect_current_directory_uri_changed(move |term: &vte4::Terminal| {
            let path = term.current_directory_uri()
                .and_then(|uri| path_from_file_uri(&uri));

            let Some(cwd) = path else {
                sidebar.update_branch(&sid, None);
                sidebar.update_pr(&sid, None);
                return;
            };

            if last_cwd.borrow().as_deref() == Some(cwd.as_str()) {
                return;
            }
            *last_cwd.borrow_mut() = Some(cwd.clone());
            cwds.borrow_mut().insert(pid.clone(), cwd.clone());

            // Update tab title (folder name) and subtitle (full display path)
            sidebar.update_cwd(&sid, folder_name(&cwd), &display_path(&cwd));

            // Cancel any pending git-command re-detection since CWD change supersedes it
            if let Some(source_id) = pending_redetect.take() {
                source_id.remove();
            }

            let sidebar = sidebar.clone();
            let sid = sid.clone();
            detect_branch_and_pr(&cwd, &sidebar, &sid);
        });
    }

    pub fn destroy_session(&mut self, session_id: &str) {
        // Capture group context before removing from sidebar
        let group_siblings = self.sidebar.group_id_for_session(session_id)
            .map(|gid| self.sidebar.ordered_session_ids_in_group(&gid));

        // Clean up pane CWDs before removing the split view
        if let Some(sv) = self.split_views.get(session_id) {
            let mut cwds = self.session_cwds.borrow_mut();

            for (pane_id, _) in sv.collect_vte_terminals() {
                cwds.remove(&pane_id);
            }
        }

        self.bell_timestamps.borrow_mut().remove(session_id);

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
        } else if self.active_id.as_deref() == Some(session_id)
            && let Some(next_id) = self.find_focus_after_close(session_id, group_siblings) {
                self.switch_to(&next_id);
            }
    }

    /// Find the best tab to focus after closing a session.
    /// Priority: next in group → previous in group → first in default → first in any group.
    fn find_focus_after_close(
        &self,
        closed_id: &str,
        group_siblings_before_close: Option<Vec<String>>,
    ) -> Option<String> {
        if let Some(ids) = group_siblings_before_close
            && let Some(pos) = ids.iter().position(|id| id == closed_id) {
                if pos + 1 < ids.len() {
                    return Some(ids[pos + 1].clone());
                }

                if pos > 0 {
                    return Some(ids[pos - 1].clone());
                }
            }

        if let Some(id) = self.sidebar.first_session_id_in_group(crate::session::DEFAULT_GROUP) {
            return Some(id);
        }

        for gid in self.sidebar.ordered_group_ids() {
            if let Some(id) = self.sidebar.first_session_id_in_group(&gid) {
                return Some(id);
            }
        }

        None
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
        if let Some(session) = self.find_session_mut(session_id) {
            session.status = status.clone();
            self.sidebar.update_status(session_id, &status);
        }
    }

    pub fn set_claude_pid(&mut self, session_id: &str, pid: Option<u32>) {
        if let Some(session) = self.find_session_mut(session_id) {
            session.claude_pid = pid;
        }
    }

    pub fn set_claude_session_id(&mut self, session_id: &str, claude_session_id: Option<String>) {
        if let Some(session) = self.find_session_mut(session_id) {
            session.claude_session_id = claude_session_id;
        }
    }

    pub fn session_terminal(&self, session_id: &str) -> Option<vte4::Terminal> {
        self.split_views.get(session_id).and_then(|sv| sv.focused_terminal())
    }

    pub fn sessions_pending_resume(&self) -> Vec<(String, String, bool)> {
        self.sessions.iter()
            .filter_map(|s| {
                s.claude_session_id.as_ref().map(|cid| {
                    let collapsed = self.sidebar.is_group_collapsed(&s.group_id);
                    (s.id.clone(), cid.clone(), collapsed)
                })
            })
            .collect()
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

        if let Some(sv) = self.split_views.get(session_id)
            && let Some(term) = sv.focused_terminal() {
                term.grab_focus();
            }
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    pub fn active_group_id(&self) -> Option<&str> {
        self.active_id.as_deref()
            .and_then(|id| self.sessions.iter().find(|s| s.id == id))
            .map(|s| s.group_id.as_str())
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

        // Get the focused terminal's CWD before splitting (split changes focus)
        let cwd = sv.focused_terminal()
            .and_then(|t| t.current_directory_uri())
            .and_then(|uri| path_from_file_uri(&uri));

        let config = mgr.config.borrow();
        let (new_pane_id, new_vte) = sv.split(orientation, &config);
        drop(config);

        let env_vars = mgr.build_env_vars(&active_id);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        sv.spawn_pane(&new_pane_id, cwd.as_deref(), &env_refs);

        // Rebuild widget tree in the stack
        sv.rebuild_in_stack(&mgr.stack, &active_id);

        // Wire title/CWD signals on the new terminal
        mgr.wire_vte_signals(&new_vte, &active_id, &new_pane_id);

        // Focus the new pane
        let term = new_vte.clone();
        glib::idle_add_local_once(move || { term.grab_focus(); });

        drop(mgr);

        // Wire child-exited, focus tracking, and bell (needs Rc<RefCell<Self>>)
        Self::wire_pane_child_exited(self_ref, &active_id, &new_pane_id, &new_vte);
        Self::wire_pane_focus(self_ref, &active_id, &new_pane_id, &new_vte);
        Self::wire_pane_bell(self_ref, &active_id, &new_vte);

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
        let ordered = self.sidebar.ordered_visible_session_ids();

        if let Some(id) = ordered.get(index).cloned() {
            self.switch_to(&id);
        }
    }

    pub fn switch_adjacent(&mut self, forward: bool) {
        let Some(active_id) = &self.active_id else { return };
        let ordered = self.sidebar.ordered_visible_session_ids();
        let Some(pos) = ordered.iter().position(|id| id == active_id) else { return };

        let target = circular_offset(pos, ordered.len(), forward);
        let id = ordered[target].clone();
        self.switch_to(&id);
    }

    pub fn switch_adjacent_group(&mut self, forward: bool) {
        let Some(active_id) = &self.active_id else { return };
        let Some(current_group) = self.sidebar.group_id_for_session(active_id) else { return };

        let group_order = self.sidebar.ordered_visible_group_ids();
        let Some(gpos) = group_order.iter().position(|g| g == &current_group) else { return };

        for offset in 1..group_order.len() {
            let target = circular_offset_by(gpos, group_order.len(), offset, forward);

            if let Some(id) = self.sidebar.first_session_id_in_group(&group_order[target]) {
                self.switch_to(&id);
                return;
            }
        }
    }

    pub fn switch_adjacent_with_notifications(&mut self, notif_store: &NotificationStore, forward: bool) -> bool {
        let Some(active_id) = &self.active_id else { return false };
        let ordered = self.sidebar.ordered_session_ids();
        let Some(pos) = ordered.iter().position(|id| id == active_id) else { return false };

        let len = ordered.len();

        for i in 1..len {
            let idx = circular_offset_by(pos, len, i, forward);

            if notif_store.unread_count(&ordered[idx]) > 0 {
                let id = ordered[idx].clone();
                self.switch_to(&id);
                return true;
            }
        }

        false
    }

    pub fn move_session_to_position(&mut self, session_id: &str, new_group_id: &str, _position: i32) {
        if let Some(session) = self.find_session_mut(session_id) {
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

        self.session_cwds.borrow_mut().remove(pane_id);

        if should_destroy {
            self.destroy_session(session_id);
            return;
        }

        // Rebuild widget tree in the stack
        if let Some(sv) = self.split_views.get(session_id) {
            sv.rebuild_in_stack(&self.stack, session_id);
        }

        // Only grab focus if this is the active session
        if self.active_id.as_deref() == Some(session_id)
            && let Some(sv) = self.split_views.get(session_id)
                && let Some(term) = sv.focused_terminal() {
                    let term = term.clone();
                    glib::idle_add_local_once(move || { term.grab_focus(); });
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

    /// Wire all signal handlers (child-exited, focus tracking, bell) on ALL terminals
    /// in a session's split tree.
    pub fn wire_pane_signals(self_ref: &Rc<RefCell<Self>>, session_id: &str) {
        let terminals = {
            let borrow = self_ref.borrow();
            let Some(sv) = borrow.split_views.get(session_id) else { return };
            sv.collect_vte_terminals()
        };

        for (pane_id, vte_term) in &terminals {
            Self::wire_pane_child_exited(self_ref, session_id, pane_id, vte_term);
            Self::wire_pane_focus(self_ref, session_id, pane_id, vte_term);
            Self::wire_pane_bell(self_ref, session_id, vte_term);
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

    /// Wire bell notification on a single terminal pane.
    fn wire_pane_bell(
        self_ref: &Rc<RefCell<Self>>,
        session_id: &str,
        vte_term: &vte4::Terminal,
    ) {
        let mgr = Rc::downgrade(self_ref);
        let sid = session_id.to_string();

        vte_term.connect_bell(move |_term| {
            let Some(mgr) = mgr.upgrade() else { return };
            let Ok(m) = mgr.try_borrow() else { return };

            if m.active_id.as_deref() == Some(sid.as_str()) {
                return;
            }

            // Debounce: max 1 bell notification per 2 seconds per session
            let now = glib::DateTime::now_local()
                .map(|dt| dt.to_unix())
                .unwrap_or(0);

            let mut timestamps = m.bell_timestamps.borrow_mut();

            if let Some(&last) = timestamps.get(&sid)
                && now - last < 2 {
                    return;
                }

            timestamps.insert(sid.clone(), now);
            drop(timestamps);

            let notification = Notification::new(&sid, "Bell", "Terminal bell received");
            m.notification_store.borrow_mut().add_notification(notification);
        });
    }

    /// Save current session state for restoration on next launch.
    pub fn save_state(&self) {
        let groups = self.sidebar.group_ids().iter().map(|(id, name, collapsed)| {
            crate::config::SavedGroup { id: id.clone(), name: name.clone(), collapsed: *collapsed }
        }).collect();

        let cwds = self.session_cwds.borrow();
        let ordered_ids = self.sidebar.ordered_session_ids();

        let active_session_index = self.active_id.as_ref()
            .and_then(|id| ordered_ids.iter().position(|sid| sid == id));

        let sessions_map: std::collections::HashMap<&str, &Session> = self.sessions.iter()
            .map(|s| (s.id.as_str(), s))
            .collect();

        let state = SessionState {
            sessions: ordered_ids.iter().filter_map(|id| {
                let s = sessions_map.get(id.as_str())?;

                let split_tree = self.split_views.get(&s.id)
                    .map(|sv| sv.to_saved(&cwds))
                    .unwrap_or_else(|| crate::config::SavedSplitNode::Leaf {
                        cwd: cwds.get(&s.id).cloned().or_else(|| s.cwd.clone()),
                    });

                Some(SavedSession {
                    title: s.title.clone(),
                    split_tree,
                    group_id: s.group_id.clone(),
                    claude_session_id: s.claude_session_id.clone(),
                })
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
        claude_session_id: Option<&str>,
    ) -> String {
        let mut session = crate::session::Session::new(title.to_string());
        session.group_id = group_id.to_string();
        session.claude_session_id = claude_session_id.map(|s| s.to_string());
        let id = session.id.clone();

        let config = self.config.borrow();
        let (split_view, panes) = SplitView::from_saved(split_tree, &config);
        drop(config);

        // Wire signals for all panes
        for (pane_id, vte_term) in split_view.collect_vte_terminals() {
            self.wire_vte_signals(&vte_term, &id, &pane_id);
        }

        self.register_session(session, split_view, None);

        // Store pane CWDs for deferred spawning
        let mut cwds = self.session_cwds.borrow_mut();

        for (pane_id, cwd) in &panes {
            if let Some(c) = cwd {
                cwds.insert(pane_id.clone(), c.clone());
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
        let mut vars = vec![
            ("SEEMUX_SOCKET".to_string(), self.socket_path.to_string_lossy().to_string()),
            ("SEEMUX_SESSION_ID".to_string(), session_id.to_string()),
        ];

        if self.config.borrow().agent_teams_shim {
            // Derive paths from the socket_path rather than recomputing XDG_RUNTIME_DIR
            let seemux_dir = self.socket_path.parent().unwrap();
            let bin_dir = seemux_dir.join("bin");
            let existing_path = std::env::var("PATH").unwrap_or_default();

            vars.push(("TMUX".to_string(), format!(
                "{},{},0",
                self.socket_path.display(),
                std::process::id(),
            )));
            vars.push(("PATH".to_string(), format!("{}:{existing_path}", bin_dir.display())));
            vars.push(("COLORTERM".to_string(), "truecolor".to_string()));
        }

        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_shell_title_detects_user_at_host_path() {
        assert!(is_shell_title("agus@archlinux:/home/agus/projects"));
        assert!(is_shell_title("root@server:~/workspace"));
        assert!(is_shell_title("user@host:/"));
    }

    #[test]
    fn is_shell_title_ignores_command_names() {
        assert!(!is_shell_title("vim"));
        assert!(!is_shell_title("htop"));
        assert!(!is_shell_title("cargo build"));
        assert!(!is_shell_title(""));
    }

    #[test]
    fn is_shell_title_ignores_titles_without_path() {
        assert!(!is_shell_title("user@host:"));
        assert!(!is_shell_title("user@host:something"));
    }
}
