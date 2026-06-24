use crate::core::utils::mount_point_join;
use std::path::{Path, PathBuf};

#[derive(Debug)]
/// Snapshots of a single subvolume
pub struct SubvolumeSnapshot {
    path: PathBuf,
    related_subvolume: Option<PathBuf>,
}

impl SubvolumeSnapshot {
    pub fn new<T: Into<PathBuf>>(path: T, related_subvolume: Option<PathBuf>) -> Self {
        Self {
            path: path.into(),
            related_subvolume,
        }
    }

    #[inline]
    pub fn get_fullpath(&self) -> PathBuf {
        mount_point_join(&self.path)
    }

    #[inline]
    pub fn get_fullpath_string(&self) -> String {
        self.get_fullpath().to_string_lossy().into()
    }

    #[inline]
    /// returns None if there's no subvolume related to this snapshot
    /// or the subvolume path is not a valid UTF-8 string
    pub fn get_relate_subvolume_path(&self) -> Option<&str> {
        self.related_subvolume.as_ref().and_then(|x| x.to_str())
    }

    #[inline]
    pub fn get_path(&self) -> &Path {
        &self.path
    }
}
