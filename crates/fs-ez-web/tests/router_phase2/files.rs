use super::*;

#[tokio::test]
async fn list_supports_sort_search_and_untagged() {
    let fixture = web_fixture().await;
    fixture.add_file("alpha.txt", b"a", &["facture"]);
    fixture.add_file("beta.txt", b"bb", &["archive"]);
    fixture.add_file("Rapport-annuel.txt", b"ccc", &[]);

    let (status, html) = fixture.page("/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 3);

    let (status, html) = fixture.page("/?sort=name:desc").await;
    assert_eq!(status, StatusCode::OK);
    let rapport = html.find("Rapport-annuel.txt").expect("rapport");
    let beta = html.find("beta.txt").expect("beta");
    let alpha = html.find("alpha.txt").expect("alpha");
    assert!(beta < alpha, "{beta} < {alpha}");
    assert!(alpha < rapport, "{alpha} < {rapport}");

    let (status, html) = fixture.page("/?q=rapport").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 1);
    assert!(html.contains("Rapport-annuel.txt"));

    let (status, html) = fixture.page("/?q=alpha&tag=facture").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 1);
    assert!(html.contains("alpha.txt"));
    assert!(html.contains("name=\"tag\" value=\"facture\""));

    let (status, html) = fixture.page("/?q=alpha&tag=archive").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 0);

    let (status, html) = fixture.page("/?tag=facture&tag=archive").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("searchbar__badge\" aria-hidden=\"true\">2</span>"));
    assert_eq!(html.matches("name=\"q\"").count(), 1);

    let (status, html) = fixture.page("/?filter=untagged").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 1);
    assert!(html.contains("Rapport-annuel.txt"));
}

#[tokio::test]
async fn search_submit_precedes_tag_filters_to_keep_selected_tags() {
    let fixture = web_fixture().await;
    fixture.add_file("alpha.txt", b"a", &["facture"]);

    let (status, html) = fixture.page("/?q=alpha&tag=facture").await;
    assert_eq!(status, StatusCode::OK);
    let input = html.find("id=\"global-search\"").expect("champ q");
    let submit = html
        .find("class=\"sr-only\" type=\"submit\"")
        .expect("bouton de soumission");
    let filters = html
        .find("class=\"searchbar__filters")
        .expect("filtres de tags");
    assert!(input < submit && submit < filters);
    assert!(html.contains("name=\"tag\" value=\"facture\" checked"));
}

#[tokio::test]
async fn excluded_tags_blacklist_matching_files() {
    let fixture = web_fixture().await;
    fixture.add_file("alpha.txt", b"a", &["facture"]);
    fixture.add_file("beta.txt", b"bb", &["archive"]);
    fixture.add_file("gamma.txt", b"ccc", &["facture", "archive"]);
    fixture.add_file("delta.txt", b"dddd", &[]);

    let (status, html) = fixture.page("/?not_tag=facture").await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert_eq!(row_count(&html), 2);
    assert!(html.contains("beta.txt") && html.contains("delta.txt"));
    assert!(!html.contains("alpha.txt") && !html.contains("gamma.txt"));
    assert!(html.contains("is-excluded"), "{html}");

    let (status, html) = fixture.page("/?tag=facture&not_tag=archive").await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert_eq!(row_count(&html), 1);
    assert!(html.contains("alpha.txt"));

    let (status, html) = fixture.page("/?not_tag=facture&not_tag=archive").await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert_eq!(row_count(&html), 1);
    assert!(html.contains("delta.txt"));

    let (status, _html) = fixture.page("/?filter=untagged&not_tag=facture").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_filter_panel_marks_excluded_tags() {
    let fixture = web_fixture().await;
    fixture.add_file("alpha.txt", b"a", &["facture"]);
    fixture.add_file("beta.txt", b"bb", &["archive"]);

    let (status, html) = fixture
        .page("/partials/search_filter?tag=facture&not_tag=archive")
        .await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(
        html.contains("name=\"not_tag\" value=\"archive\" data-not-tag>"),
        "{html}"
    );
    assert!(html.contains("value=\"archive\" data-excluded"), "{html}");
    assert!(
        html.contains("dropdown__item dropdown__item--excluded"),
        "{html}"
    );
}

#[tokio::test]
async fn filters_combine_multiple_tags_and_text() {
    let fixture = web_fixture().await;
    fixture.add_file("alpha.txt", b"a", &["facture", "2025"]);
    fixture.add_file("beta.txt", b"bb", &["facture"]);
    fixture.add_file("gamma.txt", b"ccc", &["2025"]);

    let (status, html) = fixture.page("/?tag=facture&tag=2025").await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert_eq!(row_count(&html), 1);
    assert!(html.contains("alpha.txt"));

    let (status, html) = fixture.page("/?q=alpha&tag=facture&tag=2025").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 1);

    let (status, html) = fixture.page("/?q=beta&tag=2025").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 0);

    let (status, html) = fixture
        .page("/partials/search_filter?tag=facture&tag=2025")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("value=\"facture\" checked"));
    assert!(html.contains("value=\"2025\" checked"));
}
