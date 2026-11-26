//! Contrôle de disposition : les tests vivent dans des fichiers de test, jamais dans le code.
//!
//! Règle projet (`AGENTS.md`) : un module source ne contient ni bloc `mod tests { … }`
//! ni fonction `#[test]`. Les tests vont dans `src/<module>/tests.rs` (fichier frère)
//! ou dans `src/<module>/tests/` (plusieurs thèmes), le source ne déclarant que
//! `#[cfg(test)] mod tests;`.

use std::path::{Path, PathBuf};

/// Vérifie tous les fichiers Rust de `<crate_root>/src`, panique en listant les violations.
pub fn assert_tests_outside_sources(crate_root: impl AsRef<Path>) {
    let crate_root = crate_root.as_ref();
    let mut problems = check(crate_root);
    problems.sort();
    assert!(
        problems.is_empty(),
        "les tests doivent vivre dans un fichier de test, pas dans le code source :\n{}",
        problems.join("\n")
    );
}

/// Liste les violations `chemin:ligne : motif`, triée par appelant si besoin.
pub fn check(crate_root: &Path) -> Vec<String> {
    let src = crate_root.join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    files.sort();

    let mut problems = Vec::new();
    for file in &files {
        if is_test_file(file) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        let relative = file.strip_prefix(crate_root).unwrap_or(file).display();
        if let Some(line) = declares_inline_test_module(&lines) {
            problems.push(format!(
                "{relative}:{} : module de test en ligne, à déplacer dans `tests.rs`",
                line + 1
            ));
        }
        for line in test_function_lines(&lines) {
            problems.push(format!(
                "{relative}:{} : fonction de test dans le code source, à déplacer dans `tests.rs`",
                line + 1
            ));
        }
    }
    problems
}

/// Liste récursive des fichiers `.rs` sous `dir`.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Fichier autorisé à contenir des tests : `tests.rs`, `*_test.rs`, `test_support.rs`, ou sous `tests/`.
fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if name.ends_with("tests.rs") || name.ends_with("_test.rs") || name == "test_support.rs" {
        return true;
    }
    path.ancestors()
        .filter_map(|dir| dir.file_name().and_then(|n| n.to_str()))
        .any(|dir| dir == "tests")
}

/// Détecte `#[cfg(test)]` suivi d'une **déclaration de module avec corps** (test en ligne).
fn declares_inline_test_module(lines: &[&str]) -> Option<usize> {
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(test)]")
            && trimmed.contains(" mod ")
            && trimmed.ends_with('{')
        {
            return Some(index);
        }
        if trimmed != "#[cfg(test)]" {
            continue;
        }
        for next in lines.iter().skip(index + 1) {
            let candidate = next.trim();
            if candidate.is_empty() || candidate.starts_with("#[") || candidate.starts_with("//") {
                continue;
            }
            let is_module = candidate.starts_with("mod ")
                || candidate.starts_with("pub mod ")
                || candidate.starts_with("pub(crate) mod ")
                || candidate.starts_with("pub(super) mod ");
            if is_module && candidate.ends_with('{') {
                return Some(index);
            }
            break;
        }
    }
    None
}

/// Lignes des fonctions de test (`#[test]`, `#[tokio::test]`).
fn test_function_lines(lines: &[&str]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| matches!(line.trim(), "#[test]" | "#[tokio::test]"))
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests;
