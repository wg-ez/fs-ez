#![forbid(unsafe_code)]

use std::process::ExitCode;

use fs_ez_cli::output::Output;
use fs_ez_cli::run::{self, Outcome};
use fs_ez_cli::{CliError, USAGE, parse};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Cli(error)) => {
            eprintln!("{}", fs_ez_core::display_safe(&error.to_string()));
            if matches!(error, CliError::Usage(_)) {
                eprintln!("{USAGE}");
            }
            ExitCode::FAILURE
        }
        Err(Failure::Serve(error)) => {
            eprintln!("serve error: {}", fs_ez_core::display_safe(&error));
            ExitCode::FAILURE
        }
    }
}

enum Failure {
    Cli(CliError),
    Serve(String),
}

impl From<CliError> for Failure {
    fn from(error: CliError) -> Self {
        Self::Cli(error)
    }
}

fn run(args: &[String]) -> Result<(), Failure> {
    let parsed = parse(args)?;
    let ctx = fs_ez::context::build(&parsed.globals)?;
    match run::execute(&parsed, &ctx)? {
        Outcome::Output(output) => emit(output, parsed.globals.json),
        Outcome::Version => {
            let schema_version = fs_ez_sqlite::SqliteStore::schema_version(&ctx.db_path)
                .map_err(|error| CliError::Io(error.to_string()))?;
            let output = Output::new(
                format!(
                    "fs-ez {}\nschema {}\n",
                    env!("CARGO_PKG_VERSION"),
                    schema_version
                ),
                serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "schema_version": schema_version,
                }),
            );
            emit(output, parsed.globals.json)
        }
        Outcome::Serve { host, port } => {
            fs_ez::serve::serve(host, port, ctx.app.clone(), &ctx.db_path).map_err(Failure::Serve)
        }
    }
}

fn emit(output: Output, json: bool) -> Result<(), Failure> {
    print!("{}", output.render(json));
    Ok(())
}
