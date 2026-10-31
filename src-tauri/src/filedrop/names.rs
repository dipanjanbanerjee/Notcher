//! Safe file names for files arriving from another device.

/// Longest name kept, in characters (Windows paths get long fast).
const MAX_CHARS: usize = 150;

/// Turns a name sent by the phone into one that is safe to save on Windows: no
/// folders, no reserved characters or device names, no trailing dots or spaces.
pub fn sanitize(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut name = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if name.is_empty() || name.chars().all(|c| c == '.') {
        name = "file".into();
    }
    let (stem, _) = split_ext(&name);
    let device = stem.trim_end().to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((device.starts_with("COM") || device.starts_with("LPT"))
            && device.len() == 4
            && device.as_bytes()[3].is_ascii_digit());
    if reserved {
        name = format!("_{name}");
    }
    shorten(&name)
}

/// `name` for the first copy, then "name (2).ext", "name (3).ext"…
pub fn numbered(name: &str, n: u32) -> String {
    if n <= 1 {
        return name.to_string();
    }
    let (stem, ext) = split_ext(name);
    format!("{stem} ({n}){ext}")
}

/// Splits "photo.jpg" into ("photo", ".jpg"). Names that start with a dot, like
/// ".env", have no extension.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 12 => name.split_at(i),
        _ => (name, ""),
    }
}

fn shorten(name: &str) -> String {
    if name.chars().count() <= MAX_CHARS {
        return name.to_string();
    }
    let (stem, ext) = split_ext(name);
    let keep = MAX_CHARS.saturating_sub(ext.chars().count());
    let stem: String = stem.chars().take(keep).collect();
    format!("{stem}{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_folders_and_bad_characters() {
        assert_eq!(sanitize("../../Windows/evil.exe"), "evil.exe");
        assert_eq!(sanitize("C:\\Users\\a\\b.txt"), "b.txt");
        assert_eq!(sanitize("what?<is>this*.png"), "what__is_this_.png");
        assert_eq!(sanitize("line\nbreak.txt"), "line_break.txt");
    }

    #[test]
    fn handles_empty_dots_and_trailing_junk() {
        assert_eq!(sanitize(""), "file");
        assert_eq!(sanitize(".."), "file");
        assert_eq!(sanitize("report.pdf. . "), "report.pdf");
        assert_eq!(sanitize(".env"), ".env");
    }

    #[test]
    fn avoids_windows_device_names() {
        assert_eq!(sanitize("CON"), "_CON");
        assert_eq!(sanitize("nul.txt"), "_nul.txt");
        assert_eq!(sanitize("com1.log"), "_com1.log");
        assert_eq!(sanitize("console.log"), "console.log");
        assert_eq!(sanitize("COMX.log"), "COMX.log");
    }

    #[test]
    fn keeps_unicode_and_shortens_long_names() {
        assert_eq!(sanitize("ফটো 📷.jpg"), "ফটো 📷.jpg");
        let long = format!("{}.jpg", "a".repeat(400));
        let short = sanitize(&long);
        assert_eq!(short.chars().count(), MAX_CHARS);
        assert!(short.ends_with(".jpg"));
    }

    #[test]
    fn numbers_copies_before_the_extension() {
        assert_eq!(numbered("photo.jpg", 1), "photo.jpg");
        assert_eq!(numbered("photo.jpg", 2), "photo (2).jpg");
        assert_eq!(numbered("README", 3), "README (3)");
        assert_eq!(numbered(".env", 2), ".env (2)");
    }
}
