use super::*;

#[test]
fn css_source_defines_the_tag_palette_in_both_themes() {
    let css = css_source();
    for token in TAG_TOKENS {
        assert!(css.contains(&format!("--tag-{token}:")), "{token}");
        assert!(css.contains(&format!(".tag-pill--{token}")), "{token}");
        assert!(css.contains(&format!(".dot--{token}")), "{token}");
    }
    assert_eq!(css.matches("html[data-theme=\"dark\"]").count(), 1);
    assert!(css.contains("@media (prefers-color-scheme: dark)"));
    assert!(css.contains("prefers-reduced-motion"));
    assert!(css.contains(":focus-visible"));
    assert!(css.contains("overflow-x: clip"));
    assert!(css.contains("overflow-anchor: none"));
    assert!(css.contains(".topbar__toggle"));
    assert!(css.contains("[data-file-drop].is-drop"));
    assert!(css.contains("[data-file-drop-zone].is-drop"));
    assert!(css.contains("min-height: calc(var(--h-body)"));
    assert!(css.contains(".table__row--create"));
    assert!(css.contains(".table__folder-icon"));
    assert!(css.contains(".table__name-link--folder:hover"));
    assert!(!css.contains("#files-page > .breadcrumb"));
    assert!(!css.contains(".sidebar__toggle"));
}

#[test]
fn file_row_selection_does_not_animate_after_a_full_page_swap() {
    let css = css_source();
    let start = css
        .find("transition: outline-color .12s ease;")
        .expect("transition de base");
    let block = &css[start.saturating_sub(240)..start + 45];
    assert!(block.contains("[data-file-drop]"));
    assert!(!block.contains("background-color"));
    assert!(css.contains("transition: background-color .12s ease, outline-color .12s ease"));
}

#[test]
fn folder_name_link_is_centered_like_file_names() {
    let css = css_source();
    let start = css
        .find(".table__name-link--folder {")
        .expect("regle dossier");
    let block = &css[start..start + 200];
    assert!(block.contains("display: flex;"));
    assert!(block.contains("width: fit-content;"));
    assert!(!block.contains("display: inline-flex;"));
}
