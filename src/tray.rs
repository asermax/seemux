use std::cell::RefCell;
use std::rc::Rc;

use ksni::{self, Status, ToolTip, Tray, TrayService};

struct SeemuxTray {
    count: u32,
}

impl Tray for SeemuxTray {
    fn id(&self) -> String {
        "seemux".into()
    }

    fn title(&self) -> String {
        "Seemux".into()
    }

    fn icon_name(&self) -> String {
        "utilities-terminal".into()
    }

    fn status(&self) -> Status {
        if self.count > 0 {
            Status::NeedsAttention
        } else {
            Status::Active
        }
    }

    fn tool_tip(&self) -> ToolTip {
        if self.count > 0 {
            ToolTip {
                title: format!("Seemux \u{2014} {} unread", self.count),
                ..Default::default()
            }
        } else {
            ToolTip {
                title: "Seemux".into(),
                ..Default::default()
            }
        }
    }
}

#[derive(Clone)]
pub struct TrayHandle {
    handle: Rc<RefCell<Option<ksni::Handle<SeemuxTray>>>>,
}

impl TrayHandle {
    pub fn update_count(&self, count: u32) {
        if let Some(ref handle) = *self.handle.borrow() {
            handle.update(|tray| {
                tray.count = count;
            });
        }
    }

    pub fn shutdown(&self) {
        if let Some(ref handle) = *self.handle.borrow() {
            handle.shutdown();
        }
    }
}

pub fn setup_tray() -> TrayHandle {
    let inner: Rc<RefCell<Option<ksni::Handle<SeemuxTray>>>> = Rc::new(RefCell::new(None));

    let service = TrayService::new(SeemuxTray { count: 0 });
    let handle = service.handle();
    service.spawn();

    *inner.borrow_mut() = Some(handle);

    TrayHandle { handle: inner }
}
