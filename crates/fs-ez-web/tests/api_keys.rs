//! Gestion des clés d'API depuis le menu d'apparence.

mod common;

use common::*;
use fs_ez_core::datetime::parse_rfc3339;
use time::macros::{date, format_description};
use time::{Duration, OffsetDateTime};

fn token_from_page(page: &str) -> String {
    let marker = "settings__secret-value\">";
    let start = page.find(marker).expect("jeton affiché") + marker.len();
    let rest = &page[start..];
    let end = rest.find('<').expect("fin du jeton");
    rest[..end].to_string()
}

#[tokio::test]
async fn menu_links_to_api_keys_page() {
    let fixture = Fixture::new(FixtureOptions::default()).await;
    let (status, page) = fixture.page("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("href=\"/settings\""), "{page}");
    assert!(page.contains("aria-label=\"Settings\""), "{page}");
}

#[tokio::test]
async fn page_lists_and_creates_keys_with_a_single_plaintext_display() {
    let fixture = Fixture::new(FixtureOptions::default()).await;
    let (status, page) = fixture.page("/settings").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("action=\"/settings/tokens\""), "{page}");

    let response = fixture.post("/settings/tokens", "name=ci+deploy").await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    let token = token_from_page(&page);
    assert_eq!(token.len(), 43, "{token}");
    assert!(page.contains("ci deploy"), "{page}");
    assert!(!page.contains("token_hash"), "{page}");

    let stored = fixture.app.list_api_tokens().expect("jetons");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].name, "ci deploy");

    let response = fixture.post("/settings/tokens", "name=ci+deploy").await;
    let page = body_text(response).await;
    assert!(
        page.contains("A key with this name already exists."),
        "{page}"
    );
    assert_eq!(fixture.app.list_api_tokens().expect("jetons").len(), 1);
}

#[tokio::test]
async fn invalid_name_and_expiration_are_refused() {
    let fixture = Fixture::new(FixtureOptions::default()).await;
    let response = fixture.post("/settings/tokens", "name=+++").await;
    let page = body_text(response).await;
    assert!(page.contains("Enter a valid key name"), "{page}");
    assert!(fixture.app.list_api_tokens().expect("jetons").is_empty());

    let response = fixture
        .post("/settings/tokens", "name=ci&expires_at=demain")
        .await;
    let page = body_text(response).await;
    assert!(page.contains("dd-mm-yyyy"), "{page}");
    assert!(fixture.app.list_api_tokens().expect("jetons").is_empty());

    let format = format_description!("[day]-[month]-[year]");
    let past = (OffsetDateTime::now_utc() - Duration::days(1))
        .format(&format)
        .expect("date");
    let body = format!("name=ci&expires_at={past}");
    let response = fixture.post("/settings/tokens", &body).await;
    let page = body_text(response).await;
    assert!(page.contains("dd-mm-yyyy"), "{page}");
    assert!(fixture.app.list_api_tokens().expect("jetons").is_empty());
}

#[tokio::test]
async fn expiration_is_stored_and_displayed() {
    let fixture = Fixture::new(FixtureOptions::default()).await;
    let expires = date!(2030 - 01 - 01).midnight().assume_utc();
    let response = fixture
        .post("/settings/tokens", "name=ci&expires_at=01-01-2030")
        .await;
    let page = body_text(response).await;
    assert!(page.contains("ci"), "{page}");
    let stored = fixture.app.list_api_tokens().expect("jetons");
    assert_eq!(stored[0].expires_at, Some(expires));
}

#[tokio::test]
async fn revoke_removes_the_key() {
    let fixture = Fixture::new(FixtureOptions::default()).await;
    let created = fixture
        .app
        .create_api_token(
            "ci",
            Some(parse_rfc3339("2030-01-01T00:00:00Z").expect("date")),
        )
        .expect("jeton");
    let response = fixture
        .post(
            &format!("/settings/tokens/{}/delete", created.record.id),
            "",
        )
        .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/settings");
    assert!(fixture.app.list_api_tokens().expect("jetons").is_empty());
}

#[tokio::test]
async fn htmx_mutations_return_the_manageable_fragment() {
    let fixture = Fixture::new(FixtureOptions::default()).await;
    let response = fixture.post_hx("/settings/tokens", "name=ci").await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("id=\"settings-tokens\""), "{page}");
    assert!(!page.contains("<!doctype html>"), "{page}");
    assert!(page.contains("New API key created"), "{page}");
    assert!(
        page.contains("data-clipboard-target=\".settings__secret-value\""),
        "{page}"
    );
    assert!(page.contains(">Copy</button>"), "{page}");

    let stored = fixture.app.list_api_tokens().expect("jetons");
    let response = fixture
        .post_hx(&format!("/settings/tokens/{}/delete", stored[0].id), "")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(fixture.app.list_api_tokens().expect("jetons").is_empty());
}

#[tokio::test]
async fn bearer_tokens_cannot_manage_api_keys() {
    let fixture = Fixture::new(FixtureOptions::default()).await;
    let created = fixture.app.create_api_token("ci", None).expect("jeton");
    let auth = format!("Bearer {}", created.token);
    let mut request = get("/settings");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&auth).expect("en-tête"),
    );
    let response = fixture.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let mut request = post_form("/settings/tokens", "name=autre");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&auth).expect("en-tête"),
    );
    let response = fixture.send(request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(fixture.app.list_api_tokens().expect("jetons").len(), 1);
}
