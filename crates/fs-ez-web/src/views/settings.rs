//! Modèles de la page réglages : préférences, inbox et clés d'API.

use askama::Template;
use fs_ez_core::ApiTokenId;

use crate::ctx::{Base, SidebarData};
use crate::views::files::format_mtime;

pub struct TokenRow {
    pub id: ApiTokenId,
    pub name: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
}

pub struct TokensData {
    pub rows: Vec<TokenRow>,
    pub created: Option<String>,
    pub error: Option<String>,
    pub draft_name: String,
    pub draft_expires: String,
}

impl TokensData {
    pub fn new(
        records: Vec<fs_ez_store::ApiTokenRecord>,
        created: Option<String>,
        error: Option<String>,
        draft_name: String,
        draft_expires: String,
    ) -> Self {
        let rows = records
            .into_iter()
            .map(|record| TokenRow {
                id: record.id,
                name: record.name,
                created_at: format_mtime(record.created_at),
                expires_at: record.expires_at.map(format_mtime),
                last_used_at: record.last_used_at.map(format_mtime),
            })
            .collect();
        Self {
            rows,
            created,
            error,
            draft_name,
            draft_expires,
        }
    }
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsPageTemplate {
    pub base: Base,
    pub sidebar: SidebarData,
    pub data: TokensData,
    pub q: String,
    pub query: String,
}

#[derive(Template)]
#[template(path = "partials/settings_tokens.html")]
pub struct SettingsTokensTemplate {
    pub base: Base,
    pub data: TokensData,
}
