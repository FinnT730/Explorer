//! Preview pane: a large image preview, a text head, or file metadata.

use std::path::Path;

use egui::{Color32, CornerRadius, Ui};
use explorer_core::{Entry, FileClass};
use explorer_thumbs::State as ThumbState;

use crate::views::ViewCtx;

/// Largest bucket the thumbnail cache keeps; the preview scales it to fit.
const PREVIEW_BUCKET: u32 = 256;
const TEXT_HEAD_BYTES: usize = 64 * 1024;

pub fn show(ui: &mut Ui, entry: Option<&Entry>, ctx: &mut ViewCtx<'_>) {
    let Some(entry) = entry else {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(egui::RichText::new("Select a file to preview").color(ctx.pal.text_dim));
        });
        return;
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(6.0);
            match entry.class {
                FileClass::Image => image_preview(ui, entry, ctx),
                FileClass::Code | FileClass::Document if is_texty(entry) => {
                    text_preview(ui, &entry.path, ctx)
                }
                _ => {
                    let size = ui.available_width().min(140.0);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                    crate::views::draw_icon(ui, rect, entry, ctx);
                }
            }
            ui.add_space(8.0);
            metadata(ui, entry, ctx);
        });
}

fn image_preview(ui: &mut Ui, entry: &Entry, ctx: &mut ViewCtx<'_>) {
    let avail = ui.available_width();
    match ctx.thumbs.get(&entry.path, entry.modified, PREVIEW_BUCKET) {
        ThumbState::Ready(thumb) => {
            let tex = ctx
                .textures
                .get(ui.ctx(), &entry.path, PREVIEW_BUCKET, &thumb);
            let natural = egui::vec2(thumb.width as f32, thumb.height as f32);
            let scale = (avail / natural.x).min(1.6);
            let size = natural * scale;
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect.expand(1.0), CornerRadius::same(3), ctx.pal.separator);
            ui.painter().image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("{} × {} px", thumb.source.0, thumb.source.1))
                    .color(ctx.pal.text_dim),
            );
        }
        ThumbState::Pending | ThumbState::Missing => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("Decoding…").color(ctx.pal.text_dim));
            });
        }
        ThumbState::Failed => {
            ui.label(egui::RichText::new("Cannot preview this image").color(ctx.pal.text_dim));
        }
    }
}

fn text_preview(ui: &mut Ui, path: &Path, ctx: &mut ViewCtx<'_>) {
    // Read only the head: previewing a multi-gigabyte log must stay cheap.
    let text = read_head(path, TEXT_HEAD_BYTES);
    match text {
        Some(text) => {
            ui.label(
                egui::RichText::new(text)
                    .monospace()
                    .size(11.0)
                    .color(ui.visuals().text_color()),
            );
        }
        None => {
            ui.label(egui::RichText::new("Not previewable as text").color(ctx.pal.text_dim));
        }
    }
}

fn metadata(ui: &mut Ui, entry: &Entry, ctx: &mut ViewCtx<'_>) {
    ui.separator();
    ui.label(egui::RichText::new(&entry.name).strong());
    let dim = ctx.pal.text_dim;
    let mut row = |k: &str, v: String| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(k).color(dim).small());
            ui.label(egui::RichText::new(v).small());
        });
    };
    row("Kind", entry.class.label().to_string());
    if let Some(b) = entry.size {
        row("Size", explorer_core::fmt::size(b));
    }
    if let Some(t) = entry.modified {
        row("Modified", explorer_core::fmt::time(t));
    }
    if let Some(t) = entry.created {
        row("Created", explorer_core::fmt::time(t));
    }
    if entry.readonly {
        row("Access", "Read-only".into());
    }
    if entry.is_symlink {
        let target = std::fs::read_link(&entry.path)
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        row("Link target", target);
    }
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(entry.path.display().to_string())
            .small()
            .color(dim),
    );
}

fn is_texty(entry: &Entry) -> bool {
    matches!(
        entry.ext.as_str(),
        "txt"
            | "md"
            | "rst"
            | "log"
            | "nfo"
            | "csv"
            | "tsv"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "xml"
            | "ini"
            | "cfg"
            | "conf"
    ) || entry.class == FileClass::Code
}

/// Read up to `max` bytes and keep it only if it looks like UTF-8 text.
pub fn read_head(path: &Path, max: usize) -> Option<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; max];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.iter().take(1024).any(|b| *b == 0) {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}
