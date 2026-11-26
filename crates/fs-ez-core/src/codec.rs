//! Jetons TEXT stables des enums, partagés par la CLI, le store et le web.

use crate::view::{Sort, SortDir, SortField};

/// Génère `as_*`/`from_*` symétriques depuis une liste `Variante => "jeton"`, `ref` = `&self`, `val` = `self`.
macro_rules! token_enum {
    (ref $ty:ident, $as_fn:ident, $from_fn:ident, $($variant:ident => $token:literal),+ $(,)?) => {
        impl $ty {
            pub fn $as_fn(&self) -> &'static str {
                match self {
                    $(Self::$variant => $token),+
                }
            }

            pub fn $from_fn(token: &str) -> Option<Self> {
                match token {
                    $($token => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
    (val $ty:ident, $as_fn:ident, $from_fn:ident, $($variant:ident => $token:literal),+ $(,)?) => {
        impl $ty {
            pub fn $as_fn(self) -> &'static str {
                match self {
                    $(Self::$variant => $token),+
                }
            }

            pub fn $from_fn(token: &str) -> Option<Self> {
                match token {
                    $($token => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use token_enum;

token_enum!(val SortField, as_token, from_token,
    Id => "id",
    Name => "name",
    Size => "size",
    Mtime => "mtime",
);

token_enum!(val SortDir, as_token, from_token,
    Asc => "asc",
    Desc => "desc",
);

impl Sort {
    /// Forme `champ:direction`.
    pub fn encode(&self) -> String {
        format!("{}:{}", self.field.as_token(), self.dir.as_token())
    }

    /// Forme `champ:direction` stricte.
    pub fn decode(raw: &str) -> Option<Self> {
        let (field, dir) = raw.split_once(':')?;
        Some(Self {
            field: SortField::from_token(field)?,
            dir: SortDir::from_token(dir)?,
        })
    }

    /// Forme `champ` ou `champ:direction`, direction par défaut si absente.
    pub fn decode_with_default_dir(raw: &str, default_dir: SortDir) -> Option<Self> {
        let (field, dir) = raw.split_once(':').unwrap_or((raw, default_dir.as_token()));
        Some(Self {
            field: SortField::from_token(field)?,
            dir: SortDir::from_token(dir)?,
        })
    }
}
