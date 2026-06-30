use color_eyre::Section;
use time::{Date, Month};
use tracing::instrument;

use crate::core::btrfs_objects::snapshot_type::SnapshotType;
use crate::core::btrfs_objects::subvolume_snapshot::SubvolumeSnapshot;
use crate::core::error::CResult;
use crate::core::utils::{self, exec_command};
use crate::globals;
use std::fs::remove_dir_all;
use std::path::{Path, PathBuf};

#[derive(Debug)]
/// Snapshots of a group
/// consists of snapshots of subvolumes in that group
/// also store infomations like date, time, type(Manually, Daily, Monthly, Weekly)
pub struct GroupSnapshot {
    subvolume_snapshots: Vec<SubvolumeSnapshot>,
    date: String,
    time: String,
    snapshot_type: SnapshotType,
}

impl GroupSnapshot {
    pub fn new<T: Into<String>>(date: T, time: T, snapshot_type: SnapshotType) -> Self {
        Self {
            date: date.into(),
            time: time.into(),
            subvolume_snapshots: Vec::new(),
            snapshot_type,
        }
    }

    /// record a snapshot when loading configuration
    pub fn add_snapshot<T: AsRef<Path>, E: Into<PathBuf>>(
        &mut self,
        full_path: T,
        related_subvolume: E,
    ) {
        self.subvolume_snapshots.push(SubvolumeSnapshot::new(
            full_path.as_ref().to_path_buf(),
            Some(related_subvolume.into()),
        ));
    }

    #[instrument]
    pub fn delete(self, group_name: &str) -> CResult<()> {
        // do nothing if no subvolumes included in this snapshot
        if self.subvolume_snapshots.is_empty() {
            return Ok(());
        }
        let fullpaths = self
            .subvolume_snapshots
            .iter()
            .map(|x| x.get_fullpath_string());
        let args: Vec<String> = ["subvolume".to_string(), "delete".to_string()]
            .into_iter()
            .chain(fullpaths)
            .collect();
        exec_command("btrfs", args)?;

        // remove the directory of the current snapshot group if exists
        let group_snapshot_fullpath = globals::SNAPSHOT_GROUP_DIR_PATH
            .join(group_name)
            .join(self.snapshot_type.as_ref())
            .join(self.date + "_" + &self.time);

        if std::fs::exists(&group_snapshot_fullpath)? {
            remove_dir_all(&group_snapshot_fullpath)
                .warning("Fail to remove snapshot directory.")
                .with_suggestion(|| {
                    format!(
                        "Please run the program again and manually remove '{}' before it exits.",
                        group_snapshot_fullpath.to_string_lossy()
                    )
                })?;
        }
        Ok(())
    }

    /// restore subvolumes from this snapshot
    #[instrument]
    pub fn restore(&self) -> CResult<()> {
        let broken_snapshot_dir = utils::gen_broken_dir()?;
        for x in self.subvolume_snapshots.iter() {
            x.restore(&broken_snapshot_dir)?;
        }
        Ok(())
    }

    #[inline]
    pub fn get_type(&self) -> SnapshotType {
        self.snapshot_type
    }

    #[inline]
    pub fn get_date(&self) -> String {
        self.date.clone()
    }

    #[inline]
    pub fn get_time(&self) -> String {
        self.time.clone()
    }

    #[inline]
    pub fn get_subvolume_snapshots(&self) -> &Vec<SubvolumeSnapshot> {
        &self.subvolume_snapshots
    }

    #[inline]
    /// returns a string containing all valid snapshots in the form like:
    /// "@  @home"
    pub fn get_snapshoted_subvolumes(&self) -> Vec<&str> {
        self.subvolume_snapshots
            .iter()
            .filter_map(|x| x.get_relate_subvolume_path())
            .collect()
    }

    /// return Some( (year, month, day) ) if the format of `self.data` is correct(yyyy-mm-dd)
    pub fn get_date_integer(&self) -> Option<Date> {
        let mut parts = self.date.split('-');
        if let Some(raw_year) = parts.next()
            && let Ok(year) = raw_year.parse::<i32>()
            && let Some(raw_month) = parts.next()
            && let Ok(month) = raw_month.parse::<u8>()
            && let Some(raw_day) = parts.next()
            && let Ok(day) = raw_day.parse::<u8>()
        {
            let month = Month::try_from(month).ok()?;
            Date::from_calendar_date(year, month, day).ok()
        } else {
            None
        }
    }
}

impl PartialEq<(&str, &str, &SnapshotType)> for GroupSnapshot {
    #[inline]
    /// test the equality of GroupSnapshot and (date, time, snapshot_type)
    fn eq(&self, other: &(&str, &str, &SnapshotType)) -> bool {
        self.date == other.0 && self.time == other.1 && self.snapshot_type.eq(other.2)
    }
}
