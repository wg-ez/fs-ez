use super::*;

#[test]
fn rejects_traversal_and_absolute_paths() {
    assert!(normalize("../Cargo.toml").is_none());
    assert!(normalize("css/../../Cargo.toml").is_none());
    assert!(normalize("/etc/passwd").is_none());
    assert!(normalize("css/\0app.css").is_none());
    assert_eq!(normalize("css//app.css").as_deref(), Some("css/app.css"));
}

#[test]
fn content_types_never_fall_back_to_html() {
    assert_eq!(content_type("css/app.css"), "text/css; charset=utf-8");
    assert_eq!(content_type("js/app.js"), "text/javascript; charset=utf-8");
    assert_eq!(content_type("img/favicon.svg"), "image/svg+xml");
    assert_eq!(content_type("inconnu.bin"), "application/octet-stream");
}

#[test]
fn embedded_htmx_is_gzipped() {
    let asset = embedded("vendor/htmx.min.js").expect("htmx pré-gzippé");
    assert!(!asset.data.is_empty());
    assert_eq!(cache_control(asset.path), IMMUTABLE_CACHE);
}
