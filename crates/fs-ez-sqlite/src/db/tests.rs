use super::*;

const NOW: &str = "2026-01-01T00:00:00Z";

fn insert_file(
    conn: &Connection,
    path: &str,
    fingerprint: Option<&str>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO file (path, path_display, name, dir, size, mtime, kind, fingerprint, created_at, updated_at)
         VALUES (?1, ?1, 'x', '/', 0, ?2, 'file', ?3, ?2, ?2)",
        rusqlite::params![path, NOW, fingerprint],
    )
}

fn insert_tag(conn: &Connection, norm: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO tag (name, name_norm, created_at) VALUES (?1, ?1, ?2)",
        rusqlite::params![norm, NOW],
    )
}

fn file_id(conn: &Connection, path: &str) -> rusqlite::Result<i64> {
    conn.query_row("SELECT id FROM file WHERE path = ?1", [path], |row| {
        row.get(0)
    })
}

#[test]
fn open_applies_pragmas_and_migration() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let conn = open(&dir.path().join("fs-ez.db"))?;
    assert_eq!(schema_version(&conn)?, 2);
    let journal: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    assert_eq!(journal, "wal");
    let foreign_keys: i64 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
    assert_eq!(foreign_keys, 1);
    Ok(())
}

#[test]
fn reopen_keeps_schema_and_data() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("fs-ez.db");
    {
        let conn = open(&path)?;
        insert_file(&conn, "/a", None)?;
    }
    let conn = open(&path)?;
    assert_eq!(schema_version(&conn)?, 2);
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM file", [], |row| row.get(0))?;
    assert_eq!(count, 1);
    Ok(())
}

#[test]
fn migration_adds_api_token_credential_generation() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("fs-ez.db");
    {
        let conn = Connection::open(&path)?;
        conn.execute_batch(MIGRATIONS[0].1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    let conn = open(&path)?;
    assert_eq!(schema_version(&conn)?, 2);
    conn.execute(
        "INSERT INTO api_token(name, token_hash, created_at) VALUES ('old', 'hash', ?1)",
        [NOW],
    )?;
    let generation: i64 = conn.query_row(
        "SELECT credential_generation FROM api_token WHERE name = 'old'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(generation, 0);
    Ok(())
}

#[test]
fn in_memory_has_all_tables() -> Result<(), Box<dyn std::error::Error>> {
    let conn = open_in_memory()?;
    assert_eq!(schema_version(&conn)?, 2);
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables: Vec<String> = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    assert_eq!(
        tables,
        vec![
            "api_token",
            "file",
            "file_tag",
            "saved_view",
            "session",
            "setting",
            "tag",
            "tag_group",
            "tag_group_member",
            "user",
            "view_group",
            "view_group_member",
            "virtual_folder",
            "virtual_folder_file",
        ]
    );
    Ok(())
}

#[test]
fn fingerprint_unique_when_present() -> Result<(), Box<dyn std::error::Error>> {
    let conn = open_in_memory()?;
    insert_file(&conn, "/a", Some("abc"))?;
    assert!(insert_file(&conn, "/b", Some("abc")).is_err());
    insert_file(&conn, "/c", None)?;
    insert_file(&conn, "/d", None)?;
    insert_file(&conn, "/e", Some(""))?;
    insert_file(&conn, "/f", Some(""))?;
    Ok(())
}

#[test]
fn file_tag_unique_per_file_and_tag() -> Result<(), Box<dyn std::error::Error>> {
    let conn = open_in_memory()?;
    insert_file(&conn, "/a", None)?;
    insert_tag(&conn, "x")?;
    let file_id = file_id(&conn, "/a")?;
    let tag_id: i64 = conn.query_row("SELECT id FROM tag WHERE name_norm = 'x'", [], |row| {
        row.get(0)
    })?;
    conn.execute(
        "INSERT INTO file_tag (file_id, tag_id, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![file_id, tag_id, NOW],
    )?;
    assert!(
        conn.execute(
            "INSERT INTO file_tag (file_id, tag_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![file_id, tag_id, NOW],
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn tag_belongs_to_single_group() -> Result<(), Box<dyn std::error::Error>> {
    let conn = open_in_memory()?;
    conn.execute(
        "INSERT INTO tag_group(name, position) VALUES ('Premier', 0), ('Second', 1)",
        [],
    )?;
    conn.execute(
        "INSERT INTO tag(name, name_norm, color, created_at, position)
         VALUES ('alpha', 'alpha', 'neutral', ?1, 0), ('beta', 'beta', 'neutral', ?1, 1)",
        [NOW],
    )?;
    conn.execute(
        "INSERT INTO tag_group_member(group_id, tag_id, position) VALUES (1, 1, 0)",
        [],
    )?;
    assert!(
        conn.execute(
            "INSERT INTO tag_group_member(group_id, tag_id, position) VALUES (2, 1, 0)",
            [],
        )
        .is_err()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn database_file_is_private() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("fs-ez.db");
    let conn = open(&path)?;
    drop(conn);
    let mode = fs::metadata(&path)?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_database_parent_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let target = dir.path().join("target");
    std::fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    let link = dir.path().join("state");
    std::os::unix::fs::symlink(&target, &link)?;
    let path = link.join("fs-ez.db");

    assert!(open(&path).is_err());
    assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o755);
    assert!(!target.join("fs-ez.db").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_database_ancestor_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let target = dir.path().join("target");
    let sub = target.join("sub");
    std::fs::create_dir_all(&sub)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o755))?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link)?;
    let path = link.join("sub").join("fs-ez.db");

    assert!(open(&path).is_err());
    assert_eq!(fs::metadata(&sub)?.permissions().mode() & 0o777, 0o755);
    assert!(!sub.join("fs-ez.db").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_database_ancestor_creates_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let target = dir.path().join("target");
    std::fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link)?;

    assert!(open(&link.join("neuf").join("fs-ez.db")).is_err());
    assert!(!target.join("neuf").exists());
    assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o755);
    Ok(())
}

#[cfg(unix)]
#[test]
fn open_creates_private_database_parent() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let parent = dir.path().join("state");
    let path = parent.join("fs-ez.db");

    let conn = open(&path)?;
    drop(conn);
    assert_eq!(fs::metadata(&parent)?.permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_database_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let target = dir.path().join("target.db");
    fs::write(&target, b"")?;
    let link = dir.path().join("fs-ez.db");
    std::os::unix::fs::symlink(&target, &link)?;
    assert!(open(&link).is_err());
    Ok(())
}
