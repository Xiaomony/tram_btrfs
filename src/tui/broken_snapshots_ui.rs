use std::{cell::RefCell, rc::Rc};

use ratatui::{
    Frame,
    layout::{Constraint, HorizontalAlignment, Layout, Rect},
    style::{Modifier, Style, Stylize},
    widgets::{Block, BorderType, Padding, Paragraph, Row, Table, TableState, Wrap},
};

use crate::{
    core::{btrfs_manager::BtrfsManager, error::CResult},
    globals,
    tui::{
        app_tui::{self, AppEvent},
        menu::Menu,
    },
};

pub struct BrokenSnapshotsUI {
    btrfs_mgr: Rc<RefCell<BtrfsManager>>,
    broken_snapshot_table_state: TableState,
}

impl BrokenSnapshotsUI {
    pub fn new(btrfs_mgr: Rc<RefCell<BtrfsManager>>) -> Self {
        Self {
            btrfs_mgr,
            broken_snapshot_table_state: TableState::new().with_selected(None),
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
    }

    pub fn handle_events(&mut self, event: AppEvent) -> CResult<bool> {
        use AppEvent::*;
        match event {
            Up => self.broken_snapshot_table_state.select_previous(),
            Down => self.broken_snapshot_table_state.select_next(),
            Top => self.broken_snapshot_table_state.select_first(),
            Bottom => self.broken_snapshot_table_state.select_last(),
            Left | WindowLeft | Escape => return Ok(true),
            _ => (),
        }
        Ok(false)
    }
}
