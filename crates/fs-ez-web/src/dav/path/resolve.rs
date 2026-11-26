use std::os::unix::fs::MetadataExt;

use dav_server::fs::FsError;
use fs_ez_app::App;
use fs_ez_core::{FileRecord, Tag, VirtualFolderId, virtual_file_name};

use super::super::logging;
use super::cache::SelectionCache;
use super::children::{find_tag, find_view_group, find_view_in_group, find_virtual_folder_child};
use super::error::{fs_error, store_error};
use super::model::{
    Collection, CollectionKind, EMPTY_DIR, FILES_DIR, INBOX_DIR, Node, SmartView, TAGS_DIR,
    VIEWS_DIR, VIRTUAL_FOLDERS_DIR,
};
use super::page::PageQuery;

const ALIAS_SCAN_PAGE: usize = 500;
/// Profondeur maximale d'une arborescence d'import ou d'une sélection de tags, garde-fou contre les URL démesurées.
const MAX_TAG_COMPONENTS: usize = 64;

/// Segments décodés, sans slash initial ni final, validés avant tout accès disque.
pub fn parse_components(raw: &[u8]) -> Result<Vec<String>, FsError> {
    let rest = raw.strip_prefix(b"/").unwrap_or(raw);
    let mut components = Vec::new();
    for segment in rest.split(|byte| *byte == b'/') {
        if segment.is_empty() {
            continue;
        }
        let Ok(decoded) = std::str::from_utf8(segment) else {
            return Err(FsError::Forbidden);
        };
        if decoded == "." || decoded == ".." {
            return Err(FsError::Forbidden);
        }
        if decoded.contains('\0')
            || decoded.contains('/')
            || decoded.contains('\\')
            || decoded.chars().any(char::is_control)
            || decoded.len() > 255
        {
            return Err(FsError::Forbidden);
        }
        components.push(decoded.to_string());
    }
    // views imbrique groupe puis vue, tags et inbox acceptent une arborescence d'import, les autres gardent deux niveaux.
    let max = match components.first().map(String::as_str) {
        Some(VIEWS_DIR) => 4,
        Some(TAGS_DIR | INBOX_DIR | VIRTUAL_FOLDERS_DIR) => MAX_TAG_COMPONENTS,
        _ => 3,
    };
    if components.len() > max {
        return Err(FsError::NotFound);
    }
    Ok(components)
}

/// Résolution sans cache partagé : cache neuf par appel, aucune fraîcheur conservée.
pub fn resolve(app: &App, components: &[String]) -> Result<Node, FsError> {
    resolve_with(app, components, None)
}

/// Variante DAV : cache de sélection fourni par `DavFs`, `None` pour les vérifications ponctuelles.
pub fn resolve_with(
    app: &App,
    components: &[String],
    cache: Option<&SelectionCache>,
) -> Result<Node, FsError> {
    match cache {
        Some(cache) => resolve_inner(app, components, cache),
        None => resolve_inner(app, components, &SelectionCache::default()),
    }
}

/// Chemins dont la structure impose un fichier : aucun scan complet de collection.
pub fn structure_is_file(components: &[String]) -> bool {
    match components.first().map(String::as_str) {
        Some(TAGS_DIR) => components
            .iter()
            .position(|component| component == FILES_DIR)
            .is_some_and(|index| components.len() == index + 2),
        Some(VIEWS_DIR) => components.len() == 4,
        Some(INBOX_DIR) => components.len() == 2,
        Some(name) => SmartView::from_name(name).is_some() && components.len() == 2,
        None => false,
    }
}

fn resolve_inner(
    app: &App,
    components: &[String],
    cache: &SelectionCache,
) -> Result<Node, FsError> {
    // Inbox et vues système sont des collections racines d'un seul segment, views garde trois niveaux.
    let split = match components.first().map(String::as_str) {
        Some(TAGS_DIR) => return resolve_tags(app, components, cache),
        Some(VIRTUAL_FOLDERS_DIR) => return resolve_virtual_folders(app, components),
        Some(INBOX_DIR) => {
            let collection = resolve_collection(app, &components[..1])?;
            // Arborescence d'import : les segments suivants forment un dossier virtuel aplatissant les fichiers.
            if components.len() > 2 {
                return Ok(Node::Collection(Collection::new(
                    CollectionKind::Inbox,
                    components.to_vec(),
                )));
            }
            let Some(name) = components.get(1) else {
                return Ok(Node::Collection(collection));
            };
            return inbox_node(app, collection, name);
        }
        Some(VIEWS_DIR) => components.len().min(3),
        Some(name) if SmartView::from_name(name).is_some() => 1,
        _ => components.len().min(2),
    };
    let collection = resolve_collection(app, &components[..split])?;
    if components.len() > split + 1 {
        return Err(FsError::NotFound);
    }
    let Some(name) = components.get(split) else {
        return Ok(Node::Collection(collection));
    };
    named_node(app, collection, name, !structure_is_file(components))
}

/// Sélection `tags/{tag}/{tag}...`, `All Files` terminal ou suivi d'un alias.
fn resolve_tags(app: &App, components: &[String], cache: &SelectionCache) -> Result<Node, FsError> {
    if components.len() == 1 {
        return Ok(Node::Collection(Collection::new(
            CollectionKind::Tags,
            components.to_vec(),
        )));
    }
    let mut tags: Vec<Tag> = Vec::new();
    let mut empty_scope = false;
    let mut empty_selected = false;
    let last = components.len() - 1;
    for (index, name) in components.iter().enumerate().skip(1) {
        // Marqueur d'accès aux tags vides, valable à la racine comme dans toute sélection.
        if name == EMPTY_DIR {
            empty_scope = true;
            continue;
        }
        if name == FILES_DIR {
            if tags.is_empty() {
                return Err(FsError::NotFound);
            }
            ensure_selection(app, &tags, cache, empty_scope)?;
            let collection = tag_files_collection(components, index, tags.clone());
            let Some(file_name) = components.get(index + 1) else {
                return Ok(Node::Collection(collection));
            };
            if index + 2 == components.len() {
                return named_node(app, collection, file_name, false);
            }
            // Sous-dossier d'import : tous les fichiers sont aplatis avec les tags de la sélection.
            return Ok(tag_files_upload(tags, components));
        }
        // Sélection complète : plus aucun tag à proposer, le dernier segment est un fichier.
        if index == last && !empty_scope && !tags.is_empty() && cache.complete(app, &tags)? {
            ensure_selection(app, &tags, cache, false)?;
            return named_node(
                app,
                tag_files_collection(components, index, tags),
                name,
                false,
            );
        }
        let tag = match find_tag(app, name, empty_scope, cache) {
            Ok(tag) => tag,
            Err(FsError::NotFound) if !tags.is_empty() => {
                ensure_selection(app, &tags, cache, empty_scope)?;
                // Dernier segment inconnu : nouveau fichier à importer avec les tags du chemin.
                if index == last {
                    return named_node(
                        app,
                        tag_files_collection(components, index, tags),
                        name,
                        false,
                    );
                }
                // Segment inconnu avant la fin : sous-dossier d'import sous les tags déjà résolus.
                return Ok(tag_files_upload(tags, components));
            }
            Err(error) => return Err(error),
        };
        if tags.iter().any(|selected| selected.id == tag.id) {
            return Err(FsError::NotFound);
        }
        if empty_scope {
            empty_selected = true;
        }
        tags.push(tag);
    }
    if empty_scope && !empty_selected {
        return Ok(Node::Collection(Collection::new(
            CollectionKind::EmptyTags { tags },
            components.to_vec(),
        )));
    }
    ensure_selection(app, &tags, cache, empty_scope)?;
    Ok(Node::Collection(Collection::new(
        CollectionKind::TagSelection { tags },
        components.to_vec(),
    )))
}

/// Arborescence `virtual folders/{dossier}/{dossier}` puis un alias de fichier.
fn resolve_virtual_folders(app: &App, components: &[String]) -> Result<Node, FsError> {
    if components.len() == 1 {
        return Ok(Node::Collection(Collection::new(
            CollectionKind::VirtualFolders,
            components.to_vec(),
        )));
    }
    let mut parent: Option<VirtualFolderId> = None;
    let last = components.len() - 1;
    for (index, name) in components.iter().enumerate().skip(1) {
        let folder = find_virtual_folder_child(app, parent, name)?;
        match folder {
            Some(folder) => {
                if index == last {
                    return Ok(Node::Collection(Collection::new(
                        CollectionKind::VirtualFolder { folder },
                        components.to_vec(),
                    )));
                }
                parent = Some(folder.id);
            }
            None if index == last => {
                let collection = virtual_folder_collection(app, components, parent)?;
                return named_node(app, collection, name, false);
            }
            // Ancêtre absent : le PUT ou le MKCOL crée la chaîne avant d'écrire la cible.
            None => {
                return Ok(Node::Unknown {
                    collection: Collection::new(
                        CollectionKind::VirtualFolderPath,
                        components[..last].to_vec(),
                    ),
                    name: components[last].clone(),
                });
            }
        }
    }
    Err(FsError::NotFound)
}

fn virtual_folder_collection(
    app: &App,
    components: &[String],
    parent: Option<VirtualFolderId>,
) -> Result<Collection, FsError> {
    let (kind, end) = match parent {
        Some(id) => (
            CollectionKind::VirtualFolder {
                folder: app.get_virtual_folder(id).map_err(fs_error)?,
            },
            components.len().saturating_sub(1),
        ),
        // Fichier à la racine : garder le segment `virtual folders` comme clé DAV.
        None => (CollectionKind::VirtualFolders, 1),
    };
    Ok(Collection::new(kind, components[..end].to_vec()))
}

/// Collection `All Files` canonique, que le chemin la nomme ou non.
fn tag_files_collection(components: &[String], tag_end: usize, tags: Vec<Tag>) -> Collection {
    let mut path = components[..tag_end].to_vec();
    path.push(FILES_DIR.to_string());
    Collection::new(CollectionKind::TagFiles { tags }, path)
}

/// Dossier d'import sous une sélection, aplati avec ses tags.
fn tag_files_upload(tags: Vec<Tag>, components: &[String]) -> Node {
    Node::Collection(Collection::new(
        CollectionKind::TagFiles { tags },
        components.to_vec(),
    ))
}

/// Alias existant ou temporaire DAV potentiel, scan complet seulement si la structure ne l'impose pas.
fn named_node(app: &App, collection: Collection, name: &str, scan: bool) -> Result<Node, FsError> {
    if let Some(record) = alias(app, &collection, name, scan)? {
        return Ok(Node::File { collection, record });
    }
    Ok(Node::Unknown {
        collection,
        name: name.to_string(),
    })
}

/// Une collection de sélection n'existe que si au moins un fichier porte tous ses tags, `All Empty` exempté.
fn ensure_selection(
    app: &App,
    tags: &[Tag],
    cache: &SelectionCache,
    empty_scope: bool,
) -> Result<(), FsError> {
    if empty_scope || cache.exists(app, tags)? {
        Ok(())
    } else {
        Err(FsError::NotFound)
    }
}

/// Drop direct encore présent dans le dossier inbox physique.
fn inbox_node(app: &App, collection: Collection, name: &str) -> Result<Node, FsError> {
    let path = app.inbox_path().map_err(fs_error)?.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_file() => Ok(Node::FilesEntry {
            path,
            dev: meta.dev(),
            ino: meta.ino(),
        }),
        Ok(_) => Err(FsError::Forbidden),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Node::Unknown {
            collection,
            name: name.to_string(),
        }),
        Err(error) => {
            tracing::warn!(target: logging::TARGET, %error, "dav inbox metadata failed");
            Err(FsError::GeneralFailure)
        }
    }
}

pub fn resolve_collection(app: &App, components: &[String]) -> Result<Collection, FsError> {
    let kind = match components {
        [] => CollectionKind::Root,
        [views] if views == VIEWS_DIR => CollectionKind::Views,
        [inbox] if inbox == INBOX_DIR => CollectionKind::Inbox,
        [folders] if folders == VIRTUAL_FOLDERS_DIR => CollectionKind::VirtualFolders,
        [name] => match SmartView::from_name(name) {
            Some(view) => CollectionKind::SmartView { view },
            None => return Err(FsError::NotFound),
        },
        [views, group] if views == VIEWS_DIR => CollectionKind::ViewGroup {
            group: find_view_group(app, group)?,
        },
        [views, group, view] if views == VIEWS_DIR => {
            let group = find_view_group(app, group)?;
            CollectionKind::View {
                view: find_view_in_group(app, group.id, view)?,
            }
        }
        _ => return Err(FsError::NotFound),
    };
    Ok(Collection::new(kind, components.to_vec()))
}

fn alias(
    app: &App,
    collection: &Collection,
    name: &str,
    scan: bool,
) -> Result<Option<FileRecord>, FsError> {
    let found = app.find_by_name(name).map_err(fs_error)?;
    if let Some(record) = found {
        if virtual_file_name(&record) == name && member(app, collection, &record)? {
            return Ok(Some(record));
        }
        // Homonyme hors collection : le membre réel peut porter le même nom, scan nécessaire.
    } else if !scan {
        return Ok(None);
    }
    // Un homonyme hors collection peut masquer le membre réel, parcourir la collection par pages.
    let Some(query) = PageQuery::from_collection(collection) else {
        return Ok(None);
    };
    let mut offset = 0;
    loop {
        let page = query.page(app, offset, ALIAS_SCAN_PAGE).map_err(fs_error)?;
        if let Some(record) = page
            .items
            .iter()
            .map(|item| &item.file)
            .find(|record| virtual_file_name(record) == name)
        {
            return Ok(Some(record.clone()));
        }
        match page.next_offset {
            Some(next) => offset = next,
            None => return Ok(None),
        }
    }
}

fn member(app: &App, collection: &Collection, record: &FileRecord) -> Result<bool, FsError> {
    let member = match &collection.kind {
        CollectionKind::TagFiles { tags } => {
            let applied = app.store.tags_for_file(record.id).map_err(store_error)?;
            tags.iter()
                .all(|tag| applied.iter().any(|entry| entry.tag.id == tag.id))
        }
        CollectionKind::View { view } => app
            .file_matches_view(record.id, view.id)
            .map_err(fs_error)?,
        CollectionKind::SmartView { view } => app
            .file_matches_mode(record, view.mode())
            .map_err(fs_error)?,
        CollectionKind::VirtualFolders => app
            .store
            .folder_of_file(record.id)
            .map_err(store_error)?
            .is_none(),
        CollectionKind::VirtualFolder { folder } => {
            app.store.folder_of_file(record.id).map_err(store_error)? == Some(folder.id)
        }
        CollectionKind::Root
        | CollectionKind::Tags
        | CollectionKind::Views
        | CollectionKind::TagSelection { .. }
        | CollectionKind::EmptyTags { .. }
        | CollectionKind::ViewGroup { .. }
        | CollectionKind::VirtualFolderPath
        | CollectionKind::Inbox => false,
    };
    Ok(member)
}
