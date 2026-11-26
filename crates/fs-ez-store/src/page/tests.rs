use super::*;

#[test]
fn empty_page_ends_list() {
    let page: Page<u8> = Page {
        items: Vec::new(),
        next_offset: None,
    };
    assert!(page.items.is_empty());
    assert_eq!(page.next_offset, None);
}

#[test]
fn next_offset_is_carried_verbatim() {
    let page = Page {
        items: vec![1, 2],
        next_offset: Some(4),
    };
    assert_eq!(page.next_offset, Some(4));
}
