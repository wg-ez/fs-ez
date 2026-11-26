//! Sidebar du shell : vues, groupes de vues et tags peuplés depuis l'application.

use fs_ez_app::{App, AppError, random_tag_color};
use fs_ez_core::{SavedView, Settings, TagColor};

use crate::ctx::{GroupNav, SidebarData, TagNav, ViewGroupNav, ViewNav};
use crate::query_params::sort_token;

pub(crate) fn build_sidebar(app: &App) -> Result<(Settings, SidebarData), AppError> {
    let settings = app.settings()?;

    let mut groups = Vec::new();
    for (group, tags) in app.groups_with_tags()? {
        let navs = tags
            .iter()
            .map(|tag| TagNav {
                id: tag.id,
                name: tag.name.clone(),
                color: color_token(tag.color),
            })
            .collect();
        groups.push(GroupNav {
            id: group.id,
            name: group.name,
            color: group.color.map(color_token),
            collapsed: group.collapsed,
            tags: navs,
            tag_create_color: color_token(random_tag_color()?),
        });
    }

    let mut views = Vec::new();
    let mut view_groups = Vec::new();
    for (group, group_views) in app.view_groups_with_views()? {
        let navs: Vec<ViewNav> = group_views.into_iter().map(view_nav).collect();
        views.extend(navs.iter().cloned());
        view_groups.push(ViewGroupNav {
            id: group.id,
            name: group.name,
            color: group.color.map(color_token),
            collapsed: group.collapsed,
            views: navs,
        });
    }
    Ok((
        settings,
        SidebarData {
            views,
            view_groups,
            editing_view: None,
            groups,
            error: None,
            group_create_error: None,
            view_group_create_error: None,
            tag_create_error: None,
            view_create_error: None,
            group_create_color: color_token(random_tag_color()?),
            view_group_create_color: color_token(random_tag_color()?),
            view_create_color: random_tag_color()?,
        },
    ))
}

fn view_nav(view: SavedView) -> ViewNav {
    ViewNav {
        id: view.id,
        name: view.name,
        filter_json: serde_json::to_string(&view.filter).unwrap_or_default(),
        sort: sort_token(view.sort),
        color: view.color.map(color_token),
    }
}

fn color_token(color: TagColor) -> String {
    color.as_token().to_string()
}
