use super::*;

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

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut source =
        std::fs::read_to_string(root.join("static/css/input.css")).expect("css source");
    let mut files = Vec::new();
    collect(&root.join("static/css/src"), &mut files);
    files.sort();
    for path in files {
        source.push('\n');
        source.push_str(&std::fs::read_to_string(path).expect("fichier css"));
    }
    source
}

#[tokio::test]
async fn locales_are_at_parity_and_french_pages_have_no_raw_key() {
    let locales = Locales::load().expect("locales");
    assert!(
        locales.missing_keys().is_empty(),
        "{:?}",
        locales.missing_keys()
    );
    assert_eq!(locales.keys("en"), locales.keys("fr"));

    let fixture = seeded().await;
    let (status, page) = fixture.page("/?lang=fr").await;
    assert_eq!(status, StatusCode::OK, "/?lang=fr");
    assert!(page.contains("lang=\"fr\""), "/?lang=fr");
    assert!(
        !page.contains("nav."),
        "/?lang=fr contient une cle nav brute"
    );
}

#[tokio::test]
async fn each_theme_renders_its_data_attribute_and_all_twelve_palette_classes_exist() {
    let fixture = seeded().await;
    for theme in ["light", "dark", "system"] {
        let (status, page) = fixture.page(&format!("/?theme={theme}")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            page.contains(&format!("data-theme=\"{theme}\"")),
            "{theme}: {page}"
        );
    }

    let css = css_source();
    assert_eq!(TagColor::ALL.len(), 12);
    for color in TagColor::ALL {
        let token = color.as_token();
        assert!(css.contains(&format!(".tag-pill--{token}")), "{token}");
        assert!(css.contains(&format!(".dot--{token}")), "{token}");
        assert!(css.contains(&format!("--tag-{token}")), "{token}");
    }
}
