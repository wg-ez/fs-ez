use super::*;

mod css;
mod js_app;
mod js_file_menu;
mod js_scroll;
mod js_search;
mod js_work;
mod markup;

/* Facade `input.css` + tous les partiels `static/css/src/`, ordre deterministe. */
fn css_source() -> String {
    fn collect(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("repertoire css") {
            let path = entry.expect("entree css").path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "css") {
                files.push(path);
            }
        }
    }

    let mut source = static_source("static/css/input.css");
    let mut files = Vec::new();
    collect(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("static/css/src"),
        &mut files,
    );
    files.sort();
    for path in files {
        source.push('\n');
        source.push_str(&std::fs::read_to_string(path).expect("fichier css"));
    }
    source
}

fn static_source(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path).expect("fichier statique")
}

/* Tranche `source` du marqueur `start` inclus au marqueur `end` exclu, panique si un marqueur manque. */
fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let begin = source
        .find(start)
        .unwrap_or_else(|| panic!("marqueur {start} absent"));
    let tail = &source[begin..];
    let stop = tail
        .find(end)
        .unwrap_or_else(|| panic!("marqueur {end} absent"));
    &tail[..stop]
}

/* Entrée + modules `static/js/app/` dans l'ordre des balises script de `head.html`. */
fn app_js_source() -> String {
    let mut source = static_source("static/js/app.js");
    for relative in [
        "static/js/app/sidebar.js",
        "static/js/app/dropdowns.js",
        "static/js/app/columns.js",
        "static/js/app/file-menu.js",
        "static/js/app/dragdrop.js",
        "static/js/app/folders.js",
        "static/js/app/scroll.js",
        "static/js/app/events.js",
    ] {
        source.push('\n');
        source.push_str(&static_source(relative));
    }
    source
}

const TAG_TOKENS: [&str; 12] = [
    "neutral", "red", "orange", "amber", "green", "teal", "cyan", "blue", "indigo", "violet",
    "pink", "brown",
];
