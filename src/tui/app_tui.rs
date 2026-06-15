use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, HorizontalAlignment, Layout, Rect},
    style::{Color, Modifier, Stylize},
    widgets::{Block, BorderType, Borders, Clear, List, ListState, Padding, Paragraph, Wrap},
};
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use crate::tui::menu::Menu;
use crate::tui::snapshots_ui::SnapshotsUI;
use crate::{
    core::{btrfs_manager::BtrfsManager, btrfs_objects::group::Group, error::CResult},
    tui::subvolumes_ui::SubvolumesUI,
};
use crate::{globals, tui::groups_ui::GroupsUI};

#[derive(PartialEq)]
enum AppFocus {
    Menu,
    Body,
    KeyPrompt,
}

#[derive(PartialEq)]
pub enum AppEvent {
    // Navigate
    Up,
    Down,
    Left,
    Right,
    Upward,
    Downward,
    Top,
    Bottom,

    // move focus to neighboring windows
    WindowUp,
    WindowDown,
    WindowLeft,
    WindowRight,

    // operations
    Create,
    Delete,
    Rename,
    Escape,
    Enter,
    Yes, // press `y` to confirm
    No,  // press `n` to cancle
}

pub struct AppTUI {
    snapshots_ui: SnapshotsUI,
    subvolumes_ui: SubvolumesUI,
    groups_ui: GroupsUI,
    menu_state: ListState,
    _btrfs_mgr: Rc<RefCell<BtrfsManager>>,
    /// the index of current selected snapshot group
    _selected_group: Rc<RefCell<Option<usize>>>,
    focus: AppFocus,
}

impl AppTUI {
    pub fn new(btrfs_mgr: Rc<RefCell<BtrfsManager>>) -> Self {
        let selected_group = Rc::new(RefCell::new(None));
        Self {
            snapshots_ui: SnapshotsUI::new(btrfs_mgr.clone(), selected_group.clone()),
            subvolumes_ui: SubvolumesUI::new(btrfs_mgr.clone()),
            groups_ui: GroupsUI::new(btrfs_mgr.clone(), selected_group.clone()),
            menu_state: ListState::default().with_selected(Some(0)),
            _btrfs_mgr: btrfs_mgr,
            _selected_group: selected_group,
            focus: AppFocus::Menu,
        }
    }

    fn render_menu(&mut self, frame: &mut Frame, area: Rect) {
        let main_color = if self.focus == AppFocus::Menu {
            globals::FOCUSED_COLOR
        } else {
            globals::MENU_COLOR
        };

        let menu_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" 󰍜 Menu ")
            .title_alignment(HorizontalAlignment::Center)
            .style(main_color);

        let list = List::new(globals::MENU_ITEMS)
            .style(main_color)
            .highlight_style(Modifier::REVERSED);

        if let Some(crr_group) = get_sel_group(&self._btrfs_mgr, &self._selected_group) {
            let vert_layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(2)])
                .split(menu_block.inner(area));
            let crr_group_prompt = Paragraph::new(vec![
                "Current Selected Group:\n".into(),
                crr_group.get_name().yellow().bold().italic().into(),
            ])
            .alignment(Alignment::Center);
            frame.render_widget(menu_block, area);
            frame.render_stateful_widget(list, vert_layout[0], &mut self.menu_state);
            frame.render_widget(crr_group_prompt, vert_layout[1]);
        } else {
            frame.render_stateful_widget(list.block(menu_block), area, &mut self.menu_state);
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let horizontal_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(79)])
            .margin(1)
            .spacing(1)
            .split(frame.area());
        self.render_menu(frame, horizontal_layout[0]);

        // render main block
        let crr_menu_item = self.get_crr_menu_item();
        use Menu::*;
        let focused = self.focus == AppFocus::Body;
        match crr_menu_item {
            Snapshots => self
                .snapshots_ui
                .render(frame, horizontal_layout[1], focused),
            Groups => self.groups_ui.render(frame, horizontal_layout[1], focused),
            Subvolumes => self
                .subvolumes_ui
                .render(frame, horizontal_layout[1], focused),
            BrokenSnapshots => (),
            Settings => (),
        }
    }

    // returns whether the program should exit
    pub fn read_events(&mut self) -> CResult<bool> {
        if let Event::Key(key_event) = event::read()? {
            use KeyCode::*;
            let mods = key_event.modifiers;
            let app_event = match key_event.code {
                Char('k') | Up if mods == KeyModifiers::NONE => AppEvent::Up,
                Char('j') | Down if mods == KeyModifiers::NONE => AppEvent::Down,
                Char('h') | Left if mods == KeyModifiers::NONE => AppEvent::Left,
                Char('l') | Right if mods == KeyModifiers::NONE => AppEvent::Right,
                // navigate to top / bottom
                Char('g') | Home => AppEvent::Top,
                Char('G') | End => AppEvent::Bottom,
                // navigate upward / downward
                Char('u') | Char('b') if mods == KeyModifiers::CONTROL => AppEvent::Upward,
                Char('d') | Char('f') if mods == KeyModifiers::CONTROL => AppEvent::Downward,
                // move focus to neighboring windows
                Char('k') | Up if mods == KeyModifiers::CONTROL => AppEvent::WindowUp,
                Char('j') | Down if mods == KeyModifiers::CONTROL => AppEvent::WindowDown,
                Char('h') | Left if mods == KeyModifiers::CONTROL => AppEvent::WindowLeft,
                Char('l') | Right if mods == KeyModifiers::CONTROL => AppEvent::WindowRight,

                // operations
                Char('a') => AppEvent::Create,
                Char('d') | Char('x') => AppEvent::Delete,
                Char('r') => AppEvent::Rename,
                Char(' ') | Enter => AppEvent::Enter,
                Char('y') | Char('Y') => AppEvent::Yes,
                Char('n') | Char('N') => AppEvent::No,
                Char('q') => return Ok(true),
                Esc => AppEvent::Escape,
                _ => return Ok(false),
            };

            match self.focus {
                AppFocus::Menu => {
                    if app_event == AppEvent::Escape {
                        return Ok(true);
                    } else {
                        self.handle_menu_events(app_event)
                    }
                }
                AppFocus::Body => {
                    use crate::tui::menu::Menu::*;
                    if match self.get_crr_menu_item() {
                        Snapshots => self.snapshots_ui.handle_events(app_event)?,
                        Groups => self.groups_ui.handle_events(app_event)?,
                        Subvolumes => self.subvolumes_ui.handle_events(app_event)?,
                        BrokenSnapshots => false,
                        Settings => false,
                    } {
                        self.focus = AppFocus::Menu;
                    }
                }
                AppFocus::KeyPrompt => (),
            }
        }
        Ok(false)
    }

    pub fn handle_menu_events(&mut self, event: AppEvent) {
        use AppEvent::*;
        match event {
            Up => self.menu_state.select_previous(),
            Down => self.menu_state.select_next(),
            Upward | Top => self.menu_state.select_first(),
            Downward | Bottom => self.menu_state.select_last(),
            Right | Enter | WindowRight => self.focus = AppFocus::Body,
            _ => (),
        }
    }
    #[inline]
    pub fn get_crr_menu_item(&self) -> Menu {
        globals::MENU_ITEMS[self.menu_state.selected().unwrap()]
    }
}

/// return the reference of the current selected group
/// if the index is invalid, try to select and return the first group
/// if the group list is empty, return `None`
pub fn get_sel_group<'a>(
    btrfs_mgr: &'a Rc<RefCell<BtrfsManager>>,
    selected_group: &'a Rc<RefCell<Option<usize>>>,
) -> Option<Ref<'a, Group>> {
    let mgr = btrfs_mgr.borrow();
    if let Some(index) = *selected_group.borrow()
        && index < mgr.get_groups().len()
    {
        Some(Ref::map(mgr, |m| m.get_groups().get(index).unwrap()))
    } else if !mgr.get_groups().is_empty() {
        *selected_group.borrow_mut() = Some(0);
        Some(Ref::map(mgr, |m| m.get_groups().first().unwrap()))
    } else {
        None
    }
}

pub fn get_sel_group_mut<'a>(
    btrfs_mgr: &'a Rc<RefCell<BtrfsManager>>,
    selected_group: &'a Rc<RefCell<Option<usize>>>,
) -> Option<RefMut<'a, Group>> {
    let mgr = btrfs_mgr.borrow_mut();
    if let Some(index) = *selected_group.borrow()
        && index < mgr.get_groups().len()
    {
        Some(RefMut::map(mgr, |m| {
            m.get_mut_groups().get_mut(index).unwrap()
        }))
    } else if !mgr.get_groups().is_empty() {
        *selected_group.borrow_mut() = Some(0);
        Some(RefMut::map(mgr, |m| {
            m.get_mut_groups().first_mut().unwrap()
        }))
    } else {
        None
    }
}

/// show confirming widget, wrap the content automatically
/// `yes_no_confirming`:
/// if true, it will display a "Yes" and a "No" at the bottom.
/// Otherwise, only display a "Ok".
pub fn show_confirm_popup(
    frame: &mut Frame,
    area: Rect,
    title: impl Into<String>,
    content: Paragraph,
    yes_no_confirming: bool,
) {
    let centered_area = area.centered(Constraint::Percentage(40), Constraint::Percentage(40));

    let confirm_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(globals::FOCUSED_COLOR)
        .padding(Padding::new(2, 2, 1, 0))
        .title(title.into())
        .title_alignment(Alignment::Center);

    let [content_area, bottom_area] =
        confirm_block
            .inner(centered_area)
            .layout(&Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(1),
            ]));
    frame.render_widget(Clear, centered_area);
    frame.render_widget(confirm_block, centered_area);
    frame.render_widget(
        content
            .block(
                Block::new()
                    .borders(Borders::BOTTOM)
                    .style(globals::FOCUSED_COLOR),
            )
            .wrap(Wrap { trim: false }),
        content_area,
    );
    if yes_no_confirming {
        let [yes_area, no_area] = bottom_area.layout(&Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]));
        frame.render_widget(
            Paragraph::new("[Y]es").style(Modifier::REVERSED).centered(),
            yes_area,
        );
        frame.render_widget(Paragraph::new("(N)o").centered(), no_area);
    } else {
        let bottom_area = bottom_area.centered_horizontally(Constraint::Ratio(1, 2));
        frame.render_widget(
            Paragraph::new("Ok").style(Modifier::REVERSED).centered(),
            bottom_area,
        );
    }
}

#[inline]
pub fn get_body_color(focused: bool) -> Color {
    if focused {
        globals::FOCUSED_COLOR
    } else {
        globals::BODY_COLOR
    }
}
