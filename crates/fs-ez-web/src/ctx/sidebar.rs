use fs_ez_core::{GroupId, TagColor, TagId, ViewGroupId, ViewId};

pub struct CreationError {
    pub message: String,
    pub draft: String,
    pub group: Option<GroupId>,
}

/// Brouillon de création de vue réaffiché tel quel après un refus.
pub struct ViewCreationError {
    pub message: String,
    pub name: String,
    pub filter_json: String,
    pub group: Option<ViewGroupId>,
    pub color: Option<TagColor>,
}

pub struct ViewGroupCreationError {
    pub message: String,
    pub draft: String,
}

pub struct SidebarData {
    /// Vues aplaties, pour retrouver l'éditeur ouvert par identifiant.
    pub views: Vec<ViewNav>,
    pub view_groups: Vec<ViewGroupNav>,
    /// Vue ouverte dans l'éditeur de la sidebar, `None` ferme le panneau.
    pub editing_view: Option<ViewNav>,
    pub groups: Vec<GroupNav>,
    pub error: Option<String>,
    pub group_create_error: Option<CreationError>,
    pub view_group_create_error: Option<ViewGroupCreationError>,
    /// `group` identifie le formulaire de création de tag à réafficher.
    pub tag_create_error: Option<CreationError>,
    pub view_create_error: Option<ViewCreationError>,
    /// Couleur tirée au hasard pour le prochain groupe de tags, prévisualisée et soumise.
    pub group_create_color: String,
    /// Couleur tirée au hasard pour le prochain groupe de vues, prévisualisée et soumise.
    pub view_group_create_color: String,
    /// Couleur tirée au hasard pour la prochaine vue, sélectionnée dans l'éditeur.
    pub view_create_color: TagColor,
}

impl SidebarData {
    pub fn group_create_message(&self) -> Option<&str> {
        self.group_create_error
            .as_ref()
            .map(|error| error.message.as_str())
    }

    pub fn group_create_draft(&self) -> &str {
        self.group_create_error
            .as_ref()
            .map(|error| error.draft.as_str())
            .unwrap_or_default()
    }

    pub fn view_group_create_message(&self) -> Option<&str> {
        self.view_group_create_error
            .as_ref()
            .map(|error| error.message.as_str())
    }

    pub fn view_group_create_draft(&self) -> &str {
        self.view_group_create_error
            .as_ref()
            .map(|error| error.draft.as_str())
            .unwrap_or_default()
    }

    pub fn view_create_message(&self) -> Option<&str> {
        self.view_create_error
            .as_ref()
            .map(|error| error.message.as_str())
    }

    pub fn view_create_draft(&self) -> &str {
        self.view_create_error
            .as_ref()
            .map(|error| error.name.as_str())
            .unwrap_or_default()
    }

    pub fn view_create_filter(&self) -> &str {
        self.view_create_error
            .as_ref()
            .map(|error| error.filter_json.as_str())
            .unwrap_or_default()
    }

    pub fn view_create_group(&self) -> String {
        self.view_create_error
            .as_ref()
            .and_then(|error| error.group)
            .map(|group| group.to_string())
            .unwrap_or_default()
    }

    pub fn view_create_colors(&self) -> Vec<SidebarColor> {
        let selected = self
            .view_create_error
            .as_ref()
            .and_then(|error| error.color)
            .unwrap_or(self.view_create_color);
        SidebarColor::choices(Some(selected))
    }

    fn tag_create_error_for(&self, group: &GroupId) -> Option<&CreationError> {
        self.tag_create_error
            .as_ref()
            .filter(|error| error.group == Some(*group))
    }

    pub fn tag_create_message_for(&self, group: &GroupId) -> Option<&str> {
        self.tag_create_error_for(group)
            .map(|error| error.message.as_str())
    }

    pub fn tag_create_draft_for(&self, group: &GroupId) -> &str {
        self.tag_create_error_for(group)
            .map(|error| error.draft.as_str())
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct SidebarColor {
    /// Jeton de couleur soumis.
    pub token: String,
    pub selected: bool,
}

impl SidebarColor {
    /// Palette commune tags/groupes/vues.
    pub fn choices(selected: Option<TagColor>) -> Vec<Self> {
        Self::for_token(selected.map(|color| color.as_token()))
    }

    fn for_token(selected: Option<&str>) -> Vec<Self> {
        TagColor::ALL
            .iter()
            .map(|color| Self {
                token: color.as_token().to_string(),
                selected: selected == Some(color.as_token()),
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct ViewNav {
    pub id: ViewId,
    pub name: String,
    pub filter_json: String,
    pub sort: String,
    pub color: Option<String>,
}

impl ViewNav {
    pub fn colors(&self) -> Vec<SidebarColor> {
        SidebarColor::for_token(self.color.as_deref())
    }
}

pub struct ViewGroupNav {
    pub id: ViewGroupId,
    pub name: String,
    pub color: Option<String>,
    pub collapsed: bool,
    pub views: Vec<ViewNav>,
}

impl ViewGroupNav {
    pub fn colors(&self) -> Vec<SidebarColor> {
        SidebarColor::for_token(self.color.as_deref())
    }
}

pub struct GroupNav {
    pub id: GroupId,
    pub name: String,
    pub color: Option<String>,
    pub collapsed: bool,
    pub tags: Vec<TagNav>,
    /// Couleur tirée au hasard pour le prochain tag du groupe, prévisualisée et soumise.
    pub tag_create_color: String,
}

impl GroupNav {
    pub fn colors(&self) -> Vec<SidebarColor> {
        SidebarColor::for_token(self.color.as_deref())
    }
}

pub struct TagNav {
    pub id: TagId,
    pub name: String,
    pub color: String,
}

impl TagNav {
    pub fn colors(&self) -> Vec<SidebarColor> {
        SidebarColor::for_token(Some(&self.color))
    }
}
