//! Flux paresseux des entrées DAV, pagination SQL de 500 éléments par collection.

use std::collections::VecDeque;
use std::sync::Arc;

use dav_server::fs::{DavDirEntry, FsError, FsStream};
use fs_ez_app::App;
use fs_ez_core::virtual_file_name;
use futures_util::{StreamExt, stream};

use super::PAGE_SIZE;
use super::meta::{Entry, FileMeta};
use crate::backend::blocking_join;
use crate::dav::logging;
use crate::dav::path::{self, PageQuery};

struct PageState {
    app: Arc<App>,
    source: PageQuery,
    offset: usize,
    done: bool,
    buffer: VecDeque<Entry>,
}

pub(super) fn boxed_entries(entries: Vec<Entry>) -> FsStream<Box<dyn DavDirEntry>> {
    prepend_entries(entries, Box::pin(stream::empty()))
}

/// Une entrée virtuelle devant un flux paginé, pour mêler un dossier aux fichiers directs.
pub(super) fn chain_entry(
    entry: Entry,
    tail: FsStream<Box<dyn DavDirEntry>>,
) -> FsStream<Box<dyn DavDirEntry>> {
    prepend_entries(vec![entry], tail)
}

/// Plusieurs entrées virtuelles devant un flux paginé, ordre conservé.
pub(super) fn prepend_entries(
    entries: Vec<Entry>,
    tail: FsStream<Box<dyn DavDirEntry>>,
) -> FsStream<Box<dyn DavDirEntry>> {
    Box::pin(
        stream::iter(
            entries
                .into_iter()
                .map(|entry| Ok(Box::new(entry) as Box<dyn DavDirEntry>)),
        )
        .chain(tail),
    )
}

pub(super) fn page_stream(app: Arc<App>, source: PageQuery) -> FsStream<Box<dyn DavDirEntry>> {
    Box::pin(stream::unfold(
        PageState {
            app,
            source,
            offset: 0,
            done: false,
            buffer: VecDeque::new(),
        },
        |mut state| async move {
            loop {
                if let Some(entry) = state.buffer.pop_front() {
                    return Some((Ok(Box::new(entry) as Box<dyn DavDirEntry>), state));
                }
                if state.done {
                    return None;
                }
                let app = Arc::clone(&state.app);
                let source = state.source.clone();
                let offset = state.offset;
                let page = blocking_join(app, move |app| source.page(app, offset, PAGE_SIZE)).await;
                match page {
                    Ok(Ok(page)) => {
                        match page.next_offset {
                            Some(next) => state.offset = next,
                            None => state.done = true,
                        }
                        state.buffer.extend(page.items.iter().map(|item| Entry {
                            name: virtual_file_name(&item.file).into_bytes(),
                            meta: FileMeta::file(&item.file),
                        }));
                    }
                    Ok(Err(error)) => {
                        state.done = true;
                        return Some((Err(path::fs_error(error)), state));
                    }
                    Err(error) => {
                        state.done = true;
                        tracing::warn!(
                            target: logging::TARGET,
                            %error,
                            "dav read_dir page failed"
                        );
                        return Some((Err(FsError::GeneralFailure), state));
                    }
                }
            }
        },
    ))
}
