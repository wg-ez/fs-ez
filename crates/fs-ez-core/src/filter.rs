//! Arbre de filtres, représentation de référence des vues.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::normalize::normalize_tag_name;

/// Profondeur maximale de l'arbre, racine incluse.
const MAX_DEPTH: usize = 32;
/// Nombre total de nœuds maximal.
const MAX_NODES: usize = 256;
/// Tags maximaux dans un ensemble exact : couvre les installations volumineuses.
const MAX_SET_TAGS: usize = 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FilterExpr {
    Tag {
        tag: String,
        #[serde(default)]
        negated: bool,
    },
    /// Fichier dont le tag est le seul tag applique.
    OnlyTag {
        tag: String,
        #[serde(default)]
        negated: bool,
    },
    /// Ensemble de tags exact : le fichier n'a aucun autre tag.
    TagSet {
        tags: Vec<String>,
        #[serde(default)]
        negated: bool,
    },
    And {
        items: Vec<FilterExpr>,
    },
    Or {
        items: Vec<FilterExpr>,
    },
    Not {
        item: Box<FilterExpr>,
    },
}

impl FilterExpr {
    fn tag(name: &str) -> Self {
        Self::Tag {
            tag: name.to_string(),
            negated: false,
        }
    }

    /// Tags simples en ET, `None` pour une liste vide.
    pub fn all_tags(tags: &[String]) -> Option<Self> {
        combine_items(tags.iter().map(|tag| Self::tag(tag)).collect(), false)
    }

    /// Combine des filtres en ET, sans nœud `And` inutile.
    pub fn and_all(filters: impl IntoIterator<Item = Self>) -> Option<Self> {
        combine_items(filters.into_iter().collect(), false)
    }

    /// Retire les références au tag `name`, `None` si l'expression ne filtre plus rien.
    pub fn without_tag(&self, name: &str) -> Option<Self> {
        let target = tag_key(name);
        match self {
            Self::Tag { tag, .. } | Self::OnlyTag { tag, .. } if tag_key(tag) == target => None,
            Self::TagSet { tags, negated } => {
                let kept: Vec<String> = tags
                    .iter()
                    .filter(|tag| tag_key(tag) != target)
                    .cloned()
                    .collect();
                if kept.is_empty() {
                    None
                } else {
                    Some(Self::TagSet {
                        tags: kept,
                        negated: *negated,
                    })
                }
            }
            Self::And { items } => without_items(items, false, &target),
            Self::Or { items } => without_items(items, true, &target),
            Self::Not { item } => item.without_tag(&target).map(|item| Self::Not {
                item: Box::new(item),
            }),
            _ => Some(self.clone()),
        }
    }

    /// Tags portés par tout fichier satisfaisant le filtre, vide si aucun n'est imposé.
    pub fn required_tags(&self) -> Vec<String> {
        let mut tags = Vec::new();
        self.collect_required_tags(&mut tags);
        tags
    }

    /// Union des contraintes positives, une branche `Or` ou `Not` n'impose rien.
    fn collect_required_tags(&self, out: &mut Vec<String>) {
        match self {
            Self::Tag {
                tag,
                negated: false,
            }
            | Self::OnlyTag {
                tag,
                negated: false,
            } => push_required_tag(out, tag),
            Self::TagSet {
                tags,
                negated: false,
            } => {
                for tag in tags {
                    push_required_tag(out, tag);
                }
            }
            Self::And { items } => {
                for item in items {
                    item.collect_required_tags(out);
                }
            }
            Self::Tag { .. }
            | Self::OnlyTag { .. }
            | Self::TagSet { .. }
            | Self::Or { .. }
            | Self::Not { .. } => {}
        }
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        let mut nodes = 0;
        self.validate_node(1, &mut nodes)
    }

    fn validate_node(&self, depth: usize, nodes: &mut usize) -> Result<(), CoreError> {
        if depth > MAX_DEPTH {
            return Err(CoreError::InvalidFilter(format!(
                "depth exceeds {MAX_DEPTH}"
            )));
        }
        *nodes += 1;
        if *nodes > MAX_NODES {
            return Err(CoreError::InvalidFilter(format!(
                "node count exceeds {MAX_NODES}"
            )));
        }
        match self {
            Self::Tag { tag, .. } => validate_tag_name(tag),
            Self::OnlyTag { tag, .. } => validate_tag_name(tag),
            Self::TagSet { tags, .. } => {
                if tags.is_empty() {
                    return Err(CoreError::InvalidFilter(
                        "tag set must have at least one tag".to_string(),
                    ));
                }
                if tags.len() > MAX_SET_TAGS {
                    return Err(CoreError::InvalidFilter(format!(
                        "tag set exceeds {MAX_SET_TAGS} tags"
                    )));
                }
                for tag in tags {
                    validate_tag_name(tag)?;
                }
                Ok(())
            }
            Self::And { items } | Self::Or { items } => {
                if items.is_empty() {
                    return Err(CoreError::InvalidFilter(
                        "and/or must have at least one item".to_string(),
                    ));
                }
                for item in items {
                    item.validate_node(depth + 1, nodes)?;
                }
                Ok(())
            }
            Self::Not { item } => item.validate_node(depth + 1, nodes),
        }
    }
}

fn validate_tag_name(raw: &str) -> Result<(), CoreError> {
    normalize_tag_name(raw)
        .map(|_| ())
        .map_err(|error| CoreError::InvalidFilter(error.to_string()))
}

/// Ajoute un tag absent de la liste, comparaison insensible à la casse et aux espaces.
fn push_required_tag(out: &mut Vec<String>, tag: &str) {
    let key = tag_key(tag);
    if !out.iter().any(|present| tag_key(present) == key) {
        out.push(tag.to_string());
    }
}

fn tag_key(name: &str) -> String {
    normalize_tag_name(name)
        .map(|(_, normalized)| normalized)
        .unwrap_or_else(|_| name.trim().to_lowercase())
}

fn without_items(items: &[FilterExpr], any: bool, target: &str) -> Option<FilterExpr> {
    let kept: Vec<FilterExpr> = items
        .iter()
        .filter_map(|item| item.without_tag(target))
        .collect();
    combine_items(kept, any)
}

fn combine_items(items: Vec<FilterExpr>, any: bool) -> Option<FilterExpr> {
    match items.len() {
        0 => None,
        1 => items.into_iter().next(),
        _ if any => Some(FilterExpr::Or { items }),
        _ => Some(FilterExpr::And { items }),
    }
}

#[cfg(test)]
mod tests;
