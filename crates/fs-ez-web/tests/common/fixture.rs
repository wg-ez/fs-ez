use std::path::{Path, PathBuf};

use fs_ez_core::ViewId;

use super::*;

#[derive(Clone, Debug)]
pub struct FixtureOptions {
    page_size: Option<usize>,
    max_upload: u64,
    files: Option<PathBuf>,
    persistent_db: bool,
    create_files_dir: bool,
    create_inbox_dir: bool,
    password: &'static str,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            page_size: None,
            max_upload: 1024,
            files: None,
            persistent_db: false,
            create_files_dir: false,
            create_inbox_dir: false,
            password: PASSWORD,
        }
    }
}

impl FixtureOptions {
    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = Some(page_size);
        self
    }

    pub fn max_upload(mut self, max_upload: u64) -> Self {
        self.max_upload = max_upload;
        self
    }

    pub fn files(mut self, files: Option<PathBuf>) -> Self {
        self.files = files;
        self
    }

    pub fn persistent_db(mut self) -> Self {
        self.persistent_db = true;
        self
    }

    pub fn create_files_dir(mut self) -> Self {
        self.create_files_dir = true;
        self
    }

    pub fn create_inbox_dir(mut self) -> Self {
        self.create_inbox_dir = true;
        self
    }

    pub fn password(mut self, password: &'static str) -> Self {
        self.password = password;
        self
    }
}

pub struct Fixture {
    pub app: Arc<App>,
    pub state: AppState<AppBackend>,
    pub cookie: String,
    pub set_cookie: String,
    pub router: Router,
    pub dir: TempDir,
    pub db_path: PathBuf,
    pub view: Option<ViewId>,
}

impl Fixture {
    pub async fn new(options: FixtureOptions) -> Self {
        let dir = TempDir::new().expect("tempdir");
        if options.create_files_dir {
            std::fs::create_dir(dir.path().join("files")).expect("files");
        }
        if options.create_inbox_dir {
            std::fs::create_dir(dir.path().join("inbox")).expect("inbox");
        }
        let db_path = dir.path().join("config/fs-ez.db");
        let store: Arc<dyn Store> = if options.persistent_db {
            Arc::new(SqliteStore::open(&db_path).expect("base"))
        } else {
            Arc::new(SqliteStore::from_connection(
                db::open_in_memory().expect("base memoire"),
            ))
        };
        let app = Arc::new(App::new(
            store,
            Arc::new(SystemFs),
            Arc::new(Blake3Fingerprinter::default()),
            Root::discover(Some(dir.path())).expect("root"),
        ));
        app.init_user(USERNAME, options.password).expect("user");
        if let Some(size) = options.page_size {
            app.set_setting("ui.page_size", &size.to_string())
                .expect("taille de page");
        }
        let config = WebConfig {
            host: "127.0.0.1".to_string(),
            port: 8678,
            max_upload: options.max_upload,
            root: dir.path().to_path_buf(),
            db_path: db_path.clone(),
            files: options.files,
        };
        let state = AppState::new(
            Arc::new(AppBackend::new(Arc::clone(&app))),
            Arc::new(config),
            Arc::new(Locales::load().expect("locales")),
        );
        let router = build_router(state.clone());
        let response = send(&router, post_form("/login", &login_body(options.password))).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let set_cookie = header_string(&response, header::SET_COOKIE);
        let cookie = session_token(&set_cookie);
        assert!(!cookie.is_empty());
        Self {
            app,
            state,
            cookie,
            set_cookie,
            router,
            dir,
            db_path,
            view: None,
        }
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn files(&self) -> PathBuf {
        self.root().join("files")
    }

    pub fn inbox(&self) -> PathBuf {
        self.app.inbox_path().expect("inbox")
    }

    pub fn tmp_entries(&self) -> Vec<String> {
        match std::fs::read_dir(self.root().join("config/tmp")) {
            Ok(entries) => entries
                .map(|entry| {
                    entry
                        .expect("entrée")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub async fn send(&self, request: Request<Body>) -> Response<Body> {
        send(&self.router(), request).await
    }

    pub async fn get(&self, uri: &str) -> Response<Body> {
        self.send(with_cookie(get(uri), &self.cookie)).await
    }

    pub async fn post(&self, uri: &str, body: &str) -> Response<Body> {
        self.send(with_cookie(post_form(uri, body), &self.cookie))
            .await
    }

    pub async fn post_form(&self, uri: &str, body: &str) -> Response<Body> {
        self.post(uri, body).await
    }

    pub async fn post_hx(&self, uri: &str, body: &str) -> Response<Body> {
        self.send(with_hx(with_cookie(post_form(uri, body), &self.cookie)))
            .await
    }

    pub async fn page(&self, uri: &str) -> (StatusCode, String) {
        let response = self.get(uri).await;
        let status = response.status();
        (status, body_text(response).await)
    }
}

pub async fn default_fixture() -> Fixture {
    Fixture::new(FixtureOptions::default()).await
}
