//! Commandes typées produites par le parser.

use std::path::PathBuf;

use fs_ez_core::{ApiTokenId, Sort};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init,
    Tag {
        tags: Vec<String>,
        paths: Vec<PathBuf>,
        force: bool,
    },
    Untag {
        tags: Vec<String>,
        paths: Vec<PathBuf>,
    },
    Files {
        query: Option<String>,
        sort: Sort,
        limit: usize,
        offset: usize,
        untagged: bool,
        recent: bool,
        print0: bool,
    },
    Tags,
    Rename {
        old: String,
        new: String,
    },
    Merge {
        sources: Vec<String>,
        target: String,
    },
    Delete {
        names: Vec<String>,
    },
    Repair,
    Status,
    ConfigList,
    ConfigGet {
        key: String,
    },
    ConfigSet {
        key: String,
        value: String,
    },
    Serve {
        host: Option<String>,
        port: Option<u16>,
    },
    GroupList,
    GroupAdd {
        name: String,
        color: Option<String>,
    },
    GroupRm {
        name: String,
    },
    GroupAssign {
        group: String,
        tag: String,
    },
    GroupMove {
        from: String,
        to: String,
        tag: String,
    },
    ColorSet {
        tag: String,
        token: String,
    },
    ViewList,
    ViewAdd {
        name: String,
        query: Option<String>,
        filter: Option<String>,
        icon: Option<String>,
        color: Option<String>,
        sort: Sort,
        group: Option<String>,
    },
    ViewRm {
        name: String,
    },
    ViewRun {
        name: String,
        limit: usize,
        offset: usize,
        print0: bool,
    },
    ViewGroupList,
    ViewGroupAdd {
        name: String,
        color: Option<String>,
    },
    ViewGroupRm {
        name: String,
    },
    ViewGroupAssign {
        group: String,
        view: String,
    },
    ViewGroupMove {
        from: String,
        to: String,
        view: String,
    },
    InboxScan,
    TokenAdd {
        name: String,
        expires: Option<String>,
    },
    TokenList,
    TokenRevoke {
        id: ApiTokenId,
    },
    UserReset {
        username: Option<String>,
    },
    Help(Option<String>),
    Version,
}

/// Table statique des alias, résolus avant dispatch.
pub const ALIASES: &[(&str, &str)] = &[("ls", "files"), ("find", "files"), ("rm", "delete")];

pub fn resolve_alias(name: &str) -> &str {
    ALIASES
        .iter()
        .find_map(|(alias, target)| (*alias == name).then_some(*target))
        .unwrap_or(name)
}

#[cfg(test)]
mod tests;
