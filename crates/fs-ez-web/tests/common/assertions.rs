use super::*;

pub fn assert_status(response: &Response<Body>, expected: StatusCode) {
    assert_eq!(response.status(), expected);
}

pub fn row_count(html: &str) -> usize {
    html.matches("class=\"table__row\"").count()
}

pub fn selected_ids(html: &str) -> Vec<String> {
    let marker = "data-file-drop data-file-id=\"";
    let mut ids = Vec::new();
    let mut rest = html;
    while let Some(position) = rest.find(marker) {
        let after = &rest[position + marker.len()..];
        let Some(end) = after.find('"') else {
            break;
        };
        ids.push(after[..end].to_string());
        rest = &after[end..];
    }
    ids
}

pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            let plain = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
            if plain {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

pub fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<String>>()
        .join("&")
}

pub fn assert_no_inline_code(html: &str, label: &str) {
    assert!(!html.contains("style="), "{label}: style inline");
    assert!(!html.contains("onclick="), "{label}: onclick inline");
    let mut rest = html;
    while let Some(position) = rest.find("<script") {
        let after = &rest[position + 7..];
        assert!(after.starts_with(" src=\"/static/"), "{label}: {after}");
        rest = after;
    }
}
