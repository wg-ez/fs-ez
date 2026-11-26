use super::*;

fn assert_object_safe(_: &dyn Store) {}

#[test]
fn store_trait_is_object_safe() {
    let _ = assert_object_safe;
}
