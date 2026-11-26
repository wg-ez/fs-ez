-- Schema v1 fs-ez, timestamps TEXT ISO-8601 UTC, booleens INTEGER 0/1.

CREATE TABLE file (
  id INTEGER PRIMARY KEY,
  path BLOB NOT NULL,
  path_display TEXT NOT NULL,
  name TEXT NOT NULL,
  dir TEXT NOT NULL,
  size INTEGER NOT NULL,
  mtime TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('file','directory','symlink')),
  fingerprint TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX file_path_unique ON file(path);
CREATE UNIQUE INDEX file_fingerprint_unique ON file(fingerprint)
  WHERE fingerprint IS NOT NULL AND fingerprint <> '';
CREATE INDEX file_dir_idx ON file(dir);
CREATE INDEX file_name_id_idx ON file(name COLLATE BINARY, id);
CREATE INDEX file_size_id_idx ON file(size, id);
CREATE INDEX file_mtime_id_idx ON file(mtime, id);

CREATE TABLE tag (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  name_norm TEXT NOT NULL,
  color TEXT NOT NULL DEFAULT 'neutral',
  color_custom TEXT,
  position INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX tag_name_norm_unique ON tag(name_norm);
CREATE INDEX tag_position_idx ON tag(position, name_norm);

CREATE TABLE file_tag (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL REFERENCES file(id) ON DELETE CASCADE,
  tag_id INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  explicit INTEGER NOT NULL DEFAULT 1 CHECK (explicit IN (0,1)),
  created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX file_tag_unique ON file_tag(file_id, tag_id);
CREATE INDEX file_tag_tag_idx ON file_tag(tag_id);
CREATE INDEX file_tag_file_idx ON file_tag(file_id);
CREATE INDEX file_tag_created_idx ON file_tag(created_at);

CREATE TABLE tag_group (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  color TEXT,
  position INTEGER NOT NULL DEFAULT 0,
  collapsed INTEGER NOT NULL DEFAULT 0 CHECK (collapsed IN (0,1))
);
CREATE UNIQUE INDEX tag_group_name_unique ON tag_group(name);

CREATE TABLE tag_group_member (
  group_id INTEGER NOT NULL REFERENCES tag_group(id) ON DELETE CASCADE,
  tag_id INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  position INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (group_id, tag_id)
);
CREATE UNIQUE INDEX tag_group_member_tag_unique ON tag_group_member(tag_id);

CREATE TABLE saved_view (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  filter_json TEXT NOT NULL,
  icon TEXT,
  color TEXT,
  position INTEGER NOT NULL DEFAULT 0,
  sort TEXT NOT NULL DEFAULT 'id:asc',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX saved_view_name_unique ON saved_view(name);
CREATE INDEX saved_view_position_idx ON saved_view(position, id);

CREATE TABLE view_group (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  color TEXT,
  position INTEGER NOT NULL DEFAULT 0,
  collapsed INTEGER NOT NULL DEFAULT 0 CHECK (collapsed IN (0,1))
);
CREATE UNIQUE INDEX view_group_name_unique ON view_group(name);

CREATE TABLE view_group_member (
  group_id INTEGER NOT NULL REFERENCES view_group(id) ON DELETE CASCADE,
  view_id INTEGER NOT NULL REFERENCES saved_view(id) ON DELETE CASCADE,
  position INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (group_id, view_id)
);
CREATE INDEX view_group_member_group_idx ON view_group_member(group_id);
CREATE UNIQUE INDEX view_group_member_view_unique ON view_group_member(view_id);

CREATE TABLE virtual_folder (
  id INTEGER PRIMARY KEY,
  parent_id INTEGER REFERENCES virtual_folder(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  position INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX virtual_folder_sibling_unique
  ON virtual_folder(COALESCE(parent_id, 0), name);
CREATE INDEX virtual_folder_parent_idx ON virtual_folder(parent_id, position, id);

CREATE TABLE virtual_folder_file (
  file_id INTEGER PRIMARY KEY REFERENCES file(id) ON DELETE CASCADE,
  folder_id INTEGER NOT NULL REFERENCES virtual_folder(id) ON DELETE CASCADE
);
CREATE INDEX virtual_folder_file_folder_idx ON virtual_folder_file(folder_id);

CREATE TABLE setting (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE user (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  username TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  credential_generation INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX user_username_unique ON user(username);

CREATE TABLE session (
  id INTEGER PRIMARY KEY,
  token_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  credential_generation INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX session_token_hash_unique ON session(token_hash);
CREATE INDEX session_expires_idx ON session(expires_at);

CREATE TABLE api_token (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT,
  last_used_at TEXT
);
CREATE UNIQUE INDEX api_token_name_unique ON api_token(name);
CREATE UNIQUE INDEX api_token_hash_unique ON api_token(token_hash);
