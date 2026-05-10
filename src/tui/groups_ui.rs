use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Stylize,
    widgets::{Block, BorderType, Row, Table},
};
use std::{cell::RefCell, rc::Rc};

use crate::core::{btrfs_manager::BtrfsManager, error::CResult};
use crate::tui::app_tui::{AppEvent, get_body_color};
use crate::tui::menu::Menu;

pub struct GroupsUI {
    btrfs_mgr: Rc<RefCell<BtrfsManager>>,
    selected_group: Rc<RefCell<Option<usize>>>,
}

impl GroupsUI {
    pub fn new(
        btrfs_mgr: Rc<RefCell<BtrfsManager>>,
        selected_group: Rc<RefCell<Option<usize>>>,
    ) -> Self {
        Self {
            btrfs_mgr,
            selected_group,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let [groups_area, groupinfo_area] = area.layout(&Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ]));
        let groups_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Menu::Groups)
            .title_alignment(Alignment::Center);
        let groupinfo_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("  Group Info ")
            .title_alignment(Alignment::Center);
        let mgr = self.btrfs_mgr.borrow();

        // render groups table
        let group_rows: Vec<Row> = mgr
            .get_groups()
            .iter()
            .map(|x| {
                Row::new([
                    x.get_name().to_string(),
                    x.get_snapshots().len().to_string(),
                    // join the subvolumes together with "  " as delimiter
                    x.get_subvolumes()
                        .iter()
                        .map(|y| y.to_string_lossy())
                        .fold(String::new(), |a, b| a + "  " + b.as_ref()),
                ])
            })
            .collect();
        let header = Row::new(["Group Name", "Snapshot Count", "Contained Subvolumes"])
            .italic()
            .bold()
            .underlined();
        let group_table = Table::new(
            group_rows,
            [
                Constraint::Percentage(25),
                Constraint::Percentage(20),
                Constraint::Percentage(55),
            ],
        )
        .header(header)
        .block(groups_block)
        .style(get_body_color(focused));
        frame.render_widget(group_table, groups_area);

        // render current selected group information
        frame.render_widget(groupinfo_block, groupinfo_area);
    }

    pub fn handle_events(&mut self, event: AppEvent) -> CResult<bool> {
        use AppEvent::*;
        match event {
            Left | WindowLeft | Escape => return Ok(true),
            _ => (),
        }
        Ok(false)
    }
}
