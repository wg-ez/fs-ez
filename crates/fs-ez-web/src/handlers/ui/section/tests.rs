use axum::http::{HeaderMap, HeaderValue};

use super::*;

fn headers(url: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("hx-current-url", HeaderValue::from_str(url).expect("url"));
    headers
}

#[test]
fn virtual_folder_page_stays_active_on_fragments() {
    assert_eq!(
        from_headers(&headers("http://localhost/virtual-folders")),
        "virtual-folders"
    );
    assert_eq!(
        from_headers(&headers(
            "http://localhost/virtual-folders/42?sort=name:desc"
        )),
        "virtual-folders"
    );
}

#[test]
fn list_pages_map_to_their_nav_key() {
    assert_eq!(from_headers(&headers("http://localhost/")), "files");
    assert_eq!(
        from_headers(&headers("http://localhost/?filter=recent-files")),
        "recent-files"
    );
    assert_eq!(from_headers(&headers("http://localhost/?tag=blue")), "tags");
    assert_eq!(from_headers(&headers("http://localhost/views/7")), "view:7");
    assert_eq!(
        from_headers(&headers("http://localhost/settings")),
        "settings"
    );
}

#[test]
fn missing_or_foreign_url_has_no_section() {
    assert_eq!(from_headers(&HeaderMap::new()), "");
    assert_eq!(
        from_headers(&headers("http://localhost/partials/sidebar")),
        ""
    );
}
