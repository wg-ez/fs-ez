use std::path::PathBuf;

use fs_ez_app::{App, FileListMode};
use fs_ez_core::{FileRecord, SavedView, Tag, ViewGroup, VirtualFolder};

/// Nom réservé de la collection de fichiers sous une sélection de tags.
pub const FILES_DIR: &str = "All Files";
/// Nom réservé de la collection des tags sans fichier, présent à la racine et dans chaque sélection.
pub const EMPTY_DIR: &str = "All Empty";
/// Nom réservé de la racine des dossiers virtuels.
pub const VIRTUAL_FOLDERS_DIR: &str = "virtual folders";
/// Nom réservé de la collection des tags.
pub const TAGS_DIR: &str = "tags";
/// Nom réservé de la collection des vues.
pub const VIEWS_DIR: &str = "views";
/// Nom réservé de la collection inbox.
pub const INBOX_DIR: &str = "inbox";

/// Chemin sous `virtual folders`, la racine elle-même exclue.
pub fn is_virtual_folder_target(components: &[String]) -> bool {
    components
        .first()
        .is_some_and(|name| name == VIRTUAL_FOLDERS_DIR)
        && components.len() > 1
}

/// Noms de tags dans l'ordre de la sélection, entrée des requêtes applicatives.
pub fn tag_names(tags: &[Tag]) -> Vec<String> {
    tags.iter().map(|tag| tag.name.clone()).collect()
}

/// Vues système de la barre latérale, exposées à la racine DAV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartView {
    Untagged,
    Recent,
    RecentFiles,
}

impl SmartView {
    pub const ALL: [Self; 3] = [Self::Untagged, Self::Recent, Self::RecentFiles];

    /// Nom canonique, alias stables acceptés en plus.
    const NAMES: [(Self, &'static str, &'static [&'static str]); 3] = [
        (Self::Untagged, "untagged", &[]),
        (Self::Recent, "recently tagged", &["recent"]),
        (Self::RecentFiles, "recent files", &["recent-files"]),
    ];

    /// Nom canonique du dossier DAV, aligné sur les libellés de navigation.
    pub fn name(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(view, _, _)| *view == self)
            .map_or("", |(_, name, _)| *name)
    }

    /// Alias stables acceptés en plus du nom affiché.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .find(|(_, canonical, aliases)| *canonical == name || aliases.contains(&name))
            .map(|(view, _, _)| *view)
    }

    pub fn mode(self) -> FileListMode {
        match self {
            Self::Untagged => FileListMode::Untagged,
            Self::Recent => FileListMode::Recent,
            Self::RecentFiles => FileListMode::RecentFiles,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CollectionKind {
    Root,
    Tags,
    Views,
    Inbox,
    /// Racine de l'arborescence de dossiers virtuels.
    VirtualFolders,
    /// Dossier virtuel, enfants = sous-dossiers puis fichiers directs.
    VirtualFolder {
        folder: VirtualFolder,
    },
    /// Chemin de dossier virtuel absent, créé par le PUT ou le MKCOL qui le vise.
    VirtualFolderPath,
    /// Vue système calculée depuis les réglages, sans ligne en base.
    SmartView {
        view: SmartView,
    },
    /// Sélection de tags, enfants `All Files`, tags co-occurrents non sélectionnés puis `All Empty`.
    TagSelection {
        tags: Vec<Tag>,
    },
    /// Dossier `All Empty`, enfants = tags sans fichier non sélectionnés.
    EmptyTags {
        tags: Vec<Tag>,
    },
    /// Fichiers portant tous les tags de la sélection.
    TagFiles {
        tags: Vec<Tag>,
    },
    ViewGroup {
        group: ViewGroup,
    },
    View {
        view: SavedView,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub kind: CollectionKind,
    pub components: Vec<String>,
}

impl Collection {
    pub fn new(kind: CollectionKind, components: Vec<String>) -> Self {
        Self { kind, components }
    }

    pub fn key(&self) -> String {
        self.components.join("/")
    }

    /// Dossier virtuel d'import créé par MKCOL : segments au-delà de la collection canonique.
    pub fn is_upload_dir(&self) -> bool {
        self.shape(&self.components, false).is_upload
    }

    /// Tags appliqués à un fichier déposé dans ce dossier virtuel, inbox sans tag.
    pub fn upload_tags(&self) -> Option<Vec<String>> {
        if !self.is_upload_dir() {
            return None;
        }
        match &self.kind {
            CollectionKind::Inbox => Some(Vec::new()),
            CollectionKind::TagFiles { tags } => Some(tag_names(tags)),
            _ => None,
        }
    }

    /// Dossiers virtuels d'un fichier déposé, dernier segment étant son nom.
    pub fn upload_folder(&self, app: &App) -> Vec<String> {
        self.folder_after(app, true)
    }

    /// Dossiers virtuels d'une collection MKCOL, tous les segments sont des dossiers.
    pub fn collection_folder(&self, app: &App) -> Vec<String> {
        self.folder_after(app, false)
    }

    /// Dossier virtuel parent d'un fichier DAV, `Some([])` à la racine.
    pub fn virtual_folder_path(&self) -> Option<Vec<String>> {
        match &self.kind {
            CollectionKind::VirtualFolders
            | CollectionKind::VirtualFolder { .. }
            | CollectionKind::VirtualFolderPath => Some(
                self.components
                    .get(1..)
                    .map(<[String]>::to_vec)
                    .unwrap_or_default(),
            ),
            _ => None,
        }
    }

    /// Profondeur des segments canoniques précédant le dossier d'import.
    pub fn upload_base_len(&self) -> usize {
        self.upload_base_len_for(&self.components)
    }

    /// Profondeur canonique d'un chemin DAV réel, tags sélectionnés et marqueurs retirés.
    pub fn upload_base_len_for(&self, components: &[String]) -> usize {
        self.shape(components, false).base_len
    }

    /// Dossier relatif, import inbox et tag préfixé par `inbox/<date locale du jour>`.
    fn folder_after(&self, app: &App, skip_last: bool) -> Vec<String> {
        let folder = self.shape(&self.components, skip_last).relative;
        if !matches!(
            self.kind,
            CollectionKind::Inbox | CollectionKind::TagFiles { .. }
        ) {
            return folder;
        }
        let mut prefixed = app.inbox_virtual_prefix();
        prefixed.extend(folder);
        prefixed
    }

    /// Forme du chemin : dossier d'import, profondeur canonique et segments relatifs.
    fn shape(&self, components: &[String], skip_last: bool) -> Shape {
        let end = components.len().saturating_sub(usize::from(skip_last));
        let depth = components
            .iter()
            .filter(|c| c.as_str() != EMPTY_DIR)
            .count();
        let (is_upload, relative) = match &self.kind {
            CollectionKind::Inbox if components.len() > 2 => (true, components[1..end].to_vec()),
            // `All Empty` est un marqueur d'accès, pas un niveau de sélection.
            CollectionKind::TagFiles { tags }
                if components.last().map(String::as_str) != Some(FILES_DIR)
                    || depth > tags.len() + 2 =>
            {
                let relative = relative_tag_folder(&components[1.min(end)..end], tags.len());
                (true, relative)
            }
            _ => (false, Vec::new()),
        };
        Shape {
            is_upload,
            base_len: components.len().saturating_sub(relative.len()),
            relative,
        }
    }
}

/// Segments sous la racine canonique, tags sélectionnés et marqueurs réservés retirés.
fn relative_tag_folder(components: &[String], tag_count: usize) -> Vec<String> {
    let mut folder = Vec::new();
    let mut remaining = tag_count;
    let mut in_selection = true;
    for component in components {
        if in_selection {
            // `All Empty` est un marqueur d'accès, jamais un niveau de dossier.
            if component == EMPTY_DIR {
                continue;
            }
            if remaining > 0 {
                remaining -= 1;
                continue;
            }
            in_selection = false;
            if component == FILES_DIR {
                continue;
            }
        }
        folder.push(component.clone());
    }
    folder
}

struct Shape {
    is_upload: bool,
    base_len: usize,
    relative: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Node {
    Collection(Collection),
    File {
        collection: Collection,
        record: FileRecord,
    },
    /// Fichier physique de `inbox`, sans ligne en base.
    FilesEntry {
        path: PathBuf,
        /// Identité lstat validée, l'ouverture doit retrouver le même inode.
        dev: u64,
        ino: u64,
    },
    /// Nom absent d'une collection valide, ou temporaire DAV éventuel.
    Unknown {
        collection: Collection,
        name: String,
    },
}
