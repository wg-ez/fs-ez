//! DTO JSON de l'API : jamais de `PathBuf`, dates en RFC3339, identifiants typés.

use fs_ez_core::{
    ApiTokenId, AppliedTag, FileId, FileKind, FileWithTags, FilterExpr, GroupId, SavedView, Sort,
    Tag, TagColor, TagCount, TagGroup, ViewGroup, ViewGroupId, ViewId,
};
use fs_ez_store::{ApiTokenRecord, Page};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Serialize)]
pub struct FileDto {
    pub id: FileId,
    pub path: String,
    pub name: String,
    pub dir: String,
    pub size: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub mtime: OffsetDateTime,
    pub kind: FileKind,
    pub fingerprint: Option<String>,
    pub tags: Vec<TagApplicationDto>,
}

#[derive(Debug, Serialize)]
pub struct TagApplicationDto {
    pub name: String,
    pub color: TagColor,
    pub explicit: bool,
}

#[derive(Debug, Serialize)]
pub struct TagDto {
    pub id: fs_ez_core::TagId,
    pub name: String,
    pub name_norm: String,
    pub color: TagColor,
    pub color_custom: Option<String>,
    pub count: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct GroupDto {
    pub id: GroupId,
    pub name: String,
    pub color: Option<TagColor>,
    pub position: i64,
    pub collapsed: bool,
    pub tags: Vec<TagDto>,
}

#[derive(Debug, Serialize)]
pub struct ViewDto {
    pub id: ViewId,
    pub name: String,
    pub filter: FilterExpr,
    pub query: String,
    pub icon: Option<String>,
    pub color: Option<TagColor>,
    pub position: i64,
    pub sort: Sort,
    pub count: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize)]
pub struct ViewGroupDto {
    pub id: ViewGroupId,
    pub name: String,
    pub color: Option<TagColor>,
    pub position: i64,
    pub collapsed: bool,
    pub views: Vec<ViewDto>,
}

#[derive(Debug, Serialize)]
pub struct PageDto<T> {
    pub items: Vec<T>,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct TokenDto {
    pub id: ApiTokenId,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize)]
pub struct TokenCreatedDto {
    #[serde(flatten)]
    pub token_record: TokenDto,
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct StatusDto {
    pub version: String,
    pub root: String,
    pub db_path: String,
    pub db_size: u64,
    pub files: u64,
    pub untagged: u64,
    pub recent: u64,
    pub tags: u64,
    pub views: u64,
}

impl FileDto {
    pub fn from_file(value: FileWithTags) -> Self {
        let file = value.file;
        Self {
            id: file.id,
            // Chemins non-UTF8 : `path_display` lossy, jamais le `PathBuf`.
            path: file.path_display,
            name: file.name,
            dir: file.dir,
            size: file.size,
            mtime: file.mtime,
            kind: file.kind,
            fingerprint: file.fingerprint.map(|value| value.as_str().to_string()),
            tags: value
                .tags
                .into_iter()
                .map(TagApplicationDto::from)
                .collect(),
        }
    }
}

impl From<AppliedTag> for TagApplicationDto {
    fn from(value: AppliedTag) -> Self {
        Self {
            name: value.tag.name,
            color: value.tag.color,
            explicit: value.explicit,
        }
    }
}

impl TagDto {
    pub fn from_tag(tag: Tag, count: Option<u64>) -> Self {
        Self {
            id: tag.id,
            name: tag.name,
            name_norm: tag.name_norm,
            color: tag.color,
            color_custom: tag.color_custom,
            count,
        }
    }

    pub fn from_count(value: TagCount) -> Self {
        Self::from_tag(value.tag, Some(value.count))
    }
}

impl GroupDto {
    pub fn from_group(group: TagGroup, tags: Vec<Tag>) -> Self {
        Self {
            id: group.id,
            name: group.name,
            color: group.color,
            position: group.position,
            collapsed: group.collapsed,
            tags: tags
                .into_iter()
                .map(|tag| TagDto::from_tag(tag, None))
                .collect(),
        }
    }
}

impl ViewDto {
    pub fn from_view(view: SavedView, count: u64) -> Self {
        let query = fs_ez_query::pretty(&view.filter);
        Self {
            id: view.id,
            name: view.name,
            filter: view.filter,
            query,
            icon: view.icon,
            color: view.color,
            position: view.position,
            sort: view.sort,
            count,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}

impl ViewGroupDto {
    pub fn from_group(group: ViewGroup, views: Vec<ViewDto>) -> Self {
        Self {
            id: group.id,
            name: group.name,
            color: group.color,
            position: group.position,
            collapsed: group.collapsed,
            views,
        }
    }
}

impl<T> PageDto<T> {
    pub fn from_page<U>(page: Page<U>, map: impl FnMut(U) -> T) -> Self {
        Self {
            items: page.items.into_iter().map(map).collect(),
            next_offset: page.next_offset,
        }
    }
}

impl TokenDto {
    /// Le hash du jeton reste en base : jamais exposé, même tronqué.
    pub fn from_record(record: ApiTokenRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            created_at: record.created_at,
            expires_at: record.expires_at,
            last_used_at: record.last_used_at,
        }
    }
}

impl StatusDto {
    pub fn from_report(report: fs_ez_app::StatusReport) -> Self {
        Self {
            version: report.version.to_string(),
            root: report.root.to_string_lossy().into_owned(),
            db_path: report.db_path.to_string_lossy().into_owned(),
            db_size: report.db_size,
            files: report.files,
            untagged: report.untagged,
            recent: report.recent,
            tags: report.tags,
            views: report.views,
        }
    }
}

#[cfg(test)]
mod tests;
