use std::time::SystemTime;

use chrono::{DateTime, Local};
use humansize::{FormatSizeOptions, DECIMAL};

/// Byte size as Explorer shows it in the Size column (KB/MB/GB, no decimals
/// below a megabyte).
pub fn size(bytes: u64) -> String {
    let opts =
        FormatSizeOptions::from(DECIMAL).decimal_places(if bytes < 1024 * 1024 { 0 } else { 1 });
    humansize::format_size(bytes, opts)
}

/// Exact byte count with thousands separators, for the properties dialog.
pub fn size_exact(bytes: u64) -> String {
    let digits = bytes.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{out} bytes")
}

/// Local date and time, matching the Explorer "Date modified" column.
pub fn time(t: SystemTime) -> String {
    let dt: DateTime<Local> = t.into();
    dt.format("%Y-%m-%d %H:%M").to_string()
}

pub fn time_long(t: SystemTime) -> String {
    let dt: DateTime<Local> = t.into();
    dt.format("%A, %e %B %Y, %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_bytes_are_grouped() {
        assert_eq!(super::size_exact(1234567), "1,234,567 bytes");
        assert_eq!(super::size_exact(12), "12 bytes");
    }
}
