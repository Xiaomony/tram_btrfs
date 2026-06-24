use crate::core::btrfs_objects::group::Group;
use crate::core::error::{CResult, throw_invalid_index};
use crate::globals;
use color_eyre::Section;
use serde::{Deserialize, Serialize};
use std::fs::{self, create_dir_all};
use std::path::PathBuf;
use tracing::instrument;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct AutoSnapshotSchedule {
    pub daily_max: usize,
    pub weekly_max: usize,
    pub monthly_max: usize,
    pub boot_max: usize,
}
impl AutoSnapshotSchedule {
    #[inline]
    pub fn new_default() -> Self {
        Self {
            daily_max: 0,
            monthly_max: 0,
            weekly_max: 0,
            boot_max: 0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    schedule: AutoSnapshotSchedule,
    pub groups: Vec<Group>,
    #[serde(skip, default)]
    first_time_launch: bool,
}

impl AppConfig {
    #[instrument]
    pub fn load_config() -> CResult<AppConfig> {
        create_dir_all(&*globals::CONFIG_DIR)
            .warning("Fail to create or open configuration directory.")?;
        let config_file_path = &*globals::MAIN_CONFIG_FILE_PATH;
        if fs::exists(config_file_path)? {
            let s = std::fs::read_to_string(config_file_path)
                .warning("Fail to read configuration file.")?;
            Ok(toml::from_str::<AppConfig>(&s).warning("Fail to parse configuration file.")?)
        } else {
            let config = Self {
                schedule: AutoSnapshotSchedule::new_default(),
                groups: Vec::new(),
                first_time_launch: true,
            };
            config.write_config()?;
            Ok(config)
        }
    }

    #[inline]
    pub fn is_first_time_launch(&self) -> bool {
        self.first_time_launch
    }

    #[inline]
    #[instrument]
    pub fn write_config(&self) -> CResult<()> {
        std::fs::write(&*globals::MAIN_CONFIG_FILE_PATH, toml::to_string(self)?)
            .warning("Fail to write to configuration file.")?;
        Ok(())
    }

    #[inline]
    #[instrument]
    /// return false if there's a duplicated group name or the name contains invalid characters
    pub fn add_new_group(
        &mut self,
        new_group_name: impl Into<String> + std::fmt::Debug,
        subvolumes: Vec<PathBuf>,
    ) -> CResult<bool> {
        let new_group_name = new_group_name.into();
        if !self.check_group_name_validity(&new_group_name) {
            return Ok(false);
        }
        self.groups.push(Group::new(new_group_name, subvolumes));
        self.write_config()?;
        Ok(true)
    }

    #[instrument]
    pub fn delete_group(&mut self, index: usize) -> CResult<()> {
        if index >= self.groups.len() {
            return throw_invalid_index(index, "deleting group");
        }
        self.groups.remove(index).delete_group()?;
        self.write_config()
    }

    #[inline]
    /// return false if the group name has already existed or contains invalid characters
    pub fn check_group_name_validity(&self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        self.groups.iter().all(|x| x.get_name() != name)
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// return false if there's a duplicated group name
    #[inline]
    #[instrument]
    pub fn rename_group(
        &mut self,
        index: usize,
        new_name: impl Into<String> + std::fmt::Debug,
    ) -> CResult<bool> {
        let new_name = new_name.into();
        // check for duplicated name and name validity
        if !self.check_group_name_validity(&new_name) {
            return Ok(false);
        }
        let Some(group) = self.groups.get_mut(index) else {
            return throw_invalid_index(index, "renaming group");
        };
        group.rename_group(new_name)?;
        self.write_config()?;
        Ok(true)
    }

    #[inline]
    pub fn get_schedule(&self) -> AutoSnapshotSchedule {
        self.schedule
    }

    #[inline]
    #[instrument]
    pub fn change_schedule(&mut self, new_schedule: AutoSnapshotSchedule) -> CResult<()> {
        self.schedule = new_schedule;
        self.write_config()
    }
}

impl Drop for AppConfig {
    fn drop(&mut self) {
        let _ = self.write_config();
    }
}
