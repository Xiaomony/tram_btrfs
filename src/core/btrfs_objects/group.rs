use crate::core::btrfs_objects::group_snapshot::GroupSnapshot;
use crate::core::btrfs_objects::snapshot_type::SnapshotType;
use crate::core::error::{AppError, CResult, throw_invalid_index};
use crate::core::utils::{exec_command, get_current_date_time, mount_point_join};
use crate::globals;
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct Group {
    group_name: String,
    // subvolume pathes loaded from configs
    subvolumes: Vec<PathBuf>,
    #[serde(skip, default)]
    snapshots: Vec<GroupSnapshot>,
}

impl Group {
    pub fn new(group_name: String, subvolumes: Vec<PathBuf>) -> Self {
        Self {
            group_name,
            subvolumes,
            snapshots: Vec::new(),
        }
    }

    #[inline]
    pub fn get_name(&self) -> &str {
        self.group_name.as_str()
    }

    /**
    related_subvolume_path: path of the related subvolume
    e.g. if subvolume path is `archlinux/@home`
    snapshot path should be `tram_btrfs/snapshot_groups/default/manually/2026-04-16_21:26:00/archlinux/@home`
    return: if the snapshot is successfully added
    */
    pub fn add_snapshot<T: AsRef<Path>>(
        &mut self,
        raw_path: T,
        snapshot_type: &str,
        datetime: &str,
        related_subvolume: PathBuf,
    ) -> bool {
        if let Some(snapshot_type) = SnapshotType::get_type(snapshot_type)
            && let Some((date, time)) = datetime.split_once('_')
        {
            // find if the snapshot group has existed
            if let Some(group_snapshot) = self
                .snapshots
                .iter_mut()
                .find(|x| *x == &(date, time, &snapshot_type))
            {
                group_snapshot.add_snapshot(raw_path, related_subvolume);
            } else {
                let mut new_group = GroupSnapshot::new(date, time, snapshot_type);
                new_group.add_snapshot(raw_path, related_subvolume);
                self.snapshots.push(new_group);
            }
            true
        } else {
            false
        }
    }

    /// this function guarantee to only cause `ConfigErrSubvolumeNotExist` error
    /// removed_subvolume: a Vec passed in to store those invalid and removed subvolumes
    pub fn verify_subvolumes(
        &mut self,
        available_subvolumes: &[PathBuf],
        removed_subvolume: &mut Vec<PathBuf>,
    ) {
        let mut i = 0;
        while i < self.subvolumes.len() {
            let crr = self.subvolumes.get(i).unwrap();
            if available_subvolumes.contains(crr) {
                i += 1;
            } else {
                // WARN: here uses `swap_remove`, which won't preserve the original order of subvolumes
                removed_subvolume.push(self.subvolumes.swap_remove(i));
            }
        }
    }

    pub fn create_snapshot(&mut self, snapshot_type: SnapshotType) -> CResult<()> {
        let (date, time) = get_current_date_time();
        let group_snapshot_fullpath = globals::SNAPSHOT_GROUP_DIR_PATH
            .join(&self.group_name)
            .join(snapshot_type.as_ref())
            .join(format!("{date}_{time}"));
        let mut new_snapshot = GroupSnapshot::new(date, time, snapshot_type);
        for subvol in self.subvolumes.iter() {
            let subvol_fullpath = mount_point_join(subvol);
            let subvol_snapshot_fullpath = group_snapshot_fullpath.join(subvol);
            if let Some(p) = subvol_snapshot_fullpath.parent() {
                create_dir_all(p)?;
                exec_command(
                    "btrfs",
                    [
                        "subvolume",
                        "snapshot",
                        "-r",
                        subvol_fullpath.to_string_lossy().as_ref(),
                        subvol_snapshot_fullpath.to_string_lossy().as_ref(),
                    ],
                )?;
                new_snapshot.add_snapshot(&subvol_snapshot_fullpath, subvol);
            } else {
                return Err(AppError::Bug(format!(
                    "No parent for directory {}",
                    subvol_snapshot_fullpath.to_string_lossy()
                ))
                .into());
            }
        }
        self.snapshots.push(new_snapshot);
        Ok(())
    }

    pub fn rename_group<T: Into<String>>(&mut self, new_name: T) -> CResult<()> {
        let new_name = new_name.into();
        if self.group_name == new_name {
            return Ok(());
        }
        let new_group_path = globals::SNAPSHOT_GROUP_DIR_PATH.join(&new_name);
        let old_name = std::mem::replace(&mut self.group_name, new_name);
        let old_group_path = globals::SNAPSHOT_GROUP_DIR_PATH.join(old_name);

        std::fs::rename(old_group_path, new_group_path)?;

        Ok(())
    }

    pub fn delete_snapshot(&mut self, index: usize) -> CResult<()> {
        if index >= self.snapshots.len() {
            return throw_invalid_index(index, "deleting snapshot(invalid snapshot index)");
        }
        let snapshot = self.snapshots.remove(index);
        snapshot.delete(&self.group_name)
    }

    #[inline]
    /// add a subvolume to this group, `subvol_path` should be valid
    pub fn add_subvolume<T: Into<PathBuf>>(&mut self, subvol_path: T) {
        self.subvolumes.push(subvol_path.into())
    }

    #[inline]
    pub fn get_subvolumes(&self) -> &[PathBuf] {
        &self.subvolumes
    }

    #[inline]
    pub fn get_snapshots(&self) -> &Vec<GroupSnapshot> {
        &self.snapshots
    }
}

impl PartialEq<str> for Group {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.group_name == other
    }
}
