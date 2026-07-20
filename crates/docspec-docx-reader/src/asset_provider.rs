//! Per-asset handle for DOCX embedded images.

use std::{
    borrow::Cow,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use docspec_core::AssetHandle;

use crate::{content_types::ContentTypes, package::ReadSeek};

/// Resolves a single embedded asset from a shared DOCX ZIP archive.
pub struct DocxAssetHandle {
    archive: Arc<Mutex<zip::ZipArchive<Box<dyn ReadSeek + 'static>>>>,
    content_types: Arc<ContentTypes>,
    asset_id: String,
}

impl core::fmt::Debug for DocxAssetHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DocxAssetHandle")
            .field("asset_id", &self.asset_id)
            .finish_non_exhaustive()
    }
}

impl DocxAssetHandle {
    pub(crate) fn new(
        archive: Arc<Mutex<zip::ZipArchive<Box<dyn ReadSeek + 'static>>>>,
        content_types: Arc<ContentTypes>,
        asset_id: String,
    ) -> Self {
        Self {
            archive,
            content_types,
            asset_id,
        }
    }
}

impl AssetHandle for DocxAssetHandle {
    fn content_type(&self) -> Option<Cow<'_, str>> {
        self.asset_id
            .strip_prefix("zip://")
            .and_then(|p| self.content_types.lookup(p))
            .map(Cow::Borrowed)
    }

    fn stream_to(&self, writer: &mut dyn Write) -> io::Result<u64> {
        let path = self
            .asset_id
            .strip_prefix("zip://")
            .ok_or_else(|| io::Error::other(format!("not a zip:// asset_id: {}", self.asset_id)))?;
        let mut archive = self
            .archive
            .lock()
            .map_err(|_e| io::Error::other("docx archive mutex poisoned"))?;
        let mut entry = archive
            .by_name(path)
            .map_err(|e| io::Error::other(format!("zip by_name({path}): {e}")))?;
        io::copy(&mut entry, writer)
    }

    fn asset_id(&self) -> &str {
        &self.asset_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_send_sync_static() {
        fn assert_send_sync_static<T>()
        where
            T: Send + Sync + 'static,
        {
        }
        assert_send_sync_static::<DocxAssetHandle>();
        assert_send_sync_static::<Arc<dyn AssetHandle>>();
    }
}
