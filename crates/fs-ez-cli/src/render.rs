//! Rendu texte des sorties humaines, colonnes alignées sans couleur.

use fs_ez_app::{RepairReport, StatusReport};
use fs_ez_core::{
    AppliedTag, FileWithTags, SavedView, Tag, TagColor, TagCount, TagGroup, ViewGroup, display_safe,
};
use fs_ez_store::ApiTokenRecord;

const MAX_PATH_CHARS: usize = 80;

/// Tronque à 80 caractères avec `…` final, les chemins non-UTF8 sont déjà lossy.
fn truncate_path(text: &str) -> String {
    if text.chars().count() <= MAX_PATH_CHARS {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX_PATH_CHARS - 1).collect();
    out.push('…');
    out
}

fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(|cell| display_safe(cell)).collect())
        .collect();
    let mut widths: Vec<usize> = headers.iter().map(|head| head.chars().count()).collect();
    for row in &rows {
        for (index, cell) in row.iter().enumerate().take(widths.len()) {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    push_row(&mut out, headers, &widths);
    for row in &rows {
        push_row(&mut out, row, &widths);
    }
    out
}

fn push_row<T: AsRef<str>>(out: &mut String, cells: &[T], widths: &[usize]) {
    let mut line = String::new();
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            line.push_str("  ");
        }
        let cell = cells.get(index).map(|cell| cell.as_ref()).unwrap_or("");
        line.push_str(cell);
        for _ in cell.chars().count()..*width {
            line.push(' ');
        }
    }
    out.push_str(line.trim_end());
    out.push('\n');
}

/// `Option` affichée, `-` si absente.
fn or_dash<T: std::fmt::Display>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn tags_label(tags: &[AppliedTag]) -> String {
    tags.iter()
        .map(|applied| applied.tag.name.clone())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn file_table(files: &[FileWithTags]) -> String {
    let rows: Vec<Vec<String>> = files
        .iter()
        .map(|item| {
            vec![
                item.file.id.0.to_string(),
                tags_label(&item.tags),
                truncate_path(&item.file.path_display),
            ]
        })
        .collect();
    table(&["ID", "TAGS", "PATH"], &rows)
}

pub fn tag_table(tags: &[TagCount]) -> String {
    let rows: Vec<Vec<String>> = tags
        .iter()
        .map(|item| {
            vec![
                item.tag.id.0.to_string(),
                item.count.to_string(),
                item.tag.color.as_token().to_string(),
                item.tag.name.clone(),
            ]
        })
        .collect();
    table(&["ID", "COUNT", "COLOR", "NAME"], &rows)
}

/// 5 colonnes communes aux deux familles de groupes.
fn group_cells(
    id: impl std::fmt::Display,
    name: &str,
    color: Option<TagColor>,
    position: i64,
    collapsed: bool,
    members: &[String],
) -> Vec<String> {
    vec![
        id.to_string(),
        name.to_string(),
        or_dash(color.map(|color| color.as_token().to_string())),
        position.to_string(),
        if collapsed { "yes" } else { "no" }.to_string(),
        members.join(","),
    ]
}

pub fn group_table(groups: &[(TagGroup, Vec<Tag>)]) -> String {
    let rows: Vec<Vec<String>> = groups
        .iter()
        .map(|(group, tags)| {
            let members: Vec<String> = tags.iter().map(|tag| tag.name.clone()).collect();
            group_cells(
                group.id.0,
                &group.name,
                group.color,
                group.position,
                group.collapsed,
                &members,
            )
        })
        .collect();
    table(
        &["ID", "NAME", "COLOR", "POSITION", "COLLAPSED", "TAGS"],
        &rows,
    )
}

pub fn view_group_table(groups: &[(ViewGroup, Vec<SavedView>)]) -> String {
    let rows: Vec<Vec<String>> = groups
        .iter()
        .map(|(group, views)| {
            let members: Vec<String> = views.iter().map(|view| view.name.clone()).collect();
            group_cells(
                group.id.0,
                &group.name,
                group.color,
                group.position,
                group.collapsed,
                &members,
            )
        })
        .collect();
    table(
        &["ID", "NAME", "COLOR", "POSITION", "COLLAPSED", "VIEWS"],
        &rows,
    )
}

pub fn view_table(views: &[SavedView]) -> String {
    let rows: Vec<Vec<String>> = views
        .iter()
        .map(|view| {
            vec![
                view.id.0.to_string(),
                view.name.clone(),
                or_dash(view.color.map(|color| color.as_token().to_string())),
                fs_ez_query::pretty(&view.filter),
            ]
        })
        .collect();
    table(&["ID", "NAME", "COLOR", "QUERY"], &rows)
}

pub fn token_table(tokens: &[ApiTokenRecord]) -> String {
    let rows: Vec<Vec<String>> = tokens
        .iter()
        .map(|token| {
            vec![
                token.id.0.to_string(),
                token.name.clone(),
                token.created_at.to_string(),
                or_dash(token.expires_at),
                or_dash(token.last_used_at),
            ]
        })
        .collect();
    table(&["ID", "NAME", "CREATED", "EXPIRES", "LAST_USED"], &rows)
}

pub fn status_text(status: &StatusReport) -> String {
    let rows = vec![
        vec!["version".to_string(), status.version.to_string()],
        vec![
            "root".to_string(),
            truncate_path(&status.root.to_string_lossy()),
        ],
        vec![
            "db".to_string(),
            truncate_path(&status.db_path.to_string_lossy()),
        ],
        vec!["db_size".to_string(), status.db_size.to_string()],
        vec!["files".to_string(), status.files.to_string()],
        vec!["untagged".to_string(), status.untagged.to_string()],
        vec!["recent".to_string(), status.recent.to_string()],
        vec!["tags".to_string(), status.tags.to_string()],
        vec!["views".to_string(), status.views.to_string()],
    ];
    table(&["FIELD", "VALUE"], &rows)
}

pub fn repair_text(report: &RepairReport) -> String {
    format!(
        "updated {}\nmoved {}\nmissing {}\n",
        report.updated, report.moved, report.missing
    )
}

pub fn config_text(settings: &[(String, String)]) -> String {
    let rows: Vec<Vec<String>> = settings
        .iter()
        .map(|(key, value)| vec![key.clone(), value.clone()])
        .collect();
    table(&["KEY", "VALUE"], &rows)
}

#[cfg(test)]
mod tests;
