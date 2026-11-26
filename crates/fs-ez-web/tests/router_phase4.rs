//! Porte phase 4 : CLI.

#[path = "router_phase4/cli.rs"]
mod cli;

mod common;

use std::path::Path;

use common::*;
use fs_ez_cli::context::CliContext;
use fs_ez_cli::run::Outcome;
use fs_ez_fingerprint::Blake3Fingerprinter;
use serde_json::Value;

fn cli_json(root: &Path, db_path: &Path, args: &[&str]) -> Value {
    let store = Arc::new(SqliteStore::open(db_path).expect("base CLI")) as Arc<dyn Store>;
    let app = App::new(
        store,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(root)).expect("root CLI"),
    );
    let ctx = CliContext {
        app,
        globals: fs_ez_cli::GlobalOptions::default(),
        db_path: db_path.to_path_buf(),
    };
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let parsed = fs_ez_cli::parse(&owned).expect("parse CLI");
    match fs_ez_cli::run::execute(&parsed, &ctx).expect("exécution CLI") {
        Outcome::Output(output) => output.json,
        _ => panic!("sortie attendue pour {args:?}"),
    }
}
