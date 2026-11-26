use super::*;

#[test]
fn inbox_scan_imports_and_consumes_drop() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let first = fixture.write("inbox/one.txt", b"one");
    let second = fixture.write("inbox/two.txt", b"two");

    let scanned = fixture.json(&["inbox", "scan"]);
    assert_eq!(scanned["imported"], json!(2));
    assert_eq!(scanned["duplicates"], json!(0));

    assert!(!first.exists(), "dépôt consommé");
    assert!(!second.exists(), "dépôt consommé");
    assert_eq!(items(&fixture.json(&["files"])).len(), 2);
}
