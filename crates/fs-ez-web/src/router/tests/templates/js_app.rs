use super::*;

#[test]
fn app_js_is_progressive_and_script_is_external_only() {
    let js = app_js_source();
    assert!(js.contains("document.documentElement.classList.remove(\"no-js\")"));
    assert!(js.contains("data-confirm"));
    assert!(!js.contains("data-copy"));
    assert!(js.contains("is-rail-open"));
    assert!(js.contains("\"shell--sidebar-hidden\""));
    assert!(js.contains("/partials/sidebar/toggle"));
    assert!(js.contains("(min-width: 48rem)"));
    assert!(js.contains("data-label-expand"));
    assert!(js.contains("data-label-collapse"));
    assert!(js.contains("[data-file-drop]"));
    assert!(js.contains("[data-file-drop-zone]"));
    assert!(js.contains("/partials/files/tags"));
    assert!(js.contains(".ajax("));
    assert!(js.contains("\"data-changed\""));
    assert!(js.contains("fs-ez:column-widths:v1:"));
    assert!(js.contains("data-col-resizer"));
    assert!(js.contains("\"table--columns\""));
    assert!(!js.contains("delayTagNavigation"));
    assert!(!js.contains("eval("));
    assert!(!js.contains(".innerHTML"));
}

#[test]
fn file_selection_keeps_ctrl_a_global_across_ctrl_click() {
    let js = app_js_source();
    assert!(js.contains("excludedIds"));
    assert!(js.contains("excludedMap"));
    assert!(js.contains("exclude_ids"));
    assert!(js.contains("function addSelectionScope"));
    assert!(!js.contains("app.dropSelectAll"));
}

#[test]
fn file_delete_starts_on_first_click_without_browser_confirm() {
    let js = app_js_source();
    assert!(!js.contains("FILE_DELETE_ARM_MS"));
    assert!(!js.contains("fileDeleteArmedAt"));
    assert!(!js.contains("data-delete-confirm"));

    let delete_files = between(
        &js,
        "function deleteFiles",
        "document.addEventListener(\"contextmenu\"",
    );
    assert!(!delete_files.contains("window.confirm"));
}

#[test]
fn toasts_dismiss_after_five_seconds() {
    let events = static_source("static/js/app/events.js");
    assert!(events.contains("var TOAST_MS = 5000"));
    assert!(events.contains("dismissToasts(document)"));
    assert!(events.contains("htmx:afterSwap"));
    let dropdowns = static_source("static/js/app/dropdowns.js");
    assert!(dropdowns.contains("}, 5000);"));
}
