use std::cell::RefCell;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ksni::menu::StandardItem;
use ksni::{self, Icon, MenuItem, Status, ToolTip, Tray, TrayService};

struct SeemuxTray {
    count: u32,
    icon_name: String,
    socket_path: PathBuf,
    quake: bool,
    badge_color: (u8, u8, u8),
}

impl SeemuxTray {
    fn send_event(&self, event: &str) {
        let Ok(mut stream) = UnixStream::connect(&self.socket_path) else { return };
        let msg = format!("{{\"event\":\"{event}\",\"session_id\":\"\",\"payload\":{{}}}}\n");
        let _ = stream.write_all(msg.as_bytes());
    }
}

impl Tray for SeemuxTray {
    fn id(&self) -> String {
        "seemux".into()
    }

    fn title(&self) -> String {
        "Seemux".into()
    }

    fn icon_name(&self) -> String {
        self.icon_name.clone()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn overlay_icon_pixmap(&self) -> Vec<Icon> {
        if self.count == 0 {
            return vec![];
        }

        render_badge(self.count, self.badge_color)
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

    fn activate(&mut self, _x: i32, _y: i32) {
        if self.quake {
            self.send_event("toggle-dropdown");
        } else {
            self.send_event("activate-window");
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            MenuItem::Standard(StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| {
                    tray.send_event("quit");
                }),
                ..Default::default()
            }),
        ]
    }
}

#[derive(Clone)]
pub struct TrayHandle {
    handle: Rc<RefCell<Option<ksni::Handle<SeemuxTray>>>>,
}

impl TrayHandle {
    pub fn disabled() -> Self {
        Self {
            handle: Rc::new(RefCell::new(None)),
        }
    }

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

/// Parse a CSS hex color like "#89b4fa" into (R, G, B).
fn parse_hex_color(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');

    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0x89);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0xB4);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0xFA);
        return (r, g, b);
    }

    (0x89, 0xB4, 0xFA) // fallback to Catppuccin blue
}

pub fn setup_tray(icon_name: &str, socket_path: &Path, quake: bool, accent_color: &str) -> TrayHandle {
    let tray = SeemuxTray {
        count: 0,
        icon_name: icon_name.to_string(),
        socket_path: socket_path.to_path_buf(),
        quake,
        badge_color: parse_hex_color(accent_color),
    };

    let service = TrayService::new(tray);
    let handle = service.handle();
    service.spawn();

    TrayHandle {
        handle: Rc::new(RefCell::new(Some(handle))),
    }
}

// --- Badge rendering ---

/// 4x6 bitmap font for digits 0-9 and "+".
/// Each digit is 4 columns wide, 6 rows tall. Stored as row-major [u8; 6],
/// where each byte's lower 4 bits represent the 4 pixel columns (MSB = left).
const DIGIT_GLYPHS: [[u8; 6]; 11] = [
    // 0
    [0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110],
    // 1
    [0b0010, 0b0110, 0b0010, 0b0010, 0b0010, 0b0111],
    // 2
    [0b0110, 0b1001, 0b0010, 0b0100, 0b1000, 0b1111],
    // 3
    [0b0110, 0b1001, 0b0010, 0b0001, 0b1001, 0b0110],
    // 4
    [0b1001, 0b1001, 0b1111, 0b0001, 0b0001, 0b0001],
    // 5
    [0b1111, 0b1000, 0b1110, 0b0001, 0b1001, 0b0110],
    // 6
    [0b0110, 0b1000, 0b1110, 0b1001, 0b1001, 0b0110],
    // 7
    [0b1111, 0b0001, 0b0010, 0b0100, 0b0100, 0b0100],
    // 8
    [0b0110, 0b1001, 0b0110, 0b1001, 0b1001, 0b0110],
    // 9
    [0b0110, 0b1001, 0b1001, 0b0111, 0b0001, 0b0110],
    // + (index 10)
    [0b0000, 0b0010, 0b0111, 0b0010, 0b0000, 0b0000],
];

const BADGE_SIZE: i32 = 44;

/// Render a notification badge as ARGB32 overlay icon.
fn render_badge(count: u32, color: (u8, u8, u8)) -> Vec<Icon> {
    let size = BADGE_SIZE as usize;
    let mut buf = vec![0u8; size * size * 4];
    let (r, g, b) = color;

    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let radius = size as f32 / 2.0 - 1.0;

    // Draw filled circle in the theme accent color
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;

            if dx * dx + dy * dy <= radius * radius {
                let offset = (y * size + x) * 4;

                // ARGB32 network byte order (big-endian): A, R, G, B
                buf[offset] = 0xFF;
                buf[offset + 1] = r;
                buf[offset + 2] = g;
                buf[offset + 3] = b;
            }
        }
    }

    // Determine which glyphs to draw
    let glyphs: Vec<usize> = if count > 9 {
        vec![9, 10] // "9+"
    } else {
        vec![count as usize]
    };

    let scale = 2usize;
    let glyph_width = 4 * scale;
    let glyph_height = 6 * scale;
    let spacing = scale;
    let total_width = glyphs.len() * glyph_width + (glyphs.len() - 1) * spacing;
    let start_x = (size - total_width) / 2;
    let start_y = (size - glyph_height) / 2;

    // Draw white digit(s), scaled up
    for (gi, &glyph_idx) in glyphs.iter().enumerate() {
        let gx = start_x + gi * (glyph_width + spacing);
        let glyph = &DIGIT_GLYPHS[glyph_idx];

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..4usize {
                if bits & (0b1000 >> col) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = gx + col * scale + sx;
                            let py = start_y + row * scale + sy;

                            if px < size && py < size {
                                let offset = (py * size + px) * 4;
                                buf[offset] = 0xFF;     // A
                                buf[offset + 1] = 0xFF; // R
                                buf[offset + 2] = 0xFF; // G
                                buf[offset + 3] = 0xFF; // B
                            }
                        }
                    }
                }
            }
        }
    }

    vec![Icon {
        width: BADGE_SIZE,
        height: BADGE_SIZE,
        data: buf,
    }]
}
