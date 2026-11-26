use super::*;

mod commands;
mod errors;
mod files;
mod globals;
mod groups_views;
mod inbox_tokens;
mod tags;

fn parsed(args: &[&str]) -> Parsed {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    parse(&owned).expect("parse")
}

fn failed(args: &[&str]) -> CliError {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    parse(&owned).expect_err("usage error")
}
