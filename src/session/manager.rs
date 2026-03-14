use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::Stack;

use crate::config::{Config, SavedSession, SessionState};
use crate::session::{Session, SessionStatus};
use crate::sidebar::Sidebar;
use crate::terminal::VteTerminal;

pub struct SessionManager {
    sessions: Vec<Session>,
    terminals: HashMap<String, VteTerminal>,
    active_id: Option<String>,
    stack: Stack,
    sidebar: Rc<Sidebar>,
    config: Rc<Config>,
    on_empty: Option<Box<dyn Fn()>>,
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
        config: Rc<Config>,
    ) -> Rc<RefCell<Self>> {
        let hook_script_path = bin_dir.join("seemux-hook.sh");

        let manager = Rc::new(RefCell::new(Self {
            sessions: Vec::new(),
            terminals: HashMap::new(),
            active_id: None,
            stack,
            sidebar,
            config,
            on_empty: None,
            socket_path,
            bin_dir,
            hook_script_path,
        }));

        // Wire sidebar tab selection
        let mgr = manager.clone();
        manager.borrow().sidebar.connect_tab_selected(move |id| {
            if let Ok(mut m) = mgr.try_borrow_mut() {
                m.switch_to(id);
            }
        });

        // Wire new tab button
        let mgr = manager.clone();
        manager.borrow().sidebar.connect_new_tab(move || {
            mgr.borrow_mut().create_session(None, None);
        });

        manager
    }

    pub fn set_on_empty<F: Fn() + 'static>(&mut self, f: F) {
        self.on_empty = Some(Box::new(f));
    }

    pub fn create_session(&mut self, title: Option<&str>, cwd: Option<&str>) -> String {
        let index = self.sessions.len() + 1;
        let session = Session::new(title.unwrap_or(&format!("Tab {index}")).to_string());
        let id = session.id.clone();

        let env_vars = self.build_env_vars(&id);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        let terminal = VteTerminal::new_with_config(&self.config);

        if self.stack.is_realized() {
            terminal.spawn_shell(cwd, &env_refs);
        }

        self.stack.add_named(terminal.widget(), Some(&id));
        self.sidebar.add_tab(&session);

        let sidebar = self.sidebar.clone();
        let session_id = id.clone();
        terminal.connect_title_changed(move |new_title| {
            sidebar.update_title(&session_id, new_title);
        });

        self.terminals.insert(id.clone(), terminal);
        self.sessions.push(session);
        self.switch_to(&id);

        id
    }

    pub fn destroy_session(&mut self, session_id: &str) {
        if let Some(terminal) = self.terminals.get(session_id) {
            if terminal.widget().parent().as_ref() == Some(self.stack.upcast_ref()) {
                self.stack.remove(terminal.widget());
            }
        }

        self.sidebar.remove_tab(session_id);
        self.terminals.remove(session_id);
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
            if let Some(terminal) = self.terminals.get(&session.id) {
                if terminal.needs_spawn() {
                    let env_vars = self.build_env_vars(&session.id);
                    let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                    terminal.spawn_shell(session.cwd.as_deref(), &env_refs);
                }
            }
        }
    }

    pub fn switch_to(&mut self, session_id: &str) {
        self.active_id = Some(session_id.to_string());
        self.stack.set_visible_child_name(session_id);
        self.sidebar.set_active(session_id);

        if let Some(terminal) = self.terminals.get(session_id) {
            terminal.widget().grab_focus();
        }
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    pub fn active_terminal(&self) -> Option<&VteTerminal> {
        self.active_id.as_deref().and_then(|id| self.terminals.get(id))
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

    pub fn sessions_with_claude_pid(&self) -> Vec<(String, u32)> {
        self.sessions.iter()
            .filter_map(|s| s.claude_pid.map(|pid| (s.id.clone(), pid)))
            .collect()
    }

    pub fn wire_child_exited(self_ref: &Rc<RefCell<Self>>, session_id: &str) {
        let mgr = self_ref.clone();
        let id = session_id.to_string();
        let borrow = self_ref.borrow();

        if let Some(terminal) = borrow.terminals.get(session_id) {
            terminal.connect_child_exited(move |_status| {
                let id = id.clone();
                let mgr = mgr.clone();

                glib::idle_add_local_once(move || {
                    mgr.borrow_mut().destroy_session(&id);
                });
            });
        }
    }

    /// Save current session state for restoration on next launch.
    pub fn save_state(&self, sidebar_width: i32) {
        let state = SessionState {
            sessions: self.sessions.iter().map(|s| SavedSession {
                title: s.title.clone(),
                cwd: s.cwd.clone(),
            }).collect(),
            sidebar_width: Some(sidebar_width),
        };

        state.save();
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
