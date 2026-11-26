//! Formatage des tailles binaires et des dates locales.

use time::OffsetDateTime;
use time::macros::format_description;

const DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]");

pub fn format_size(size: u64) -> String {
    if size < 1024 {
        return format!("{size} o");
    }
    let units = ["Kio", "Mio", "Gio"];
    let mut value = size as f64 / 1024.0;
    let mut unit = units[0];
    for next in &units[1..] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    if value < 10.0 {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.0} {unit}")
    }
}

pub fn format_mtime(mtime: OffsetDateTime) -> String {
    fs_ez_core::datetime::to_local(mtime)
        .format(DATE_FORMAT)
        .unwrap_or_default()
}
