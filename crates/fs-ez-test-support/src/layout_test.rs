#![forbid(unsafe_code)]

//! Contrôle automatique : les tests vivent dans des fichiers de test, jamais dans le code.
//!
//! Règle `AGENTS.md` : un source ne contient ni `mod tests { … }` ni `#[test]`.
//! Ce test rend la règle exécutable, un test dans le code fait échouer la suite.

use std::path::Path;

#[test]
fn les_tests_vivent_dans_des_fichiers_de_test() {
    crate::layout::assert_tests_outside_sources(Path::new(env!("CARGO_MANIFEST_DIR")));
}
