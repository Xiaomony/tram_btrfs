use color_eyre::Section;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, HorizontalAlignment, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Padding, Paragraph, Row, Table, TableState},
};
use std::{cell::RefCell, rc::Rc};

use crate::{
    core::error::CResult,
    tui::{
        app_tui::{self, AppEvent, get_sel_group, get_sel_group_mut},
        menu::Menu,
    },
};
use crate::{
    core::{btrfs_manager::BtrfsManager, btrfs_objects::snapshot_type::SnapshotType},
    globals,
};

#[derive(PartialEq)]
enum SnapshotUIFocus {
    ManualSnapshot,
    ScheduledSnapshot,
    ConfirmingDelete { msg: String, index: usize },
}

pub struct SnapshotsUI {
    btrfs_mgr: Rc<RefCell<BtrfsManager>>,
    manual_snapshot_table_state: TableState,
    scheduled_snapshot_table_state: TableState,
    /// the index of current selected snapshot group
    selected_group: Rc<RefCell<Option<usize>>>,
    focus: SnapshotUIFocus,
    manual_snapshot_infos: Vec<(usize, [String; 3])>,
    scheduled_snapshot_infos: Vec<(usize, [String; 4])>,
    no_valid_group: bool,
}

impl SnapshotsUI {
    pub fn new(
        btrfs_mgr: Rc<RefCell<BtrfsManager>>,
        selected_group: Rc<RefCell<Option<usize>>>,
    ) -> Self {
        let mut new_obj = Self {
            btrfs_mgr,
            manual_snapshot_table_state: TableState::default().with_selected(None),
            scheduled_snapshot_table_state: TableState::default().with_selected(None),
            selected_group,
            focus: SnapshotUIFocus::ManualSnapshot,
            manual_snapshot_infos: Vec::new(),
            scheduled_snapshot_infos: Vec::new(),
            no_valid_group: false,
        };
        new_obj.refresh_table_data();
        new_obj
    }

    pub fn refresh_table_data(&mut self) {
        self.manual_snapshot_infos.clear();
        self.scheduled_snapshot_infos.clear();
        let Some(group) = get_sel_group(&self.btrfs_mgr, &self.selected_group) else {
            self.no_valid_group = true;
            return;
        };
        self.no_valid_group = false;
        let snapshots = group.get_snapshots();
        for (i, x) in snapshots.iter().enumerate() {
            let subvols = x.get_snapshoted_subvolumes().join("  ");
            if x.get_type() == SnapshotType::Manually {
                self.manual_snapshot_infos
                    .push((i, [x.get_date(), x.get_time(), subvols]));
            } else {
                self.scheduled_snapshot_infos.push((
                    i,
                    [
                        x.get_date(),
                        x.get_time(),
                        x.get_type().to_string(),
                        subvols,
                    ],
                ));
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        self.refresh_table_data();
        if self.no_valid_group {
            let lines: Vec<Line<'static>> = vec![
                Menu::Snapshots
                    .as_ref()
                    .bold()
                    .italic()
                    .patch_style(globals::BODY_COLOR)
                    .into(),
                "No groups. Please create one.".into(),
            ];
            frame.render_widget(
                Paragraph::new(lines).alignment(Alignment::Center),
                area.centered_vertically(Constraint::Length(2)),
            );
            return;
        }
        let manual_snapshot_rows: Vec<Row<'_>> = self
            .manual_snapshot_infos
            .iter()
            .map(|x| Row::new(x.1.clone()))
            .collect();
        let scheduled_snapshot_rows: Vec<Row<'_>> = self
            .scheduled_snapshot_infos
            .iter()
            .map(|x| Row::new(x.1.clone()))
            .collect();

        // determine the height of each block dynamically
        let l1 = manual_snapshot_rows.len();
        let l2 = scheduled_snapshot_rows.len();
        let manual_percentage = if l1 == 0 && l2 == 0 {
            50
        } else {
            ((l1 * 100 / (l1 + l2)) as u16).clamp(20, 80)
        };
        let vertical_layout = Layout::vertical([
            Constraint::Percentage(manual_percentage),
            Constraint::Percentage(100 - manual_percentage),
        ])
        .split(area);

        self.render_manual_block(
            frame,
            vertical_layout[0],
            manual_snapshot_rows,
            focused && self.focus == SnapshotUIFocus::ManualSnapshot,
        );
        self.render_scheduled_block(
            frame,
            vertical_layout[1],
            scheduled_snapshot_rows,
            focused && self.focus == SnapshotUIFocus::ScheduledSnapshot,
        );

        if let SnapshotUIFocus::ConfirmingDelete { ref msg, .. } = self.focus {
            app_tui::show_confirm_popup(
                frame,
                frame.area(),
                "Delete the following snapshot?",
                Paragraph::new(msg.as_str()),
            );
        }
    }

    #[inline]
    fn get_color(focused: bool) -> Color {
        if focused {
            globals::FOCUSED_COLOR
        } else {
            globals::BODY_COLOR
        }
    }

    fn render_manual_block(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        rows: Vec<Row>,
        focused: bool,
    ) {
        if self.manual_snapshot_infos.is_empty() {
            self.manual_snapshot_table_state.select(None);
        } else if self.manual_snapshot_table_state.selected().is_none() {
            self.manual_snapshot_table_state.select_first();
        }
        let main_color = Self::get_color(focused);
        let manual_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .style(main_color)
            .title(" Manual Snapshots ")
            .padding(Padding::uniform(1))
            .title_alignment(HorizontalAlignment::Center);
        if rows.is_empty() {
            frame.render_widget(
                Paragraph::new("No manual snapshots")
                    .alignment(Alignment::Center)
                    .style(globals::WARNING_COLOR)
                    .block(manual_block),
                area,
            );
        } else {
            let header = Row::new(["Date", "Time", "Contained Subvolumes"])
                .style(Style::new().bold().italic().underlined());
            let widths = [
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(40),
            ];
            let mut table = Table::new(rows, widths)
                .header(header)
                .column_spacing(1)
                .style(main_color);
            if focused {
                table = table.row_highlight_style(Modifier::REVERSED);
            }
            frame.render_stateful_widget(
                table.block(manual_block),
                area,
                &mut self.manual_snapshot_table_state,
            );
        };
    }

    fn render_scheduled_block(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        rows: Vec<Row>,
        focused: bool,
    ) {
        if self.scheduled_snapshot_infos.is_empty() {
            self.scheduled_snapshot_table_state.select(None);
        } else if self.scheduled_snapshot_table_state.selected().is_none() {
            self.scheduled_snapshot_table_state.select_first();
        }
        let main_color = Self::get_color(focused);
        let scheduled_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .style(main_color)
            .padding(Padding::uniform(1))
            .title(" Scheduled Snapshots ")
            .title_alignment(HorizontalAlignment::Center);
        if rows.is_empty() {
            frame.render_widget(
                Paragraph::new("No scheduled snapshots")
                    .alignment(Alignment::Center)
                    .style(globals::WARNING_COLOR)
                    .block(scheduled_block),
                area,
            );
        } else {
            let header = Row::new(["Date", "Time", "Type", "Contained Subvolumes"])
                .bold()
                .italic()
                .underlined();
            let widths = [
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(40),
            ];
            let mut table = Table::new(rows, widths)
                .header(header)
                .column_spacing(1)
                .style(main_color);
            if focused {
                table = table.row_highlight_style(Modifier::REVERSED);
            }
            frame.render_stateful_widget(
                table.block(scheduled_block),
                area,
                &mut self.scheduled_snapshot_table_state,
            );
        };
    }

    /// returns whether the focus should be returned to menu
    pub fn handle_events(&mut self, event: AppEvent) -> CResult<bool> {
        // handle events if it's confirming currently
        if let SnapshotUIFocus::ConfirmingDelete { index, .. } = self.focus {
            use AppEvent::*;
            match event {
                Yes => {
                    if let Some(mut group) =
                        get_sel_group_mut(&self.btrfs_mgr, &self.selected_group)
                    {
                        group
                            .delete_snapshot(index)
                            .warning("Fail to delete snapshot")?;
                    }
                    self.focus = SnapshotUIFocus::ManualSnapshot;
                }
                Escape | No => self.focus = SnapshotUIFocus::ManualSnapshot,
                _ => (),
            }
            return Ok(false);
        }

        let table_state;
        let info_len;
        if self.focus == SnapshotUIFocus::ManualSnapshot {
            table_state = &mut self.manual_snapshot_table_state;
            info_len = self.manual_snapshot_infos.len();
        } else {
            table_state = &mut self.scheduled_snapshot_table_state;
            info_len = self.scheduled_snapshot_infos.len();
        }
        use AppEvent::*;
        match event {
            Left | WindowLeft => return Ok(true),

            // move focus up/down if the selected table item has been the first/last one
            // these codes are really weird and ugly but there're no better ways to do so...
            Up | Upward
                if self.focus == SnapshotUIFocus::ScheduledSnapshot
                    && let Some(0) | None = table_state.selected() =>
            {
                self.focus = SnapshotUIFocus::ManualSnapshot
            }
            Down | Downward
                if self.focus == SnapshotUIFocus::ManualSnapshot
                    && matches!(table_state.selected(), Some(sel) if sel + 1 >= info_len)
                    || table_state.selected().is_none() =>
            {
                self.focus = SnapshotUIFocus::ScheduledSnapshot
            }

            Up => table_state.select_previous(),
            Down => table_state.select_next(),
            Top => table_state.select_first(),
            Bottom => table_state.select_last(),
            Upward => {
                if let Some(sel) = table_state.selected() {
                    table_state.select(Some(sel.saturating_sub(4)));
                }
            }
            Downward => {
                if let Some(sel) = table_state.selected()
                    && info_len > 0
                {
                    table_state.select(Some((sel + 4).min(info_len - 1)));
                }
            }
            WindowUp => self.focus = SnapshotUIFocus::ManualSnapshot,
            WindowDown => self.focus = SnapshotUIFocus::ScheduledSnapshot,
            Create
                if let Some(mut group) =
                    get_sel_group_mut(&self.btrfs_mgr, &self.selected_group) =>
            {
                group
                    .create_snapshot(SnapshotType::Manually)
                    .warning("Fail to create new snapshot.")?;
            }
            Delete => {
                if self.focus == SnapshotUIFocus::ManualSnapshot
                    && let Some(i) = self.manual_snapshot_table_state.selected()
                {
                    let info = self
                        .manual_snapshot_infos
                        .get(i.clamp(0, self.manual_snapshot_infos.len() - 1))
                        .unwrap();
                    self.focus = SnapshotUIFocus::ConfirmingDelete {
                        msg: format!(
                            "Type: {}\nData: {}\nTime: {}\nContained Subvolumes:\n{}",
                            SnapshotType::Manually,
                            info.1[0],
                            info.1[1],
                            info.1[2],
                        ),
                        index: info.0,
                    };
                } else if self.focus == SnapshotUIFocus::ScheduledSnapshot
                    && let Some(i) = self.scheduled_snapshot_table_state.selected()
                {
                    let info = self
                        .scheduled_snapshot_infos
                        .get(i.clamp(0, self.scheduled_snapshot_infos.len() - 1))
                        .unwrap();
                    self.focus = SnapshotUIFocus::ConfirmingDelete {
                        msg: format!(
                            "Type: {}\nData: {}\nTime: {}\nContained Subvolumes:\n{}",
                            info.1[2], info.1[0], info.1[1], info.1[3],
                        ),
                        index: info.0,
                    };
                }
            }
            Enter => todo!(),
            _ => (),
        }

        Ok(false)
    }
}
