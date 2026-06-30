use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Modifier, Stylize},
    text::Line,
    widgets::{Block, BorderType, List, ListState, Padding, Paragraph, Row, Table, TableState},
};
use std::{cell::RefCell, rc::Rc};
use tracing::instrument;
use tui_input::{Input, backend::crossterm::EventHandler};

use crate::tui::app_tui::{self, AppEvent, get_body_color};
use crate::tui::menu::Menu;
use crate::{
    core::{btrfs_manager::BtrfsManager, error::CResult},
    globals,
};

#[derive(PartialEq, Debug)]
enum GroupsUIFocus {
    GroupList,
    IncludedSubvols,
    ExcludedSubvols,
    DeleteGroupConfirming { msg: String, index: usize },
    CreateGroupInputing,
    RenameGroupInputing { index: usize },
    InvalidGroupNamePopup,
}

#[derive(Debug)]
pub struct GroupsUI {
    btrfs_mgr: Rc<RefCell<BtrfsManager>>,
    /// the index of current selected snapshot group
    selected_group: Rc<RefCell<Option<usize>>>,
    group_list_table_state: TableState,
    focus: GroupsUIFocus,
    crr_focus_group_excluded_subvols: Vec<usize>,
    included_subvols_list_state: ListState,
    excluded_subvols_list_state: ListState,
    input: Input,
}

impl GroupsUI {
    pub fn new(
        btrfs_mgr: Rc<RefCell<BtrfsManager>>,
        selected_group: Rc<RefCell<Option<usize>>>,
    ) -> Self {
        Self {
            btrfs_mgr,
            selected_group,
            group_list_table_state: TableState::default().with_selected(None),
            focus: GroupsUIFocus::GroupList,
            crr_focus_group_excluded_subvols: Vec::new(),
            included_subvols_list_state: ListState::default().with_selected(Some(0)),
            excluded_subvols_list_state: ListState::default().with_selected(Some(0)),
            input: Input::default(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let [groups_area, groupinfo_area] = area.layout(&Layout::vertical([
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ]));

        self.render_group_table(frame, focused, groups_area);
        self.render_group_info(frame, focused, groupinfo_area);
        match self.focus {
            GroupsUIFocus::DeleteGroupConfirming { ref msg, .. } => {
                app_tui::show_confirm_popup(
                    frame,
                    frame.area(),
                    "Delete the following group?",
                    Paragraph::new(msg.as_str()),
                    true,
                    false,
                );
            }
            GroupsUIFocus::CreateGroupInputing => app_tui::render_input_widget(
                frame,
                area,
                &self.input,
                "Create Group (letters, numbers and _ only)",
            ),
            GroupsUIFocus::RenameGroupInputing { .. } => app_tui::render_input_widget(
                frame,
                area,
                &self.input,
                "Rename Group (letters, numbers and _ only)",
            ),
            GroupsUIFocus::InvalidGroupNamePopup => {
                app_tui::show_confirm_popup(
                    frame,
                    frame.area(),
                    "Invalid Group Name",
                    Paragraph::new(
                        r"You inputed a invalid group name.
May caused by one of the following reasons:
  1. The group name has already existed.
  2. Your group name contains characters other than letters, numbers, and underscores",
                    ),
                    false,
                    false,
                );
            }
            _ => (),
        }
    }

    /// body_focused: whether the focus is inside 'Groups' (not need to be inside the group table)
    fn render_group_table(&mut self, frame: &mut Frame, body_focused: bool, area: Rect) {
        let sel_group_index = self.selected_group.borrow();
        if self.group_list_table_state.selected().is_none() {
            self.group_list_table_state.select(*sel_group_index);
        }

        let mgr = self.btrfs_mgr.borrow();
        let groups_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Menu::Groups)
            .padding(Padding::uniform(1))
            .title_alignment(Alignment::Center);

        let group_rows: Vec<Row> = mgr
            .get_groups()
            .iter()
            .enumerate()
            .map(|(i, x)| {
                let row = Row::new([
                    x.get_name().to_string(),
                    x.get_snapshots().len().to_string(),
                    // join the subvolumes together with "  " as delimiter
                    x.get_subvolumes()
                        .iter()
                        .map(|y| y.to_string_lossy())
                        .fold(String::new(), |a, b| a + "  " + b.as_ref()),
                ]);
                if let Some(index) = *sel_group_index
                    && index == i
                {
                    return row.yellow().add_modifier(Modifier::BOLD | Modifier::ITALIC);
                }
                row
            })
            .collect();
        let header = Row::new(["Group Name", "Snapshot Count", "Included Subvolumes"])
            .italic()
            .bold()
            .underlined();

        let mut group_table = Table::new(
            group_rows,
            [
                Constraint::Percentage(25),
                Constraint::Percentage(20),
                Constraint::Percentage(55),
            ],
        )
        .header(header)
        .block(groups_block)
        .style(get_body_color(
            body_focused && self.focus == GroupsUIFocus::GroupList,
        ));
        if body_focused {
            group_table = group_table.row_highlight_style(Modifier::REVERSED);
        }
        frame.render_stateful_widget(group_table, area, &mut self.group_list_table_state);
    }

    /// body_focused: whether the focus is inside 'Groups' (not need to be inside the group info)
    fn render_group_info(&mut self, frame: &mut Frame, body_focused: bool, area: Rect) {
        let groupinfo_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .padding(Padding::uniform(1))
            .title("  Group Info ")
            .title_alignment(Alignment::Center)
            .style(get_body_color(
                body_focused
                    && (self.focus == GroupsUIFocus::IncludedSubvols
                        || self.focus == GroupsUIFocus::ExcludedSubvols),
            ));
        let mgr = self.btrfs_mgr.borrow();
        let groups = mgr.get_groups();

        if let Some(focused_group) = self
            .group_list_table_state
            .selected()
            .and_then(|x| groups.get(x.clamp(0, groups.len() - 1)))
        {
            let [left_area, right_area] =
                groupinfo_block.inner(area).layout(&Layout::horizontal([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]));
            let [
                group_name_area,
                included_subvol_list_title_area,
                included_subvol_list_area,
            ] = left_area.layout(&Layout::vertical([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Fill(1),
            ]));
            let included_subvol_list_area = included_subvol_list_area.inner(Margin::new(2, 0));
            let [excluded_subvol_list_title_area, excluded_subvol_list_area] =
                right_area.layout(&Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Fill(1),
                ]));
            let excluded_subvol_list_area = excluded_subvol_list_area.inner(Margin::new(2, 0));
            frame.render_widget(groupinfo_block, area);

            // render group name
            let group_name = Paragraph::new(vec![
                Line::from("Group Name:").style(globals::BODY_COLOR),
                format!("    {}", focused_group.get_name())
                    .bold()
                    .italic()
                    .cyan()
                    .into(),
            ]);
            frame.render_widget(group_name, group_name_area);

            // render included subvolumes
            let included_subvol_list_focused =
                body_focused && self.focus == GroupsUIFocus::IncludedSubvols;
            let included_subvol_list_color = get_body_color(included_subvol_list_focused);
            let mut included_subvol_list = List::from_iter(
                focused_group
                    .get_subvolumes()
                    .iter()
                    .map(|x| x.to_string_lossy()),
            )
            .style(included_subvol_list_color);
            frame.render_widget(
                Line::from("Included Subvolumes(Press Enter to exclude):")
                    .style(included_subvol_list_color),
                included_subvol_list_title_area,
            );

            if included_subvol_list_focused {
                included_subvol_list = included_subvol_list.highlight_style(Modifier::REVERSED);
            }

            if focused_group.get_subvolumes().is_empty() {
                frame.render_widget(
                    Line::from("No included subvolumes")
                        .style(globals::WARNING_COLOR)
                        .bold()
                        .italic(),
                    included_subvol_list_area,
                );
            } else {
                frame.render_stateful_widget(
                    included_subvol_list,
                    included_subvol_list_area,
                    &mut self.included_subvols_list_state,
                );
            }

            // render excluded snapshots
            let excluded_subvol_list_focused =
                body_focused && self.focus == GroupsUIFocus::ExcludedSubvols;
            let excluded_subvol_list_color = get_body_color(excluded_subvol_list_focused);
            frame.render_widget(
                Line::from("Excluded Subvolumes(Press Enter to include):")
                    .style(excluded_subvol_list_color),
                excluded_subvol_list_title_area,
            );
            self.crr_focus_group_excluded_subvols = mgr
                .get_subvolumes()
                .iter()
                .enumerate()
                .filter_map(|(i, x)| {
                    if focused_group.get_subvolumes().iter().all(|y| x != y) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect(); // update the excluded subvolumes
            let mut excluded_subvol_list = List::from_iter(
                self.crr_focus_group_excluded_subvols
                    .iter()
                    .map(|&x| mgr.get_subvolumes().get(x).unwrap().to_string_lossy()),
            )
            .style(excluded_subvol_list_color);
            if excluded_subvol_list_focused {
                excluded_subvol_list = excluded_subvol_list.highlight_style(Modifier::REVERSED);
            }
            if self.crr_focus_group_excluded_subvols.is_empty() {
                frame.render_widget(
                    Line::from("No excluded subvolumes")
                        .style(globals::WARNING_COLOR)
                        .bold()
                        .italic(),
                    excluded_subvol_list_area,
                );
            } else {
                frame.render_stateful_widget(
                    excluded_subvol_list,
                    excluded_subvol_list_area,
                    &mut self.excluded_subvols_list_state,
                );
            }
        } else {
            let msg = Paragraph::new("No avaliable group to display.")
                .block(groupinfo_block)
                .alignment(Alignment::Center)
                .style(globals::WARNING_COLOR)
                .italic()
                .bold();

            frame.render_widget(msg, area);
        }
    }

    #[instrument]
    /// return a tuple containing
    /// (whether the focus is returned, is inputing)
    pub fn handle_events(
        &mut self,
        event: AppEvent,
        raw_event: crossterm::event::Event,
    ) -> CResult<(bool, bool)> {
        use AppEvent::*;

        match self.focus {
            GroupsUIFocus::CreateGroupInputing | GroupsUIFocus::RenameGroupInputing { .. } => {
                match event {
                    Escape => self.focus = GroupsUIFocus::GroupList,

                    Confirm => {
                        let succeed = match self.focus {
                            GroupsUIFocus::CreateGroupInputing => {
                                let new_name = self.input.value();
                                self.btrfs_mgr.borrow_mut().add_group(new_name)?
                            }
                            GroupsUIFocus::RenameGroupInputing { index } => self
                                .btrfs_mgr
                                .borrow_mut()
                                .rename_group(index, self.input.value())?,
                            _ => true,
                        };
                        if succeed {
                            self.focus = GroupsUIFocus::GroupList;
                        } else {
                            self.focus = GroupsUIFocus::InvalidGroupNamePopup;
                        }
                    }
                    _ => {
                        self.input.handle_event(&raw_event);
                        return Ok((false, true));
                    }
                }
                return Ok((false, false));
            }
            GroupsUIFocus::GroupList => match event {
                Left => return Ok((true, false)),
                WindowLeft | Escape => return Ok((true, false)),
                WindowDown => self.focus = GroupsUIFocus::IncludedSubvols,
                Up => self.group_list_table_state.select_previous(),
                Down => self.group_list_table_state.select_next(),
                Top => self.group_list_table_state.select_first(),
                Bottom => self.group_list_table_state.select_last(),
                Confirm => {
                    let mgr = self.btrfs_mgr.borrow();
                    let groups = mgr.get_groups();
                    if !groups.is_empty()
                        && let Some(focused_group_index) = self.group_list_table_state.selected()
                    {
                        *self.selected_group.borrow_mut() =
                            Some(focused_group_index.clamp(0, groups.len() - 1));
                    }
                }
                Delete => {
                    let mgr = self.btrfs_mgr.borrow();
                    if !mgr.get_groups().is_empty()
                        && let Some(i) = self.group_list_table_state.selected()
                    {
                        let i = i.clamp(0, mgr.get_groups().len() - 1);
                        let group = mgr.get_groups().get(i).unwrap();
                        self.focus = GroupsUIFocus::DeleteGroupConfirming {
                            msg: format!(
                                "DANGER: this will delete the following group along with all its snapshots!\n\nGroup Name: {}\nSnapshot Count: {}\nIncluded Subvolumes:{}",
                                group.get_name(),
                                group.get_snapshots().len(),
                                group
                                    .get_subvolumes()
                                    .iter()
                                    .map(|x| x.to_string_lossy())
                                    .fold(String::new(), |acc, y| acc + "\n  " + &y)
                            ),
                            index: i,
                        };
                    }
                }
                Create => {
                    self.input.reset();
                    self.focus = GroupsUIFocus::CreateGroupInputing;
                    return Ok((false, true));
                }
                RenameOrRestore => {
                    let mgr = self.btrfs_mgr.borrow();
                    let groups = mgr.get_groups();
                    if let Some(index) = self.group_list_table_state.selected()
                        && !groups.is_empty()
                    {
                        let index = index.clamp(0, groups.len() - 1);
                        let old_name = groups.get(index).unwrap().get_name();
                        self.input = Input::new(old_name.into());
                        self.focus = GroupsUIFocus::RenameGroupInputing { index };
                        return Ok((false, true));
                    }
                }
                _ => (),
            },
            GroupsUIFocus::IncludedSubvols => match event {
                Left => return Ok((true, false)),
                Right | WindowRight => self.focus = GroupsUIFocus::ExcludedSubvols,
                WindowLeft | Escape => return Ok((true, false)),
                WindowUp => self.focus = GroupsUIFocus::GroupList,
                Up => {
                    if let Some(0) = self.included_subvols_list_state.selected() {
                        self.focus = GroupsUIFocus::GroupList;
                    } else {
                        self.included_subvols_list_state.select_previous();
                    }
                }
                Down => self.included_subvols_list_state.select_next(),
                Top => self.included_subvols_list_state.select_first(),
                Bottom => self.included_subvols_list_state.select_last(),
                Confirm => {
                    let mut mgr = self.btrfs_mgr.borrow_mut();
                    let groups = mgr.get_groups();
                    if !groups.is_empty()
                        && let Some(focused_group_index) = self
                            .group_list_table_state
                            .selected()
                            .map(|x| x.clamp(0, groups.len() - 1))
                        && let Some(i) = self.included_subvols_list_state.selected()
                    {
                        mgr.remove_subvol_from_group(focused_group_index, i)?;
                    }
                }
                _ => (),
            },
            GroupsUIFocus::ExcludedSubvols => match event {
                Left | WindowLeft => self.focus = GroupsUIFocus::IncludedSubvols,
                Escape => return Ok((true, false)),
                WindowUp => self.focus = GroupsUIFocus::GroupList,
                Up => {
                    if let Some(0) = self.excluded_subvols_list_state.selected() {
                        self.focus = GroupsUIFocus::GroupList;
                    } else {
                        self.excluded_subvols_list_state.select_previous();
                    }
                }
                Down => self.excluded_subvols_list_state.select_next(),
                Top => self.excluded_subvols_list_state.select_first(),
                Bottom => self.excluded_subvols_list_state.select_last(),
                Confirm => {
                    let mut mgr = self.btrfs_mgr.borrow_mut();
                    if !mgr.get_groups().is_empty()
                        && !self.crr_focus_group_excluded_subvols.is_empty()
                        && let Some(focused_group_index) = self
                            .group_list_table_state
                            .selected()
                            .map(|x| x.clamp(0, mgr.get_groups().len() - 1))
                        && let Some(i) = self.excluded_subvols_list_state.selected()
                        && let Some(&subvol_index) = self
                            .crr_focus_group_excluded_subvols
                            .get(i.clamp(0, self.crr_focus_group_excluded_subvols.len() - 1))
                    {
                        mgr.add_subvol_to_group(focused_group_index, subvol_index)?;
                    }
                }
                _ => (),
            },
            GroupsUIFocus::DeleteGroupConfirming { index, .. } => match event {
                Yes => {
                    self.btrfs_mgr.borrow_mut().delete_group(index)?;
                    self.focus = GroupsUIFocus::GroupList;
                }
                No | Escape => self.focus = GroupsUIFocus::GroupList,
                _ => (),
            },
            GroupsUIFocus::InvalidGroupNamePopup => match event {
                Confirm | Escape => self.focus = GroupsUIFocus::GroupList,
                _ => (),
            },
        }
        Ok((false, false))
    }

    pub fn get_key_prompt(&self) -> (Vec<(AppEvent, &str)>, bool) {
        use AppEvent::*;
        match self.focus {
            GroupsUIFocus::GroupList => (
                vec![
                    (Create, "Create Group"),
                    (Delete, "Delete Group"),
                    (RenameOrRestore, "Rename"),
                    (Confirm, "Select Group"),
                ],
                true,
            ),
            GroupsUIFocus::IncludedSubvols | GroupsUIFocus::ExcludedSubvols => {
                (vec![(Confirm, "Toggle")], true)
            }

            GroupsUIFocus::DeleteGroupConfirming { .. } => {
                (globals::YES_NO_PROMPTS.to_vec(), false)
            }
            GroupsUIFocus::InvalidGroupNamePopup => (globals::CONFIRM_PROMPTS.to_vec(), false),

            GroupsUIFocus::CreateGroupInputing | GroupsUIFocus::RenameGroupInputing { .. } => {
                (vec![], false)
            }
        }
    }
}
