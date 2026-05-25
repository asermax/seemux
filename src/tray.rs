use std::cell::RefCell;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ksni::menu::StandardItem;
use ksni::{self, Icon, MenuItem, Status, ToolTip, Tray, TrayService};

const ICON_PNG_128: &[u8] = include_bytes!("../extra/logo/seemux-128x128.png");
const ICON_PNG_48: &[u8] = include_bytes!("../extra/logo/seemux-48x48.png");

struct SeemuxTray {
    count: u32,
    icon_name: String,
    socket_path: PathBuf,
    quake: bool,
    badge_color: (u8, u8, u8),
    base_icons: Vec<(i32, Vec<u8>)>,
    /// Cached rendered icons with badge — recomputed only when count changes.
    /// Avoids redundant rendering since ksni calls icon_pixmap() and
    /// attention_icon_pixmap() multiple times per update cycle.
    cached_badge_icons: Vec<Icon>,
}

impl SeemuxTray {
    fn send_event(&self, event: &str) {
        let Ok(mut stream) = UnixStream::connect(&self.socket_path) else { return };
        let msg = format!("{{\"jsonrpc\":\"2.0\",\"method\":\"{event}\",\"params\":{{}}}}\n");
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
        // Empty name forces ksni to use icon_pixmap() instead of theme lookup
        if self.count > 0 {
            String::new()
        } else {
            self.icon_name.clone()
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.cached_badge_icons.clone()
    }

    fn status(&self) -> Status {
        if self.count > 0 {
            Status::NeedsAttention
        } else {
            Status::Active
        }
    }

    fn attention_icon_pixmap(&self) -> Vec<Icon> {
        self.cached_badge_icons.clone()
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
            self.send_event("app.dropdown.toggle");
        } else {
            self.send_event("app.window.activate");
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            MenuItem::Standard(StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| {
                    tray.send_event("app.quit");
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
                if tray.count == count {
                    return;
                }

                tray.count = count;
                tray.cached_badge_icons = render_badge_icons(&tray.base_icons, count, tray.badge_color);
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
    let fallback = (0x89, 0xB4, 0xFA); // Catppuccin blue

    if hex.len() < 6 {
        return fallback;
    }

    let Ok(r) = u8::from_str_radix(&hex[0..2], 16) else { return fallback };
    let Ok(g) = u8::from_str_radix(&hex[2..4], 16) else { return fallback };
    let Ok(b) = u8::from_str_radix(&hex[4..6], 16) else { return fallback };

    (r, g, b)
}

/// Decode a PNG from raw bytes into ARGB32 network byte order pixels.
fn decode_icon_png(png_bytes: &[u8]) -> Vec<u8> {
    let decoder = png::Decoder::new(png_bytes);
    let mut reader = decoder.read_info().expect("valid embedded PNG");
    let mut rgba = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).expect("valid embedded PNG frame");
    rgba.truncate(info.buffer_size());

    let mut argb = Vec::with_capacity(rgba.len());

    for chunk in rgba.chunks_exact(4) {
        argb.push(chunk[3]); // A
        argb.push(chunk[0]); // R
        argb.push(chunk[1]); // G
        argb.push(chunk[2]); // B
    }

    argb
}

/// Box-filter downscale of ARGB32 pixel data from `src_size` to `dst_size`.
fn downscale_argb(src: &[u8], src_size: usize, dst_size: usize) -> Vec<u8> {
    let mut dst = vec![0u8; dst_size * dst_size * 4];
    let scale = src_size as f32 / dst_size as f32;

    for dy in 0..dst_size {
        for dx in 0..dst_size {
            let src_x0 = (dx as f32 * scale) as usize;
            let src_y0 = (dy as f32 * scale) as usize;
            let src_x1 = (((dx + 1) as f32 * scale) as usize).min(src_size);
            let src_y1 = (((dy + 1) as f32 * scale) as usize).min(src_size);

            let mut a_sum: u32 = 0;
            let mut r_sum: u32 = 0;
            let mut g_sum: u32 = 0;
            let mut b_sum: u32 = 0;
            let mut count: u32 = 0;

            for sy in src_y0..src_y1 {
                for sx in src_x0..src_x1 {
                    let off = (sy * src_size + sx) * 4;
                    a_sum += src[off] as u32;
                    r_sum += src[off + 1] as u32;
                    g_sum += src[off + 2] as u32;
                    b_sum += src[off + 3] as u32;
                    count += 1;
                }
            }

            if count > 0 {
                let off = (dy * dst_size + dx) * 4;
                dst[off] = (a_sum / count) as u8;
                dst[off + 1] = (r_sum / count) as u8;
                dst[off + 2] = (g_sum / count) as u8;
                dst[off + 3] = (b_sum / count) as u8;
            }
        }
    }

    dst
}

pub fn setup_tray(icon_name: &str, socket_path: &Path, quake: bool, accent_color: &str) -> TrayHandle {
    let argb_128 = decode_icon_png(ICON_PNG_128);
    let argb_48 = decode_icon_png(ICON_PNG_48);
    let argb_32 = downscale_argb(&argb_48, 48, 32);
    let argb_22 = downscale_argb(&argb_48, 48, 22);

    let base_icons = vec![
        (128, argb_128),
        (48, argb_48),
        (32, argb_32),
        (22, argb_22),
    ];

    let tray = SeemuxTray {
        count: 0,
        icon_name: icon_name.to_string(),
        socket_path: socket_path.to_path_buf(),
        quake,
        badge_color: parse_hex_color(accent_color),
        base_icons,
        cached_badge_icons: vec![],
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

fn render_badge_icons(base_icons: &[(i32, Vec<u8>)], count: u32, color: (u8, u8, u8)) -> Vec<Icon> {
    if count == 0 {
        return vec![];
    }

    base_icons
        .iter()
        .map(|(size, argb)| render_icon_with_badge(argb, *size, count, color))
        .collect()
}

/// Badge size scales proportionally — larger fraction at small icon sizes for visibility.
fn render_icon_with_badge(base_argb: &[u8], icon_size: i32, count: u32, color: (u8, u8, u8)) -> Icon {
    let size = icon_size as usize;
    let mut buf = base_argb.to_vec();
    let (r, g, b) = color;

    // Badge takes a larger fraction of the icon at small sizes so it stays readable
    let badge_fraction = if icon_size <= 24 {
        0.50
    } else if icon_size <= 32 {
        0.45
    } else if icon_size <= 48 {
        0.42
    } else {
        0.38
    };

    let badge_radius = (icon_size as f32 * badge_fraction * 0.5).round();
    let margin = (icon_size as f32 * 0.02).max(1.0);
    let cx = size as f32 - badge_radius - margin;
    let cy = size as f32 - badge_radius - margin;

    let y_min = (cy - badge_radius).floor().max(0.0) as usize;
    let y_max = ((cy + badge_radius).ceil() as usize).min(size - 1);
    let x_min = (cx - badge_radius).floor().max(0.0) as usize;
    let x_max = ((cx + badge_radius).ceil() as usize).min(size - 1);

    for y in y_min..=y_max {
        for x in x_min..=x_max {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;

            if dx * dx + dy * dy <= badge_radius * badge_radius {
                let offset = (y * size + x) * 4;
                buf[offset] = 0xFF;
                buf[offset + 1] = r;
                buf[offset + 2] = g;
                buf[offset + 3] = b;
            }
        }
    }

    let glyphs: Vec<usize> = if count > 9 {
        vec![9, 10] // "9+"
    } else {
        vec![count as usize]
    };

    let scale = if icon_size <= 24 { 1usize } else if icon_size <= 48 { 2 } else { 3 };
    let glyph_width = 4 * scale;
    let glyph_height = 6 * scale;
    let spacing = scale;
    let total_width = glyphs.len() * glyph_width + glyphs.len().saturating_sub(1) * spacing;
    let start_x = (cx.round() as usize).saturating_sub(total_width / 2);
    let start_y = (cy.round() as usize).saturating_sub(glyph_height / 2);

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

    Icon {
        width: icon_size,
        height: icon_size,
        data: buf,
    }
}
