use tracing::instrument;

use crate::core::error::{CResult, throw_bug};
use crate::core::utils::{exec_command, mount_point_join};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
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

    /// restore the subvolume from a snapshot
    /// and put the subvolume to the given `broken_snapshot_dir`
    /// return `false` if no subvolume related
    #[instrument]
    pub fn restore(&self, broken_snapshot_dir: impl AsRef<Path> + std::fmt::Debug) -> CResult<()> {
        if let Some(ref subvol) = self.related_subvolume {
            // move the subvolume to the broken area
            let subvol_path = mount_point_join(subvol);
            let subvol_path_string = subvol_path.to_string_lossy().to_string();
            let move_to_path = broken_snapshot_dir.as_ref().join(subvol);
            std::fs::create_dir_all(move_to_path.parent().unwrap())?;
            std::fs::rename(subvol_path, move_to_path)?;

            // 'snapshot' the snapshot to the path of original subvolume
            let snapshot_path = self.get_fullpath_string();
            exec_command(
                "btrfs",
                [
                    "subvolume".to_string(),
                    "snapshot".to_string(),
                    snapshot_path,
                    subvol_path_string,
                ],
            )?;
            Ok(())
        } else {
            throw_bug(format!(
                "No related subvolume when restoring from snapshot: {}",
                self.path.to_string_lossy()
            ))
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

    #[inline]
    pub fn has_related_subvol(&self) -> bool {
        self.related_subvolume.is_some()
    }
}
