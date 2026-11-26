use super::*;

fn assert_dyn_store(_: &dyn Store) {}
fn assert_arc_store(_: std::sync::Arc<dyn Store>) {}

#[test]
fn dyn_store_compiles() {
    let _ = assert_dyn_store;
    let _ = assert_arc_store;
}
