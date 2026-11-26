//! Fragments Askama de la sidebar.

use askama::Template;

use crate::ctx::{Base, SidebarData};

#[derive(Template)]
#[template(path = "partials/sidebar_oob.html")]
pub struct SidebarOobTemplate {
    pub base: Base,
    pub sidebar: SidebarData,
}

#[derive(Template)]
#[template(path = "partials/sidebar.html")]
pub struct SidebarTemplate {
    pub base: Base,
    pub sidebar: SidebarData,
}

#[derive(Template)]
#[template(path = "partials/search_filter_panel.html")]
pub struct SearchFilterPanelTemplate {
    pub base: Base,
    pub sidebar: SidebarData,
}
