//! Harnais partagé des tests d'intégration de la CLI.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_ez_app::App;
use fs_ez_cli::context::CliContext;
use fs_ez_cli::error::CliError;
use fs_ez_cli::parse::parse;
use fs_ez_cli::run::{self, Outcome};
use fs_ez_cli::{GlobalOptions, Output};
use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::SqliteStore;
use serde_json::Value;
use tempfile::TempDir;

/// Localise le binaire fs-ez : hors crate binaire `CARGO_BIN_EXE` manque, cargo 1.100 range les tests sous `build/`.
pub fn fs_ez_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_fs-ez") {
        return PathBuf::from(path);
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("target")
        });
    target
        .join("debug")
        .join(format!("fs-ez{}", std::env::consts::EXE_SUFFIX))
}

pub struct Harness {
    pub temp: TempDir,
    pub ctx: CliContext,
}

impl Harness {
    pub fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let db_path = temp.path().join("config").join("fs-ez.db");
        let store = SqliteStore::open(&db_path).expect("base");
        let root = Root::discover(Some(temp.path())).expect("root");
        let app = App::new(
            Arc::new(store),
            Arc::new(SystemFs),
            Arc::new(Blake3Fingerprinter::default()),
            root,
        );
        let ctx = CliContext {
            app,
            globals: GlobalOptions::default(),
            db_path,
        };
        Self { temp, ctx }
    }

    pub fn init(&self) -> Value {
        self.json(&["init"])
    }

    pub fn write(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.temp.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("dossier");
        }
        std::fs::write(&path, content).expect("écriture");
        path
    }

    pub fn run(&self, args: &[&str]) -> Result<Outcome, CliError> {
        let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
        let parsed = parse(&owned)?;
        run::execute(&parsed, &self.ctx)
    }

    pub fn output(&self, args: &[&str]) -> Output {
        match self.run(args).expect("commande") {
            Outcome::Output(output) => output,
            _ => panic!("sortie attendue pour {args:?}"),
        }
    }

    pub fn json(&self, args: &[&str]) -> Value {
        self.output(args).json
    }

    pub fn rendered_json(&self, args: &[&str]) -> Value {
        let text = self.output(args).render(true);
        serde_json::from_str(&text).expect("JSON valide")
    }

    pub fn error(&self, args: &[&str]) -> CliError {
        match self.run(args) {
            Err(error) => error,
            Ok(_) => panic!("erreur attendue pour {args:?}"),
        }
    }
}

pub fn items(value: &Value) -> &Vec<Value> {
    value["items"].as_array().expect("items")
}
