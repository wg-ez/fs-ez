use super::*;

#[test]
fn scroll_js_renders_one_offset_range_without_sentinel() {
    let js = static_source("static/js/app/scroll.js");
    assert!(js.contains("MARGIN_VIEWPORTS"));
    assert!(js.contains("AbortController"));
    assert!(js.contains("generation"));
    assert!(js.contains("targetRange"));
    assert!(js.contains("applyRange"));
    assert!(js.contains("data-files-spacer"));
    assert!(js.contains("data-folder-spacer"));
    assert!(js.contains("data-folder-total"));
    assert!(js.contains("data-folder-rows-url"));
    assert!(js.contains("folderRowsInfo"));
    assert!(!js.contains("sentinel"));
}

#[test]
fn scroll_js_preserves_anchor_across_files_page_refresh() {
    let js = static_source("static/js/app/scroll.js");
    assert!(js.contains("pendingAnchor"));
    assert!(js.contains("function captureAnchor"));
    assert!(js.contains("function restoreAnchor"));
    assert!(js.contains("localFilesRefresh"));
    assert!(js.contains("function isLocalMutation"));
    assert!(js.contains("shouldSwap = false"));
    assert!(js.contains("htmx:beforeRequest"));
    assert!(js.contains("htmx:beforeSwap"));
    assert!(js.contains("htmx:afterSwap"));
}

#[test]
fn scroll_js_clamps_stale_anchor_after_bulk_delete() {
    let js = static_source("static/js/app/scroll.js");
    assert!(js.contains("clamp(anchor.index"));
    assert!(js.contains("clamp(first"));
}
