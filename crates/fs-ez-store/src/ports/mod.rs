//! Ports de persistance : traits synchrones consommés par `fs-ez-app`.

pub mod auth;
pub mod backup;
pub mod files;
pub mod groups;
pub mod settings;
pub mod tags;
pub mod view_groups;
pub mod views;
pub mod virtual_folders;

pub use auth::AuthStore;
pub use backup::BackupStore;
pub use files::FileStore;
pub use groups::GroupStore;
pub use settings::SettingsStore;
pub use tags::TagStore;
pub use view_groups::ViewGroupStore;
pub use views::ViewStore;
pub use virtual_folders::VirtualFolderStore;

pub trait Store:
    FileStore
    + TagStore
    + GroupStore
    + ViewStore
    + ViewGroupStore
    + VirtualFolderStore
    + AuthStore
    + SettingsStore
    + BackupStore
    + Send
    + Sync
{
}

impl<T> Store for T where
    T: FileStore
        + TagStore
        + GroupStore
        + ViewStore
        + ViewGroupStore
        + VirtualFolderStore
        + AuthStore
        + SettingsStore
        + BackupStore
        + Send
        + Sync
{
}

#[cfg(test)]
mod tests;
