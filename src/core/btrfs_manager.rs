use color_eyre::Section;
use nix::fcntl::{Flock, FlockArg};
use regex::Regex;
use std::cell::RefCell;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::instrument;

use crate::core::app_config::{AppConfig, AutoSnapshotSchedule};
use crate::core::btrfs_objects::group::Group;
use crate::core::btrfs_objects::subvolume_snapshot::SubvolumeSnapshot;
use crate::core::error::{AppError, CResult, throw_invalid_index};
use crate::core::utils::{self, *};
use crate::globals;

#[derive(Debug)]
pub struct BtrfsManager {
    /// The file lock will release automatically on drop
    /// So it's never read
    _file_lock: Flock<std::fs::File>,
    subvolumes: Vec<PathBuf>,
    app_config: AppConfig,
    device: String,
    /// The application should take a snapshot before restore to a subvolume
    /// and place it at tram_btrfs/broken/
    /// Also, snapshots should be store in this variable
    /// when fail to parse the path, determine the owner group, snapshot type or date and time
    /// of a snapshot under tram_btrfs/
    broken_snapshots: Vec<SubvolumeSnapshot>,
}

impl BtrfsManager {
    #[instrument]
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
            _file_lock: file_lock,
            subvolumes: Vec::new(),
            app_config,
            device,
            broken_snapshots: Vec::new(),
        };
        // create a directory to store snapshots under the mounted device
        create_dir_all(&*globals::SNAPSHOT_GROUP_DIR_PATH)?;
        // create a directory to store broken subvolumes
        // (when restoring a snapshot to a subvolume, it will be regarded as a broken one)
        create_dir_all(globals::TOP_DIR_PATH.join(globals::BROKEN_DIR_NAME))?;
        new_obj.get_subvolumes_and_snapshots()?;

        Ok(new_obj)
    }

    /// create an object based on the partion at which the current system root is located
    pub fn new_default_partion() -> CResult<Self> {
        Self::new(get_crr_os_device()?)
    }

    #[instrument]
    fn create_file_lock() -> CResult<Flock<std::fs::File>> {
        let file = std::fs::File::create(globals::FILE_LOCK)?;
        match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
            Ok(lock) => Ok(lock),
            Err((_, error)) => Err(error)
                .warning("Fail to create file lock.")
                .suggestion("Another Tram TUI instance is running, please close it!"),
        }
    }

    #[instrument]
    /// returns if the subvolume layout satisfies `@` and `@home`
    fn get_subvolumes_and_snapshots(&mut self) -> CResult<()> {
        let btrfs_output = exec_command("btrfs", ["subvolume", "list", "-o", globals::MOUNT_POINT])
            .warning("Error occurs when getting the subvolume list")?;
        let r = Regex::new(r"(?m)^ID.*top level 5 path (.+)$")
            .warning("Fail to compile regex expression")
            .suggestion("It's a bug report it pls.")?;

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
                .with_warning(|| {
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
            && let Some(slice) = path_parts.get(4..)
            && let related_subvolume_path = slice.join("/")
            && let Some(related_subvolume) = self.subvolumes.iter().find(|&x| x.eq(&related_subvolume_path))
        {
            if !group.add_snapshot(raw_path, snapshot_type, datetime, related_subvolume.clone()) {
                // regard it as a broken snapshot with related subvolume
                self.broken_snapshots.push(SubvolumeSnapshot::new(
                    raw_path,
                    Some(related_subvolume.clone()),
                ));
            }
        } else if let Some(&globals::BROKEN_DIR_NAME) = path_parts.first()
            && let Some(slice) = path_parts.get(2..)
            && let related_subvolume_path = slice.join("/")
            && let Some(related_subvolume) = self
                .subvolumes
                .iter()
                .find(|&x| x.eq(&related_subvolume_path))
        {
            self.broken_snapshots.push(SubvolumeSnapshot::new(
                raw_path,
                Some(related_subvolume.clone()),
            ));
        } else {
            // regard it as a broken snapshot without related subvolume
            self.broken_snapshots
                .push(SubvolumeSnapshot::new(raw_path, None));
        }
    }

    /// 1. Reload and reparse the snapshots after renaming.
    ///    Otherwise the GroupSnapshots contain old snapshot pathes and will panick when deleting them.
    /// 2. Reload and reparse the snapshots after restoring.
    ///    Otherwise the broken snapshots won't show properly.
    pub fn reload_snapshots(&mut self) -> CResult<()> {
        self.subvolumes.clear();
        self.broken_snapshots.clear();
        self.app_config
            .groups
            .iter_mut()
            .for_each(|x| x.clear_snapshots());
        self.get_subvolumes_and_snapshots()
    }

    #[inline]
    pub fn add_group(&mut self, group_name: impl Into<String> + std::fmt::Debug) -> CResult<bool> {
        self.app_config.add_new_group(group_name, Vec::new())
    }

    #[inline]
    #[instrument]
    pub fn rename_group(
        &mut self,
        index: usize,
        new_name: impl Into<String> + std::fmt::Debug,
    ) -> CResult<bool> {
        let succeed = self.app_config.rename_group(index, new_name)?;
        if succeed {
            self.reload_snapshots()?;
        }
        Ok(succeed)
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
        self.app_config.write_config()?;
        Ok(())
    }

    /// subvol_index here is the index of the subvolume inside `subvolumes: Vec<PathBuf>` of the group
    pub fn remove_subvol_from_group(
        &mut self,
        group_index: usize,
        subvol_index: usize,
    ) -> CResult<()> {
        let Some(group) = self.app_config.groups.get_mut(group_index) else {
            return throw_invalid_index(
                group_index,
                "remove subvolume to group(invalid group index)",
            );
        };
        group.remove_subvolume(subvol_index);
        self.app_config.write_config()?;
        Ok(())
    }

    #[instrument]
    /// Delete a broken snapshot and remove the directory if there're no snapshots under it
    pub fn delete_broken_snapshot(&mut self, index: usize) -> CResult<()> {
        if index >= self.broken_snapshots.len() {
            return throw_invalid_index(index, "deleting broken snapshot");
        }

        let full_path = self.broken_snapshots.remove(index).get_fullpath();
        exec_command(
            "btrfs",
            [
                "subvolume".to_string(),
                "delete".to_string(),
                full_path.to_string_lossy().to_string(),
            ],
        )?;

        for x in std::fs::read_dir(&*globals::BROKEN_SNAPSHOTS_DIR_PATH)? {
            let x = x?;
            if x.file_type()?.is_dir()
                && let x_path = x.path()
                && self
                    .broken_snapshots
                    .iter()
                    .all(|y| !y.get_fullpath().starts_with(&x_path))
            {
                std::fs::remove_dir_all(x_path)?;
            }
        }

        Ok(())
    }

    #[instrument]
    pub fn restore_broken_snapshot(&mut self, index: usize) -> CResult<()> {
        let Some(broken_snapshot) = self.broken_snapshots.get(index) else {
            return throw_invalid_index(index, "restoring broken snapshot");
        };

        let broken_snapshot_dir = utils::gen_broken_dir()?;
        broken_snapshot.restore(broken_snapshot_dir)?;
        self.reload_snapshots()
    }

    #[instrument]
    #[inline]
    pub fn delete_group(&mut self, index: usize) -> CResult<()> {
        self.app_config.delete_group(index)
    }

    #[inline]
    pub fn get_groups(&self) -> &Vec<Group> {
        self.app_config.groups.as_ref()
    }

    #[inline]
    pub fn get_mut_groups(&mut self) -> &mut Vec<Group> {
        self.app_config.groups.as_mut()
    }

    #[inline]
    pub fn get_subvolumes(&self) -> &[PathBuf] {
        &self.subvolumes
    }

    #[inline]
    pub fn get_broken_snapshots(&self) -> &Vec<SubvolumeSnapshot> {
        self.broken_snapshots.as_ref()
    }

    #[inline]
    pub fn get_sel_group(&self) -> Rc<RefCell<Option<usize>>> {
        self.app_config.get_sel_group()
    }

    #[inline]
    pub fn get_schedule(&self) -> AutoSnapshotSchedule {
        self.app_config.get_schedule()
    }

    #[inline]
    pub fn change_schedule(&mut self, new_schedule: AutoSnapshotSchedule) -> CResult<()> {
        self.app_config.change_schedule(new_schedule)
    }

    #[inline]
    pub fn is_first_time_launch(&self) -> bool {
        self.app_config.is_first_time_launch()
    }

    #[inline]
    pub fn check_schedule(&mut self, is_boot: bool) -> CResult<()> {
        self.app_config.check_schedule(is_boot)
    }

    #[inline]
    pub fn get_device(&self) -> &String {
        &self.device
    }
}

impl Drop for BtrfsManager {
    fn drop(&mut self) {
        let _ = umount_from_default_point();
    }
}
