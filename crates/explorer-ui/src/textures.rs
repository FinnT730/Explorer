//! Uploads decoded thumbnails to GPU textures and keeps them around for as
//! long as the thumbnail cache does.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{ColorImage, TextureHandle, TextureOptions};
use explorer_thumbs::Thumb;

#[derive(Default)]
pub struct TextureStore {
    map: HashMap<(PathBuf, u32), TextureHandle>,
}

impl TextureStore {
    /// Texture for `thumb`, uploading it on first use.
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        bucket: u32,
        thumb: &Thumb,
    ) -> &TextureHandle {
        let key = (path.to_path_buf(), bucket);
        self.map.entry(key).or_insert_with(|| {
            let image = ColorImage::from_rgba_unmultiplied(
                [thumb.width as usize, thumb.height as usize],
                &thumb.rgba,
            );
            ctx.load_texture(
                format!("thumb:{}:{bucket}", path.display()),
                image,
                TextureOptions::LINEAR,
            )
        })
    }

    /// Drop textures once there are far more than a screenful.
    pub fn trim(&mut self, keep: usize) {
        if self.map.len() <= keep {
            return;
        }
        // Textures are refcounted by egui; dropping handles frees GPU memory.
        let excess = self.map.len() - keep;
        let drop_keys: Vec<_> = self.map.keys().take(excess).cloned().collect();
        for k in drop_keys {
            self.map.remove(&k);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
