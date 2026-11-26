use fs_ez_core::{FileId, GroupId, SavedView, Settings, Sort, TagId, ViewId};
use fs_ez_store::FileListQuery;

use super::*;

impl Fixture {
    /// Groupe de test créé à la demande : un tag n'existe plus hors groupe.
    pub fn tag_group(&self) -> GroupId {
        self.app
            .list_groups()
            .expect("groupes")
            .into_iter()
            .next()
            .map(|group| group.id)
            .unwrap_or_else(|| self.app.create_group("Tests", None).expect("groupe").id)
    }

    pub fn create_tag(&self, name: &str, color: fs_ez_core::TagColor) -> fs_ez_core::Tag {
        self.app
            .create_tag(self.tag_group(), name, color)
            .expect("tag")
    }

    pub fn tag_names(&self, id: FileId) -> Vec<String> {
        self.app
            .file_with_tags(id)
            .expect("fichier")
            .tags
            .into_iter()
            .filter(|tag| tag.explicit)
            .map(|tag| tag.tag.name)
            .collect()
    }

    pub fn tag_id(&self, name: &str) -> TagId {
        self.app
            .store
            .get_tag_by_name(name)
            .expect("recherche")
            .expect("tag")
            .id
    }

    pub fn group_id(&self, name: &str) -> GroupId {
        self.app
            .list_groups()
            .expect("groupes")
            .into_iter()
            .find(|group| group.name == name)
            .expect("groupe")
            .id
    }

    pub fn view_id(&self, name: &str) -> ViewId {
        self.app
            .list_views()
            .expect("vues")
            .into_iter()
            .find(|view| view.name == name)
            .expect("vue")
            .id
    }

    pub fn view(&self, id: ViewId) -> SavedView {
        self.app.get_view(id).expect("vue")
    }

    pub fn file_id(&self, name: &str) -> FileId {
        self.app
            .store
            .list_files(&FileListQuery {
                filter: None,
                untagged: false,
                tagged_since_days: None,
                modified_since_days: None,
                text: Some(name.to_string()),
                virtual_folder: None,
                sort: Sort::default(),
                offset: 0,
                limit: 10,
            })
            .expect("fichiers")
            .items
            .into_iter()
            .find(|item| item.file.name == name)
            .expect("fichier")
            .file
            .id
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.app.get_setting(key).expect("reglage")
    }

    /// Nouvelle instance sur la meme base : simule un redemarrage.
    pub fn reopened_settings(&self) -> Settings {
        let store = Arc::new(SqliteStore::open(&self.db_path).expect("base")) as Arc<dyn Store>;
        let app = App::new(
            store,
            Arc::new(SystemFs),
            Arc::new(Blake3Fingerprinter::default()),
            Root::discover(Some(self.root())).expect("root"),
        );
        app.settings().expect("reglages")
    }
}
