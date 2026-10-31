//! Small HTTP helpers for the Drop server: URL parsing and canned responses.

use std::{fs::File, io::Cursor};
use tiny_http::{Header, Response};

/// A request target split into its path and decoded query pairs.
pub struct Target {
    pub path: String,
    query: Vec<(String, String)>,
}

impl Target {
    pub fn parse(url: &str) -> Target {
        let (path, query) = url.split_once('?').unwrap_or((url, ""));
        let query = query
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|pair| {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                (decode(k), decode(v))
            })
            .collect();
        Target {
            path: decode(path),
            query,
        }
    }

    pub fn query(&self, key: &str) -> Option<String> {
        self.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }
}

/// Decodes `%XX` escapes (as produced by `encodeURIComponent`). Invalid escapes are
/// kept as they are.
pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Encodes everything except unreserved characters, for `filename*=UTF-8''…`.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Compares two secrets in time that doesn't depend on where they differ.
pub fn same_secret(given: &str, expected: &str) -> bool {
    let (a, b) = (given.as_bytes(), expected.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

/// Headers every response gets: no caching, and the key never leaks in a referrer.
fn private<R: std::io::Read>(response: Response<R>) -> Response<R> {
    response
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("Referrer-Policy", "no-referrer"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
}

pub fn text(status: u16, body: &str) -> Response<Cursor<Vec<u8>>> {
    private(
        Response::from_string(body)
            .with_status_code(status)
            .with_header(header("Content-Type", "text/plain; charset=utf-8")),
    )
}

pub fn html(body: &str) -> Response<Cursor<Vec<u8>>> {
    private(
        Response::from_string(body).with_header(header("Content-Type", "text/html; charset=utf-8")),
    )
}

pub fn json(value: &serde_json::Value) -> Response<Cursor<Vec<u8>>> {
    private(
        Response::from_string(value.to_string())
            .with_header(header("Content-Type", "application/json")),
    )
}

pub fn download(file: File, name: &str) -> Response<File> {
    let ascii: String = name
        .chars()
        .map(|c| {
            if (c.is_ascii_graphic() && c != '"' && c != '\\') || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    private(
        Response::from_file(file)
            .with_header(header("Content-Type", "application/octet-stream"))
            .with_header(header(
                "Content-Disposition",
                &format!(
                    "attachment; filename=\"{ascii}\"; filename*=UTF-8''{}",
                    encode(name)
                ),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_path_and_query() {
        let t = Target::parse("/api/upload?k=abc&name=My%20photo%20%F0%9F%93%B7.jpg");
        assert_eq!(t.path, "/api/upload");
        assert_eq!(t.query("k").as_deref(), Some("abc"));
        assert_eq!(t.query("name").as_deref(), Some("My photo 📷.jpg"));
        assert_eq!(t.query("missing"), None);
    }

    #[test]
    fn decode_keeps_broken_escapes() {
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
        assert_eq!(decode("a%2"), "a%2");
        assert_eq!(decode("a+b"), "a+b");
    }

    #[test]
    fn encode_round_trips() {
        let name = "résumé (final).pdf";
        assert_eq!(decode(&encode(name)), name);
        assert!(!encode(name).contains(' '));
    }

    #[test]
    fn secrets_compare_exactly() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abd", "abc"));
        assert!(!same_secret("ab", "abc"));
        assert!(!same_secret("", "abc"));
    }
}
