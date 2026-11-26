use std::fs::{self, File, FileTimes};
use std::os::unix::fs::symlink;
use std::process::Command;
use std::time::{Duration, SystemTime};

use tempfile::TempDir;

use super::*;

fn fingerprinter() -> Blake3Fingerprinter {
    Blake3Fingerprinter::default()
}

fn fp_of(path: &Path, kind: FileKind) -> String {
    fingerprinter()
        .fingerprint(path, kind)
        .expect("empreinte calculée")
        .as_str()
        .to_string()
}

fn set_mtime(path: &Path, seconds: u64) {
    let file = File::options().write(true).open(path).expect("ouverture");
    let times =
        FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
    file.set_times(times).expect("mtime fixée");
}

#[test]
fn same_content_same_fingerprint_and_one_byte_differs() {
    let temp = TempDir::new().expect("tempdir");
    let a = temp.path().join("a.bin");
    let b = temp.path().join("b.bin");
    let c = temp.path().join("c.bin");
    fs::write(&a, b"contenu identique").expect("écriture");
    fs::write(&b, b"contenu identique").expect("écriture");
    fs::write(&c, b"contenu identiquf").expect("écriture");

    assert_eq!(fp_of(&a, FileKind::File), fp_of(&b, FileKind::File));
    assert_ne!(fp_of(&a, FileKind::File), fp_of(&c, FileKind::File));
}

#[test]
fn complete_fingerprint_separates_sampled_edge_collisions() {
    let temp = TempDir::new().expect("tempdir");
    let base = vec![7u8; 4096];
    let mut middle = base.clone();
    middle[2048] = 8;
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    fs::write(&a, &base).expect("écriture");
    fs::write(&b, &middle).expect("écriture");

    let sampling = Blake3Fingerprinter::new(1024, 256).expect("configuration valide");
    let sampled = |path: &Path| {
        sampling
            .fingerprint(path, FileKind::File)
            .expect("empreinte")
            .as_str()
            .to_string()
    };
    let complete = |path: &Path| {
        sampling
            .fingerprint_complete(path)
            .expect("empreinte intégrale")
            .as_str()
            .to_string()
    };
    assert_eq!(sampled(&a), sampled(&b), "bords identiques");
    assert_ne!(complete(&a), complete(&b), "milieu distinct");
    assert_ne!(sampled(&a), complete(&a), "empreinte intégrale séparée");
}

#[test]
fn complete_fingerprint_is_stable_for_identical_content() {
    let temp = TempDir::new().expect("tempdir");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    fs::write(&a, b"contenu identique").expect("écriture");
    fs::write(&b, b"contenu identique").expect("écriture");
    let fp = |path: &Path| {
        fingerprinter()
            .fingerprint_complete(path)
            .expect("empreinte intégrale")
            .as_str()
            .to_string()
    };
    assert_eq!(fp(&a), fp(&b));
}

#[test]
fn fingerprint_has_blake3_prefix_and_64_hex_chars() {
    let temp = TempDir::new().expect("tempdir");
    let file = temp.path().join("f");
    fs::write(&file, b"x").expect("écriture");
    let fp = fp_of(&file, FileKind::File);
    let hex = fp.strip_prefix("blake3:").expect("préfixe blake3:");
    assert_eq!(hex.len(), 64);
    assert!(
        hex.bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
}

#[test]
fn sampling_ignores_middle_but_sees_edges() {
    let temp = TempDir::new().expect("tempdir");
    let base = vec![7u8; 4096];
    let mut middle = base.clone();
    middle[2048] = 8;
    let mut end = base.clone();
    if let Some(last) = end.last_mut() {
        *last = 9;
    }
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    let c = temp.path().join("c");
    fs::write(&a, &base).expect("écriture");
    fs::write(&b, &middle).expect("écriture");
    fs::write(&c, &end).expect("écriture");

    let sampling = Blake3Fingerprinter::new(1024, 256).expect("configuration valide");
    let get = |path: &Path| {
        sampling
            .fingerprint(path, FileKind::File)
            .expect("empreinte")
            .as_str()
            .to_string()
    };
    assert_eq!(get(&a), get(&b));
    assert_ne!(get(&a), get(&c));
}

#[test]
fn short_sampled_file_falls_back_to_full_read() {
    let temp = TempDir::new().expect("tempdir");
    let a = temp.path().join("a");
    let mut content = vec![1u8; 1024];
    content[512] = 2;
    fs::write(&a, &content).expect("écriture");
    let first = fp_of(&a, FileKind::File);
    content[512] = 3;
    fs::write(&a, &content).expect("écriture");
    assert_ne!(first, fp_of(&a, FileKind::File));
}

#[test]
fn dir_fingerprint_is_order_independent_and_sensitive_to_members() {
    let temp = TempDir::new().expect("tempdir");
    let one = temp.path().join("one");
    let two = temp.path().join("two");
    fs::create_dir(&one).expect("mkdir");
    fs::create_dir(&two).expect("mkdir");
    for (dir, names) in [(&one, ["a", "b"]), (&two, ["b", "a"])] {
        for name in names {
            let path = dir.join(name);
            fs::write(&path, name.as_bytes()).expect("écriture");
            set_mtime(&path, 1_700_000_000);
        }
    }
    assert_eq!(
        fp_of(&one, FileKind::Directory),
        fp_of(&two, FileKind::Directory)
    );

    let extra = temp.path().join("extra");
    fs::create_dir(&extra).expect("mkdir");
    for name in ["a", "b"] {
        let path = extra.join(name);
        fs::write(&path, name.as_bytes()).expect("écriture");
        set_mtime(&path, 1_700_000_000);
    }
    let added = extra.join("c");
    fs::write(&added, b"c").expect("écriture");
    set_mtime(&added, 1_700_000_000);
    assert_ne!(
        fp_of(&one, FileKind::Directory),
        fp_of(&extra, FileKind::Directory)
    );
}

#[test]
fn file_and_dir_never_share_a_fingerprint() {
    let temp = TempDir::new().expect("tempdir");
    let file = temp.path().join("f");
    fs::write(&file, b"").expect("écriture");
    let dir = temp.path().join("d");
    fs::create_dir(&dir).expect("mkdir");
    assert_ne!(
        fp_of(&file, FileKind::File),
        fp_of(&dir, FileKind::Directory)
    );
}

#[test]
fn symlink_fingerprint_tracks_target() {
    let temp = TempDir::new().expect("tempdir");
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    let c = temp.path().join("c");
    symlink(&first, &a).expect("symlink");
    symlink(&first, &b).expect("symlink");
    symlink(&second, &c).expect("symlink");

    assert_eq!(fp_of(&a, FileKind::Symlink), fp_of(&b, FileKind::Symlink));
    assert_ne!(fp_of(&a, FileKind::Symlink), fp_of(&c, FileKind::Symlink));
}

#[test]
fn fifo_is_unsupported_without_blocking() {
    let temp = TempDir::new().expect("tempdir");
    let fifo = temp.path().join("pipe");
    let status = Command::new("mkfifo").arg(&fifo).status();
    let Ok(status) = status else {
        return;
    };
    if !status.success() {
        return;
    }
    assert!(matches!(
        fingerprinter().fingerprint(&fifo, FileKind::File),
        Err(FingerprintError::Unsupported(_))
    ));
}

#[test]
fn invalid_configuration_is_rejected() {
    assert!(matches!(
        Blake3Fingerprinter::new(1024, 0),
        Err(FingerprintError::Config)
    ));
    assert!(matches!(
        Blake3Fingerprinter::new(0, 0),
        Err(FingerprintError::Config)
    ));
    assert!(matches!(
        Blake3Fingerprinter::new(100, 200),
        Err(FingerprintError::Config)
    ));

    let invalid = Blake3Fingerprinter {
        sample_threshold: 0,
        sample_size: 0,
    };
    assert!(matches!(
        invalid.fingerprint(Path::new("/inexistant"), FileKind::File),
        Err(FingerprintError::Config)
    ));
}
