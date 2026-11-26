//! Assets statiques pre-gzippes au build ; `PRECOMPRESSED_ASSETS` vient du manifeste genere.

include!(concat!(env!("OUT_DIR"), "/static_precompressed.rs"));

/// Empreinte du contenu statique : change a chaque deploiement pour casser le cache navigateur.
pub const ASSET_VERSION: &str = env!("ASSET_VERSION");
