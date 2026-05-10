use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    widgets::{Block, BorderType, List, Padding},
};
use std::{cell::RefCell, rc::Rc};

use crate::{
    core::{btrfs_manager::BtrfsManager, error::CResult},
    globals,
    tui::{app_tui::AppEvent, menu::Menu},
};

pub struct SubvolumesUI {
    btrfs_mgr: Rc<RefCell<BtrfsManager>>,
}

impl SubvolumesUI {
    pub fn new(btrfs_mgr: Rc<RefCell<BtrfsManager>>) -> Self {
        Self { btrfs_mgr }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let main_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Menu::Subvolumes)
            .title_alignment(Alignment::Center)
            .padding(Padding::uniform(1));
        let mgr = self.btrfs_mgr.borrow();
        let list = List::from_iter(mgr.get_subvolumes().iter().map(|x| x.to_string_lossy()))
            .block(main_block)
            .style(if focused {
                globals::FOCUSED_COLOR
            } else {
                globals::BODY_COLOR
            });

        frame.render_widget(list, area);
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
