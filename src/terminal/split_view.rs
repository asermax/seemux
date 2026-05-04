use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Orientation, Paned, Stack, Widget};

pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

use crate::config::{Config, SavedSplitNode};
use crate::session::manager::BrowserPaneState;
use crate::terminal::VteTerminal;

/// Per-pane restoration info: (pane_id, cwd, url, page_title).
type PaneRestoreInfo = (String, Option<String>, Option<String>, Option<String>);

/// Lightweight tree tracking the split layout.
/// Terminals are stored separately in a flat HashMap, so tree
/// transformations never need to move heavy objects.
enum SplitTree {
    Leaf(String),
    Split {
        orientation: Orientation,
        first: Box<SplitTree>,
        second: Box<SplitTree>,
    },
}

impl SplitTree {
    fn build_widget(&self, panes: &HashMap<String, Rc<VteTerminal>>) -> Widget {
        match self {
            SplitTree::Leaf(id) => panes[id].widget().clone(),

            SplitTree::Split { orientation, first, second } => {
                let paned = Paned::new(*orientation);
                paned.set_start_child(Some(&first.build_widget(panes)));
                paned.set_end_child(Some(&second.build_widget(panes)));
                paned.set_shrink_start_child(false);
                paned.set_shrink_end_child(false);
                paned.set_resize_start_child(true);
                paned.set_resize_end_child(true);
                paned.upcast()
            }
        }
    }

    fn first_pane_id(&self) -> &str {
        match self {
            SplitTree::Leaf(id) => id,
            SplitTree::Split { first, .. } => first.first_pane_id(),
        }
    }

    fn last_pane_id(&self) -> &str {
        match self {
            SplitTree::Leaf(id) => id,
            SplitTree::Split { second, .. } => second.last_pane_id(),
        }
    }

    /// Find the neighbor pane in a given direction.
    fn find_neighbor(&self, focused_id: &str, direction: &Direction) -> Option<String> {
        let (matching_orient, toward_second) = match direction {
            Direction::Left  => (Orientation::Horizontal, false),
            Direction::Right => (Orientation::Horizontal, true),
            Direction::Up    => (Orientation::Vertical, false),
            Direction::Down  => (Orientation::Vertical, true),
        };

        self.find_neighbor_inner(focused_id, matching_orient, toward_second)
    }

    fn find_neighbor_inner(
        &self,
        focused_id: &str,
        matching_orient: Orientation,
        toward_second: bool,
    ) -> Option<String> {
        let SplitTree::Split { orientation, first, second } = self else { return None };

        if *orientation == matching_orient {
            if toward_second && first.contains(focused_id) {
                return Some(second.first_pane_id().to_string());
            }

            if !toward_second && second.contains(focused_id) {
                return Some(first.last_pane_id().to_string());
            }
        }

        // Perpendicular split, or focused is already on the target side — recurse deeper
        if first.contains(focused_id) {
            first.find_neighbor_inner(focused_id, matching_orient, toward_second)
        } else {
            second.find_neighbor_inner(focused_id, matching_orient, toward_second)
        }
    }

    fn contains(&self, target_id: &str) -> bool {
        match self {
            SplitTree::Leaf(id) => id == target_id,
            SplitTree::Split { first, second, .. } => {
                first.contains(target_id) || second.contains(target_id)
            }
        }
    }

    /// Split the target leaf in place — no ownership transfer needed.
    fn split(&mut self, target_id: &str, orientation: Orientation, new_pane_id: &str) {
        match self {
            SplitTree::Leaf(id) if id == target_id => {
                let old_id = std::mem::take(id);
                *self = SplitTree::Split {
                    orientation,
                    first: Box::new(SplitTree::Leaf(old_id)),
                    second: Box::new(SplitTree::Leaf(new_pane_id.to_string())),
                };
            }

            SplitTree::Split { first, second, .. } => {
                if first.contains(target_id) {
                    first.split(target_id, orientation, new_pane_id);
                } else if second.contains(target_id) {
                    second.split(target_id, orientation, new_pane_id);
                }
            }

            _ => {}
        }
    }

    /// Remove a leaf and promote its sibling. Returns the new focus pane ID.
    fn remove_leaf(&mut self, target_id: &str) -> Option<String> {
        let SplitTree::Split { first, second, .. } = self else { return None };

        // First child is the target — promote second
        if matches!(**first, SplitTree::Leaf(ref id) if id == target_id) {
            let old = std::mem::replace(self, SplitTree::Leaf(String::new()));
            let SplitTree::Split { second, .. } = old else { unreachable!() };
            let new_focus = second.first_pane_id().to_string();
            *self = *second;
            return Some(new_focus);
        }

        // Second child is the target — promote first
        if matches!(**second, SplitTree::Leaf(ref id) if id == target_id) {
            let old = std::mem::replace(self, SplitTree::Leaf(String::new()));
            let SplitTree::Split { first, .. } = old else { unreachable!() };
            let new_focus = first.first_pane_id().to_string();
            *self = *first;
            return Some(new_focus);
        }

        // Recurse into the child containing the target
        if first.contains(target_id) {
            return first.remove_leaf(target_id);
        }

        if second.contains(target_id) {
            return second.remove_leaf(target_id);
        }

        None
    }

    fn to_saved(&self, cwds: &HashMap<String, String>, browser_panes: &HashMap<String, BrowserPaneState>) -> SavedSplitNode {
        match self {
            SplitTree::Leaf(id) => {
                let (url, page_title) = browser_panes.get(id)
                    .map(|bp| (Some(bp.url.clone()), bp.page_title.clone()))
                    .unwrap_or((None, None));

                SavedSplitNode::Leaf {
                    cwd: cwds.get(id).cloned(),
                    url,
                    page_title,
                }
            }
            SplitTree::Split { orientation, first, second } => SavedSplitNode::Split {
                orientation: match orientation {
                    Orientation::Horizontal => "horizontal".to_string(),
                    _ => "vertical".to_string(),
                },
                first: Box::new(first.to_saved(cwds, browser_panes)),
                second: Box::new(second.to_saved(cwds, browser_panes)),
            },
        }
    }
}

/// Manages the split tree for a single session.
///
/// Terminals stored in a flat HashMap, tree structure tracks layout only.
/// Split/close modify data in place, then `rebuild_in_stack` updates the GTK widgets.
pub struct SplitView {
    panes: RefCell<HashMap<String, Rc<VteTerminal>>>,
    tree: RefCell<SplitTree>,
    focused_pane_id: RefCell<String>,
}

impl SplitView {
    pub fn new(terminal: VteTerminal, pane_id: String) -> Self {
        let mut panes = HashMap::new();
        panes.insert(pane_id.clone(), Rc::new(terminal));

        Self {
            panes: RefCell::new(panes),
            tree: RefCell::new(SplitTree::Leaf(pane_id.clone())),
            focused_pane_id: RefCell::new(pane_id),
        }
    }

    /// Build the widget tree for initial display.
    pub fn build_widget(&self) -> Widget {
        self.tree.borrow().build_widget(&self.panes.borrow())
    }

    /// Navigate from the focused pane in a direction. Returns the target terminal if found.
    pub fn navigate(&self, direction: Direction) -> Option<Rc<VteTerminal>> {
        let focused_id = self.focused_pane_id.borrow().clone();
        let new_id = self.tree.borrow().find_neighbor(&focused_id, &direction)?;

        *self.focused_pane_id.borrow_mut() = new_id.clone();
        self.panes.borrow().get(&new_id).cloned()
    }

    /// Remove old widget tree from stack, rebuild, re-add.
    pub fn rebuild_in_stack(&self, stack: &Stack, name: &str) {
        // Move focus away from the pane tree before tearing it down
        // to avoid GTK warnings about focus on detached widgets
        stack.grab_focus();

        if let Some(old) = stack.child_by_name(name) {
            // Recursively clear Paned children using the proper API
            // (direct unparent() leaves dangling pointers in Paned internals)
            Self::clear_paned_children(&old);
            stack.remove(&old);
        }

        let panes = self.panes.borrow();
        let new_widget = self.tree.borrow().build_widget(&panes);
        stack.add_named(&new_widget, Some(name));
        stack.set_visible_child_name(name);
    }

    /// Recursively detach all children from Paned widgets using set_start_child/set_end_child.
    fn clear_paned_children(widget: &Widget) {
        let Some(paned) = widget.downcast_ref::<Paned>() else { return };

        if let Some(start) = paned.start_child() {
            Self::clear_paned_children(&start);
        }

        if let Some(end) = paned.end_child() {
            Self::clear_paned_children(&end);
        }

        // Clear the Paned's internal focus child before detaching,
        // otherwise GTK warns about set_focus_child on a non-child widget
        paned.set_focus_child(None::<&Widget>);
        paned.set_start_child(None::<&Widget>);
        paned.set_end_child(None::<&Widget>);
    }

    /// Split the focused pane.
    /// Returns (new_pane_id, new_terminal) for the caller to wire signals.
    /// Caller must call `rebuild_in_stack` after.
    pub fn split(&self, orientation: Orientation, config: &Config) -> (String, Rc<VteTerminal>) {
        let new_pane_id = uuid::Uuid::new_v4().to_string();
        let focused_id = self.focused_pane_id.borrow().clone();

        let terminal = Rc::new(VteTerminal::new_with_config(config));
        self.panes.borrow_mut().insert(new_pane_id.clone(), terminal.clone());

        self.tree.borrow_mut().split(&focused_id, orientation, &new_pane_id);

        *self.focused_pane_id.borrow_mut() = new_pane_id.clone();
        (new_pane_id, terminal)
    }

    /// Close the focused pane. Returns true if the session should be destroyed (last pane).
    /// Caller must call `rebuild_in_stack` after if this returns false.
    pub fn close_focused_pane(&self) -> bool {
        if self.panes.borrow().len() <= 1 {
            return true;
        }

        let focused_id = self.focused_pane_id.borrow().clone();
        self.panes.borrow_mut().remove(&focused_id);

        if let Some(new_focus) = self.tree.borrow_mut().remove_leaf(&focused_id) {
            *self.focused_pane_id.borrow_mut() = new_focus;
        }

        false
    }

    pub fn set_focused_pane_id(&self, id: &str) {
        *self.focused_pane_id.borrow_mut() = id.to_string();
    }

    pub fn focused_terminal(&self) -> Option<Rc<VteTerminal>> {
        let focused_id = self.focused_pane_id.borrow();
        self.panes.borrow().get(focused_id.as_str()).cloned()
    }

    pub fn has_pane(&self, id: &str) -> bool {
        self.panes.borrow().contains_key(id)
    }

    /// Return any one pane ID, or None if the view is empty.
    pub fn any_pane_id(&self) -> Option<String> {
        self.panes.borrow().keys().next().cloned()
    }

    /// Return all pane IDs.
    pub fn pane_ids(&self) -> Vec<String> {
        self.panes.borrow().keys().cloned().collect()
    }

    /// Collect all (pane_id, terminal) pairs for signal wiring.
    pub fn collect_terminals(&self) -> Vec<(String, Rc<VteTerminal>)> {
        self.panes.borrow().iter()
            .map(|(id, t)| (id.clone(), t.clone()))
            .collect()
    }

    /// Get a terminal by pane ID.
    pub fn terminal_for_pane(&self, pane_id: &str) -> Option<Rc<VteTerminal>> {
        self.panes.borrow().get(pane_id).cloned()
    }

    /// Returns true if any pane has not yet been spawned.
    pub fn needs_spawn(&self) -> bool {
        self.panes.borrow().values().any(|t| t.needs_spawn())
    }

    /// Returns true if any pane has a running command detected via VTE title heuristics.
    pub fn has_running_command(&self) -> bool {
        self.panes.borrow().values().any(|t| t.is_running().get())
    }

    /// Spawn a shell in a specific pane by ID.
    pub fn spawn_pane(&self, pane_id: &str, cwd: Option<&str>, env_vars: &[(&str, &str)]) {
        if let Some(terminal) = self.panes.borrow().get(pane_id)
            && terminal.needs_spawn() {
                terminal.spawn_shell(cwd, env_vars);
            }
    }

    /// Convert the entire split tree to serializable form.
    pub fn to_saved(&self, cwds: &HashMap<String, String>, browser_panes: &HashMap<String, BrowserPaneState>) -> SavedSplitNode {
        self.tree.borrow().to_saved(cwds, browser_panes)
    }

    /// Build a SplitView from a saved tree, creating terminals with per-pane CWDs.
    /// Returns the view and a list of (pane_id, cwd, url, page_title) tuples for spawning.
    pub fn from_saved(saved: &SavedSplitNode, config: &Config) -> (Self, Vec<PaneRestoreInfo>) {
        let mut panes_map = HashMap::new();
        let mut panes_list = Vec::new();
        let tree = Self::tree_from_saved(saved, config, &mut panes_map, &mut panes_list);
        let focused = tree.first_pane_id().to_string();

        let view = Self {
            panes: RefCell::new(panes_map),
            tree: RefCell::new(tree),
            focused_pane_id: RefCell::new(focused),
        };

        (view, panes_list)
    }

    fn tree_from_saved(
        saved: &SavedSplitNode,
        config: &Config,
        panes: &mut HashMap<String, Rc<VteTerminal>>,
        pane_list: &mut Vec<PaneRestoreInfo>,
    ) -> SplitTree {
        match saved {
            SavedSplitNode::Leaf { cwd, url, page_title } => {
                let pane_id = uuid::Uuid::new_v4().to_string();
                let terminal = Rc::new(VteTerminal::new_with_config(config));
                panes.insert(pane_id.clone(), terminal);
                pane_list.push((pane_id.clone(), cwd.clone(), url.clone(), page_title.clone()));
                SplitTree::Leaf(pane_id)
            }
            SavedSplitNode::Split { orientation, first, second } => {
                let orient = if orientation == "horizontal" {
                    Orientation::Horizontal
                } else {
                    Orientation::Vertical
                };

                SplitTree::Split {
                    orientation: orient,
                    first: Box::new(Self::tree_from_saved(first, config, panes, pane_list)),
                    second: Box::new(Self::tree_from_saved(second, config, panes, pane_list)),
                }
            }
        }
    }
}
