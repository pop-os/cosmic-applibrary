use std::fmt::Debug;

use std::sync::Arc;

use cosmic::cosmic_config::{config_subscription, Config, CosmicConfigEntry};
use cosmic::cosmic_theme::util::CssColor;
use cosmic::iced::id::Id;
use cosmic::iced::subscription::events_with;
use cosmic::iced::wayland::actions::data_device::ActionInner;
use cosmic::iced::wayland::actions::layer_surface::SctkLayerSurfaceSettings;
use cosmic::iced::wayland::actions::popup::{SctkPopupSettings, SctkPositioner};
use cosmic::iced::wayland::layer_surface::{
    destroy_layer_surface, get_layer_surface, Anchor, KeyboardInteractivity,
};
use cosmic::iced::wayland::InitialSurface;
use cosmic::iced::widget::{column, container, horizontal_rule, row, scrollable, text, text_input};
use cosmic::iced::{alignment::Horizontal, executor, Alignment, Application, Command, Length};
use cosmic::iced::{Color, Limits, Subscription};
use cosmic::iced_core::Rectangle;
use cosmic::iced_runtime::core::event::wayland::LayerEvent;
use cosmic::iced_runtime::core::event::{wayland, PlatformSpecific};
use cosmic::iced_runtime::core::keyboard::KeyCode;
use cosmic::iced_runtime::core::window::Id as SurfaceId;
use cosmic::iced_sctk::commands;
use cosmic::iced_style::{
    application::{self, Appearance},
    button::StyleSheet as ButtonStyleSheet,
};
use cosmic::iced_widget::text_input::{focus, Icon, Side};
use cosmic::iced_widget::{horizontal_space, mouse_area, Container};
use cosmic::theme::{self, Button, TextInput};
use cosmic::widget::{button, icon};
use cosmic::{iced, sctk, settings, Element, Theme};
use iced::wayland::actions::layer_surface::IcedMargin;

use itertools::Itertools;
use log::error;
use once_cell::sync::Lazy;

use crate::app_group::{AppLibraryConfig, DesktopEntryData};
use crate::config::APP_ID;
use crate::subscriptions::desktop_files::desktop_files;
use crate::subscriptions::toggle_dbus::dbus_toggle;
use crate::widgets::application::ApplicationButton;
use crate::widgets::group::GroupButton;
use crate::{config, fl};

// popovers should show options, but also the desktop info options
// should be a way to add apps to groups
// should be a way to remove apps from groups

static SEARCH_ID: Lazy<Id> = Lazy::new(|| Id::new("search"));
static EDIT_GROUP_ID: Lazy<Id> = Lazy::new(|| Id::new("edit_group"));
static NEW_GROUP_ID: Lazy<Id> = Lazy::new(|| Id::new("new_group"));

static SEARCH_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!("search-placeholder"));
static NEW_GROUP_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!("new-group-placeholder"));
static OK: Lazy<String> = Lazy::new(|| fl!("ok"));
static CANCEL: Lazy<String> = Lazy::new(|| fl!("cancel"));
static RUN: Lazy<String> = Lazy::new(|| fl!("run"));
static REMOVE: Lazy<String> = Lazy::new(|| fl!("remove"));

pub(crate) const WINDOW_ID: SurfaceId = SurfaceId(1);
const NEW_GROUP_WINDOW_ID: SurfaceId = SurfaceId(2);
pub(crate) const DND_ICON_ID: SurfaceId = SurfaceId(3);
pub(crate) const MENU_ID: SurfaceId = SurfaceId(4);

pub fn run() -> cosmic::iced::Result {
    let mut settings = settings();
    settings.exit_on_close_request = false;
    settings.initial_surface = InitialSurface::None;
    CosmicAppLibrary::run(settings)
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
    theme: Theme,
    locale: Option<String>,
    edit_name: Option<String>,
    new_group: Option<String>,
    dnd_icon: Option<usize>,
    offer_group: Option<usize>,
    scroll_offset: f32,
}

#[derive(Clone, Debug)]
enum Message {
    InputChanged(String),
    Closed(SurfaceId),
    Layer(LayerEvent),
    Toggle,
    Hide,
    Clear,
    ActivateApp(usize),
    SelectGroup(usize),
    Delete(usize),
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
    Theme(Theme),
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

fn theme() -> Theme {
    let Ok(helper) = cosmic::cosmic_config::Config::new(
        cosmic::cosmic_theme::NAME,
        cosmic::cosmic_theme::Theme::<CssColor>::version() as u64,
    ) else {
        return cosmic::theme::Theme::dark();
    };
    let t = cosmic::cosmic_theme::Theme::get_entry(&helper)
        .map(|t| t.into_srgba())
        .unwrap_or_else(|(errors, theme)| {
            for err in errors {
                error!("{:?}", err);
            }
            theme.into_srgba()
        });
    cosmic::theme::Theme::custom(Arc::new(t))
}

impl Application for CosmicAppLibrary {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let helper = Config::new(APP_ID, AppLibraryConfig::version()).ok();

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
                helper,
                config,
                theme: theme(),
                ..Default::default()
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        config::APP_ID.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Theme(t) => {
                self.theme = t;
            }
            Message::InputChanged(value) => {
                self.search_value = value;
                self.load_apps();
            }
            Message::Closed(id) => {
                if self.active_surface && id == WINDOW_ID {
                    self.active_surface = false;
                    self.edit_name = None;
                    self.new_group = None;
                    return Command::perform(async {}, |_| Message::Clear);
                }
                // TODO handle popups closed
            }
            Message::Layer(e) => match e {
                LayerEvent::Focused => {
                    return text_input::focus(SEARCH_ID.clone());
                }
                LayerEvent::Unfocused => {
                    if self.active_surface && self.new_group.is_none() {
                        self.active_surface = false;
                        return Command::batch(vec![
                            destroy_layer_surface(WINDOW_ID),
                            Command::perform(async {}, |_| Message::Clear),
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
                    return Command::batch(vec![
                        destroy_layer_surface(NEW_GROUP_WINDOW_ID),
                        destroy_layer_surface(WINDOW_ID),
                        Command::perform(async {}, |_| Message::Clear),
                    ]);
                }
            }
            Message::Clear => {
                self.search_value.clear();
                self.edit_name = None;
                self.cur_group = 0;
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
                    return Command::perform(async {}, |_| Message::Hide);
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
                        size: Some((Some(1200), Some(860))),
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
                self.config.remove(group);
                if let Some(helper) = self.helper.as_ref() {
                    if let Err(err) = self.config.write_entry(helper) {
                        error!("{:?}", err);
                    }
                }
                self.cur_group = 0;
                self.load_apps();
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
                return focus(SEARCH_ID.clone());
            }
            Message::StartNewGroup => {
                self.new_group = Some(String::new());
                return get_layer_surface(SctkLayerSurfaceSettings {
                    id: NEW_GROUP_WINDOW_ID,
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                    anchor: Anchor::empty(),
                    namespace: "dialog".into(),
                    size: None,
                    ..Default::default()
                });
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
                            return Command::perform(async {}, |_| Message::Hide);
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
        }
        Command::none()
    }

    fn view(&self, id: SurfaceId) -> Element<Message> {
        if id == DND_ICON_ID {
            let Some(icon_path) = self.dnd_icon.clone().and_then(|i| self.entry_path_input.get(i).map(|e| e.icon.clone())) else {
                return container(horizontal_space(Length::Fixed(1.0))).width(Length::Fixed(1.0)).height(Length::Fixed(1.0)).into();
            };
            return icon(icon_path, 64).into();
        }
        if id == MENU_ID {
            let Some((menu, i)) = self.menu.as_ref().and_then(|i| self.entry_path_input.get(*i).map(|e| (e, i))) else {
                return container(horizontal_space(Length::Fixed(1.0))).width(Length::Fixed(1.0)).height(Length::Fixed(1.0)).into();
            };
            let mut list_column = column![cosmic::iced::widget::button(text(RUN.clone()))
                .style(theme::Button::Custom {
                    active: Box::new(|theme| {
                        let mut appearance = theme.active(&theme::Button::Text);
                        appearance.border_radius = 0.0.into();
                        appearance
                    }),
                    hover: Box::new(|theme| {
                        let mut appearance = theme.hovered(&theme::Button::Text);
                        appearance.border_radius = 0.0.into();
                        appearance
                    })
                })
                .on_press(Message::ActivateApp(*i))
                .padding([8, 24])
                .width(Length::Fill)]
            .spacing(8);

            if menu.desktop_actions.len() > 0 {
                list_column = list_column.push(menu_divider());
                for action in menu.desktop_actions.iter() {
                    list_column = list_column.push(
                        cosmic::iced::widget::button(text(&action.name))
                            .style(theme::Button::Custom {
                                active: Box::new(|theme| {
                                    let mut appearance = theme.active(&theme::Button::Text);
                                    appearance.border_radius = 0.0.into();
                                    appearance
                                }),
                                hover: Box::new(|theme| {
                                    let mut appearance = theme.hovered(&theme::Button::Text);
                                    appearance.border_radius = 0.0.into();
                                    appearance
                                }),
                            })
                            .on_press(Message::SelectAction(MenuAction::DesktopAction(
                                action.exec.clone(),
                            )))
                            .padding([8, 24])
                            .width(Length::Fill),
                    );
                }
                list_column = list_column.push(menu_divider());
            }

            list_column = list_column.push(
                cosmic::iced::widget::button(text(REMOVE.clone()))
                    .style(theme::Button::Custom {
                        active: Box::new(|theme| {
                            let mut appearance = theme.active(&theme::Button::Text);
                            appearance.border_radius = 0.0.into();
                            appearance
                        }),
                        hover: Box::new(|theme| {
                            let mut appearance = theme.hovered(&theme::Button::Text);
                            appearance.border_radius = 0.0.into();
                            appearance
                        }),
                    })
                    .on_press(Message::SelectAction(MenuAction::Remove))
                    .padding([8, 24])
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
                    }
                })))
                .padding([16.0, 0.0, 16.0, 0.0])
                .into();
        }
        if id == NEW_GROUP_WINDOW_ID {
            let Some(group_name) = self.new_group.as_ref() else {
                return container(horizontal_space(Length::Fixed(1.0))).width(Length::Fixed(1.0)).height(Length::Fixed(1.0)).into();
            };
            let dialog = column![
                text_input(&NEW_GROUP_PLACEHOLDER, &group_name)
                    .on_input(Message::NewGroup)
                    .on_submit(Message::SubmitNewGroup)
                    .style(TextInput::Default)
                    .padding([8, 24])
                    .width(Length::Fixed(400.0))
                    .size(14)
                    .id(NEW_GROUP_ID.clone()),
                row![
                    button(theme::Button::Secondary)
                        .text(&OK)
                        .on_press(Message::SubmitNewGroup)
                        .padding([8, 24]),
                    button(theme::Button::Secondary)
                        .text(&CANCEL)
                        .on_press(Message::CancelNewGroup)
                        .padding([8, 24])
                ]
                .spacing(16.0)
            ]
            .align_items(Alignment::Center)
            .spacing(16.0);
            return container(dialog)
                .style(theme::Container::Custom(Box::new(|theme| {
                    container::Appearance {
                        text_color: Some(theme.cosmic().on_bg_color().into()),
                        background: Some(Color::from(theme.cosmic().background.base).into()),
                        border_radius: 16.0.into(),
                        border_width: 1.0,
                        border_color: theme.cosmic().bg_divider().into(),
                    }
                })))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .padding(16.0)
                .into();
        }
        let cur_group = self.config.groups()[self.cur_group];
        let top_row = if self.cur_group == 0 {
            row![text_input(&SEARCH_PLACEHOLDER, &self.search_value)
                .on_input(Message::InputChanged)
                .on_paste(Message::InputChanged)
                .style(TextInput::Search)
                .padding([8, 24])
                .width(Length::Fixed(400.0))
                .size(14)
                .icon(Icon {
                    font: iced::Font::default(),
                    code_point: '🔍',
                    size: Some(12.0),
                    spacing: 12.0,
                    side: Side::Left,
                })
                .id(SEARCH_ID.clone())]
            .spacing(8)
        } else if let Some(edit_name) = self.edit_name.as_ref() {
            row![
                horizontal_space(Length::FillPortion(1)),
                text_input(&cur_group.name(), edit_name)
                    .on_input(Message::EditName)
                    .on_paste(Message::EditName)
                    .on_submit(Message::SubmitName)
                    .id(EDIT_GROUP_ID.clone())
                    .style(TextInput::Default)
                    .padding([8, 24])
                    .width(Length::Fixed(200.0))
                    .size(14),
                button(theme::Button::Text)
                    .text(&OK)
                    .style(theme::Button::Primary)
                    .on_press(Message::SubmitName)
            ]
            .spacing(8.0)
            .width(Length::FillPortion(1))
        } else {
            row![
                horizontal_space(Length::FillPortion(1)),
                text(&cur_group.name()).size(24),
                row![
                    horizontal_space(Length::Fill),
                    button(theme::Button::Text)
                        .icon(theme::Svg::Symbolic, "edit-symbolic", 16)
                        .on_press(Message::StartEditName(cur_group.name())),
                    button(theme::Button::Text)
                        .icon(theme::Svg::Symbolic, "edit-delete-symbolic", 16)
                        .on_press(Message::Delete(self.cur_group))
                ]
                .spacing(8.0)
                .width(Length::FillPortion(1))
            ]
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
                row(new_row).spacing(8).padding([0, 16, 0, 0]).into()
            })
            .collect();

        let app_scrollable = scrollable(column(app_grid_list).width(Length::Fill).spacing(8))
            .on_scroll(|viewport| Message::ScrollYOffset(viewport.absolute_offset().y))
            .id(Id::new(
                self.config
                    .groups()
                    .get(self.cur_group)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| "unknown-group".to_string()),
            ))
            .height(Length::Fixed(600.0));

        let group_row = {
            let mut group_row = row![]
                .height(Length::Fixed(100.0))
                .spacing(8)
                .align_items(Alignment::Center);
            for (i, group) in self.config.groups().iter().enumerate() {
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
                        Button::Primary
                    } else {
                        Button::Secondary
                    },
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
            group_row = group_row.push(
                iced::widget::button(
                    column![
                        icon("folder-new-symbolic", 32),
                        text("Add group").horizontal_alignment(Horizontal::Center)
                    ]
                    .spacing(8)
                    .align_items(Alignment::Center)
                    .width(Length::Fill),
                )
                .height(Length::Fill)
                .width(Length::Fixed(128.0))
                .style(Button::Secondary)
                .padding([16, 8])
                .on_press(Message::StartNewGroup),
            );
            group_row
        };

        let content = column![top_row, app_scrollable, horizontal_rule(1), group_row]
            .spacing(16)
            .align_items(Alignment::Center)
            .padding([32, 64, 16, 64]);

        let window = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::Container::Custom(Box::new(|theme| {
                container::Appearance {
                    text_color: Some(theme.cosmic().on_bg_color().into()),
                    background: Some(Color::from(theme.cosmic().background.base).into()),
                    border_radius: 16.0.into(),
                    border_width: 1.0,
                    border_color: theme.cosmic().bg_divider().into(),
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
                config_subscription::<u64, cosmic::cosmic_theme::Theme<CssColor>>(
                    0,
                    cosmic::cosmic_theme::NAME.into(),
                    cosmic::cosmic_theme::Theme::<CssColor>::version() as u64,
                )
                .map(|(_, res)| {
                    let theme =
                        res.map(|theme| theme.into_srgba())
                            .unwrap_or_else(|(errors, theme)| {
                                for err in errors {
                                    error!("{:?}", err);
                                }
                                theme.into_srgba()
                            });
                    Message::Theme(cosmic::theme::Theme::custom(Arc::new(theme)))
                }),
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

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn style(&self) -> <Self::Theme as application::StyleSheet>::Style {
        <Self::Theme as application::StyleSheet>::Style::Custom(Box::new(|theme| Appearance {
            background_color: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
            text_color: theme.cosmic().on_bg_color().into(),
        }))
    }

    fn close_requested(&self, id: SurfaceId) -> Self::Message {
        Message::Closed(id)
    }
}

fn menu_divider<'a>() -> Container<'a, Message, cosmic::Renderer> {
    container(horizontal_rule(1))
        .padding([0, 16])
        .width(Length::Fill)
}
