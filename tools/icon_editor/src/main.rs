//! TinyGate OLED icon editor: visual editing for the 11x7 icons stored as
//! paired half-glyphs in components/tg_display/src/display_canvas.c.
//!
//! Run from the project root: `cargo run --manifest-path tools/icon_editor/Cargo.toml`
//! (or pass the .c path as the first argument).

use eframe::egui::{self, Align, Color32, Sense, Stroke, Vec2};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const TABLE_ANCHOR: &str = "static const uint8_t icons[16][5]";
const ROWS: usize = 7;
/// Column x==5 is the character gap between the two half-glyph cells: not
/// part of the icon, rendered as a disabled column.
const GAP_X: usize = 5;
const COLS: usize = 11;
const ICON_COUNT: usize = 8;

struct IconDef {
    name: String,
    /// Column bytes, bit k = row k, bit 0 = top. x0..x4 of the 11px grid.
    left: [u8; 5],
    /// x6..x10 of the grid.
    right: [u8; 5],
}

impl IconDef {
    fn get(&self, x: usize, y: usize) -> bool {
        let byte = if x < 5 {
            &self.left[x]
        } else if x > GAP_X {
            &self.right[x - GAP_X - 1]
        } else {
            return false;
        };
        *byte & (1 << y) != 0
    }

    fn set(&mut self, x: usize, y: usize, on: bool) {
        let byte = if x < 5 {
            &mut self.left[x]
        } else if x > GAP_X {
            &mut self.right[x - GAP_X - 1]
        } else {
            return;
        };
        if on {
            *byte |= 1 << y;
        } else {
            *byte &= !(1 << y);
        }
    }

    /// Copy the left half mirrored onto the right half.
    fn mirror(&mut self) {
        for (dst, src) in self.right.iter_mut().zip(self.left.iter().rev()) {
            *dst = *src;
        }
    }

    fn clear(&mut self) {
        self.left = [0; 5];
        self.right = [0; 5];
    }
}

fn fmt_half(half: &[u8; 5]) -> String {
    let parts: Vec<String> = half.iter().map(|b| format!("0x{b:02X}")).collect();
    format!("{{{}}}", parts.join(", "))
}

fn load(path: &Path) -> Result<(String, Vec<IconDef>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let anchor = text.find(TABLE_ANCHOR).ok_or("icons table anchor not found")?;
    let end = text[anchor..]
        .find("};")
        .map(|i| anchor + i + 2)
        .ok_or("icons table terminator not found")?;
    let block = &text[anchor..end];

    let mut names: Vec<String> = Vec::new();
    for line in block.lines() {
        if let Some(rest) = line.trim().strip_prefix("/* ") {
            if let Some(word) = rest.split_whitespace().next() {
                names.push(word.to_string());
            }
        }
    }

    // Byte scan must ignore comments: name comments carry the 0xNN code points.
    let stripped = strip_comments(block);
    let mut bytes: Vec<u8> = Vec::new();
    let raw = stripped.as_bytes();
    let mut i = 0;
    while i + 3 < raw.len() {
        if &raw[i..i + 2] == b"0x" {
            let hex = std::str::from_utf8(&raw[i + 2..i + 4]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                bytes.push(b);
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    if bytes.len() != 5 * 2 * ICON_COUNT {
        return Err(format!(
            "expected {} bytes in the table, found {}",
            5 * 2 * ICON_COUNT,
            bytes.len()
        ));
    }
    if names.len() != ICON_COUNT {
        return Err(format!(
            "expected {} icon name comments, found {}",
            ICON_COUNT,
            names.len()
        ));
    }

    let icons = names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let base = 10 * i;
            let mut left = [0u8; 5];
            let mut right = [0u8; 5];
            left.copy_from_slice(&bytes[base..base + 5]);
            right.copy_from_slice(&bytes[base + 5..base + 10]);
            IconDef { name, left, right }
        })
        .collect();
    Ok((text, icons))
}

/// Remove /* ... */ comments so the byte scanner never sees the 0xNN code
/// points embedded in the per-icon name comments.
fn strip_comments(block: &str) -> String {
    let mut out = String::new();
    let mut rest = block;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("*/") {
            Some(end) => rest = &after[end + 2..],
            None => rest = "",
        }
    }
    out.push_str(rest);
    out
}

fn save(path: &Path, icons: &[IconDef]) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let anchor = text.find(TABLE_ANCHOR).ok_or("icons table anchor not found")?;
    let end = text[anchor..]
        .find("};")
        .map(|i| anchor + i + 2)
        .ok_or("icons table terminator not found")?;

    let mut table = String::new();
    let _ = write!(table, "{TABLE_ANCHOR} = {{\n");
    for (i, icon) in icons.iter().enumerate() {
        let _ = write!(
            table,
            "    /* {} 0x{:02X}/0x{:02X} */\n",
            icon.name,
            0x80 + 2 * i as u8,
            0x81 + 2 * i as u8
        );
        let _ = writeln!(
            table,
            "    {}, {},",
            fmt_half(&icon.left),
            fmt_half(&icon.right)
        );
    }
    table.push_str("};");

    let new_text = format!("{}{}{}", &text[..anchor], table, &text[end..]);
    std::fs::write(path, new_text).map_err(|e| format!("write {path:?}: {e}"))
}

struct App {
    path: PathBuf,
    icons: Vec<IconDef>,
    selected: usize,
    status: String,
    paint_mode: Option<bool>,
    cell: f32,
}

impl App {
    fn cell_at(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<(usize, usize)> {
        if !rect.contains(pos) {
            return None;
        }
        let x = ((pos.x - rect.left()) / self.cell) as usize;
        let y = ((pos.y - rect.top()) / self.cell) as usize;
        if x >= COLS || y >= ROWS || x == GAP_X {
            return None;
        }
        Some((x, y))
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("TinyGate OLED icons");
                ui.separator();
                ui.label(self.path.display().to_string());
                if ui.button("Save to display_canvas.c").clicked() {
                    match save(&self.path, &self.icons) {
                        Ok(()) => {
                            self.status = format!(
                                "saved {} — rebuild with: cargo clean -p esp-idf-sys && cargo build",
                                chrono_like_now()
                            );
                        }
                        Err(e) => self.status = format!("save failed: {e}"),
                    }
                }
            });
        });
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&self.status)
                        .small()
                        .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                );
            });
        });

        egui::SidePanel::left("list")
            .resizable(false)
            .default_width(190.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                for (i, icon) in self.icons.iter().enumerate() {
                    let is_selected = i == self.selected;
                    if ui
                        .selectable_label(
                            is_selected,
                            format!("{}  0x{:02X}/0x{:02X}", icon.name, 0x80 + 2 * i as u8, 0x81 + 2 * i as u8),
                        )
                        .clicked()
                    {
                        self.selected = i;
                    }
                    let thumb = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                    draw_grid(ui, thumb.0, icon, 18.0 / COLS as f32, is_selected);
                    ui.add_space(4.0);
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let icon = &mut self.icons[self.selected];
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(&icon.name);
                ui.separator();
                if ui.button("Mirror L→R").clicked() {
                    icon.mirror();
                }
                if ui.button("Invert").clicked() {
                    for y in 0..ROWS {
                        for x in 0..COLS {
                            let v = icon.get(x, y);
                            icon.set(x, y, !v);
                        }
                    }
                }
                if ui.button("Clear").clicked() {
                    icon.clear();
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label("x5 is the character gap — it stays blank by design");
                });
            });
            ui.add_space(8.0);

            let grid_size = Vec2::new(self.cell * COLS as f32, self.cell * ROWS as f32);
            let (rect, response) = ui.allocate_exact_size(grid_size, Sense::click_and_drag());
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some((x, y)) = self.cell_at(rect, pos) {
                    if response.drag_started() || response.clicked() {
                        let next = !self.icons[self.selected].get(x, y);
                        self.paint_mode = Some(next);
                    }
                    if (response.dragged() || response.clicked()) && self.selected < self.icons.len() {
                        if let Some(mode) = self.paint_mode {
                            self.icons[self.selected].set(x, y, mode);
                        }
                    }
                }
            }
            draw_grid_big(ui, rect, self.cell, &self.icons[self.selected]);

            ui.add_space(10.0);
            let icon = &self.icons[self.selected];
            let (l, r) = (&icon.left, &icon.right);
            ui.label(
                egui::RichText::new(format!(
                    "/* {} 0x{:02X}/0x{:02X} */\n{}\n{}",
                    icon.name,
                    0x80 + 2 * self.selected as u8,
                    0x81 + 2 * self.selected as u8,
                    fmt_half(l),
                    fmt_half(r),
                ))
                .monospace(),
            );
        });
    }
}

fn draw_grid_big(ui: &egui::Ui, rect: egui::Rect, cell: f32, icon: &IconDef) {
    let painter = ui.painter_at(rect);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::GRAY));
    for y in 0..ROWS {
        for x in 0..COLS {
            let origin = egui::Pos2::new(rect.left() + x as f32 * cell, rect.top() + y as f32 * cell);
            let cell_rect = egui::Rect::from_min_size(origin, Vec2::splat(cell));
            let color = if x == GAP_X {
                Color32::from_rgb(0x2a, 0x2a, 0x2a)
            } else if icon.get(x, y) {
                Color32::WHITE
            } else {
                Color32::from_rgb(0x12, 0x12, 0x12)
            };
            painter.rect_filled(cell_rect, 0.0, color);
            painter.rect_stroke(cell_rect, 0.0, Stroke::new(0.5_f32, Color32::from_gray(60)));
        }
    }
}

fn draw_grid(ui: &egui::Ui, rect: egui::Rect, icon: &IconDef, cell: f32, selected: bool) {
    let painter = ui.painter_at(rect);
    for y in 0..ROWS {
        for x in 0..COLS {
            let origin = egui::Pos2::new(rect.left() + x as f32 * cell, rect.top() + y as f32 * cell);
            let cell_rect = egui::Rect::from_min_size(origin, Vec2::splat(cell));
            let color = if x == GAP_X {
                Color32::from_rgb(0x24, 0x24, 0x24)
            } else if icon.get(x, y) {
                if selected {
                    Color32::LIGHT_GREEN
                } else {
                    Color32::WHITE
                }
            } else {
                Color32::from_rgb(0x14, 0x14, 0x14)
            };
            painter.rect_filled(cell_rect, 0.0, color);
        }
    }
}

fn chrono_like_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{:02}:{:02}", (secs / 60) % 60 % 24 + 8, secs % 60)
}

fn default_path() -> PathBuf {
    let candidates = [
        "components/tg_display/src/display_canvas.c",
        "../../components/tg_display/src/display_canvas.c",
    ];
    candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(candidates[0]))
}

fn main() -> eframe::Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_path);
    let (_text, icons) = load(&path).unwrap_or_else(|e| {
        eprintln!("cannot load icon table from {path:?}: {e}");
        std::process::exit(1);
    });

    let app = App {
        path,
        icons,
        selected: 0,
        status: "edit, then Save — remember cargo clean -p esp-idf-sys before rebuild".into(),
        paint_mode: None,
        cell: 34.0,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TinyGate Icon Editor",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
}
