use std::fmt::Debug;

use std::sync::Arc;

use cosmic::app::{Command, Core, Settings};
use cosmic::cosmic_config::{Config, ConfigSet, CosmicConfigEntry};
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::id::Id;
use cosmic::iced::subscription::events_with;
use cosmic::iced::wayland::actions::data_device::ActionInner;
use cosmic::iced::wayland::actions::layer_surface::SctkLayerSurfaceSettings;
use cosmic::iced::wayland::actions::popup::{SctkPopupSettings, SctkPositioner};
use cosmic::iced::wayland::layer_surface::{
    destroy_layer_surface, get_layer_surface, Anchor, KeyboardInteractivity,
};
use cosmic::iced::widget::{column, container, horizontal_rule, row, scrollable, text};
use cosmic::iced::{alignment::Horizontal, executor, Alignment, Length};
use cosmic::iced::{Color, Limits, Subscription};
use cosmic::iced_core::Rectangle;
use cosmic::iced_runtime::core::event::wayland::LayerEvent;
use cosmic::iced_runtime::core::event::{wayland, PlatformSpecific};
use cosmic::iced_runtime::core::keyboard::KeyCode;
use cosmic::iced_runtime::core::window::Id as SurfaceId;
use cosmic::iced_sctk::commands;
use cosmic::iced_sctk::commands::data_device::cancel_dnd;
use cosmic::iced_style::application::{self, Appearance};
use cosmic::iced_widget::text_input::focus;
use cosmic::iced_widget::{horizontal_space, mouse_area, Container};
use cosmic::theme::{self, Button, TextInput};
use cosmic::widget::button::StyleSheet as ButtonStyleSheet;
use cosmic::widget::icon::{from_name, from_path};
use cosmic::widget::{button, icon, search_input, text_input, tooltip, Column};
use cosmic::{iced, sctk, Element, Theme};
use iced::wayland::actions::layer_surface::IcedMargin;

use itertools::Itertools;
use log::error;
use once_cell::sync::Lazy;

use crate::app_group::{AppLibraryConfig, DesktopEntryData};
use crate::config::APP_ID;
use crate::fl;
use crate::subscriptions::desktop_files::desktop_files;
use crate::subscriptions::toggle_dbus::dbus_toggle;
use crate::widgets::application::ApplicationButton;
use crate::widgets::group::GroupButton;

// popovers should show options, but also the desktop info options
// should be a way to add apps to groups
// should be a way to remove apps from groups

static SEARCH_ID: Lazy<Id> = Lazy::new(|| Id::new("search"));
static EDIT_GROUP_ID: Lazy<Id> = Lazy::new(|| Id::new("edit_group"));
static NEW_GROUP_ID: Lazy<Id> = Lazy::new(|| Id::new("new_group"));

static CREATE_NEW: Lazy<String> = Lazy::new(|| fl!("create-new"));
static SEARCH_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!("search-placeholder"));
static NEW_GROUP_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!("new-group-placeholder"));
static SAVE: Lazy<String> = Lazy::new(|| fl!("save"));
static CANCEL: Lazy<String> = Lazy::new(|| fl!("cancel"));
static RUN: Lazy<String> = Lazy::new(|| fl!("run"));
static REMOVE: Lazy<String> = Lazy::new(|| fl!("remove"));

pub(crate) const WINDOW_ID: SurfaceId = SurfaceId(1);
const NEW_GROUP_WINDOW_ID: SurfaceId = SurfaceId(2);
const DELETE_GROUP_WINDOW_ID: SurfaceId = SurfaceId(3);
pub(crate) const DND_ICON_ID: SurfaceId = SurfaceId(4);
pub(crate) const MENU_ID: SurfaceId = SurfaceId(5);

pub fn run() -> cosmic::iced::Result {
    cosmic::app::run::<CosmicAppLibrary>(
        Settings::default()
            .antialiasing(true)
            .client_decorations(true)
            .debug(false)
            .default_icon_theme("Pop")
            .default_text_size(16.0)
            .scale_factor(1.0)
            .no_main_window(true),
        (),
    )?;
    Ok(())
}

#[derive(Default)]
struct CosmicAppLibrary {
    search_value: String,
    entry_path_input: Vec<DesktopEntryData>,
    menu: Option<usize>,
    helper: Option<Config>,
    config: AppLibraryConfig,
    cur_group: usize,
    active_surface: bool,
    locale: Option<String>,
    edit_name: Option<String>,
    new_group: Option<String>,
    dnd_icon: Option<usize>,
    offer_group: Option<usize>,
    scroll_offset: f32,
    core: Core,
    group_to_delete: Option<usize>,
}

#[derive(Clone, Debug)]
enum Message {
    InputChanged(String),
    Layer(LayerEvent),
    Toggle,
    Hide,
    Clear,
    ActivateApp(usize),
    SelectGroup(usize),
    Delete(usize),
    ConfirmDelete,
    CancelDelete,
    StartEditName(String),
    EditName(String),
    SubmitName,
    StartNewGroup,
    NewGroup(String),
    SubmitNewGroup,
    CancelNewGroup,
    LoadApps,
    OpenContextMenu(Rectangle, usize),
    CloseContextMenu,
    SelectAction(MenuAction),
    StartDrag(usize),
    DndCommandProduced(DndCommand),
    FinishDrag(bool),
    CancelDrag,
    StartDndOffer(usize),
    FinishDndOffer(usize, DesktopEntryData),
    LeaveDndOffer,
    Ignore,
    ScrollYOffset(f32),
}

#[derive(Clone)]
struct DndCommand(Arc<Box<dyn Send + Sync + Fn() -> ActionInner>>);

impl Debug for DndCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DndCommand").finish()
    }
}

#[derive(Clone, Debug)]
enum MenuAction {
    Remove,
    DesktopAction(String),
}

impl CosmicAppLibrary {
    pub fn load_apps(&mut self) {
        self.entry_path_input =
            self.config
                .filtered(self.cur_group, self.locale.as_deref(), &self.search_value);
        self.entry_path_input.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl cosmic::Application for CosmicAppLibrary {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();
    const APP_ID: &'static str = "com.system76.CosmicAppLibrary";

    fn core(&self) -> &Core {
        &self.core
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::InputChanged(value) => {
                self.search_value = value;
                self.load_apps();
            }
            Message::Layer(e) => match e {
                LayerEvent::Focused => {
                    return text_input::focus(SEARCH_ID.clone());
                }
                LayerEvent::Unfocused => {
                    if self.active_surface
                        && self.new_group.is_none()
                        && self.group_to_delete.is_none()
                    {
                        self.active_surface = false;
                        return iced::Command::batch(vec![
                            destroy_layer_surface(WINDOW_ID),
                            iced::Command::perform(async {}, |_| {
                                cosmic::app::Message::App(Message::Clear)
                            }),
                        ]);
                    }
                }
                _ => {}
            },
            Message::Hide => {
                if self.menu.take().is_some() {
                    return commands::popup::destroy_popup(MENU_ID);
                }
                if self.active_surface {
                    self.active_surface = false;
                    self.edit_name = None;
                    self.new_group = None;
                    return iced::Command::batch(vec![
                        destroy_layer_surface(NEW_GROUP_WINDOW_ID),
                        destroy_layer_surface(DELETE_GROUP_WINDOW_ID),
                        destroy_layer_surface(WINDOW_ID),
                        cancel_dnd(),
                        iced::Command::perform(async {}, |_| {
                            cosmic::app::Message::App(Message::Clear)
                        }),
                    ]);
                }
            }
            Message::Clear => {
                self.search_value.clear();
                self.edit_name = None;
                self.cur_group = 0;
                self.group_to_delete = None;
                self.load_apps();
            }
            Message::ActivateApp(i) => {
                self.edit_name = None;
                if let Some(de) = self.entry_path_input.get(i) {
                    let mut exec = shlex::Shlex::new(&de.exec);
                    let mut cmd = match exec.next() {
                        Some(cmd) if !cmd.contains("=") => tokio::process::Command::new(cmd),
                        _ => return Command::none(),
                    };
                    for arg in exec {
                        // TODO handle "%" args here if necessary?
                        if !arg.starts_with("%") {
                            cmd.arg(arg);
                        }
                    }
                    let _ = cmd.spawn();
                    return self.update(Message::Hide);
                }
            }
            Message::SelectGroup(i) => {
                self.edit_name = None;
                self.search_value.clear();
                self.cur_group = i;
                self.scroll_offset = 0.0;
                self.load_apps();
            }
            Message::Toggle => {
                if self.active_surface {
                    self.active_surface = false;
                    self.new_group = None;
                    return Command::batch(vec![
                        destroy_layer_surface(NEW_GROUP_WINDOW_ID),
                        destroy_layer_surface(DELETE_GROUP_WINDOW_ID),
                        destroy_layer_surface(WINDOW_ID),
                    ]);
                } else {
                    let mut cmds = Vec::new();
                    self.edit_name = None;
                    self.search_value = "".to_string();
                    self.active_surface = true;
                    self.scroll_offset = 0.0;
                    self.cur_group = 0;
                    cmds.push(text_input::focus(SEARCH_ID.clone()));
                    cmds.push(get_layer_surface(SctkLayerSurfaceSettings {
                        id: WINDOW_ID,
                        keyboard_interactivity: KeyboardInteractivity::Exclusive,
                        anchor: Anchor::TOP,
                        namespace: "app-library".into(),
                        size: None,
                        margin: IcedMargin {
                            top: 16,
                            right: 0,
                            bottom: 0,
                            left: 0,
                        },
                        ..Default::default()
                    }));
                    return Command::batch(cmds);
                }
            }
            Message::LoadApps => {
                self.load_apps();
            }
            Message::Ignore => {}
            Message::Delete(group) => {
                self.group_to_delete = Some(group);
                return get_layer_surface(SctkLayerSurfaceSettings {
                    id: DELETE_GROUP_WINDOW_ID,
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                    anchor: Anchor::empty(),
                    namespace: "dialog".into(),
                    size: None,
                    ..Default::default()
                });
            }
            Message::EditName(name) => {
                self.edit_name = Some(name);
            }
            Message::SubmitName => {
                if let Some(name) = self.edit_name.take() {
                    self.config.set_name(self.cur_group, name);
                }
                if let Some(helper) = self.helper.as_ref() {
                    if let Err(err) = self.config.write_entry(helper) {
                        error!("{:?}", err);
                    }
                }
            }
            Message::StartEditName(name) => {
                self.edit_name = Some(name);
                return focus(NEW_GROUP_ID.clone());
            }
            Message::StartNewGroup => {
                self.new_group = Some(String::new());
                return Command::batch(vec![
                    get_layer_surface(SctkLayerSurfaceSettings {
                        id: NEW_GROUP_WINDOW_ID,
                        keyboard_interactivity: KeyboardInteractivity::Exclusive,
                        anchor: Anchor::empty(),
                        namespace: "dialog".into(),
                        size: None,
                        ..Default::default()
                    }),
                    text_input::focus(NEW_GROUP_ID.clone()),
                ]);
            }
            Message::NewGroup(group_name) => {
                self.new_group = Some(group_name);
            }
            Message::SubmitNewGroup => {
                if let Some(group_name) = self.new_group.take() {
                    self.config.add(group_name);
                }
                if let Some(helper) = self.helper.as_ref() {
                    if let Err(err) = self.config.write_entry(helper) {
                        error!("{:?}", err);
                    }
                }
                return destroy_layer_surface(NEW_GROUP_WINDOW_ID);
            }
            Message::CancelNewGroup => {
                self.new_group = None;
                return destroy_layer_surface(NEW_GROUP_WINDOW_ID);
            }
            Message::OpenContextMenu(rect, i) => {
                if let Some(i) = self.menu.take() {
                    if i == i {
                        return commands::popup::destroy_popup(MENU_ID.clone());
                    }
                } else {
                    self.menu = Some(i);
                    return commands::popup::get_popup(SctkPopupSettings {
                        parent: WINDOW_ID,
                        id: MENU_ID,
                        positioner: SctkPositioner {
                            size: None,
                            size_limits: Limits::NONE.min_width(1.0).min_height(1.0).max_width(300.0).max_height(800.0),
                            anchor_rect: Rectangle {
                                x: rect.x as i32,
                                y: rect.y as i32 - self.scroll_offset as i32,
                                width: rect.width as i32,
                                height: rect.height as i32,
                            },
                            anchor:
                                sctk::reexports::protocols::xdg::shell::client::xdg_positioner::Anchor::Right,
                            gravity: sctk::reexports::protocols::xdg::shell::client::xdg_positioner::Gravity::Right,
                            reactive: true,
                            ..Default::default()
                        },
                        grab: true,
                        parent_size: None,
                    });
                }
            }
            Message::CloseContextMenu => {
                self.menu = None;
                return commands::popup::destroy_popup(MENU_ID.clone());
            }
            Message::SelectAction(action) => {
                if let Some(info) = self.menu.take().and_then(|i| self.entry_path_input.get(i)) {
                    match action {
                        MenuAction::Remove => {
                            self.config.remove_entry(self.cur_group, &info.id);
                            if let Some(helper) = self.helper.as_ref() {
                                if let Err(err) = self.config.write_entry(helper) {
                                    error!("{:?}", err);
                                }
                            }
                            self.load_apps();
                        }
                        MenuAction::DesktopAction(exec) => {
                            let mut exec = shlex::Shlex::new(&exec);

                            let mut cmd = match exec.next() {
                                Some(cmd) if !cmd.contains("=") => {
                                    tokio::process::Command::new(cmd)
                                }
                                _ => return Command::none(),
                            };
                            for arg in exec {
                                // TODO handle "%" args here if necessary?
                                if !arg.starts_with("%") {
                                    cmd.arg(arg);
                                }
                            }
                            let _ = cmd.spawn();
                            return iced::Command::batch(vec![
                                commands::popup::destroy_popup(MENU_ID.clone()),
                                iced::Command::perform(async {}, |_| {
                                    cosmic::app::Message::App(Message::Hide)
                                }),
                            ]);
                        }
                    }
                }
            }
            Message::StartDrag(i) => {
                // self.dnd_icon = self.entry_path_input.get(i).map(|e| e.icon.clone());
                self.dnd_icon = Some(i);
            }
            Message::FinishDrag(copy) => {
                if !copy {
                    if let Some(info) = self
                        .dnd_icon
                        .take()
                        .and_then(|i| self.entry_path_input.get(i))
                    {
                        let _ = self.config.remove_entry(self.cur_group, &info.id);
                        if let Some(helper) = self.helper.as_ref() {
                            if let Err(err) = self.config.write_entry(helper) {
                                error!("{:?}", err);
                            }
                        }
                        self.load_apps();
                    }
                }
            }
            Message::CancelDrag => {
                self.dnd_icon = None;
            }
            Message::DndCommandProduced(DndCommand(cmd)) => {
                let action = cmd();
                return commands::data_device::action(action);
            }
            Message::StartDndOffer(i) => {
                self.offer_group = Some(i);
            }
            Message::FinishDndOffer(i, entry) => {
                self.offer_group = None;

                if self.cur_group == i {
                    return Command::none();
                }

                self.config.add_entry(i, &entry.id);
                if let Some(helper) = self.helper.as_ref() {
                    if let Err(err) = self.config.write_entry(helper) {
                        error!("{:?}", err);
                    }
                }
            }
            Message::LeaveDndOffer => {
                self.offer_group = None;
            }
            Message::ScrollYOffset(y) => {
                self.scroll_offset = y;
            }
            Message::ConfirmDelete => {
                if let Some(group) = self.group_to_delete.take() {
                    self.config.remove(group);
                    if let Some(helper) = self.helper.as_ref() {
                        if let Err(err) = self.config.write_entry(helper) {
                            error!("{:?}", err);
                        }
                    }
                    self.cur_group = 0;
                    self.load_apps();
                }
                return destroy_layer_surface(DELETE_GROUP_WINDOW_ID);
            }
            Message::CancelDelete => {
                self.group_to_delete = None;
                return destroy_layer_surface(DELETE_GROUP_WINDOW_ID);
            }
        }
        Command::none()
    }
    fn view(&self) -> Element<Message> {
        unimplemented!()
    }

    fn view_window(&self, id: SurfaceId) -> Element<Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();
        let spacing = &cosmic.spacing;
        if id == DND_ICON_ID {
            let Some(icon_path) = self
                .dnd_icon
                .clone()
                .and_then(|i| self.entry_path_input.get(i).map(|e| e.icon.clone()))
            else {
                return container(horizontal_space(Length::Fixed(1.0)))
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(1.0))
                    .into();
            };
            return icon(from_path(icon_path).into()).size(32).into();
        }
        if id == MENU_ID {
            let Some((menu, i)) = self
                .menu
                .as_ref()
                .and_then(|i| self.entry_path_input.get(*i).map(|e| (e, i)))
            else {
                return container(horizontal_space(Length::Fixed(1.0)))
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(1.0))
                    .into();
            };
            let mut list_column = column![button(text(RUN.clone()))
                .style(theme::Button::Custom {
                    active: Box::new(|focused, theme| {
                        let mut appearance = theme.active(focused, &theme::Button::Text);
                        appearance.border_radius = 0.0.into();
                        appearance
                    }),
                    hovered: Box::new(|focused, theme| {
                        let mut appearance = theme.hovered(focused, &theme::Button::Text);
                        appearance.border_radius = 0.0.into();
                        appearance
                    }),
                    disabled: Box::new(|theme| {
                        let mut appearance = theme.disabled(&theme::Button::Text);
                        appearance.border_radius = 0.0.into();
                        appearance
                    }),
                    pressed: Box::new(|focused, theme| {
                        let mut appearance = theme.pressed(focused, &theme::Button::Text);
                        appearance.border_radius = 0.0.into();
                        appearance
                    })
                })
                .on_press(Message::ActivateApp(*i))
                .padding([spacing.space_xxs, spacing.space_m])
                .width(Length::Fill)]
            .spacing(8);

            if menu.desktop_actions.len() > 0 {
                list_column = list_column.push(menu_divider(spacing));
                for action in menu.desktop_actions.iter() {
                    list_column = list_column.push(
                        button(text(&action.name))
                            .style(theme::Button::Custom {
                                active: Box::new(|focused, theme| {
                                    let mut appearance =
                                        theme.active(focused, &theme::Button::Text);
                                    appearance.border_radius = 0.0.into();
                                    appearance
                                }),
                                hovered: Box::new(|focused, theme| {
                                    let mut appearance =
                                        theme.hovered(focused, &theme::Button::Text);
                                    appearance.border_radius = 0.0.into();
                                    appearance
                                }),
                                disabled: Box::new(|theme| {
                                    let mut appearance = theme.disabled(&theme::Button::Text);
                                    appearance.border_radius = 0.0.into();
                                    appearance
                                }),
                                pressed: Box::new(|focused, theme| {
                                    let mut appearance =
                                        theme.pressed(focused, &theme::Button::Text);
                                    appearance.border_radius = 0.0.into();
                                    appearance
                                }),
                            })
                            .on_press(Message::SelectAction(MenuAction::DesktopAction(
                                action.exec.clone(),
                            )))
                            .padding([spacing.space_xxs, spacing.space_m])
                            .width(Length::Fill),
                    );
                }
                list_column = list_column.push(menu_divider(spacing));
            }

            list_column = list_column.push(
                button(text(REMOVE.clone()))
                    .style(theme::Button::Custom {
                        active: Box::new(|focused, theme| {
                            let mut appearance = theme.active(focused, &theme::Button::Text);
                            appearance.border_radius = 0.0.into();
                            appearance
                        }),
                        hovered: Box::new(|focused, theme| {
                            let mut appearance = theme.hovered(focused, &theme::Button::Text);
                            appearance.border_radius = 0.0.into();
                            appearance
                        }),
                        disabled: Box::new(|theme| {
                            let mut appearance = theme.disabled(&theme::Button::Text);
                            appearance.border_radius = 0.0.into();
                            appearance
                        }),
                        pressed: Box::new(|focused, theme| {
                            let mut appearance = theme.pressed(focused, &theme::Button::Text);
                            appearance.border_radius = 0.0.into();
                            appearance
                        }),
                    })
                    .on_press(Message::SelectAction(MenuAction::Remove))
                    .padding([spacing.space_xxs, spacing.space_m])
                    .width(Length::Fill),
            );

            return container(scrollable(list_column))
                .style(theme::Container::Custom(Box::new(|theme| {
                    container::Appearance {
                        text_color: Some(theme.cosmic().on_bg_color().into()),
                        background: Some(Color::from(theme.cosmic().background.base).into()),
                        border_radius: 16.0.into(),
                        border_width: 1.0,
                        border_color: theme.cosmic().bg_divider().into(),
                        icon_color: Some(theme.cosmic().on_bg_color().into()),
                    }
                })))
                .padding([
                    spacing.space_s,
                    spacing.space_none,
                    spacing.space_s,
                    spacing.space_none,
                ])
                .into();
        }
        if id == NEW_GROUP_WINDOW_ID {
            let Some(group_name) = self.new_group.as_ref() else {
                return container(horizontal_space(Length::Fixed(1.0)))
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(1.0))
                    .into();
            };
            let dialog = column![
                container(text(CREATE_NEW.as_str()).size(24))
                    .align_x(Horizontal::Left)
                    .width(Length::Fixed(432.0)),
                text_input("", &group_name)
                    .label(&NEW_GROUP_PLACEHOLDER)
                    .on_input(Message::NewGroup)
                    .on_submit(Message::SubmitNewGroup)
                    .width(Length::Fixed(432.0))
                    .size(14)
                    .id(NEW_GROUP_ID.clone()),
                container(
                    row![
                        button(
                            text(&CANCEL.as_str())
                                .horizontal_alignment(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .on_press(Message::CancelNewGroup)
                        .padding([spacing.space_xxs, spacing.space_s])
                        .width(142),
                        button(
                            text(&SAVE.as_str())
                                .horizontal_alignment(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .style(Button::Suggested)
                        .on_press(Message::SubmitNewGroup)
                        .padding([spacing.space_xxs, spacing.space_s])
                        .width(142),
                    ]
                    .spacing(spacing.space_s)
                )
                .width(Length::Fixed(432.0))
                .align_x(Horizontal::Right)
            ]
            .align_items(Alignment::Center)
            .spacing(spacing.space_s);
            return container(dialog)
                .style(theme::Container::Custom(Box::new(|theme| {
                    container::Appearance {
                        text_color: Some(theme.cosmic().on_bg_color().into()),
                        icon_color: Some(theme.cosmic().on_bg_color().into()),
                        background: Some(Color::from(theme.cosmic().background.base).into()),
                        border_radius: 16.0.into(),
                        border_width: 1.0,
                        border_color: theme.cosmic().bg_divider().into(),
                    }
                })))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .padding(spacing.space_s)
                .into();
        }

        if id == DELETE_GROUP_WINDOW_ID {
            let dialog = column![
                row![
                    container(
                        icon(from_name("edit-delete-symbolic").into())
                            .width(Length::Fixed(48.0))
                            .height(Length::Fixed(48.0))
                    )
                    .padding(8),
                    column![
                        text(&fl!("delete-folder").as_str()).size(24),
                        text(&fl!("delete-folder", "msg").as_str())
                    ]
                    .spacing(8)
                    .width(Length::Fixed(360.0))
                ]
                .spacing(16),
                container(
                    row![
                        button(
                            text(&CANCEL.as_str())
                                .horizontal_alignment(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .on_press(Message::CancelDelete)
                        .padding([spacing.space_xxs, spacing.space_m])
                        .width(142),
                        button(
                            text(&fl!("delete"))
                                .horizontal_alignment(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .style(Button::Destructive)
                        .on_press(Message::ConfirmDelete)
                        .padding([spacing.space_xxs, spacing.space_m])
                        .width(142),
                    ]
                    .spacing(spacing.space_s)
                )
                .width(Length::Fixed(432.0))
                .align_x(Horizontal::Right)
            ]
            .align_items(Alignment::Center)
            .spacing(spacing.space_l);
            return container(dialog)
                .style(theme::Container::Custom(Box::new(|theme| {
                    container::Appearance {
                        text_color: Some(theme.cosmic().on_bg_color().into()),
                        icon_color: Some(theme.cosmic().on_bg_color().into()),
                        background: Some(Color::from(theme.cosmic().background.base).into()),
                        border_radius: 16.0.into(),
                        border_width: 1.0,
                        border_color: theme.cosmic().bg_divider().into(),
                    }
                })))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .padding(spacing.space_m)
                .into();
        }
        let cur_group = self.config.groups()[self.cur_group];
        let top_row = if self.cur_group == 0 {
            row![search_input(&SEARCH_PLACEHOLDER, &self.search_value)
                .on_input(Message::InputChanged)
                .on_paste(Message::InputChanged)
                .style(TextInput::Search)
                .width(Length::Fixed(400.0))
                .size(14)
                .id(SEARCH_ID.clone())]
            .spacing(spacing.space_xxs)
            .padding(spacing.space_l)
        } else {
            row![
                horizontal_space(Length::FillPortion(1)),
                if let Some(edit_name) = self.edit_name.as_ref() {
                    container(
                        text_input(&cur_group.name(), edit_name)
                            .on_input(Message::EditName)
                            .on_paste(Message::EditName)
                            .on_clear(Message::EditName(String::new()))
                            .on_submit(Message::SubmitName)
                            .id(EDIT_GROUP_ID.clone())
                            .width(Length::Fixed(200.0))
                            .size(14),
                    )
                } else {
                    container(text(&cur_group.name()).size(24))
                },
                row![
                    horizontal_space(Length::Fill),
                    tooltip(
                        {
                            let mut b = button(
                                icon(from_name("edit-symbolic").into())
                                    .width(Length::Fixed(32.0))
                                    .height(Length::Fixed(32.0)),
                            )
                            .padding(spacing.space_xs)
                            .style(Button::Icon);
                            if self.edit_name.is_none() {
                                b = b.on_press(Message::StartEditName(cur_group.name()));
                            }
                            b
                        },
                        fl!("rename"),
                        tooltip::Position::Bottom
                    ),
                    tooltip(
                        button(
                            icon(from_name("edit-delete-symbolic").into())
                                .width(Length::Fixed(32.0))
                                .height(Length::Fixed(32.0)),
                        )
                        .padding(spacing.space_xs)
                        .style(Button::Icon)
                        .on_press(Message::Delete(self.cur_group)),
                        fl!("delete"),
                        tooltip::Position::Bottom
                    )
                ]
                .spacing(spacing.space_xxs)
                .width(Length::FillPortion(1))
            ]
            .align_items(Alignment::Center)
            .padding(spacing.space_l)
        };

        // TODO grid widget in libcosmic
        let app_grid_list: Vec<_> = self
            .entry_path_input
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let mut b = ApplicationButton::new(
                    &entry,
                    Message::Ignore,
                    move |rect| Message::OpenContextMenu(rect, i),
                    if self.menu.is_some() {
                        None
                    } else {
                        Some(Message::Ignore)
                    },
                    spacing,
                );
                if self.menu.is_none() {
                    b = b
                        .on_pressed(Message::ActivateApp(i))
                        .on_cancel(Message::CancelDrag)
                        .on_finish(Message::FinishDrag)
                        .on_create_dnd_source(Message::StartDrag(i))
                        .on_dnd_command_produced(|c| {
                            Message::DndCommandProduced(DndCommand(Arc::new(c)))
                        });
                }
                b.into()
            })
            .chunks(7)
            .into_iter()
            .map(|row_chunk| {
                let mut new_row = row_chunk.collect_vec();
                let missing = 7 - new_row.len();
                if missing > 0 {
                    new_row.push(
                        iced::widget::horizontal_space(Length::FillPortion(
                            missing.try_into().unwrap(),
                        ))
                        .into(),
                    );
                }
                row(new_row).spacing(spacing.space_xxs).into()
            })
            .collect();

        let app_scrollable = container(
            scrollable(
                column(app_grid_list)
                    .width(Length::Fill)
                    .spacing(spacing.space_xxs)
                    .padding([spacing.space_none, spacing.space_xxl]),
            )
            .on_scroll(|viewport| Message::ScrollYOffset(viewport.absolute_offset().y))
            .id(Id::new(
                self.config
                    .groups()
                    .get(self.cur_group)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| "unknown-group".to_string()),
            ))
            .height(Length::Fixed(600.0)),
        );

        // TODO use the spacing variables from the theme
        let (group_icon_size, h_padding, group_width, group_height, chunks) =
            if self.config.groups().len() > 15 {
                (16.0, spacing.space_xxs, 96.0, 60.0, 11)
            } else {
                (32.0, spacing.space_s, 128.0, 76.0, 8)
            };

        let mut add_group_btn = Some(
            button(
                column![
                    container(
                        icon(from_name("folder-new-symbolic").into())
                            .width(Length::Fixed(group_icon_size))
                            .height(Length::Fixed(group_icon_size))
                    )
                    .padding(spacing.space_xxs),
                    text("Add group").horizontal_alignment(Horizontal::Center)
                ]
                .align_items(Alignment::Center)
                .width(Length::Fill),
            )
            .height(Length::Fixed(group_height))
            .width(Length::Fixed(group_width))
            .style(theme::Button::IconVertical)
            .padding([spacing.space_none, h_padding, spacing.space_xxs, h_padding])
            .on_press(Message::StartNewGroup),
        );
        let mut group_rows: Vec<_> = self
            .config
            .groups()
            .chunks(chunks)
            .map(|groups| {
                let mut group_row = row![]
                    .spacing(spacing.space_xxs)
                    .padding([spacing.space_s, spacing.space_none])
                    .align_items(Alignment::Center);
                for (i, group) in groups.iter().enumerate() {
                    let mut group_button = GroupButton::new(
                        group.name(),
                        &group.icon,
                        if self.menu.is_some() {
                            None
                        } else {
                            Some(Message::SelectGroup(i))
                        },
                        if self.offer_group == Some(i)
                            || (self.cur_group == i && self.offer_group.is_none())
                        {
                            // TODO customize the IconVertical to highlight in the way we need
                            Button::Custom {
                                active: Box::new(|focused, theme| {
                                    let s = theme.pressed(focused, &Button::IconVertical);
                                    s
                                }),
                                disabled: Box::new(|theme| {
                                    let s = theme.disabled(&Button::IconVertical);
                                    s
                                }),
                                hovered: Box::new(|focused, theme| {
                                    let s = theme.hovered(focused, &Button::IconVertical);
                                    s
                                }),
                                pressed: Box::new(|focused, theme| {
                                    let s = theme.pressed(focused, &Button::IconVertical);
                                    s
                                }),
                            }
                        } else {
                            Button::IconVertical
                        },
                        group_icon_size,
                        [spacing.space_none, h_padding, spacing.space_xxs, h_padding],
                        group_width,
                        group_height,
                        spacing,
                    );
                    if i != 0 {
                        group_button = group_button
                            .on_finish(move |entry| Message::FinishDndOffer(i, entry))
                            .on_cancel(Message::LeaveDndOffer)
                            .on_offer(Message::StartDndOffer(i))
                            .on_dnd_command_produced(|c| {
                                Message::DndCommandProduced(DndCommand(Arc::new(c)))
                            });
                    }

                    group_row = group_row.push(group_button);
                }
                if groups.len() < chunks {
                    group_row = group_row.push(add_group_btn.take().unwrap());
                }
                group_row
            })
            .collect();

        if let Some(add_group_button) = add_group_btn.take() {
            group_rows.push(
                row![add_group_button]
                    .height(Length::Fixed(100.0))
                    .spacing(8)
                    .padding([spacing.space_s, spacing.space_none])
                    .align_items(Alignment::Center),
            );
        };
        let group_rows =
            Column::with_children(group_rows.into_iter().map(|r| r.into()).collect_vec());

        let content = column![
            top_row,
            app_scrollable,
            container(horizontal_rule(1))
                .padding([spacing.space_none, spacing.space_xxl])
                .width(Length::Fill),
            group_rows
        ]
        .align_items(Alignment::Center);

        let window = container(content)
            .width(Length::Fixed(1200.0))
            .height(Length::Shrink)
            .style(theme::Container::Custom(Box::new(|theme| {
                container::Appearance {
                    text_color: Some(theme.cosmic().on_bg_color().into()),
                    background: Some(Color::from(theme.cosmic().background.base).into()),
                    border_radius: 16.0.into(),
                    border_width: 1.0,
                    border_color: theme.cosmic().bg_divider().into(),
                    icon_color: Some(theme.cosmic().on_bg_color().into()),
                }
            })))
            .center_x();
        mouse_area(window)
            .on_release(Message::CloseContextMenu)
            .on_right_release(Message::CloseContextMenu)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(
            vec![
                dbus_toggle(0).map(|_| Message::Toggle),
                desktop_files(0).map(|_| Message::LoadApps),
                events_with(|e, _status| match e {
                    cosmic::iced::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::Layer(e, ..),
                    )) => Some(Message::Layer(e)),
                    cosmic::iced::Event::Keyboard(cosmic::iced::keyboard::Event::KeyReleased {
                        key_code,
                        modifiers: _mods,
                    }) => match key_code {
                        KeyCode::Escape => Some(Message::Hide),
                        _ => None,
                    },
                    _ => None,
                }),
            ]
            .into_iter(),
        )
    }

    fn style(&self) -> Option<<Theme as application::StyleSheet>::Style> {
        Some(<Theme as application::StyleSheet>::Style::Custom(Box::new(
            |theme| Appearance {
                background_color: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
                text_color: theme.cosmic().on_bg_color().into(),
                icon_color: theme.cosmic().on_bg_color().into(),
            },
        )))
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(
        core: Core,
        _flags: Self::Flags,
    ) -> (Self, iced::Command<cosmic::app::Message<Self::Message>>) {
        let helper = AppLibraryConfig::helper();

        let config: AppLibraryConfig = helper
            .as_ref()
            .map(|helper| {
                AppLibraryConfig::get_entry(helper).unwrap_or_else(|(errors, config)| {
                    for err in errors {
                        error!("{:?}", err);
                    }
                    config
                })
            })
            .unwrap_or_default();
        (
            CosmicAppLibrary {
                locale: current_locale::current_locale().ok(),
                config,
                core,
                helper,
                ..Default::default()
            },
            Command::none(),
        )
    }
}

fn menu_divider<'a>(spacing: &Spacing) -> Container<'a, Message, cosmic::Renderer> {
    container(horizontal_rule(1))
        .padding([spacing.space_none, spacing.space_s])
        .width(Length::Fill)
}
