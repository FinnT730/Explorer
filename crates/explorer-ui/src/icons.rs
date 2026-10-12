//! Vector icons drawn straight onto the egui painter.
//!
//! Drawing them rather than shipping bitmaps keeps the binary asset-free,
//! makes every icon crisp at any zoom level, and costs a handful of triangles
//! per tile instead of a texture upload.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use explorer_core::FileClass;

use crate::theme::Palette;

pub fn class_color(class: FileClass, pal: &Palette) -> Color32 {
    match class {
        FileClass::Folder => pal.folder,
        FileClass::Image => Color32::from_rgb(0x4a, 0xc2, 0x8f),
        FileClass::Video => Color32::from_rgb(0xd1, 0x64, 0xd1),
        FileClass::Audio => Color32::from_rgb(0xe0, 0x7a, 0x5f),
        FileClass::Document => Color32::from_rgb(0x6f, 0x9f, 0xd8),
        FileClass::Spreadsheet => Color32::from_rgb(0x3f, 0xa4, 0x62),
        FileClass::Presentation => Color32::from_rgb(0xd9, 0x8a, 0x3c),
        FileClass::Pdf => Color32::from_rgb(0xd0, 0x4a, 0x42),
        FileClass::Archive => Color32::from_rgb(0xc1, 0xa2, 0x4a),
        FileClass::Code => Color32::from_rgb(0x59, 0xa8, 0xc9),
        FileClass::Executable => Color32::from_rgb(0x8a, 0x8f, 0xf0),
        FileClass::Font => Color32::from_rgb(0xa8, 0x8c, 0xd0),
        FileClass::Disk => Color32::from_rgb(0x7d, 0x9a, 0xa8),
        FileClass::Other => Color32::from_gray(140),
    }
}

/// Short tag drawn on a file icon: the extension, uppercased and clipped.
pub fn ext_tag(ext: &str) -> String {
    let up = ext.to_ascii_uppercase();
    if up.chars().count() > 4 {
        up.chars().take(4).collect()
    } else {
        up
    }
}

/// Draw the icon for an entry inside `rect`, centred and aspect-correct.
pub fn draw(
    painter: &Painter,
    rect: Rect,
    class: FileClass,
    ext: &str,
    pal: &Palette,
    symlink: bool,
) {
    let side = rect.width().min(rect.height());
    let body = Rect::from_center_size(rect.center(), Vec2::splat(side));
    match class {
        FileClass::Folder => folder(painter, body, pal),
        _ => sheet(painter, body, class, ext, pal),
    }
    if symlink {
        link_badge(painter, body, pal);
    }
}

fn folder(painter: &Painter, rect: Rect, pal: &Palette) {
    let w = rect.width();
    let h = rect.height();
    let base = Rect::from_min_size(
        rect.min + Vec2::new(w * 0.06, h * 0.26),
        Vec2::new(w * 0.88, h * 0.52),
    );
    // Back flap, offset up and darkened, gives the folder depth.
    let tab = Rect::from_min_size(
        rect.min + Vec2::new(w * 0.06, h * 0.18),
        Vec2::new(w * 0.42, h * 0.12),
    );
    let r = CornerRadius::same((w * 0.06).round().clamp(1.0, 8.0) as u8);
    painter.rect_filled(tab, r, shade(pal.folder, 0.82));
    painter.rect_filled(base, r, pal.folder);
    painter.rect_filled(
        Rect::from_min_size(base.min, Vec2::new(base.width(), h * 0.08)),
        r,
        shade(pal.folder, 1.1),
    );
}

fn sheet(painter: &Painter, rect: Rect, class: FileClass, ext: &str, pal: &Palette) {
    let w = rect.width();
    let h = rect.height();
    let page = Rect::from_min_size(
        rect.min + Vec2::new(w * 0.2, h * 0.12),
        Vec2::new(w * 0.6, h * 0.76),
    );
    let color = class_color(class, pal);
    let r = CornerRadius::same((w * 0.04).round().clamp(1.0, 6.0) as u8);
    painter.rect_filled(page, r, shade(color, 0.3).gamma_multiply(1.0));
    painter.rect_stroke(page, r, Stroke::new(1.0, color), StrokeKind::Inside);

    // Folded corner.
    let fold = w * 0.16;
    let tr = page.right_top();
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(tr.x - fold, tr.y),
            Pos2::new(tr.x, tr.y + fold),
            Pos2::new(tr.x - fold, tr.y + fold),
        ],
        color,
        Stroke::NONE,
    ));

    // Extension tag, only where it stays legible.
    if !ext.is_empty() && page.height() > 34.0 {
        let band = Rect::from_min_size(
            Pos2::new(page.left(), page.bottom() - page.height() * 0.3),
            Vec2::new(page.width(), page.height() * 0.3),
        );
        painter.rect_filled(band, r, color);
        painter.text(
            band.center(),
            Align2::CENTER_CENTER,
            ext_tag(ext),
            FontId::proportional((band.height() * 0.62).clamp(7.0, 13.0)),
            Color32::WHITE,
        );
    } else if page.height() > 14.0 {
        // Too small for text: three content lines instead.
        for i in 0..3 {
            let y = page.top() + page.height() * (0.35 + i as f32 * 0.18);
            painter.line_segment(
                [
                    Pos2::new(page.left() + page.width() * 0.18, y),
                    Pos2::new(page.right() - page.width() * 0.18, y),
                ],
                Stroke::new(1.0, color.gamma_multiply(0.7)),
            );
        }
    }
}

fn link_badge(painter: &Painter, rect: Rect, pal: &Palette) {
    let s = rect.width() * 0.26;
    let r = Rect::from_min_size(
        Pos2::new(rect.left() + rect.width() * 0.08, rect.bottom() - s),
        Vec2::splat(s),
    );
    painter.rect_filled(r, CornerRadius::same(2), pal.accent);
    painter.text(
        r.center(),
        Align2::CENTER_CENTER,
        "\u{2197}",
        FontId::proportional(s * 0.8),
        Color32::WHITE,
    );
}

/// Draw a drive icon with a used-space bar, for the sidebar.
pub fn drive(painter: &Painter, rect: Rect, used: f32, pal: &Palette) {
    let body = Rect::from_center_size(
        rect.center(),
        Vec2::new(rect.width() * 0.9, rect.height() * 0.6),
    );
    painter.rect_filled(body, CornerRadius::same(2), Color32::from_gray(120));
    let bar = Rect::from_min_size(
        Pos2::new(body.left() + 2.0, body.bottom() - 4.0),
        Vec2::new((body.width() - 4.0) * used.clamp(0.0, 1.0), 2.0),
    );
    let color = if used > 0.9 {
        Color32::from_rgb(0xd0, 0x4a, 0x42)
    } else {
        pal.accent
    };
    painter.rect_filled(bar, CornerRadius::ZERO, color);
}

/// Simple glyph-free chevron, used by breadcrumbs and tree twisties.
pub fn chevron(painter: &Painter, center: Pos2, size: f32, open: bool, color: Color32) {
    let s = size * 0.5;
    let pts = if open {
        vec![
            Pos2::new(center.x - s, center.y - s * 0.4),
            Pos2::new(center.x, center.y + s * 0.6),
            Pos2::new(center.x + s, center.y - s * 0.4),
        ]
    } else {
        vec![
            Pos2::new(center.x - s * 0.4, center.y - s),
            Pos2::new(center.x + s * 0.6, center.y),
            Pos2::new(center.x - s * 0.4, center.y + s),
        ]
    };
    painter.add(egui::Shape::line(pts, Stroke::new(1.6, color)));
}

fn shade(c: Color32, f: f32) -> Color32 {
    let m = |v: u8| ((v as f32 * f).clamp(0.0, 255.0)) as u8;
    Color32::from_rgba_unmultiplied(m(c.r()), m(c.g()), m(c.b()), c.a())
}

/// Direction for the painter-drawn navigation arrows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Refresh,
}

/// A small navigation arrow. Drawn rather than typed, because the default
/// font set has no glyphs for the arrow characters.
pub fn nav_arrow(painter: &Painter, rect: Rect, dir: Dir, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.3;
    let stroke = Stroke::new(1.6, color);
    match dir {
        Dir::Left | Dir::Right | Dir::Up => {
            let v = match dir {
                Dir::Left => Vec2::new(-1.0, 0.0),
                Dir::Right => Vec2::new(1.0, 0.0),
                _ => Vec2::new(0.0, -1.0),
            };
            let tip = c + v * s;
            let tail = c - v * s;
            painter.line_segment([tail, tip], stroke);
            let perp = Vec2::new(-v.y, v.x) * s * 0.6;
            painter.line_segment([tip, tip - v * s * 0.7 + perp], stroke);
            painter.line_segment([tip, tip - v * s * 0.7 - perp], stroke);
        }
        Dir::Refresh => {
            // Three-quarter circle with an arrow head, i.e. a reload symbol.
            let r = s;
            let mut pts = Vec::with_capacity(24);
            for i in 0..=22 {
                let a = std::f32::consts::TAU * (0.15 + 0.78 * i as f32 / 22.0);
                pts.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
            }
            painter.add(egui::Shape::line(pts.clone(), stroke));
            if let Some(end) = pts.last() {
                painter.line_segment([*end, *end + Vec2::new(-r * 0.5, -r * 0.1)], stroke);
                painter.line_segment([*end, *end + Vec2::new(-r * 0.1, -r * 0.55)], stroke);
            }
        }
    }
}

/// Sort indicator triangle.
pub fn triangle(painter: &Painter, center: Pos2, size: f32, up: bool, color: Color32) {
    let s = size * 0.5;
    let pts = if up {
        vec![
            Pos2::new(center.x - s, center.y + s * 0.6),
            Pos2::new(center.x + s, center.y + s * 0.6),
            Pos2::new(center.x, center.y - s * 0.7),
        ]
    } else {
        vec![
            Pos2::new(center.x - s, center.y - s * 0.6),
            Pos2::new(center.x + s, center.y - s * 0.6),
            Pos2::new(center.x, center.y + s * 0.7),
        ]
    };
    painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Tiny pictogram of a view mode, for the status-bar switcher.
pub fn view_glyph(painter: &Painter, rect: Rect, mode: ViewGlyph, color: Color32) {
    let r = rect.shrink(3.0);
    let stroke = Stroke::new(1.2, color);
    match mode {
        ViewGlyph::Details => {
            for i in 0..3 {
                let y = r.top() + r.height() * (0.2 + 0.3 * i as f32);
                painter.line_segment(
                    [
                        Pos2::new(r.left(), y),
                        Pos2::new(r.left() + r.width() * 0.25, y),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(r.left() + r.width() * 0.4, y),
                        Pos2::new(r.right(), y),
                    ],
                    stroke,
                );
            }
        }
        ViewGlyph::List => {
            for i in 0..3 {
                let y = r.top() + r.height() * (0.2 + 0.3 * i as f32);
                painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], stroke);
            }
        }
        ViewGlyph::Icons => {
            let s = r.width() * 0.4;
            for (dx, dy) in [(0.0, 0.0), (0.55, 0.0), (0.0, 0.55), (0.55, 0.55)] {
                let p = Rect::from_min_size(
                    Pos2::new(r.left() + r.width() * dx, r.top() + r.height() * dy),
                    Vec2::splat(s),
                );
                painter.rect_filled(p, CornerRadius::same(1), color);
            }
        }
        ViewGlyph::Tiles => {
            let s = r.height() * 0.4;
            for dy in [0.0, 0.55] {
                let p = Rect::from_min_size(
                    Pos2::new(r.left(), r.top() + r.height() * dy),
                    Vec2::new(s, s),
                );
                painter.rect_filled(p, CornerRadius::same(1), color);
                let y = p.center().y;
                painter.line_segment(
                    [Pos2::new(p.right() + 2.0, y), Pos2::new(r.right(), y)],
                    stroke,
                );
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewGlyph {
    Details,
    List,
    Icons,
    Tiles,
}
