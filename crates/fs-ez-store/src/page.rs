//! Pagination par offset.

/// `next_offset = None` = fin de liste. Offset de la page suivante sinon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_offset: Option<usize>,
}

#[cfg(test)]
mod tests;
