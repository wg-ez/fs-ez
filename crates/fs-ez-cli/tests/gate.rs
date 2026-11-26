//! Porte des phases 0 et 1 : intégration du binaire et snapshots.

#[path = "common/mod.rs"]
mod common;
#[path = "gate/inbox.rs"]
mod inbox;
#[path = "gate/init.rs"]
mod init;
#[path = "gate/json.rs"]
mod json;
#[path = "gate/snapshots.rs"]
mod snapshots;
#[path = "gate/tagging.rs"]
mod tagging;
#[path = "gate/user.rs"]
mod user;
#[path = "gate/views.rs"]
mod views;

use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use serde_json::{Value, json};
use tempfile::TempDir;

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

struct CliFixture {
    _temp: TempDir,
    root: PathBuf,
    db: PathBuf,
}

impl CliFixture {
    fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("files")).expect("layout");
        let db = root.join("config").join("fs-ez.db");
        Self {
            _temp: temp,
            root,
            db,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(common::fs_ez_bin());
        command
            .env("NO_COLOR", "1")
            .env("TZ", "UTC")
            .current_dir(&self.root);
        command
    }

    fn run(&self, args: &[&str]) -> Run {
        let mut command = self.command();
        self.run_with(&mut command, args)
    }

    fn run_with(&self, command: &mut Command, args: &[&str]) -> Run {
        let output = command.args(args).output().expect("exécution fs-ez");
        Run {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn ok(&self, args: &[&str]) -> Run {
        let run = self.run(args);
        assert_eq!(run.code, 0, "échec {args:?}: {}", run.stderr);
        assert!(
            run.stderr.is_empty(),
            "stderr inattendu {args:?}: {}",
            run.stderr
        );
        run
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut full = vec!["--json"];
        full.extend_from_slice(args);
        let run = self.ok(&full);
        let value: Value = serde_json::from_str(&run.stdout).expect("JSON valide sur stdout");
        assert_eq!(value["schema"], json!(1), "schéma {args:?}");
        value
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("dossier");
        }
        fs::write(&path, bytes).expect("écriture");
        path
    }

    fn outside(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.parent().expect("parent").join(name);
        fs::write(&path, bytes).expect("écriture");
        path
    }
}

fn items(value: &Value) -> &Vec<Value> {
    value["items"].as_array().expect("items")
}

fn normalize(value: &mut Value, root: &str) {
    match value {
        Value::Object(object) => {
            for (key, item) in object.iter_mut() {
                match key.as_str() {
                    "id" => *item = json!("[id]"),
                    "created_at" | "updated_at" | "mtime" | "expires_at" | "last_used_at" => {
                        *item = json!("[date]");
                    }
                    "db_size" => *item = json!("[size]"),
                    _ => normalize(item, root),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize(item, root);
            }
        }
        Value::String(text) => {
            if let Some(rest) = text.strip_prefix(root) {
                *text = format!("[root]{rest}");
            }
        }
        _ => {}
    }
}

fn normalized(fixture: &CliFixture, args: &[&str]) -> Value {
    let mut value = fixture.json(args);
    normalize(&mut value, fixture.root.to_str().expect("root utf8"));
    value
}
