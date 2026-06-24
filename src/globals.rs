use ratatui::style::Color;

use crate::tui::menu::Menu;
use std::{path::PathBuf, sync::LazyLock};

pub const MOUNT_POINT: &str = "/run/tram_btrfs/";
pub const FILE_LOCK: &str = "/run/tram_btrfs.lock";
/**
snapshot folder structure:

the btrfs partion
└── tram_btrfs
    ├── broken
    │   └── broken snapshots
    └── snapshot_groups
        └── default
            ├── daily
            ├── manually
            │   └── 2026-04-16_21:26:00
            │       └── path/to/related/subvolume
            ├── monthly
            ├── weekly
            └── boot

snapshot folder name format: yyyy-mm-dd_hh-MM-ss
The application should take a snapshot before recover to a subvolume and place it at `tram_btrfs/broken/`
The application should deny a request to recover a system subvolume
*/
pub const TOP_DIRECTORY_NAME: &str = "tram_btrfs/";
pub const SNAPSHOT_GROUPS_DIR_NAME: &str = "snapshot_groups";
pub const BROKEN_DIR_NAME: &str = "broken";

/// equals to PathBuf::from("/run/tram_btrfs/tram_btrfs")
pub static TOP_DIR_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(MOUNT_POINT).join(TOP_DIRECTORY_NAME));
/// equals to PathBuf::from("/run/tram_btrfs/tram_btrfs/snapshot_groups")
pub static SNAPSHOT_GROUP_DIR_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| TOP_DIR_PATH.join(SNAPSHOT_GROUPS_DIR_NAME));
/// equals to PathBuf::from("/run/tram_btrfs/tram_btrfs/broken")
pub static BROKEN_SNAPSHOTS_DIR_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| TOP_DIR_PATH.join(BROKEN_DIR_NAME));

pub static CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or(std::env::home_dir().unwrap().join(".config"))
        .join(TOP_DIRECTORY_NAME)
});
pub static MAIN_CONFIG_FILE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| (*CONFIG_DIR).join("tram.toml"));

// TUI constants
pub const MENU_ITEMS: [Menu; 5] = [
    Menu::Snapshots,
    Menu::Groups,
    Menu::Subvolumes,
    Menu::BrokenSnapshots,
    Menu::Settings,
];

pub const WARNING_COLOR: Color = Color::Red;

pub const FOCUSED_COLOR: Color = Color::Rgb(234, 168, 128);
// pub const FOCUSED_COLOR: Color = Color::LightYellow;
pub const MENU_COLOR: Color = Color::Cyan;
pub const BODY_COLOR: Color = Color::LightBlue;
