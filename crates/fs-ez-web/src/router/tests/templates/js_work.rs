use super::*;

#[test]
fn work_js_owns_the_partial_navigation_contract() {
    let js = static_source("static/js/work.js");
    assert!(js.contains("navigateWork"));
    assert!(js.contains("\"#work\""));
    assert!(js.contains("outerHTML"));
    assert!(js.contains("push: \"true\""));
    assert!(js.contains("selectOOB: \"#sidebar:outerHTML\""));
    assert!(!js.contains("syncSidebarActive"));
    assert!(js.contains("window.location.assign"));
    assert!(js.contains("#sidebar a.sidebar__link[href]"));
    assert!(js.contains("tagSelectionUrl"));
    assert!(js.contains("lastTagLink"));
    assert!(js.contains("window.fsEz.normalizeTag"));
    assert!(js.contains("window.fsEz.activeTagNames"));
    assert!(js.contains("shiftKey"));
    assert!(js.contains("ctrlKey"));
    assert!(js.contains("altKey"));
    assert!(js.contains("tagNames(\"not_tag\")"));
    assert!(!js.contains("is-excluded"));
    assert!(!js.contains("eval("));
    assert!(!js.contains(".innerHTML"));

    let head = static_source("templates/partials/head.html");
    assert!(head.contains("/static/js/work.js"));
    assert!(head.find("/static/js/work.js") < head.find("/static/js/search.js"));
    let search = static_source("static/js/search.js");
    assert!(search.contains("navigateWork"));
    assert!(!search.contains("readTags"));
    assert!(!search.contains("getAll(\"tag\")"));
}
