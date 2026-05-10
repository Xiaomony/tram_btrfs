use color_eyre::Section;
use color_eyre::eyre::Context;
use file_lock::{FileLock, FileOptions};
use regex::Regex;
use std::fs::create_dir_all;
use std::path::PathBuf;

use crate::core::app_config::AppConfig;
use crate::core::btrfs_objects::group::Group;
use crate::core::btrfs_objects::snapshot_type::SnapshotType;
use crate::core::btrfs_objects::subvolume_snapshot::SubvolumeSnapshot;
use crate::core::error::{AppError, CResult, throw_invalid_index};
use crate::core::utils::*;
use crate::globals;

pub struct BtrfsManager {
    _device: String,
    file_lock: FileLock,
    subvolumes: Vec<PathBuf>,
    app_config: AppConfig,
    /// The application should take a snapshot before recover to a subvolume
    /// and place it at tram_btrfs/broken/
    /// Also, snapshots should be store in this variable
    /// when fail to parse the path, determine the owner group, snapshot type or date and time
    /// of a snapshot under tram_btrfs/
    broken_snapshots: Vec<SubvolumeSnapshot>,
}

impl BtrfsManager {
    /// create an object based on a specified block device
    pub fn new(device: String) -> CResult<Self> {
        check_root_permission()?;
        let file_lock = Self::create_file_lock()?;
        check_is_btrfs_filesystem(&device)?;
        // load config before mount the device
        // to make sure if this fails, the device won't be mounted
        let app_config = AppConfig::load_config()?;

        mount_to_default_point(&device).suggestion(format!(
            "Please check if `{}` has been unmounted. Please unmount it manually if not.",
            globals::MOUNT_POINT
        ))?;
        // create a temporary object here to make sure
        // if subsequent operations fail, drop() will be executed
        // to release file lock and unmount the device
        let mut new_obj = Self {
            _device: device,
            file_lock,
            subvolumes: Vec::new(),
            app_config,
            broken_snapshots: Vec::new(),
        };
        // create a directory to store snapshots under the mounted device
        create_dir_all(&*globals::SNAPSHOT_GROUP_DIR_PATH)?;
        // create a directory to store broken subvolumes
        // (when recovering a snapshot to a subvolume, it will be regarded as a broken one)
        create_dir_all(globals::TOP_DIR_PATH.join(globals::BROKEN_DIR_NAME))?;
        new_obj.get_subvolumes_and_snapshots()?;

        Ok(new_obj)
    }

    /// create an object based on the partion at which the current system root is located
    pub fn new_default_partion() -> CResult<Self> {
        Self::new(get_crr_os_device()?)
    }

    fn create_file_lock() -> CResult<FileLock> {
        let options = FileOptions::new().write(true).create(true);
        match FileLock::lock(globals::FILE_LOCK, false, options) {
            Ok(file_lock) => Ok(file_lock),
            Err(e) => Err(AppError::MultipleInstance(e).into()),
        }
    }

    /// returns if the subvolume layout satisfies `@` and `@home`
    fn get_subvolumes_and_snapshots(&mut self) -> CResult<()> {
        let btrfs_output = exec_command("btrfs", ["subvolume", "list", "-o", globals::MOUNT_POINT])
            .wrap_err("Error occurs when getting the subvolume list")?;
        let r = Regex::new(r"(?m)^ID.*top level 5 path (.+)$")?;

        let mut layout_at = false; // is there a `@` subvolume
        let mut layout_at_home = false; // is there a `@home` subvolume

        // store snapshot paths, snapshots must be parsed after subvolumes is fully added
        let mut snapshot_raw_pathes = Vec::new();
        for (_, [raw_path]) in r.captures_iter(&btrfs_output).map(|c| c.extract()) {
            if raw_path.starts_with(globals::TOP_DIRECTORY_NAME) {
                snapshot_raw_pathes.push(raw_path);
            } else {
                layout_at = layout_at || raw_path == "@";
                layout_at_home = layout_at_home || raw_path == "@home";
                self.subvolumes.push(PathBuf::from(raw_path));
            }
        }
        // verify subvolumes in config
        let mut removed_config_subvols = Vec::new();
        self.app_config
            .groups
            .iter_mut()
            .for_each(|x| x.verify_subvolumes(&(self.subvolumes), &mut removed_config_subvols));
        if !removed_config_subvols.is_empty() {
            return Err(AppError::InvalidConfig)
                .wrap_err_with(|| {
                    format!("Non-existent subvolumes occur in config:\n{removed_config_subvols:?}")
                })
                .suggestion("The invalid subvolume has been removed, please restart.");
        }

        // add default group for new user whose subvolume layout satisfies `@` and `@home`
        // do this before parsing snapshots
        if layout_at && layout_at_home && self.app_config.is_first_time_launch() {
            self.app_config
                .add_new_group("default", vec!["@".into(), "@home".into()])?;
        }

        for raw_path in snapshot_raw_pathes {
            self.parse_snapshot_path(raw_path);
        }
        Ok(())
    }

    fn parse_snapshot_path(&mut self, raw_path: &str) {
        let path_parts: Vec<&str> = raw_path.split("/").skip(1).collect();

        // check if the snapshot is under tram_btrfs/snapshot_groups
        if let Some(&globals::SNAPSHOT_GROUPS_DIR_NAME) = path_parts.first()
            // get group name
            && let Some(&group_name) = path_parts.get(1)
            // find the group object it belongs to
            && let Some(group) = self.app_config.groups.iter_mut().find(|x| *x == group_name)
            // get snapshot_types, datetime, name
            && let Some(&snapshot_type) = path_parts.get(2)
            && let Some(&datetime) = path_parts.get(3)
            // get related subvolume path
            && let related_subvolume_path = path_parts[4..].join("/")
            && let Some(related_subvolume) = self.subvolumes.iter().find(|&x| x.eq(&related_subvolume_path))
        {
            if !group.add_snapshot(raw_path, snapshot_type, datetime, related_subvolume.clone()) {
                // regard it as a broken snapshot with related subvolume
                self.broken_snapshots.push(SubvolumeSnapshot::new(
                    raw_path,
                    Some(related_subvolume.clone()),
                ));
            }
        } else {
            // regard it as a broken snapshot without related subvolume
            self.broken_snapshots
                .push(SubvolumeSnapshot::new(raw_path, None));
        }
    }

    pub fn create_snapshot(&mut self, index: usize, snapshot_type: SnapshotType) -> CResult<()> {
        let Some(group) = self.app_config.groups.get_mut(index) else {
            return throw_invalid_index(index, "creating snapshot");
        };
        group.create_snapshot(snapshot_type)
    }

    pub fn delete_snapshot(&mut self, group_index: usize, snapshot_index: usize) -> CResult<()> {
        let Some(group) = self.app_config.groups.get_mut(group_index) else {
            return throw_invalid_index(group_index, "deleting snapshot(invalid group index)");
        };
        group.delete_snapshot(snapshot_index)
    }

    #[inline]
    pub fn rename_group<T: Into<String>>(&mut self, index: usize, new_name: T) -> CResult<()> {
        self.app_config.rename_group(index, new_name)
    }

    pub fn add_subvol_to_group(&mut self, group_index: usize, subvol_index: usize) -> CResult<()> {
        let Some(subvol) = self.subvolumes.get(subvol_index) else {
            return throw_invalid_index(
                subvol_index,
                "add subvolume to group(invalid subvolume index)",
            );
        };
        let Some(group) = self.app_config.groups.get_mut(group_index) else {
            return throw_invalid_index(group_index, "add subvolume to group(invalid group index)");
        };
        group.add_subvolume(subvol);
        Ok(())
    }

    #[inline]
    pub fn get_groups(&self) -> &Vec<Group> {
        self.app_config.groups.as_ref()
    }

    #[inline]
    pub fn get_groups_mut(&mut self) -> &mut Vec<Group> {
        &mut self.app_config.groups
    }

    #[inline]
    pub fn get_subvolumes(&self) -> &[PathBuf] {
        &self.subvolumes
    }
}

impl Drop for BtrfsManager {
    fn drop(&mut self) {
        let _ = self.file_lock.unlock();
        let _ = umount_from_default_point();
    }
}
