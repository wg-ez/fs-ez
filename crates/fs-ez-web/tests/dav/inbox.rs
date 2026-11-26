use std::ffi::OsStr;
use std::os::unix::fs::symlink;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fs_ez_app::sharded_path;

use super::support::*;
use crate::common::*;

#[tokio::test]
async fn put_inbox_imports_into_library() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let content = b"%PDF-1.7\ninbox\n%%EOF";

    let response = send(
        &router,
        dav_request(
            Method::PUT,
            "/dav/inbox/rapport.pdf",
            &token,
            Body::from(&content[..]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let stored = sharded_path(&fixture.files(), OsStr::new("rapport.pdf"));
    assert_eq!(std::fs::read(&stored).expect("fichier"), content);
    assert!(!fixture.inbox().join("rapport.pdf").exists());

    let dated = inbox_date(&fixture.app);
    assert_eq!(
        files_of(&fixture.app, dated.id),
        vec![fixture.file_id("rapport.pdf")]
    );

    let response = dav_get(&router, "/dav/inbox/rapport.pdf", &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, content);
}

#[tokio::test]
async fn put_inbox_preserves_x_oc_mtime() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let request = with_header(
        dav_request(
            Method::PUT,
            "/dav/inbox/date.txt",
            &token,
            Body::from("date"),
        ),
        "x-oc-mtime",
        "1700000000",
    );

    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let stored = sharded_path(&fixture.files(), OsStr::new("date.txt"));
    assert_mtime(&stored, 1_700_000_000);
}

#[tokio::test]
async fn propfind_after_put_resolves_the_imported_library_file() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let content = b"%PDF-1.7\ncopy\n%%EOF";

    let response = send(
        &router,
        dav_request(
            Method::PUT,
            "/dav/inbox/copie.pdf",
            &token,
            Body::from(&content[..]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = send(&router, propfind("/dav/inbox/copie.pdf", &token, "0")).await;
    let body = assert_multi_status(response).await;
    assert_href(&body, "/dav/inbox/copie.pdf");

    let response = dav_get(&router, "/dav/inbox/copie.pdf", &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, content);
}

#[tokio::test]
async fn propfind_after_duplicate_put_resolves_the_existing_file() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let content = b"%PDF-1.7\nduplicate\n%%EOF";

    for name in ["premier.pdf", "second.pdf"] {
        let uri = format!("/dav/inbox/{name}");
        let response = send(
            &router,
            dav_request(Method::PUT, &uri, &token, Body::from(&content[..])),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = send(&router, propfind(&uri, &token, "0")).await;
        assert_eq!(response.status(), StatusCode::MULTI_STATUS, "{name}");
    }
}

#[tokio::test]
async fn propfind_lists_and_get_reads_an_unrescanned_drop() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    std::fs::write(fixture.inbox().join("drop.txt"), b"drop").expect("dépôt");

    let response = send(&router, propfind("/dav/inbox/", &token, "1")).await;
    let body = assert_multi_status(response).await;
    assert_href(&body, "/dav/inbox/");
    assert_href(&body, "/dav/inbox/drop.txt");

    let response = dav_get(&router, "/dav/inbox/drop.txt", &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, b"drop");
}

#[tokio::test]
async fn put_on_existing_drop_is_conflict_and_never_overwrites() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let path = fixture.inbox().join("keep.txt");
    std::fs::write(&path, b"user").expect("dépôt");

    let response = send(
        &router,
        dav_request(
            Method::PUT,
            "/dav/inbox/keep.txt",
            &token,
            Body::from("dav"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(std::fs::read(&path).expect("contenu"), b"user");
}

#[tokio::test]
async fn symlink_swap_never_serves_outside_content() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let drop = fixture.inbox().join("swap.txt");
    let link = fixture.inbox().join("swap.lnk");
    let parked = fixture.inbox().join("swap.parked");
    let outside_dir = tempfile::tempdir().expect("hors racine");
    let outside = outside_dir.path().join("outside-sentinel.txt");
    std::fs::write(&drop, b"inside").expect("dépôt");
    std::fs::write(&outside, b"outside-sentinel").expect("sentinelle");
    symlink(&outside, &link).expect("symlink");

    let stop = Arc::new(AtomicBool::new(false));
    // Lecture initiale hors course : l'état régulier est bien servi.
    let response = dav_get(&router, "/dav/inbox/swap.txt", &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, b"inside");

    let worker = {
        let stop = Arc::clone(&stop);
        let drop = drop.clone();
        let link = link.clone();
        let parked = parked.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if std::fs::rename(&drop, &parked).is_ok() {
                    let _ = std::fs::rename(&link, &drop);
                    let _ = std::fs::rename(&drop, &link);
                    let _ = std::fs::rename(&parked, &drop);
                }
            }
        })
    };

    for _ in 0..500 {
        let response = dav_get(&router, "/dav/inbox/swap.txt", &token).await;
        match response.status() {
            StatusCode::OK => {
                let body = body_bytes(response).await;
                assert_eq!(body, b"inside", "contenu hors inbox servi");
            }
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {}
            status => panic!("statut inattendu {status}"),
        }
    }
    stop.store(true, Ordering::Relaxed);
    worker.join().expect("thread d'échange");
}

#[tokio::test]
async fn destructive_methods_on_inbox_are_refused() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let drop = fixture.inbox().join("drop.txt");
    std::fs::write(&drop, b"drop").expect("dépôt");

    for uri in ["/dav/inbox/", "/dav/inbox/drop.txt"] {
        let response = send(
            &router,
            dav_request(Method::DELETE, uri, &token, Body::empty()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
    }
    let response = send(
        &router,
        dav_request(
            Method::DELETE,
            "/dav/inbox/absent.txt",
            &token,
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = send(
        &router,
        move_request(
            "/dav/inbox/drop.txt",
            "/dav/inbox/renamed.txt",
            &token,
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(drop.is_file());

    let response = send(
        &router,
        dav_request(
            Method::from_bytes(b"COPY").expect("méthode"),
            "/dav/inbox/drop.txt",
            &token,
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn nested_inbox_copy_imports_every_file() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    for directory in ["/dav/inbox/album", "/dav/inbox/album/2024"] {
        assert_mkcol_created(&router, directory, &token).await;
    }

    for (uri, name, content) in [
        (
            "/dav/inbox/album/premier.txt",
            "premier.txt",
            b"premier" as &[u8],
        ),
        ("/dav/inbox/album/2024/second.txt", "second.txt", b"second"),
    ] {
        let response = send(
            &router,
            dav_request(Method::PUT, uri, &token, Body::from(content)),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "{uri}");
        let stored = sharded_path(&fixture.files(), OsStr::new(name));
        assert_eq!(std::fs::read(&stored).expect("fichier"), content);
        assert!(!fixture.inbox().join(name).exists());
    }

    let response = send(&router, propfind("/dav/inbox/album/", &token, "0")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);

    let first = "/dav/inbox/album/premier.txt";
    let response = send(&router, propfind(first, &token, "0")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = propfind_body(&router, "/dav/inbox/album/", &token, "1").await;
    assert!(
        !body.contains("premier.txt"),
        "fichier importé absent du listing : {body}"
    );
    let body = propfind_body(&router, "/dav/inbox/", &token, "1").await;
    assert!(
        !body.contains("album"),
        "dossier importé absent du listing racine : {body}"
    );
}

#[tokio::test]
async fn propfind_inbox_listing_hides_imported_put_but_still_resolves_it() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let name = "visible-puis-importe.txt";

    let response = send(
        &router,
        dav_request(
            Method::PUT,
            &format!("/dav/inbox/{name}"),
            &token,
            Body::from("contenu"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(!fixture.inbox().join(name).exists(), "inbox physique vide");

    let response = send(&router, propfind("/dav/inbox/", &token, "1")).await;
    let body = body_text(response).await;
    assert!(!body.contains(name), "alias importé encore listé : {body}");

    // gvfs vérifie l'alias juste après le PUT, la résolution directe reste disponible.
    let response = send(
        &router,
        propfind(&format!("/dav/inbox/{name}"), &token, "0"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
}
