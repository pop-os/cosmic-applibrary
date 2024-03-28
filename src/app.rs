use std::fmt::Debug;

use clap::Parser;
use cosmic::app::{
    Command, Core, CosmicFlags, DbusActivationDetails, DbusActivationMessage, Settings,
};
use cosmic::cosmic_config::{Config, CosmicConfigEntry};
use cosmic::cosmic_theme::Spacing;
use cosmic::desktop::DesktopEntryData;
use cosmic::iced::event::listen_with;
use cosmic::iced::id::Id;
use cosmic::iced::wayland::actions::data_device::ActionInner;
use cosmic::iced::wayland::actions::layer_surface::SctkLayerSurfaceSettings;
use cosmic::iced::wayland::actions::popup::{SctkPopupSettings, SctkPositioner};
use cosmic::iced::wayland::layer_surface::{
    destroy_layer_surface, get_layer_surface, Anchor, KeyboardInteractivity,
};
use cosmic::iced::widget::{column, container, horizontal_rule, row, scrollable, text};
use cosmic::iced::{alignment::Horizontal, executor, Alignment, Length};
use cosmic::iced::{Color, Limits, Subscription};
use cosmic::iced_core::alignment::Vertical;
use cosmic::iced_core::keyboard::key::Named;
use cosmic::iced_core::keyboard::Key;
use cosmic::iced_core::{Border, Padding, Rectangle, Shadow};
use cosmic::iced_runtime::core::event::wayland::LayerEvent;
use cosmic::iced_runtime::core::event::{wayland, PlatformSpecific};
use cosmic::iced_runtime::core::window::Id as SurfaceId;
use cosmic::iced_sctk::commands;
use cosmic::iced_sctk::commands::activation::request_token;
use cosmic::iced_sctk::commands::data_device::cancel_dnd;
use cosmic::iced_sctk::commands::popup::destroy_popup;
use cosmic::iced_style::application::{self, Appearance};
use cosmic::iced_widget::text_input::focus;
use cosmic::iced_widget::{horizontal_space, mouse_area, vertical_space, Container};
use cosmic::theme::{self, Button, TextInput};
use cosmic::widget::button::StyleSheet as ButtonStyleSheet;
use cosmic::widget::icon::from_name;
use cosmic::widget::{button, icon, search_input, text_input, tooltip, Column};
use cosmic::{cctk::sctk, iced, Element, Theme};
use itertools::Itertools;
use log::error;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use switcheroo_control::Gpu;

use crate::app_group::AppLibraryConfig;
use crate::fl;
use crate::subscriptions::desktop_files::desktop_files;
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

pub(crate) static WINDOW_ID: Lazy<SurfaceId> = Lazy::new(|| SurfaceId::unique());
static NEW_GROUP_WINDOW_ID: Lazy<SurfaceId> = Lazy::new(|| SurfaceId::unique());
static DELETE_GROUP_WINDOW_ID: Lazy<SurfaceId> = Lazy::new(|| SurfaceId::unique());
pub(crate) static DND_ICON_ID: Lazy<SurfaceId> = Lazy::new(|| SurfaceId::unique());
pub(crate) static MENU_ID: Lazy<SurfaceId> = Lazy::new(|| SurfaceId::unique());

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LauncherCommands;

impl ToString for LauncherCommands {
    fn to_string(&self) -> String {
        ron::ser::to_string(self).unwrap()
    }
}

impl CosmicFlags for Args {
    type SubCommand = LauncherCommands;
    type Args = Vec<String>;

    fn action(&self) -> Option<&LauncherCommands> {
        None
    }
}

pub fn run() -> cosmic::iced::Result {
    cosmic::app::run_single_instance::<CosmicAppLibrary>(
        Settings::default()
            .antialiasing(true)
            .client_decorations(true)
            .debug(false)
            .default_icon_theme("Pop")
            .default_text_size(16.0)
            .scale_factor(1.0)
            .no_main_window(true)
            .exit_on_close(false),
        Args::parse(),
    )
}

#[derive(Default)]
struct CosmicAppLibrary {
    search_value: String,
    entry_path_input: Vec<Arc<DesktopEntryData>>,
    all_entries: Vec<Arc<DesktopEntryData>>,
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
    waiting_for_filtered: bool,
    scroll_offset: f32,
    core: Core,
    group_to_delete: Option<usize>,
    gpus: Option<Vec<Gpu>>,
}

async fn try_get_gpus() -> Option<Vec<Gpu>> {
    let connection = zbus::Connection::system().await.ok()?;
    let proxy = switcheroo_control::SwitcherooControlProxy::new(&connection)
        .await
        .ok()?;

    if !proxy.has_dual_gpu().await.ok()? {
        return None;
    }

    let gpus = proxy.get_gpus().await.ok()?;
    if gpus.is_empty() {
        return None;
    }
    Some(gpus)
}

impl CosmicAppLibrary {
    pub fn activate(&mut self) -> Command<Message> {
        if self.active_surface {
            self.hide()
        } else {
            self.edit_name = None;
            self.search_value = "".to_string();
            self.active_surface = true;
            self.scroll_offset = 0.0;
            self.cur_group = 0;
            self.load_apps();
            let fetch_gpus = Command::perform(try_get_gpus(), |gpus| {
                cosmic::app::Message::App(Message::GpuUpdate(gpus))
            });
            return Command::batch(vec![
                text_input::focus(SEARCH_ID.clone()),
                get_layer_surface(SctkLayerSurfaceSettings {
                    id: WINDOW_ID.clone(),
                    keyboard_interactivity: KeyboardInteractivity::OnDemand,
                    anchor: Anchor::all(),
                    namespace: "app-library".into(),
                    size: Some((None, None)),
                    ..Default::default()
                }),
                fetch_gpus,
            ]);
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    InputChanged(String),
    Layer(LayerEvent, SurfaceId),
    Hide,
    ActivateApp(usize, Option<usize>),
    ActivationToken(Option<String>, String, Option<usize>),
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
    FilterApps(String, Vec<Arc<DesktopEntryData>>),
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
    ScrollYOffset(f32),
    GpuUpdate(Option<Vec<Gpu>>),
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

pub fn menu_button<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> cosmic::widget::Button<'a, Message, cosmic::Theme, cosmic::Renderer> {
    cosmic::widget::Button::new(content)
        .style(Button::AppletMenu)
        .padding(menu_control_padding())
        .width(Length::Fill)
}

pub fn menu_control_padding() -> Padding {
    let theme = cosmic::theme::active();
    let cosmic = theme.cosmic();
    [cosmic.space_xxs(), cosmic.space_m()].into()
}

impl CosmicAppLibrary {
    pub fn load_apps(&mut self) {
        let locale = self.locale.as_deref();
        self.all_entries = cosmic::desktop::load_applications_filtered(locale, |entry| {
            entry.exec().is_some() && !entry.no_display()
        })
        .into_iter()
        .map(Arc::new)
        .collect();
        self.entry_path_input =
            self.config
                .filtered(self.cur_group, &self.search_value, &self.all_entries);
        self.entry_path_input.sort_by(|a, b| a.name.cmp(&b.name));
    }

    fn filter_apps(&mut self) -> Command<Message> {
        let config = self.config.clone();
        let all_entries = self.all_entries.clone();
        let cur_group = self.cur_group;
        let input = self.search_value.clone();
        if !self.waiting_for_filtered {
            self.waiting_for_filtered = true;
            iced::Command::perform(
                async move {
                    let mut apps = config.filtered(cur_group, &input, &all_entries);
                    apps.sort_by(|a, b| a.name.cmp(&b.name));
                    (input, apps)
                },
                |(input, apps)| Message::FilterApps(input, apps),
            )
            .map(cosmic::app::Message::App)
        } else {
            iced::Command::none()
        }
    }

    pub fn hide(&mut self) -> Command<Message> {
        // cancel existing dnd if it exists then try again...
        if self.dnd_icon.take().is_some() {
            return Command::batch(vec![
                cancel_dnd(),
                Command::perform(async {}, |_| cosmic::app::Message::App(Message::Hide)),
            ]);
        }
        self.active_surface = false;
        self.new_group = None;
        self.search_value.clear();
        self.edit_name = None;
        self.cur_group = 0;
        self.menu = None;
        self.group_to_delete = None;
        self.scroll_offset = 0.0;
        iced::Command::batch(vec![
            text_input::focus(SEARCH_ID.clone()),
            destroy_popup(MENU_ID.clone()),
            destroy_layer_surface(NEW_GROUP_WINDOW_ID.clone()),
            destroy_layer_surface(DELETE_GROUP_WINDOW_ID.clone()),
            destroy_layer_surface(WINDOW_ID.clone()),
        ])
    }
}

impl cosmic::Application for CosmicAppLibrary {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = Args;
    const APP_ID: &'static str = "com.system76.CosmicAppLibrary";

    fn core(&self) -> &Core {
        &self.core
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::InputChanged(value) => {
                self.search_value = value;
                return self.filter_apps();
            }
            Message::Layer(e, id) => match e {
                LayerEvent::Focused => {
                    return text_input::focus(SEARCH_ID.clone());
                }
                LayerEvent::Unfocused => {
                    if self.active_surface
                        && id == WINDOW_ID.clone()
                        && self.menu.is_none()
                        && self.new_group.is_none()
                        && self.group_to_delete.is_none()
                    {
                        return self.hide();
                    }
                }
                LayerEvent::Done if id == WINDOW_ID.clone() => {
                    // no need for commands here
                    _ = self.hide();
                }
                _ => {}
            },
            Message::Hide => {
                return self.hide();
            }
            Message::ActivateApp(i, gpu_idx) => {
                self.edit_name = None;
                if let Some(de) = self.entry_path_input.get(i) {
                    let exec = de.exec.clone().unwrap();
                    return request_token(
                        Some(String::from(Self::APP_ID)),
                        Some(WINDOW_ID.clone()),
                        move |token| {
                            cosmic::app::Message::App(Message::ActivationToken(
                                token, exec, gpu_idx,
                            ))
                        },
                    );
                }
            }
            Message::ActivationToken(token, exec, gpu_idx) => {
                let mut env_vars = Vec::new();
                if let Some(token) = token {
                    env_vars.push(("XDG_ACTIVATION_TOKEN".to_string(), token.clone()));
                    env_vars.push(("DESKTOP_STARTUP_ID".to_string(), token));
                }
                if let (Some(gpus), Some(idx)) = (self.gpus.as_ref(), gpu_idx) {
                    env_vars.extend(gpus[idx].environment.clone().into_iter());
                }
                tokio::task::spawn_blocking(move || {
                    cosmic::desktop::spawn_desktop_exec(exec, env_vars)
                });
                return self.update(Message::Hide);
            }
            Message::SelectGroup(i) => {
                self.edit_name = None;
                self.search_value.clear();
                self.cur_group = i;
                self.scroll_offset = 0.0;
                let mut cmds = vec![self.filter_apps()];
                if self.cur_group == 0 {
                    cmds.push(text_input::focus(SEARCH_ID.clone()));
                }
                return iced::Command::batch(cmds);
            }
            Message::LoadApps => {
                return self.filter_apps();
            }
            Message::Delete(group) => {
                self.group_to_delete = Some(group);
                return get_layer_surface(SctkLayerSurfaceSettings {
                    id: DELETE_GROUP_WINDOW_ID.clone(),
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
                        id: NEW_GROUP_WINDOW_ID.clone(),
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
                return destroy_layer_surface(NEW_GROUP_WINDOW_ID.clone());
            }
            Message::CancelNewGroup => {
                self.new_group = None;
                return destroy_layer_surface(NEW_GROUP_WINDOW_ID.clone());
            }
            Message::OpenContextMenu(rect, i) => {
                if self.menu.take().is_some() {
                    return destroy_popup(MENU_ID.clone());
                } else {
                    self.menu = Some(i);
                    return commands::popup::get_popup(SctkPopupSettings {
                        parent: WINDOW_ID.clone(),
                        id: MENU_ID.clone(),
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
                            return self.filter_apps();
                        }
                        MenuAction::DesktopAction(exec) => {
                            let mut exec = shlex::Shlex::new(&exec);

                            let mut cmd = match exec.next() {
                                Some(cmd) if !cmd.contains('=') => {
                                    tokio::process::Command::new(cmd)
                                }
                                _ => return Command::none(),
                            };
                            for arg in exec {
                                // TODO handle "%" args here if necessary?
                                if !arg.starts_with('%') {
                                    cmd.arg(arg);
                                }
                            }
                            let _ = cmd.spawn();
                            return self.hide();
                        }
                    }
                }
            }
            Message::StartDrag(i) => {
                self.dnd_icon = Some(i);
            }
            Message::FinishDrag(copy) => {
                if !copy {
                    if let Some(info) = self
                        .dnd_icon
                        .take()
                        .and_then(|i| self.entry_path_input.get(i))
                    {
                        self.config.remove_entry(self.cur_group, &info.id);
                        if let Some(helper) = self.helper.as_ref() {
                            if let Err(err) = self.config.write_entry(helper) {
                                error!("{:?}", err);
                            }
                        }
                        return self.filter_apps();
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
                    return self.filter_apps();
                }
                return destroy_layer_surface(DELETE_GROUP_WINDOW_ID.clone());
            }
            Message::CancelDelete => {
                self.group_to_delete = None;
                return destroy_layer_surface(DELETE_GROUP_WINDOW_ID.clone());
            }
            Message::FilterApps(input, filtered_apps) => {
                self.entry_path_input = filtered_apps;
                self.waiting_for_filtered = false;
                if self.search_value != input {
                    return self.filter_apps();
                }
            }
            Message::GpuUpdate(gpus) => {
                self.gpus = gpus;
            }
        }
        Command::none()
    }

    fn dbus_activation(&mut self, msg: DbusActivationMessage) -> Command<Self::Message> {
        if matches!(msg.msg, DbusActivationDetails::Activate) {
            self.activate()
        } else {
            Command::none()
        }
    }

    fn view(&self) -> Element<Message> {
        unimplemented!()
    }

    fn view_window(&self, id: SurfaceId) -> Element<Message> {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();
        let spacing = &cosmic.spacing;
        if id == DND_ICON_ID.clone() {
            let Some(icon_source) = self
                .dnd_icon
                .and_then(|i| self.entry_path_input.get(i).map(|e| &e.icon))
            else {
                return container(horizontal_space(Length::Fixed(1.0)))
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(1.0))
                    .into();
            };
            return icon_source.as_cosmic_icon().size(32).into();
        }
        if id == MENU_ID.clone() {
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

            let mut list_column = Vec::new();

            if let Some(gpus) = self.gpus.as_ref() {
                for (j, gpu) in gpus.iter().enumerate() {
                    let default_idx = if menu.prefers_dgpu {
                        gpus.iter().position(|gpu| !gpu.default).unwrap_or(0)
                    } else {
                        gpus.iter().position(|gpu| gpu.default).unwrap_or(0)
                    };
                    list_column.push(
                        menu_button(text(format!(
                            "{} {}",
                            fl!("run-on", gpu = gpu.name.clone()),
                            if j == default_idx {
                                fl!("run-on-default")
                            } else {
                                String::new()
                            }
                        )))
                        .on_press(Message::ActivateApp(*i, Some(j)))
                        .into(),
                    )
                }
            } else {
                list_column.push(
                    menu_button(text(RUN.clone()))
                        .on_press(Message::ActivateApp(*i, None))
                        .into(),
                );
            }

            if menu.desktop_actions.len() > 0 {
                list_column.push(menu_divider(spacing).into());
                for action in menu.desktop_actions.iter() {
                    list_column.push(
                        menu_button(text(&action.name))
                            .on_press(Message::SelectAction(
                                MenuAction::DesktopAction(action.exec.clone()).into(),
                            ))
                            .into(),
                    );
                }
                list_column.push(menu_divider(spacing).into());
            }
            if self.cur_group > 0 {
                list_column.push(
                    menu_button(text(REMOVE.clone()))
                        .on_press(Message::SelectAction(MenuAction::Remove))
                        .into(),
                );
            }

            return container(scrollable(Column::with_children(list_column)))
                .padding([8, 0])
                .style(theme::Container::Custom(Box::new(|theme| {
                    container::Appearance {
                        text_color: Some(theme.cosmic().on_bg_color().into()),
                        background: Some(Color::from(theme.cosmic().background.base).into()),
                        border: Border {
                            color: theme.cosmic().bg_divider().into(),
                            radius: theme.cosmic().corner_radii.radius_m.into(),
                            width: 1.0,
                        },
                        shadow: Shadow::default(),
                        icon_color: Some(theme.cosmic().on_bg_color().into()),
                    }
                })))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Top)
                .into();
        }
        if id == NEW_GROUP_WINDOW_ID.clone() {
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
                text_input("", group_name)
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
                                .size(14.0)
                                .horizontal_alignment(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .on_press(Message::CancelNewGroup)
                        .padding([spacing.space_xxs, spacing.space_s])
                        .width(142),
                        button(
                            text(&SAVE.as_str())
                                .size(14.0)
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
                        border: Border {
                            color: theme.cosmic().bg_divider().into(),
                            radius: theme.cosmic().corner_radii.radius_m.into(),
                            width: 1.0,
                        },
                        shadow: Shadow::default(),
                    }
                })))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .padding(spacing.space_s)
                .into();
        }

        if id == DELETE_GROUP_WINDOW_ID.clone() {
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
                        border: Border {
                            color: theme.cosmic().bg_divider().into(),
                            radius: theme.cosmic().corner_radii.radius_m.into(),
                            width: 1.0,
                        },
                        shadow: Shadow::default(),
                    }
                })))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .padding(spacing.space_m)
                .into();
        }

        let cur_group = self.config.groups()[self.cur_group];
        let top_row = if self.cur_group == 0 {
            row![container(
                search_input(SEARCH_PLACEHOLDER.as_str(), self.search_value.as_str())
                    .on_input(Message::InputChanged)
                    .on_paste(Message::InputChanged)
                    .style(TextInput::Search)
                    .width(Length::Fixed(400.0))
                    .size(14)
                    .id(SEARCH_ID.clone())
            )
            .align_y(Vertical::Center)
            .height(Length::Fixed(96.0))]
            .align_items(Alignment::Center)
            .spacing(spacing.space_xxs)
        } else {
            row![
                horizontal_space(Length::FillPortion(1)),
                if let Some(edit_name) = self.edit_name.as_ref() {
                    container(
                        text_input(cur_group.name(), edit_name)
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
                            container(b)
                                .height(Length::Fixed(96.0))
                                .align_y(Vertical::Center)
                        },
                        fl!("rename"),
                        tooltip::Position::Bottom
                    ),
                    tooltip(
                        container(
                            button(
                                icon(from_name("edit-delete-symbolic").into())
                                    .width(Length::Fixed(32.0))
                                    .height(Length::Fixed(32.0)),
                            )
                            .padding(spacing.space_xs)
                            .style(Button::Icon)
                            .on_press(Message::Delete(self.cur_group))
                        )
                        .height(Length::Fixed(96.0))
                        .align_y(Vertical::Center),
                        fl!("delete"),
                        tooltip::Position::Bottom
                    )
                ]
                .spacing(spacing.space_xxs)
                .width(Length::FillPortion(1))
            ]
            .padding([0, spacing.space_l])
            .align_items(Alignment::Center)
        };

        // TODO grid widget in libcosmic
        let app_grid_list: Vec<_> = self
            .entry_path_input
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let gpu_idx = self.gpus.as_ref().map(|gpus| {
                    if entry.prefers_dgpu {
                        gpus.iter().position(|gpu| !gpu.default).unwrap_or(0)
                    } else {
                        gpus.iter().position(|gpu| gpu.default).unwrap_or(0)
                    }
                });
                let mut b = ApplicationButton::new(
                    &entry,
                    move |rect| Message::OpenContextMenu(rect, i),
                    if self.menu.is_none() {
                        Some(Message::ActivateApp(i, gpu_idx))
                    } else {
                        None
                    },
                    spacing,
                );
                if self.menu.is_none() {
                    b = b
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
            .height(Length::Fill),
        )
        .max_height(444.0);

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
                    text("Add group")
                        .size(14.0)
                        .horizontal_alignment(Horizontal::Center)
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
                                    let s = theme.pressed(focused, false, &Button::IconVertical);
                                    s
                                }),
                                disabled: Box::new(|theme| {
                                    let s = theme.disabled(&Button::IconVertical);
                                    s
                                }),
                                hovered: Box::new(|focused, theme| {
                                    let s = theme.hovered(focused, false, &Button::IconVertical);
                                    s
                                }),
                                pressed: Box::new(|focused, theme| {
                                    let s = theme.pressed(focused, false, &Button::IconVertical);
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
            .width(Length::Fill)
            .height(Length::Fill)
            .max_height(656)
            .max_width(1200.0)
            .style(theme::Container::Custom(Box::new(|theme| {
                container::Appearance {
                    text_color: Some(theme.cosmic().on_bg_color().into()),
                    background: Some(Color::from(theme.cosmic().background.base).into()),
                    border: Border {
                        radius: theme.cosmic().corner_radii.radius_m.into(),
                        width: 1.0,
                        color: theme.cosmic().bg_divider().into(),
                    },
                    shadow: Shadow::default(),
                    icon_color: Some(theme.cosmic().on_bg_color().into()),
                }
            })))
            .center_x();
        row![
            mouse_area(
                container(horizontal_space(Length::Fixed(1.0)))
                    .width(Length::Fill)
                    .height(Length::Fill)
            )
            .on_press(Message::Hide),
            container(
                column![
                    mouse_area(
                        container(vertical_space(Length::Fill))
                            .width(Length::Fill)
                            .height(Length::Fixed(16.0))
                    )
                    .on_press(Message::Hide),
                    container(
                        mouse_area(window)
                            .on_release(Message::CloseContextMenu)
                            .on_right_release(Message::CloseContextMenu)
                    )
                    .width(Length::Shrink)
                    .height(Length::Shrink),
                    mouse_area(
                        container(vertical_space(Length::Fill))
                            .width(Length::Fill)
                            .height(Length::Fill)
                    )
                    .on_press(Message::Hide)
                ]
                .height(Length::Fill)
            )
            .max_width(1200.0)
            .width(Length::Shrink)
            .height(Length::Fill),
            mouse_area(
                container(horizontal_space(Length::Fixed(1.0)))
                    .width(Length::Fill)
                    .height(Length::Fill)
            )
            .on_press(Message::Hide),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(
            vec![
                desktop_files(0).map(|_| Message::LoadApps),
                listen_with(|e, _status| match e {
                    cosmic::iced::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::Layer(e, _, id),
                    )) => Some(Message::Layer(e, id)),
                    cosmic::iced::Event::Keyboard(cosmic::iced::keyboard::Event::KeyReleased {
                        key: Key::Named(Named::Escape),
                        modifiers: _mods,
                        ..
                    }) => Some(Message::Hide),
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
        _flags: Args,
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
        let self_ = Self {
            locale: current_locale::current_locale().ok(),
            config,
            core,
            helper,
            ..Default::default()
        };

        (self_, Command::none())
    }
}

fn menu_divider<'a>(spacing: &Spacing) -> Container<'a, Message, cosmic::Theme, cosmic::Renderer> {
    container(horizontal_rule(1))
        .padding([spacing.space_none, spacing.space_s])
        .width(Length::Fill)
}
