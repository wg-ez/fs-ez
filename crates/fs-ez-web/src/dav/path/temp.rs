use std::path::PathBuf;

use fs_ez_app::{App, staging, token};

use super::model::Collection;

/// Temporaires DAV, hash de collection puis hash de nom, jamais un nom utilisateur.
pub fn temp_path(app: &App, collection: &Collection, name: &str) -> PathBuf {
    let collection_hash = token::hash_token(&collection.key());
    let name_hash = token::hash_token(name);
    app.upload_tmp_dir()
        .join(staging::dav_hash_path(&collection_hash, &name_hash))
}

/// Temporaire de remplacement atomique, supprimé si le PUT échoue.
pub fn put_staging_path(app: &App, label: &str) -> Result<PathBuf, dav_server::fs::FsError> {
    let token = token::random_token().map_err(|_| dav_server::fs::FsError::GeneralFailure)?;
    Ok(app.upload_tmp_dir().join(staging::dav_part(label, &token)))
}
