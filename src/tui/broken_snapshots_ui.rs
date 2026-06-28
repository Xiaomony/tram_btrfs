use std::{cell::RefCell, rc::Rc};

use ratatui::{
    Frame,
    layout::{Constraint, HorizontalAlignment, Layout, Rect},
    style::{Modifier, Style, Stylize},
    widgets::{Block, BorderType, Padding, Paragraph, Row, Table, TableState, Wrap},
};
use tracing::instrument;

use crate::{
    core::{btrfs_manager::BtrfsManager, error::CResult},
    globals,
    tui::{
        app_tui::{self, AppEvent},
        menu::Menu,
    },
};

#[derive(Debug)]
enum BrokenSnapshotsFocus {
    BrokenSnapshotList,
    ConfirmingDeleting { index: usize, msg: String },
    ConfirmingRecovering { index: usize, msg: String },
}

#[derive(Debug)]
pub struct BrokenSnapshotsUI {
    btrfs_mgr: Rc<RefCell<BtrfsManager>>,
    broken_snapshot_table_state: TableState,
    focus: BrokenSnapshotsFocus,
}

impl BrokenSnapshotsUI {
    pub fn new(btrfs_mgr: Rc<RefCell<BtrfsManager>>) -> Self {
        Self {
            btrfs_mgr,
            broken_snapshot_table_state: TableState::new().with_selected(None),
            focus: BrokenSnapshotsFocus::BrokenSnapshotList,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let mgr = self.btrfs_mgr.borrow();
        let broken_snapshots = mgr.get_broken_snapshots();
        let color = app_tui::get_body_color(focused);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Menu::BrokenSnapshots)
            .padding(Padding::uniform(1))
            .style(color)
            .title_alignment(HorizontalAlignment::Center);
        let prompt = Paragraph::new(
            r"Here contains original subvolumes replaced during restore operations,
as well as snapshots with invalid paths.
They may have related subvolumes or not.",
        )
        .yellow()
        .wrap(Wrap { trim: false });

        let [top_area, bottom_area] = block
            .inner(area)
            .layout(&Layout::vertical([Constraint::Length(4), Constraint::Fill(1)]).spacing(1));
        frame.render_widget(block, area);
        frame.render_widget(prompt, top_area);

        if broken_snapshots.is_empty() {
            frame.render_widget(
                Paragraph::new("No Broken Snapshots")
                    .style(globals::WARNING_COLOR)
                    .bold()
                    .italic()
                    .alignment(HorizontalAlignment::Center),
                bottom_area,
            );
            return;
        }

        if self.broken_snapshot_table_state.selected().is_none() {
            self.broken_snapshot_table_state.select_first();
        }

        let rows: Vec<Row> = broken_snapshots
            .iter()
            .map(|x| {
                Row::new([
                    x.get_path().to_string_lossy().to_string(),
                    x.get_relate_subvolume_path().unwrap_or("").to_string(),
                ])
            })
            .collect();

        // render main table
        let header = Row::new(["Path", "Related Subvolume"])
            .style(Style::new().bold().italic().underlined());
        let widths = [Constraint::Percentage(65), Constraint::Percentage(35)];
        let table = Table::new(rows, widths)
            .header(header)
            .column_spacing(1)
            .row_highlight_style(if focused {
                Modifier::REVERSED
            } else {
                Modifier::empty()
            })
            .style(color);

        frame.render_stateful_widget(table, bottom_area, &mut self.broken_snapshot_table_state);

        match self.focus {
            BrokenSnapshotsFocus::ConfirmingDeleting { ref msg, .. } => {
                app_tui::show_confirm_popup(
                    frame,
                    frame.area(),
                    "Delete the following snapshot?",
                    Paragraph::new(msg.as_str()),
                    true,
                    false,
                )
            }
            BrokenSnapshotsFocus::ConfirmingRecovering { ref msg, .. } => {
                app_tui::show_confirm_popup(
                    frame,
                    frame.area(),
                    "DANGER!! Recover from the following broken snapshot?",
                    Paragraph::new(msg.as_str()),
                    true,
                    false,
                )
            }
            _ => (),
        }
    }

    #[instrument]
    pub fn handle_events(&mut self, event: AppEvent) -> CResult<bool> {
        use AppEvent::*;

        match self.focus {
            BrokenSnapshotsFocus::ConfirmingDeleting { index, .. } => match event {
                Yes => {
                    self.focus = BrokenSnapshotsFocus::BrokenSnapshotList;
                    self.btrfs_mgr.borrow_mut().delete_broken_snapshot(index)?;
                }
                No => self.focus = BrokenSnapshotsFocus::BrokenSnapshotList,
                _ => (),
            },
            BrokenSnapshotsFocus::ConfirmingRecovering { index, .. } => match event {
                Yes => {
                    self.focus = BrokenSnapshotsFocus::BrokenSnapshotList;
                    self.btrfs_mgr.borrow_mut().recover_broken_snapshot(index)?;
                }
                No => self.focus = BrokenSnapshotsFocus::BrokenSnapshotList,
                _ => (),
            },
            BrokenSnapshotsFocus::BrokenSnapshotList => match event {
                Up => self.broken_snapshot_table_state.select_previous(),
                Down => self.broken_snapshot_table_state.select_next(),
                Top => self.broken_snapshot_table_state.select_first(),
                Bottom => self.broken_snapshot_table_state.select_last(),
                Left | WindowLeft | Escape => return Ok(true),
                Delete | RenameOrRecover
                    if let Some(i) = self.broken_snapshot_table_state.selected()
                        && !self.btrfs_mgr.borrow().get_broken_snapshots().is_empty() =>
                {
                    let mgr = self.btrfs_mgr.borrow();
                    let brokens = mgr.get_broken_snapshots();
                    let i = i.clamp(0, brokens.len() - 1);
                    let obj = brokens.get(i).unwrap();
                    let msg = format!(
                        "Path:\n  {}\nRelated Subvolume:\n  {}",
                        obj.get_path().to_string_lossy(),
                        obj.get_relate_subvolume_path()
                            .unwrap_or("No related subvolume")
                    );
                    if event == Delete {
                        self.focus = BrokenSnapshotsFocus::ConfirmingDeleting { index: i, msg };
                    } else if obj.has_related_subvol() {
                        self.focus = BrokenSnapshotsFocus::ConfirmingRecovering { index: i, msg };
                    }
                }
                _ => (),
            },
        }
        Ok(false)
    }
}
