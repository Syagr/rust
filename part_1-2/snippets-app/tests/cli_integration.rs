use predicates::prelude::*;
use mockito::{mock, server_url};
use assert_cmd::Command;
use tempfile::NamedTempFile;

#[test]
fn download_creates_snippet_and_reads_back() {
    let _m = mock("GET", "/snippet.txt")
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("downloaded-content")
        .create();

    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap().to_string();

    let url = format!("{}/snippet.txt", server_url());

    // run binary to add snippet via download
    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("json:{}", path))
        .arg("--name")
        .arg("dl1")
        .arg("--download")
        .arg(&url)
        .assert()
        .success()
        .stdout(predicate::str::contains("Snippet 'dl1' saved."));

    // run binary to read it back
    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("json:{}", path))
        .arg("--read")
        .arg("dl1")
        .assert()
        .success()
        .stdout(predicate::str::contains("downloaded-content"));
}

#[test]
fn stdin_create_and_read_sqlite() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap().to_string();

    // create snippet from stdin into sqlite
    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("sqlite:{}", path))
        .arg("--name")
        .arg("s1")
        .write_stdin("hello sqlite")
        .assert()
        .success()
        .stdout(predicate::str::contains("Snippet 's1' saved."));

    // read it back
    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("sqlite:{}", path))
        .arg("--read")
        .arg("s1")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello sqlite"));
}

#[test]
fn cli_delete_and_not_found() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap().to_string();

    // create snippet
    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("json:{}", path))
        .arg("--name")
        .arg("todel")
        .write_stdin("to be deleted")
        .assert()
        .success();

    // delete it
    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("json:{}", path))
        .arg("--delete")
        .arg("todel")
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted"));

    // read -> not found
    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("json:{}", path))
        .arg("--read")
        .arg("todel")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn cli_read_missing_snippet_reports_not_found() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap().to_string();

    let mut cmd = Command::cargo_bin("snippets-app").unwrap();
    cmd.env("SNIPPETS_APP_STORAGE", format!("json:{}", path))
        .arg("--read")
        .arg("does_not_exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
