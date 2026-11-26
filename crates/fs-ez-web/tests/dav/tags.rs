//! Tests WebDAV des collections de tags, fichiers et `All Empty`.

use fs_ez_core::TagColor;

use super::support::*;
use crate::common::*;

#[path = "tags/empty.rs"]
mod empty;
#[path = "tags/listing.rs"]
mod listing;
#[path = "tags/upload.rs"]
mod upload;

async fn setup_nested() -> Dav {
    let dav = setup_named("three.opus", b"three").await;
    let fixture = &dav.fixture;
    fixture.create_tag("Julien", TagColor::Green);
    fixture.create_tag("Opus", TagColor::Amber);

    let three = fixture.root().join("three.opus");
    fixture
        .app
        .tag_path(
            &three,
            &["work".to_string(), "Julien".to_string(), "Opus".to_string()],
            false,
        )
        .expect("tags three");

    let two = fixture.root().join("two.opus");
    std::fs::write(&two, b"two").expect("écriture");
    fixture
        .app
        .tag_path(&two, &["work".to_string(), "Julien".to_string()], false)
        .expect("tags two");

    let other = fixture.root().join("other.opus");
    std::fs::write(&other, b"other").expect("écriture");
    fixture
        .app
        .tag_path(&other, &["Opus".to_string()], false)
        .expect("tags other");

    let alone = fixture.root().join("alone.opus");
    std::fs::write(&alone, b"alone").expect("écriture");
    fixture
        .app
        .tag_path(&alone, &["Solo".to_string()], false)
        .expect("tags alone");

    dav
}
