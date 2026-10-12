use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Broad category of a file, derived from its extension. Drives icon colour,
/// the "Kind" column and whether a thumbnail is worth generating.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum FileClass {
    Folder,
    Image,
    Video,
    Audio,
    Document,
    Spreadsheet,
    Presentation,
    Pdf,
    Archive,
    Code,
    Executable,
    Font,
    Disk,
    Other,
}

impl FileClass {
    pub fn from_ext(ext: &str) -> Self {
        match ext {
            "png" | "jpg" | "jpeg" | "jpe" | "gif" | "bmp" | "ico" | "tif" | "tiff" | "webp"
            | "tga" | "qoi" | "exr" | "hdr" | "ppm" | "pgm" | "pbm" | "pnm" | "dds" | "ff"
            | "svg" | "avif" | "heic" | "heif" | "raw" | "cr2" | "nef" | "arw" | "dng" => {
                FileClass::Image
            }
            "mp4" | "m4v" | "mkv" | "webm" | "mov" | "avi" | "wmv" | "flv" | "mpg" | "mpeg"
            | "m2ts" | "mts" | "ogv" | "3gp" => FileClass::Video,
            "mp3" | "flac" | "wav" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wma" | "aiff"
            | "aif" | "mid" | "midi" => FileClass::Audio,
            "txt" | "md" | "rst" | "doc" | "docx" | "odt" | "rtf" | "tex" | "epub" | "log"
            | "nfo" => FileClass::Document,
            "xls" | "xlsx" | "ods" | "csv" | "tsv" => FileClass::Spreadsheet,
            "ppt" | "pptx" | "odp" | "key" => FileClass::Presentation,
            "pdf" => FileClass::Pdf,
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "tgz" | "lz4" | "cab"
            | "jar" | "apk" | "deb" | "rpm" => FileClass::Archive,
            "rs" | "c" | "h" | "cc" | "cpp" | "hpp" | "cs" | "java" | "kt" | "swift" | "go"
            | "py" | "rb" | "php" | "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" | "html"
            | "htm" | "css" | "scss" | "sass" | "json" | "yaml" | "yml" | "toml" | "xml"
            | "ini" | "cfg" | "conf" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd"
            | "lua" | "sql" | "pl" | "r" | "scala" | "hs" | "ml" | "zig" | "dart" | "vim"
            | "el" => FileClass::Code,
            "exe" | "msi" | "com" | "scr" | "appimage" | "app" | "dmg" | "bin" | "run" | "so"
            | "dll" | "dylib" | "o" | "a" | "lib" | "pdb" => FileClass::Executable,
            "ttf" | "otf" | "woff" | "woff2" | "eot" | "fon" => FileClass::Font,
            "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "qcow2" | "dsk" => FileClass::Disk,
            _ => FileClass::Other,
        }
    }

    /// Human readable name used by the "Kind" column and the properties dialog.
    pub fn label(self) -> &'static str {
        match self {
            FileClass::Folder => "Folder",
            FileClass::Image => "Image",
            FileClass::Video => "Video",
            FileClass::Audio => "Audio",
            FileClass::Document => "Document",
            FileClass::Spreadsheet => "Spreadsheet",
            FileClass::Presentation => "Presentation",
            FileClass::Pdf => "PDF document",
            FileClass::Archive => "Archive",
            FileClass::Code => "Source file",
            FileClass::Executable => "Program",
            FileClass::Font => "Font",
            FileClass::Disk => "Disk image",
            FileClass::Other => "File",
        }
    }

    /// Whether a pixel thumbnail can be decoded for this class today.
    pub fn is_previewable_image(self) -> bool {
        self == FileClass::Image
    }
}

/// A single row in a directory listing.
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    /// Lowercase name, pre-computed for case-insensitive search and sorting.
    pub name_lower: String,
    pub ext: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub hidden: bool,
    pub readonly: bool,
    /// Byte size for files; `None` for directories until it is computed.
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub class: FileClass,
}

impl Entry {
    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        let meta = std::fs::symlink_metadata(&path)?;
        Ok(Self::from_meta(path, &meta))
    }

    pub fn from_meta(path: PathBuf, meta: &Metadata) -> Self {
        let is_symlink = meta.file_type().is_symlink();
        // Symlinks are described by their target, like Explorer does.
        let resolved = if is_symlink {
            std::fs::metadata(&path).ok()
        } else {
            None
        };
        let eff = resolved.as_ref().unwrap_or(meta);

        let name = file_name_of(&path);
        let is_dir = eff.is_dir();
        let ext = if is_dir {
            String::new()
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
        };

        Entry {
            name_lower: name.to_lowercase(),
            class: if is_dir {
                FileClass::Folder
            } else {
                FileClass::from_ext(&ext)
            },
            size: if is_dir { None } else { Some(eff.len()) },
            modified: eff.modified().ok(),
            created: eff.created().ok(),
            accessed: eff.accessed().ok(),
            hidden: is_hidden(&path, eff),
            readonly: eff.permissions().readonly(),
            is_dir,
            is_symlink,
            ext,
            name,
            path,
        }
    }

    /// Name without the extension, for the rename dialog's initial selection.
    pub fn stem(&self) -> &str {
        if self.is_dir || self.ext.is_empty() {
            &self.name
        } else {
            let cut = self.name.len() - self.ext.len() - 1;
            &self.name[..cut]
        }
    }
}

/// Last path component, falling back to the whole path for roots like `C:\`.
pub fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(windows)]
fn is_hidden(_path: &Path, meta: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(not(windows))]
fn is_hidden(path: &Path, _meta: &Metadata) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}
