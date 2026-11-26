use super::*;

#[test]
fn search_js_owns_url_state_badge_and_panel_refresh() {
    let js = static_source("static/js/search.js");
    assert!(js.contains("URLSearchParams"));
    assert!(js.contains("activeTagNames"));
    assert!(js.contains("syncFromLocation"));
    assert!(js.contains("syncQuery"));
    assert!(js.contains("updateBadge"));
    assert!(js.contains("data-search-filter-url"));
    assert!(js.contains("data-search-preserve"));
    assert!(js.contains(".searchbar__filters"));
    assert!(js.contains(".searchbar__panel"));
    assert!(js.contains("htmx:afterSwap"));
    assert!(js.contains("htmx:historyRestore"));
    assert!(js.contains("popstate"));
    assert!(!js.contains("data-search-dirty"));
    assert!(!js.contains("eval("));
    assert!(!js.contains(".innerHTML"));
}

#[test]
fn search_js_keeps_tag_menu_open_while_selecting_tags() {
    let js = static_source("static/js/search.js");
    let change = js.find("\"change\"").expect("gestionnaire change");
    let click = js.find("\"click\"").expect("gestionnaire click");
    assert!(change < click);

    let change_handler = &js[change..click];
    assert!(!change_handler.contains("details.open = false"));
    assert!(change_handler.contains("submit();"));

    assert_eq!(js.matches("details.open = false").count(), 1);
    assert!(js[click..].contains("details.open = false"));
}

#[test]
fn search_js_closes_the_menu_on_outside_click() {
    let js = static_source("static/js/search.js");
    let click_handler = between(&js, "\"click\"", "\"toggle\"");
    assert!(click_handler.contains("!target.closest(\".searchbar__filters\")"));
    assert!(click_handler.contains("close();"));

    let close_body = between(&js, "function close()", "function refresh()");
    assert!(close_body.contains(".open = false"));
    assert!(!close_body.contains("!== details"));
}
