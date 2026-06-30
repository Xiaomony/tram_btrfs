use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, HorizontalAlignment, Layout, Offset, Rect},
    style::{Color, Modifier, Stylize},
    text::{Line, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListState, Padding, Paragraph, Row, Table, Wrap,
    },
};
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use tracing::instrument;
use tui_input::Input;

use crate::tui::snapshots_ui::SnapshotsUI;
use crate::tui::{broken_snapshots_ui::BrokenSnapshotsUI, menu::Menu};
use crate::{
    core::{btrfs_manager::BtrfsManager, btrfs_objects::group::Group, error::CResult},
    tui::settings_ui::SettingsUI,
};
use crate::{globals, tui::groups_ui::GroupsUI};

#[derive(PartialEq, Debug)]
enum AppFocus {
    Menu,
    Body,
}

#[derive(PartialEq, Debug, Clone)]
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
    /// User ***pressed R***
    RenameOrRestore,
    Escape,
    Confirm,
    Yes, // press `y` to confirm
    No,  // press `n` to cancle

    /// This is only constructed when generating Key Pormpt
    QuitApp,
    Other, // reserved for input mode
}

impl AsRef<str> for AppEvent {
    fn as_ref(&self) -> &str {
        use AppEvent::*;
        match self {
            Up => "k / ↑",
            Down => "j / ↓",
            Left => "h / ←",
            Right => "l / →",

            Upward => "Ctrl + u/b",
            Downward => "Ctrl + d/f",

            Top => "g / Home",
            Bottom => "G / End",

            WindowUp => "Ctrl+ k/↑",
            WindowDown => "Ctrl+ j/↓",
            WindowLeft => "Ctrl+ h/←",
            WindowRight => "Ctrl+ l/→",

            Create => "a",
            Delete => "d / x",
            RenameOrRestore => "r",

            Escape => "Esc / Ctrl+[",
            Confirm => "Space / Enter",

            Yes => "y / Y",
            No => "n / N",

            QuitApp => "q",
            Other => "",
        }
    }
}

#[derive(Debug)]
pub struct AppTUI {
    snapshots_ui: SnapshotsUI,
    groups_ui: GroupsUI,
    broken_snapshots_ui: BrokenSnapshotsUI,
    settings_ui: SettingsUI,
    menu_state: ListState,
    btrfs_mgr: Rc<RefCell<BtrfsManager>>,
    /// the index of current selected snapshot group
    selected_group: Rc<RefCell<Option<usize>>>,
    focus: AppFocus,
    is_inputing: bool,
}

impl AppTUI {
    pub fn new(btrfs_mgr: Rc<RefCell<BtrfsManager>>) -> Self {
        let selected_group = btrfs_mgr.borrow().get_sel_group();
        if btrfs_mgr.borrow().is_first_time_launch() {
            Self {
                snapshots_ui: SnapshotsUI::new(btrfs_mgr.clone(), selected_group.clone()),
                groups_ui: GroupsUI::new(btrfs_mgr.clone(), selected_group.clone()),
                broken_snapshots_ui: BrokenSnapshotsUI::new(btrfs_mgr.clone()),
                settings_ui: SettingsUI::new(btrfs_mgr.clone(), true),
                menu_state: ListState::default().with_selected(Some(globals::MENU_SETTINGS_INDEX)),
                btrfs_mgr,
                selected_group,
                focus: AppFocus::Body,
                is_inputing: false,
            }
        } else {
            Self {
                snapshots_ui: SnapshotsUI::new(btrfs_mgr.clone(), selected_group.clone()),
                groups_ui: GroupsUI::new(btrfs_mgr.clone(), selected_group.clone()),
                broken_snapshots_ui: BrokenSnapshotsUI::new(btrfs_mgr.clone()),
                settings_ui: SettingsUI::new(btrfs_mgr.clone(), false),
                menu_state: ListState::default().with_selected(Some(0)),
                btrfs_mgr,
                selected_group,
                focus: AppFocus::Menu,
                is_inputing: false,
            }
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

        let [menu_area, crr_device_prompt_area, crr_group_prompt_area] =
            menu_block.inner(area).layout(&Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(2),
                Constraint::Length(2),
            ]));

        let crr_device_prompt = Paragraph::new(vec![
            "Current Device:\n".into(),
            self.btrfs_mgr
                .borrow()
                .get_device()
                .to_string()
                .yellow()
                .bold()
                .italic()
                .into(),
        ])
        .alignment(Alignment::Center);

        let crr_group_name = get_sel_group(&self.btrfs_mgr, &self.selected_group)
            .map(|x| x.get_name().to_string())
            .unwrap_or("No Available Groups".to_string());
        let crr_group_prompt = Paragraph::new(vec![
            "Current Selected Group:\n".into(),
            crr_group_name.yellow().bold().italic().into(),
        ])
        .alignment(Alignment::Center);

        frame.render_widget(menu_block, area);
        frame.render_stateful_widget(list, menu_area, &mut self.menu_state);
        frame.render_widget(crr_device_prompt, crr_device_prompt_area);
        frame.render_widget(crr_group_prompt, crr_group_prompt_area);
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [left_area, body_area] = frame.area().layout(
            &Layout::horizontal([Constraint::Percentage(20), Constraint::Percentage(79)])
                .margin(1)
                .spacing(1),
        );
        let [menu_area, key_prompt_area] = left_area.layout(&Layout::vertical([
            Constraint::Length(15),
            Constraint::Fill(1),
        ]));

        // render menu area
        self.render_menu(frame, menu_area);

        // render key prompt area
        let prompt = self.get_key_prompt();
        let rows: Vec<Row> = prompt
            .iter()
            .map(|x| {
                Row::new([
                    Text::from(x.0.as_ref()).alignment(Alignment::Center).red(),
                    Text::from(x.1).alignment(Alignment::Center).blue(),
                ])
            })
            .collect();
        let key_prompt_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("  Keybindings ")
            .title_alignment(HorizontalAlignment::Center)
            .style(globals::BODY_COLOR);
        let key_prompt_table = Table::new(
            rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(key_prompt_block);
        frame.render_widget(key_prompt_table, key_prompt_area);

        // render main block
        let crr_menu_item = self.get_crr_menu_item();
        use Menu::*;
        let focused = self.focus == AppFocus::Body;
        match crr_menu_item {
            Snapshots => self.snapshots_ui.render(frame, body_area, focused),
            Groups => self.groups_ui.render(frame, body_area, focused),
            BrokenSnapshots => self.broken_snapshots_ui.render(frame, body_area, focused),
            Settings => self.settings_ui.render(frame, body_area, focused),
        }
    }

    #[instrument]
    // returns whether the program should exit
    pub fn read_events(&mut self) -> CResult<bool> {
        let raw_event = event::read()?;
        if let Event::Key(ref key_event) = raw_event {
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
                Char('r') => AppEvent::RenameOrRestore,
                Char(' ') | Enter => AppEvent::Confirm,
                Char('y') | Char('Y') => AppEvent::Yes,
                Char('n') | Char('N') => AppEvent::No,
                Esc => AppEvent::Escape,
                Char('[') if mods == KeyModifiers::CONTROL => AppEvent::Escape,
                Char('q') if !self.is_inputing => return Ok(true),
                _ if !self.is_inputing => return Ok(false),
                _ => AppEvent::Other,
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
                        Groups => {
                            let (return_focus, is_inputing) =
                                self.groups_ui.handle_events(app_event, raw_event)?;
                            self.is_inputing = is_inputing;
                            return_focus
                        }
                        BrokenSnapshots => self.broken_snapshots_ui.handle_events(app_event)?,
                        Settings => self.settings_ui.handle_events(app_event)?,
                    } {
                        self.focus = AppFocus::Menu;
                    }
                }
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
            Right | Confirm | WindowRight => self.focus = AppFocus::Body,
            _ => (),
        }
    }

    pub fn get_key_prompt(&self) -> Vec<(AppEvent, &str)> {
        use AppEvent::*;
        let mut prompts = if self.is_inputing {
            vec![]
        } else {
            vec![(QuitApp, "Exit")]
        };
        let nevigation_prompts = [
            (Up, "Up"),
            (Down, "Down"),
            (Left, "Left"),
            (Right, "Right"),
            (Upward, "Upward"),
            (Downward, "Downward"),
            (Top, "Top"),
            (Bottom, "Bottom"),
            (WindowUp, "Focus Above"),
            (WindowDown, "Focus Below"),
            (WindowLeft, "Focus Left"),
            (WindowRight, "Focus Right"),
        ];
        if self.focus == AppFocus::Body {
            let (other_prompts, enable_navigation) = match self.get_crr_menu_item() {
                Menu::Snapshots => self.snapshots_ui.get_key_prompt(),
                Menu::Groups => self.groups_ui.get_key_prompt(),
                Menu::BrokenSnapshots => self.broken_snapshots_ui.get_key_prompt(),
                Menu::Settings => self.settings_ui.get_key_prompt(),
            };
            if enable_navigation {
                prompts.extend(nevigation_prompts);
            }
            prompts.extend(other_prompts);
        } else {
            prompts.extend(nevigation_prompts);
        }
        prompts
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
    larger_popup_window: bool,
) {
    let centered_area = if larger_popup_window {
        area.centered(Constraint::Percentage(80), Constraint::Percentage(80))
    } else {
        area.centered(Constraint::Percentage(40), Constraint::Percentage(40))
    };

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
            Text::from("[Y]es").style(Modifier::REVERSED).centered(),
            yes_area,
        );
        frame.render_widget(Text::from("(N)o").centered(), no_area);
    } else {
        let bottom_area = bottom_area.centered_horizontally(Constraint::Ratio(1, 2));
        frame.render_widget(
            Text::from("Ok (Enter/Space)")
                .style(Modifier::REVERSED)
                .centered(),
            bottom_area,
        );
    }
}

/// render a input widget at the top of given area
pub fn render_input_widget<'a>(
    frame: &mut Frame,
    area: Rect,
    input: &Input,
    title: impl Into<Line<'a>>,
) {
    let mut area = area
        .centered_horizontally(Constraint::Percentage(50))
        .offset(Offset::new(0, 4));
    area.height = 3;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title_top(title)
        .style(globals::FOCUSED_COLOR);
    let scroll = input.visual_scroll((area.width.max(3) - 3) as usize);
    let input_widget = Paragraph::new(input.value()).scroll((0, scroll as u16));

    frame.render_widget(Clear, area);
    frame.render_widget(input_widget, block.inner(area));
    frame.render_widget(block, area);
    // render cursor
    let x = input.visual_cursor().max(scroll) - scroll + 1;
    frame.set_cursor_position((area.x + x as u16, area.y + 1));
}

#[inline]
pub fn get_body_color(focused: bool) -> Color {
    if focused {
        globals::FOCUSED_COLOR
    } else {
        globals::BODY_COLOR
    }
}
