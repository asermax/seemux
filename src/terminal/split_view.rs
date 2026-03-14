use std::cell::RefCell;

use gtk4::prelude::*;
use gtk4::{Orientation, Paned, Widget};

use crate::config::Config;
use crate::terminal::VteTerminal;

/// A tree node representing either a single terminal or a split.
pub enum SplitNode {
    Leaf {
        id: String,
        terminal: VteTerminal,
    },
    Split {
        orientation: Orientation,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

impl SplitNode {
    /// Recursively build the GTK widget tree for this node.
    pub fn build_widget(&self) -> Widget {
        match self {
            SplitNode::Leaf { terminal, .. } => terminal.widget().clone(),
            SplitNode::Split { orientation, first, second } => {
                let paned = Paned::new(*orientation);
                paned.set_start_child(Some(&first.build_widget()));
                paned.set_end_child(Some(&second.build_widget()));
                paned.set_shrink_start_child(false);
                paned.set_shrink_end_child(false);
                paned.set_resize_start_child(true);
                paned.set_resize_end_child(true);
                paned.upcast()
            }
        }
    }

    /// Find a terminal by pane ID.
    pub fn find_terminal(&self, pane_id: &str) -> Option<&VteTerminal> {
        match self {
            SplitNode::Leaf { id, terminal } if id == pane_id => Some(terminal),
            SplitNode::Split { first, second, .. } => {
                first.find_terminal(pane_id).or_else(|| second.find_terminal(pane_id))
            }
            _ => None,
        }
    }

    /// Get the first leaf's pane ID.
    pub fn first_pane_id(&self) -> &str {
        match self {
            SplitNode::Leaf { id, .. } => id,
            SplitNode::Split { first, .. } => first.first_pane_id(),
        }
    }

    /// Count the number of leaves (panes).
    pub fn pane_count(&self) -> usize {
        match self {
            SplitNode::Leaf { .. } => 1,
            SplitNode::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }
}

/// Manages the split tree for a single session.
pub struct SplitView {
    root: RefCell<SplitNode>,
    focused_pane_id: RefCell<String>,
}

impl SplitView {
    pub fn new(terminal: VteTerminal, pane_id: String) -> Self {
        let focused = pane_id.clone();

        Self {
            root: RefCell::new(SplitNode::Leaf { id: pane_id, terminal }),
            focused_pane_id: RefCell::new(focused),
        }
    }

    /// Build the widget tree for display.
    pub fn build_widget(&self) -> Widget {
        self.root.borrow().build_widget()
    }

    /// Get the currently focused terminal.
    pub fn focused_terminal(&self) -> Option<vte4::Terminal> {
        let root = self.root.borrow();
        let focused_id = self.focused_pane_id.borrow();

        root.find_terminal(&focused_id)
            .map(|vt| vt.terminal().clone())
    }

    /// Split the focused pane. Returns the new pane ID.
    pub fn split(&self, orientation: Orientation, config: &Config) -> String {
        let new_pane_id = uuid::Uuid::new_v4().to_string();
        let focused_id = self.focused_pane_id.borrow().clone();

        let mut root = self.root.borrow_mut();
        *root = Self::split_node(std::mem::replace(&mut *root, SplitNode::Leaf {
            id: String::new(),
            terminal: VteTerminal::new_with_config(config),
        }), &focused_id, orientation, config, &new_pane_id);

        new_pane_id
    }

    fn split_node(
        node: SplitNode,
        target_id: &str,
        orientation: Orientation,
        config: &Config,
        new_pane_id: &str,
    ) -> SplitNode {
        match node {
            SplitNode::Leaf { id, terminal } if id == target_id => {
                let new_terminal = VteTerminal::new_with_config(config);

                SplitNode::Split {
                    orientation,
                    first: Box::new(SplitNode::Leaf { id, terminal }),
                    second: Box::new(SplitNode::Leaf {
                        id: new_pane_id.to_string(),
                        terminal: new_terminal,
                    }),
                }
            }
            SplitNode::Split { orientation: o, first, second } => {
                SplitNode::Split {
                    orientation: o,
                    first: Box::new(Self::split_node(*first, target_id, orientation, config, new_pane_id)),
                    second: Box::new(Self::split_node(*second, target_id, orientation, config, new_pane_id)),
                }
            }
            other => other,
        }
    }

    /// Close the focused pane. Returns true if the session should be destroyed (last pane).
    pub fn close_focused_pane(&self) -> bool {
        let root = self.root.borrow();

        if root.pane_count() <= 1 {
            return true;
        }

        let focused_id = self.focused_pane_id.borrow().clone();
        drop(root);

        let mut root = self.root.borrow_mut();
        let old_root = std::mem::replace(&mut *root, SplitNode::Leaf {
            id: String::new(),
            terminal: VteTerminal::new_with_config(&Config::default()),
        });

        let (new_root, new_focus) = Self::remove_leaf(old_root, &focused_id);

        if let Some(new_root) = new_root {
            *root = new_root;
            if let Some(focus_id) = new_focus {
                *self.focused_pane_id.borrow_mut() = focus_id;
            }
            false
        } else {
            true
        }
    }

    fn remove_leaf(node: SplitNode, target_id: &str) -> (Option<SplitNode>, Option<String>) {
        match node {
            SplitNode::Leaf { id, .. } if id == target_id => {
                (None, None)
            }
            SplitNode::Split { first, second, .. } => {
                // Check if first child is the target
                if matches!(&*first, SplitNode::Leaf { id, .. } if id == target_id) {
                    let new_focus = second.first_pane_id().to_string();
                    return (Some(*second), Some(new_focus));
                }

                // Check if second child is the target
                if matches!(&*second, SplitNode::Leaf { id, .. } if id == target_id) {
                    let new_focus = first.first_pane_id().to_string();
                    return (Some(*first), Some(new_focus));
                }

                // Recurse into children
                // (simplified: only handles direct children for now)
                (Some(SplitNode::Split {
                    orientation: Orientation::Horizontal,
                    first,
                    second,
                }), None)
            }
            other => (Some(other), None),
        }
    }

    pub fn pane_count(&self) -> usize {
        self.root.borrow().pane_count()
    }

    /// Spawn shells for all terminals that haven't been spawned yet.
    pub fn spawn_deferred(&self, cwd: Option<&str>, env_vars: &[(&str, &str)]) {
        Self::spawn_node(&self.root.borrow(), cwd, env_vars);
    }

    fn spawn_node(node: &SplitNode, cwd: Option<&str>, env_vars: &[(&str, &str)]) {
        match node {
            SplitNode::Leaf { terminal, .. } => {
                if terminal.needs_spawn() {
                    terminal.spawn_shell(cwd, env_vars);
                }
            }
            SplitNode::Split { first, second, .. } => {
                Self::spawn_node(first, cwd, env_vars);
                Self::spawn_node(second, cwd, env_vars);
            }
        }
    }

    /// Spawn a shell in a specific pane by ID.
    pub fn spawn_pane(&self, pane_id: &str, cwd: Option<&str>, env_vars: &[(&str, &str)]) {
        let root = self.root.borrow();

        if let Some(terminal) = root.find_terminal(pane_id) {
            if terminal.needs_spawn() {
                terminal.spawn_shell(cwd, env_vars);
            }
        }
    }
}
