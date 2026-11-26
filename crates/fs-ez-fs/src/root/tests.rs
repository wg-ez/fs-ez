//! Tests de la découverte et du confinement de la racine.

use std::os::unix::fs::{PermissionsExt, symlink};

use tempfile::TempDir;

use super::*;

fn root_of(temp: &TempDir) -> Root {
    Root::discover(Some(temp.path())).expect("root")
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).expect("meta").permissions().mode() & 0o777
}

#[test]
fn discover_prefers_explicit_then_walk_up() {
    let explicit = TempDir::new().expect("tempdir");
    let walked = TempDir::new().expect("tempdir");
    fs::write(walked.path().join(PROJECT_FILE), b"").expect("marqueur");
    let deep = walked.path().join("a/b");
    fs::create_dir_all(&deep).expect("mkdir");

    let chosen = Root::resolve(Some(explicit.path()), None, &deep).expect("explicite");
    assert_eq!(
        chosen.path(),
        fs::canonicalize(explicit.path()).expect("canon")
    );

    let chosen = Root::resolve(None, None, &deep).expect("remontée");
    assert_eq!(
        chosen.path(),
        fs::canonicalize(walked.path()).expect("canon")
    );
}

#[test]
fn discover_falls_back_to_cwd_and_rejects_non_dirs() {
    let temp = TempDir::new().expect("tempdir");
    let chosen = Root::resolve(None, None, temp.path()).expect("fallback");
    assert_eq!(chosen.path(), fs::canonicalize(temp.path()).expect("canon"));

    let file = temp.path().join("file");
    fs::write(&file, b"").expect("écriture");
    assert!(Root::discover(Some(&file)).is_err());
    assert!(Root::discover(Some(&temp.path().join("absent"))).is_err());
}

#[test]
fn config_paths_default_to_root_children() {
    let temp = TempDir::new().expect("tempdir");
    let root = root_of(&temp);
    assert_eq!(root.files("").expect("files"), root.path().join("files"));
    assert_eq!(
        root.files("incoming").expect("relatif"),
        root.path().join("incoming")
    );
    assert!(root.files("../escape").is_err());
    assert_eq!(root.state_dir(), root.path().join("config"));
    assert_eq!(
        root.default_db(),
        root.path().join("config").join("fs-ez.db")
    );
}

#[test]
fn config_override_moves_state_dir_and_relative_is_rooted() {
    let temp = TempDir::new().expect("tempdir");
    let config = TempDir::new().expect("tempdir");
    let root =
        Root::resolve(Some(temp.path()), Some(config.path()), temp.path()).expect("config absolu");
    assert_eq!(root.state_dir(), config.path());
    assert_eq!(root.default_db(), config.path().join("fs-ez.db"));

    let state = temp.path().join("state");
    let root = Root::resolve(Some(temp.path()), Some(&state), temp.path()).expect("config relatif");
    assert_eq!(
        root.state_dir(),
        fs::canonicalize(temp.path()).expect("canon").join("state")
    );

    assert!(Root::resolve(Some(temp.path()), Some(Path::new("../escape")), temp.path(),).is_err());
}

#[test]
fn init_layout_is_idempotent_and_private() {
    let temp = TempDir::new().expect("tempdir");
    let root = root_of(&temp);
    root.init_layout("").expect("init");
    root.init_layout("").expect("idempotent");
    assert_eq!(mode_of(&root.state_dir()), 0o700);
    assert!(root.path().join("files").is_dir());
    assert_eq!(root.files("").expect("files"), root.path().join("files"));
}

#[test]
fn init_layout_refuses_symlinked_state_dir() {
    let temp = TempDir::new().expect("tempdir");
    let target = TempDir::new().expect("cible");
    fs::set_permissions(target.path(), fs::Permissions::from_mode(0o755)).expect("mode cible");
    let link = temp.path().join("state");
    symlink(target.path(), &link).expect("symlink");
    let root = Root::resolve(Some(temp.path()), Some(&link), temp.path()).expect("root");

    assert!(matches!(root.init_layout(""), Err(FsError::Invalid(_))));
    assert_eq!(mode_of(target.path()), 0o755);
    assert!(
        fs::symlink_metadata(&link)
            .expect("lien")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn init_layout_refuses_symlinked_state_ancestor() {
    let base = TempDir::new().expect("base");
    let victim = TempDir::new().expect("cible");
    let sub = victim.path().join("sub");
    fs::create_dir(&sub).expect("sous-dossier");
    fs::write(sub.join("canary.txt"), b"canary").expect("canari");
    fs::set_permissions(victim.path(), fs::Permissions::from_mode(0o755)).expect("mode");
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).expect("mode");

    let link = base.path().join("link");
    symlink(victim.path(), &link).expect("symlink");
    let root =
        Root::resolve(Some(base.path()), Some(&link.join("sub")), base.path()).expect("root");

    assert!(matches!(root.init_layout(""), Err(FsError::Invalid(_))));
    assert_eq!(mode_of(&sub), 0o755);
    assert!(!sub.join("fs-ez.db").exists());
}

#[test]
fn init_layout_refuses_symlinked_state_ancestor_chain_and_missing_final() {
    let base = TempDir::new().expect("base");
    let victim = TempDir::new().expect("cible");
    fs::set_permissions(victim.path(), fs::Permissions::from_mode(0o755)).expect("mode");
    let second = base.path().join("second");
    symlink(victim.path(), &second).expect("symlink cible");
    let first = base.path().join("first");
    symlink(&second, &first).expect("symlink relais");
    let root =
        Root::resolve(Some(base.path()), Some(&first.join("neuf")), base.path()).expect("root");

    assert!(matches!(root.init_layout(""), Err(FsError::Invalid(_))));
    assert!(!victim.path().join("neuf").exists());
    assert_eq!(mode_of(victim.path()), 0o755);
}

#[test]
fn init_layout_refuses_relative_state_through_symlink() {
    let root_dir = TempDir::new().expect("root");
    let victim = TempDir::new().expect("cible");
    let sub = victim.path().join("sub");
    fs::create_dir(&sub).expect("sous-dossier");
    fs::set_permissions(victim.path(), fs::Permissions::from_mode(0o755)).expect("mode");
    symlink(victim.path(), root_dir.path().join("config")).expect("symlink");

    let root = Root::resolve(
        Some(root_dir.path()),
        Some(Path::new("config/sub")),
        root_dir.path(),
    )
    .expect("root");
    assert!(matches!(root.init_layout(""), Err(FsError::Invalid(_))));
    assert_eq!(mode_of(&sub), 0o755);
    assert!(!sub.join("fs-ez.db").exists());
}

#[test]
fn confine_existing_accepts_child_and_rejects_escapes() {
    let temp = TempDir::new().expect("tempdir");
    let outside = TempDir::new().expect("tempdir");
    let root = root_of(&temp);
    let child = temp.path().join("a.txt");
    fs::write(&child, b"").expect("écriture");

    assert_eq!(
        root.confine_existing(&child).expect("enfant"),
        fs::canonicalize(&child).expect("canon")
    );
    assert!(matches!(
        root.confine_existing(&temp.path().join("../")),
        Err(FsError::OutsideRoot(_))
    ));

    let link = temp.path().join("escape");
    symlink(outside.path(), &link).expect("symlink");
    assert!(matches!(
        root.confine_existing(&link),
        Err(FsError::OutsideRoot(_))
    ));
    assert!(matches!(
        root.confine_existing(&temp.path().join("absent")),
        Err(FsError::NotFound(_))
    ));
}

#[test]
fn confine_new_accepts_new_file_and_rejects_escapes() {
    let temp = TempDir::new().expect("tempdir");
    let outside = TempDir::new().expect("tempdir");
    let root = root_of(&temp);

    assert_eq!(
        root.confine_new(&temp.path().join("neuf.txt"))
            .expect("neuf"),
        fs::canonicalize(temp.path())
            .expect("canon")
            .join("neuf.txt")
    );
    assert!(matches!(
        root.confine_new(&outside.path().join("x")),
        Err(FsError::OutsideRoot(_))
    ));
    assert!(matches!(
        root.confine_new(&temp.path().join("../x")),
        Err(FsError::OutsideRoot(_))
    ));
    assert!(matches!(
        root.confine_new(Path::new("/")),
        Err(FsError::Invalid(_))
    ));
}

#[test]
fn contains_compares_components_not_string_prefixes() {
    let temp = TempDir::new().expect("tempdir");
    let root = root_of(&temp);
    assert!(root.contains(&root.path().join("a")));
    assert!(root.contains(root.path()));
    let sibling = PathBuf::from(format!("{}2", root.path().display()));
    assert!(!root.contains(&sibling));
    assert!(!root.contains(Path::new("/")));
    assert_eq!(
        root.relative(&root.path().join("a/b")),
        Some(PathBuf::from("a/b"))
    );
    assert_eq!(root.relative(Path::new("/elsewhere")), None);
}
