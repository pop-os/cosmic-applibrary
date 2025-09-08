use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use clap::Parser;
use cosmic::{
    Element,
    app::{Core, CosmicFlags, Settings, Task},
    cctk::sctk::{
        self,
        data_device_manager::data_offer::DataDeviceOfferInner,
        shell::wlr_layer::{Anchor, KeyboardInteractivity},
    },
    cosmic_config::{Config, CosmicConfigEntry},
    cosmic_theme::Spacing,
    dbus_activation,
    desktop::{DesktopEntryData, fde::PathSource, load_desktop_file},
    iced::{
        self, Alignment, Color, Length, Limits, Size, Subscription,
        alignment::Horizontal,
        event::{listen_with, wayland::OverlapNotifyEvent},
        executor,
        id::Id,
        /*wayland::actions::{
            data_device::ActionInner,
        },*/
        widget::{column, container, horizontal_rule, row, scrollable, text},
        window::Event as WindowEvent,
    },
    iced_core::{
        Border, Padding, Rectangle, Shadow,
        alignment::Vertical,
        keyboard::{Key, key::Named},
        widget::operation::{
            self,
            focusable::{find_focused, focus},
        },
    },
    iced_runtime::{
        self,
        core::{
            event::{
                PlatformSpecific,
                wayland::{self, LayerEvent},
            },
            window::Id as SurfaceId,
        },
        dnd::end_dnd,
        platform_specific::wayland::{
            layer_surface::SctkLayerSurfaceSettings,
            popup::{SctkPopupSettings, SctkPositioner},
        },
    },
    iced_widget::{horizontal_space, mouse_area, scrollable::RelativeOffset, vertical_space},
    iced_winit::commands::{
        self,
        activation::request_token,
        layer_surface::{destroy_layer_surface, get_layer_surface},
        overlap_notify::overlap_notify,
        popup::destroy_popup,
    },
    keyboard_nav, surface,
    theme::{self, Button, TextInput},
    widget::{
        self, Column,
        autosize::autosize,
        button::{self, Catalog as ButtonStyleSheet},
        divider,
        dnd_destination::dnd_destination_for_data,
        icon::{self, from_name},
        search_input, svg,
        text::body,
        text_input, tooltip,
    },
};
use cosmic_app_list_config::AppListConfig;
use itertools::Itertools;
use log::error;
use serde::{Deserialize, Serialize};
use switcheroo_control::Gpu;

use crate::{
    app_group::AppLibraryConfig,
    fl,
    subscriptions::desktop_files::desktop_files,
    widgets::application::{AppletString, ApplicationButton},
};

// popovers should show options, but also the desktop info options
// should be a way to add apps to groups
// should be a way to remove apps from groups

static SEARCH_ID: LazyLock<Id> = LazyLock::new(|| Id::new("search"));
static EDIT_GROUP_ID: LazyLock<Id> = LazyLock::new(|| Id::new("edit_group"));
static NEW_GROUP_ID: LazyLock<Id> = LazyLock::new(|| Id::new("new_group"));
static SUBMIT_DELETE_ID: LazyLock<Id> = LazyLock::new(|| Id::new("cancel_delete"));

static CREATE_NEW: LazyLock<String> = LazyLock::new(|| fl!("create-new"));
static ADD_GROUP: LazyLock<String> = LazyLock::new(|| fl!("add-group"));
static SEARCH_PLACEHOLDER: LazyLock<String> = LazyLock::new(|| fl!("search-placeholder"));
static NEW_GROUP_PLACEHOLDER: LazyLock<String> = LazyLock::new(|| fl!("new-group-placeholder"));
static SAVE: LazyLock<String> = LazyLock::new(|| fl!("save"));
static CANCEL: LazyLock<String> = LazyLock::new(|| fl!("cancel"));
static RUN: LazyLock<String> = LazyLock::new(|| fl!("run"));
static REMOVE: LazyLock<String> = LazyLock::new(|| fl!("remove"));
static FLATPAK: LazyLock<String> = LazyLock::new(|| fl!("flatpak"));
static LOCAL: LazyLock<String> = LazyLock::new(|| fl!("local"));
static NIX: LazyLock<String> = LazyLock::new(|| fl!("nix"));
static SNAP: LazyLock<String> = LazyLock::new(|| fl!("snap"));
static SYSTEM: LazyLock<String> = LazyLock::new(|| fl!("system"));

pub(crate) static WINDOW_ID: LazyLock<SurfaceId> = LazyLock::new(|| SurfaceId::unique());
static NEW_GROUP_WINDOW_ID: LazyLock<SurfaceId> = LazyLock::new(|| SurfaceId::unique());
static NEW_GROUP_AUTOSIZE_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(|| cosmic::widget::Id::unique());
static DELETE_GROUP_WINDOW_ID: LazyLock<SurfaceId> = LazyLock::new(|| SurfaceId::unique());
static DELETE_GROUP_AUTOSIZE_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(|| cosmic::widget::Id::unique());
pub(crate) static MENU_ID: LazyLock<SurfaceId> = LazyLock::new(|| SurfaceId::unique());
pub(crate) static MENU_AUTOSIZE_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(|| cosmic::widget::Id::unique());

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
            .default_text_size(16.0)
            .scale_factor(1.0)
            .no_main_window(true)
            .exit_on_close(false),
        Args::parse(),
    )
}

pub struct AppSource(PathSource);

impl AppSource {
    pub fn as_icon(&self) -> Option<icon::Icon> {
        let name = match &self.0 {
            PathSource::Local | PathSource::LocalDesktop => "app-source-local-symbolic",
            PathSource::System | PathSource::SystemLocal => "app-source-system-symbolic",
            PathSource::LocalFlatpak | PathSource::SystemFlatpak => "app-source-flatpak",
            PathSource::SystemSnap => "app-source-snap",
            PathSource::Nix | PathSource::LocalNix => "app-source-nix",
            PathSource::Other(_) => return None,
        };
        let handle = crate::icon_cache::icon_cache_handle(name, 16);
        let symbolic = handle.symbolic;

        Some(icon::icon(handle).size(16).class(if symbolic {
            cosmic::theme::Svg::Custom(Rc::new(|t| {
                let color = t.cosmic().on_primary_component_color().into();
                svg::Style { color: Some(color) }
            }))
        } else {
            cosmic::theme::Svg::Default
        }))
    }
}

impl<'a> From<&'a Path> for AppSource {
    fn from(path: &'a Path) -> Self {
        AppSource(PathSource::guess_from(path))
    }
}

impl<'a> Display for AppSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.7}",
            match &self.0 {
                PathSource::Local | PathSource::LocalDesktop => LOCAL.as_str(),
                PathSource::SystemFlatpak | PathSource::LocalFlatpak => FLATPAK.as_str(),
                PathSource::SystemSnap => SNAP.as_str(),
                PathSource::Nix | PathSource::LocalNix => NIX.as_str(),
                PathSource::System | PathSource::SystemLocal => SYSTEM.as_str(),
                PathSource::Other(s) => s.as_str(),
            }
        )
    }
}

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
    last_hide: Option<Instant>,
    duplicates: HashMap<PathBuf, AppSource>,
    app_list_config: AppListConfig,
    overlap: HashMap<String, Rectangle>,
    margin: f32,
    height: f32,
    needs_clear: bool,
    focused_id: Option<widget::Id>,
    entry_ids: Vec<widget::Id>,
    scrollable_id: widget::Id,
}

impl Default for CosmicAppLibrary {
    fn default() -> Self {
        Self {
            search_value: Default::default(),
            entry_path_input: Default::default(),
            all_entries: Default::default(),
            menu: Default::default(),
            helper: Default::default(),
            config: Default::default(),
            cur_group: Default::default(),
            active_surface: Default::default(),
            locale: Default::default(),
            edit_name: Default::default(),
            new_group: Default::default(),
            dnd_icon: Default::default(),
            offer_group: Default::default(),
            waiting_for_filtered: Default::default(),
            scroll_offset: Default::default(),
            core: Default::default(),
            group_to_delete: Default::default(),
            gpus: Default::default(),
            last_hide: Default::default(),
            duplicates: Default::default(),
            app_list_config: Default::default(),
            overlap: Default::default(),
            margin: Default::default(),
            height: Default::default(),
            needs_clear: Default::default(),
            focused_id: Default::default(),
            entry_ids: Default::default(),
            scrollable_id: widget::Id::unique(),
        }
    }
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
    pub fn activate(&mut self) -> Task<Message> {
        if self.active_surface {
            return self.hide();
        } else if !self
            .last_hide
            .is_some_and(|i| i.elapsed() < Duration::from_millis(100))
        {
            self.edit_name = None;
            self.search_value = "".to_string();
            self.active_surface = true;
            self.scroll_offset = 0.0;
            self.cur_group = 0;
            self.load_apps();
            self.needs_clear = true;
            let fetch_gpus = Task::perform(try_get_gpus(), |gpus| {
                cosmic::Action::App(Message::GpuUpdate(gpus))
            });
            return Task::batch(vec![
                get_layer_surface(SctkLayerSurfaceSettings {
                    id: WINDOW_ID.clone(),
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                    anchor: Anchor::all(),
                    namespace: "app-library".into(),
                    size: Some((None, None)),
                    exclusive_zone: -1,
                    ..Default::default()
                }),
                overlap_notify(WINDOW_ID.clone(), true),
                fetch_gpus,
            ])
            .chain(text_input::focus(SEARCH_ID.clone()))
            .chain(
                iced_runtime::task::widget(find_focused())
                    .map(|id| cosmic::Action::App(Message::UpdateFocused(Some(id)))),
            );
        }
        Task::none()
    }

    fn handle_overlap(&mut self) {
        if !self.active_surface {
            return;
        }

        let mid_height = self.height / 2.;
        self.margin = 0.;

        for o in self.overlap.values() {
            if self.margin + mid_height < o.y
                || self.margin > o.y + o.height
                || mid_height < o.y + o.height
            {
                continue;
            }

            self.margin = o.y + o.height;
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    UpdateFocused(Option<widget::Id>),
    InputChanged(String),
    KeyboardNav(keyboard_nav::Action),
    PrevRow,
    NextRow,
    Layer(LayerEvent, SurfaceId),
    Hide,
    ActivateApp(usize, Option<usize>),
    StartCurAppFocus,
    ActivationToken(Option<String>, String, String, Option<usize>, bool),
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
    FinishDrag(bool),
    CancelDrag,
    StartDndOffer(usize),
    FinishDndOffer(usize, Option<DesktopEntryData>),
    LeaveDndOffer(usize),
    ScrollYOffset(f32),
    GpuUpdate(Option<Vec<Gpu>>),
    PinToAppTray(usize),
    UnPinFromAppTray(usize),
    AppListConfig(AppListConfig),
    Opened(Size, SurfaceId),
    Overlap(OverlapNotifyEvent),
    Surface(surface::Action),
}

#[derive(Clone)]
struct DndCommand(Arc<Box<dyn Send + Sync + Fn() -> DataDeviceOfferInner>>);

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

pub fn menu_button<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
) -> cosmic::widget::Button<'a, Message> {
    cosmic::widget::button::custom(content)
        .class(Button::AppletMenu)
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
        let xdg_current_desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
        self.all_entries = cosmic::desktop::load_applications(
            self.locale.as_slice(),
            false,
            xdg_current_desktop.as_deref(),
        )
        .into_iter()
        .map(Arc::new)
        .collect();
        self.all_entries.sort_by(|a, b| a.name.cmp(&b.name));

        self.entry_path_input =
            self.config
                .filtered(self.cur_group, &self.search_value, &self.all_entries);

        // collect duplicates
        self.duplicates.clear();
        self.duplicates = self
            .all_entries
            .iter()
            .enumerate()
            .fold(
                (std::mem::take(&mut self.duplicates), 0, "", ""),
                |(mut dups, cur_count, cur_name, cur_id): (HashMap<_, _>, usize, &str, &str),
                 (i, e)| {
                    if cur_name.to_lowercase().trim() == e.name.to_lowercase().trim()
                        || e.id == cur_id
                    {
                        if cur_count == 1 {
                            // insert previous entry
                            if let Some(path) = self.all_entries[i - 1].path.as_ref() {
                                let source = AppSource::from(path.as_ref());
                                dups.insert(path.clone(), source);
                            }
                        }
                        if let Some(path) = e.path.as_ref() {
                            let source = AppSource::from(path.as_ref());
                            dups.insert(path.clone(), source);
                        }
                        (dups, cur_count + 1, cur_name, cur_id)
                    } else {
                        (dups, 1, e.name.as_str(), e.id.as_str())
                    }
                },
            )
            .0;
        self.entry_ids = (0..self.entry_path_input.len())
            .map(|_| widget::Id::unique())
            .collect();
    }

    fn filter_apps(&mut self) -> Task<Message> {
        let config = self.config.clone();
        let all_entries = self.all_entries.clone();
        let cur_group = self.cur_group;
        let input = self.search_value.clone();
        if !self.waiting_for_filtered {
            self.waiting_for_filtered = true;
            iced::Task::perform(
                async move {
                    let mut apps = config.filtered(cur_group, &input, &all_entries);
                    apps.sort_by(|a, b| a.name.cmp(&b.name));
                    (input, apps)
                },
                |(input, apps)| Message::FilterApps(input, apps),
            )
            .map(cosmic::Action::App)
        } else {
            iced::Task::none()
        }
    }

    pub fn hide(&mut self) -> Task<Message> {
        // cancel existing dnd if it exists then try again...
        if self.dnd_icon.take().is_some() {
            return Task::batch(vec![
                end_dnd(),
                Task::perform(async {}, |_| cosmic::Action::App(Message::Hide)),
            ]);
        }
        self.focused_id = None;
        self.entry_ids.clear();
        self.active_surface = false;
        self.new_group = None;
        self.search_value.clear();
        self.edit_name = None;
        self.cur_group = 0;
        self.menu = None;
        self.group_to_delete = None;
        self.scroll_offset = 0.0;
        iced::Task::batch(vec![
            text_input::focus(SEARCH_ID.clone()),
            destroy_popup(MENU_ID.clone()),
            destroy_layer_surface(NEW_GROUP_WINDOW_ID.clone()),
            destroy_layer_surface(DELETE_GROUP_WINDOW_ID.clone()),
            destroy_layer_surface(WINDOW_ID.clone()),
        ])
    }

    fn activate_app(
        &mut self,
        i: usize,
        gpu_idx: Option<usize>,
    ) -> Task<<Self as cosmic::Application>::Message> {
        self.edit_name = None;
        if let Some(de) = self.entry_path_input.get(i) {
            let app_id = de.id.clone();
            let exec = de.exec.clone().unwrap();
            let terminal = de.terminal;
            return request_token(
                Some(String::from(<Self as cosmic::Application>::APP_ID)),
                Some(WINDOW_ID.clone()),
            )
            .map(move |t| {
                cosmic::Action::App(Message::ActivationToken(
                    t,
                    app_id.clone(),
                    exec.clone(),
                    gpu_idx,
                    terminal,
                ))
            });
        } else {
            Task::none()
        }
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

    fn update(&mut self, message: Message) -> Task<Self::Message> {
        match message {
            Message::UpdateFocused(id) => {
                self.focused_id = id;
                let i = self
                    .focused_id
                    .as_ref()
                    .and_then(|focused| self.entry_ids.iter().position(|i| i == focused))
                    .unwrap_or(0);
                let y =
                    ((i / 7) as f32 / ((self.entry_path_input.len() / 7) as f32).max(1.)).max(0.0);

                return iced_runtime::task::widget(operation::scrollable::snap_to(
                    self.scrollable_id.clone(),
                    RelativeOffset { x: 0., y },
                ));
            }
            Message::KeyboardNav(message) => match message {
                keyboard_nav::Action::FocusNext => {
                    return iced::Task::batch(vec![
                        iced::widget::focus_next()
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(id))),
                        iced_runtime::task::widget(find_focused())
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(Some(id)))),
                    ]);
                }
                keyboard_nav::Action::FocusPrevious => {
                    return iced::Task::batch(vec![
                        iced::widget::focus_previous()
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(id))),
                        iced_runtime::task::widget(find_focused())
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(Some(id)))),
                    ]);
                }
                keyboard_nav::Action::Escape => return self.on_escape(),
                keyboard_nav::Action::Search => return self.on_search(),

                keyboard_nav::Action::Fullscreen => {}
            },

            Message::PrevRow => {
                let mut i = self
                    .focused_id
                    .as_ref()
                    .and_then(|focused| self.entry_ids.iter().position(|i| i == focused))
                    .unwrap_or(self.entry_ids.len().saturating_add(6));
                if i == 0 {
                    self.focused_id = None;

                    return iced::Task::batch(vec![
                        iced::widget::focus_previous()
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(id))),
                        iced_runtime::task::widget(find_focused())
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(Some(id)))),
                    ]);
                }
                i = i.saturating_sub(7);
                let y =
                    ((i / 7) as f32 / ((self.entry_path_input.len() / 7) as f32).max(1.)).max(0.0);

                let Some(focused) = self.entry_ids.get(i).cloned() else {
                    return Task::none();
                };
                self.focused_id = Some(focused.clone());
                return Task::batch(vec![
                    iced_runtime::task::widget(focus(focused))
                        .map(|id| cosmic::Action::App(Message::UpdateFocused(Some(id)))),
                    iced_runtime::task::widget(operation::scrollable::snap_to(
                        self.scrollable_id.clone(),
                        RelativeOffset { x: 0., y },
                    )),
                ]);
            }
            Message::NextRow => {
                let mut i: i32 = self
                    .focused_id
                    .as_ref()
                    .and_then(|focused| self.entry_ids.iter().position(|i| i == focused))
                    .map(|i| i as i32)
                    .unwrap_or(-7);
                if i == self.entry_ids.len() as i32 - 1 {
                    self.focused_id = None;
                    return iced::Task::batch(vec![
                        iced::widget::focus_next()
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(id))),
                        iced_runtime::task::widget(find_focused())
                            .map(|id| cosmic::Action::App(Message::UpdateFocused(Some(id)))),
                    ]);
                }
                i += 7;
                i = i.min(self.entry_ids.len() as i32 - 1);
                let Some(focused) = self.entry_ids.get(i as usize).cloned() else {
                    return Task::none();
                };
                self.focused_id = Some(focused.clone());
                let y =
                    ((i / 7) as f32 / ((self.entry_path_input.len() / 7) as f32).max(1.)).max(0.0);

                return Task::batch(vec![
                    iced_runtime::task::widget(operation::scrollable::snap_to(
                        self.scrollable_id.clone(),
                        RelativeOffset { x: 0., y },
                    )),
                    iced_runtime::task::widget(focus(focused))
                        .map(|id| cosmic::Action::App(Message::UpdateFocused(Some(id)))),
                ]);
            }
            Message::InputChanged(value) => {
                self.search_value = value;
                return self.filter_apps();
            }
            Message::Layer(e, id) => match e {
                LayerEvent::Focused => {
                    if self.menu.is_none() {
                        if id == WINDOW_ID.clone() {
                            return text_input::focus(SEARCH_ID.clone());
                        } else if id == DELETE_GROUP_WINDOW_ID.clone() {
                            return button::focus(SUBMIT_DELETE_ID.clone());
                        } else if id == NEW_GROUP_WINDOW_ID.clone() {
                            return text_input::focus(NEW_GROUP_ID.clone());
                        }
                    }
                }
                LayerEvent::Unfocused => {
                    self.last_hide = Some(Instant::now());
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
                return self.activate_app(i, gpu_idx);
            }
            Message::StartCurAppFocus => {
                let i = if self
                    .focused_id
                    .as_ref()
                    .is_some_and(|cur_focus| cur_focus == &*SEARCH_ID)
                {
                    0
                } else if let Some(i) = self
                    .focused_id
                    .as_ref()
                    .and_then(|focus| self.entry_ids.iter().position(|id| focus == id))
                {
                    i
                } else {
                    0
                };
                let gpu_idx = None;
                return self.activate_app(i, gpu_idx);
            }
            Message::ActivationToken(token, app_id, exec, gpu_idx, terminal) => {
                let mut env_vars = Vec::new();
                if let Some(token) = token {
                    env_vars.push(("XDG_ACTIVATION_TOKEN".to_string(), token.clone()));
                    env_vars.push(("DESKTOP_STARTUP_ID".to_string(), token));
                }
                if let (Some(gpus), Some(idx)) = (self.gpus.as_ref(), gpu_idx) {
                    env_vars.extend(gpus[idx].environment.clone().into_iter());
                }
                tokio::spawn(async move {
                    cosmic::desktop::spawn_desktop_exec(exec, env_vars, Some(&app_id), terminal)
                        .await
                });
                return self.update(Message::Hide);
            }
            Message::SelectGroup(i) => {
                self.edit_name = None;
                self.search_value.clear();
                self.cur_group = i;
                self.scroll_offset = 0.0;
                self.scrollable_id = Id::new(
                    self.config
                        .groups()
                        .get(self.cur_group)
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| "unknown-group".to_string()),
                );
                let mut cmds = vec![self.filter_apps()];
                if self.cur_group == 0 {
                    cmds.push(text_input::focus(SEARCH_ID.clone()));
                }
                return iced::Task::batch(cmds);
            }
            Message::LoadApps => {
                return self.filter_apps();
            }
            Message::Delete(group) => {
                self.group_to_delete = Some(group);
                return Task::batch(vec![
                    get_layer_surface(SctkLayerSurfaceSettings {
                        id: DELETE_GROUP_WINDOW_ID.clone(),
                        keyboard_interactivity: KeyboardInteractivity::Exclusive,
                        anchor: Anchor::empty(),
                        namespace: "dialog".into(),
                        size: None,
                        ..Default::default()
                    }),
                    button::focus(SUBMIT_DELETE_ID.clone()),
                ]);
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
                return text_input::focus(EDIT_GROUP_ID.clone());
            }
            Message::StartNewGroup => {
                if self.new_group.is_some() {
                    return Task::none();
                }
                self.new_group = Some(String::new());
                return Task::batch(vec![
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
                                        grab: false,
                                        parent_size: None,
                                        close_with_children: true,
                                        input_zone: None,
                                    });
                }
            }
            Message::CloseContextMenu => {
                self.menu = None;
                return commands::popup::destroy_popup(MENU_ID.clone());
            }
            Message::SelectAction(action) => {
                self.menu = None;
                let mut tasks = vec![commands::popup::destroy_popup(MENU_ID.clone())];
                if let Some(info) = self.menu.take().and_then(|i| self.entry_path_input.get(i)) {
                    match action {
                        MenuAction::Remove => {
                            self.config.remove_entry(self.cur_group, &info.id);
                            if let Some(helper) = self.helper.as_ref() {
                                if let Err(err) = self.config.write_entry(helper) {
                                    error!("{:?}", err);
                                }
                            }
                            tasks.push(self.filter_apps());
                        }
                        MenuAction::DesktopAction(exec) => {
                            let mut exec = shlex::Shlex::new(&exec);

                            let mut cmd = match exec.next() {
                                Some(cmd) if !cmd.contains('=') => {
                                    tokio::process::Command::new(cmd)
                                }
                                _ => return Task::none(),
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
                return cosmic::Task::batch(tasks);
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
            Message::StartDndOffer(i) => {
                self.offer_group = Some(i);
            }
            Message::FinishDndOffer(i, entry) => {
                self.offer_group = None;
                let Some(entry) = entry else {
                    return Task::none();
                };
                self.config.add_entry(i, &entry.id);
                if let Some(helper) = self.helper.as_ref() {
                    if let Err(err) = self.config.write_entry(helper) {
                        error!("{:?}", err);
                    }
                }
            }
            Message::LeaveDndOffer(i) => {
                self.offer_group = self.offer_group.filter(|g| *g != i);
            }
            Message::ScrollYOffset(y) => {
                self.scroll_offset = y;
            }
            Message::ConfirmDelete => {
                let mut cmds = vec![destroy_layer_surface(DELETE_GROUP_WINDOW_ID.clone())];
                if let Some(group) = self.group_to_delete.take() {
                    self.config.remove(group);
                    if let Some(helper) = self.helper.as_ref() {
                        if let Err(err) = self.config.write_entry(helper) {
                            error!("{:?}", err);
                        }
                    }
                    self.cur_group = 0;
                    cmds.push(self.filter_apps());
                }
                return Task::batch(cmds);
            }
            Message::CancelDelete => {
                self.group_to_delete = None;
                return destroy_layer_surface(DELETE_GROUP_WINDOW_ID.clone());
            }
            Message::FilterApps(input, filtered_apps) => {
                self.entry_path_input = filtered_apps;
                self.entry_ids = (0..self.entry_path_input.len())
                    .map(|_| widget::Id::unique())
                    .collect();
                self.waiting_for_filtered = false;
                if self.search_value != input {
                    return self.filter_apps();
                }
            }
            Message::GpuUpdate(gpus) => {
                self.gpus = gpus;
            }
            Message::PinToAppTray(usize) => {
                let pinned_id = self.entry_path_input.get(usize).map(|e| e.id.clone());
                if let Some((pinned_id, app_list_helper)) = pinned_id
                    .zip(Config::new(cosmic_app_list_config::APP_ID, AppListConfig::VERSION).ok())
                {
                    self.app_list_config.add_pinned(pinned_id, &app_list_helper);
                }
                self.menu = None;
                return commands::popup::destroy_popup(MENU_ID.clone());
            }
            Message::UnPinFromAppTray(usize) => {
                let pinned_id = self.entry_path_input.get(usize).map(|e| e.id.clone());
                if let Some((pinned_id, app_list_helper)) = pinned_id
                    .zip(Config::new(cosmic_app_list_config::APP_ID, AppListConfig::VERSION).ok())
                {
                    self.app_list_config
                        .remove_pinned(&pinned_id, &app_list_helper);
                }
                self.menu = None;
                return commands::popup::destroy_popup(MENU_ID.clone());
            }
            Message::AppListConfig(config) => {
                self.app_list_config = config;
            }
            Message::Opened(size, window_id) => {
                if window_id == WINDOW_ID.clone() {
                    self.height = size.height;
                    self.handle_overlap();
                }
            }
            Message::Overlap(overlap_notify_event) => match overlap_notify_event {
                OverlapNotifyEvent::OverlapLayerAdd {
                    identifier,
                    namespace,
                    logical_rect,
                    exclusive,
                    ..
                } => {
                    if self.needs_clear {
                        self.needs_clear = false;
                        self.overlap.clear();
                    }
                    if exclusive > 0 || namespace == "Dock" || namespace == "Panel" {
                        self.overlap.insert(identifier, logical_rect);
                    }
                    self.handle_overlap();
                }
                OverlapNotifyEvent::OverlapLayerRemove { identifier } => {
                    self.overlap.remove(&identifier);
                    self.handle_overlap();
                }
                _ => {}
            },
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
        }
        Task::none()
    }

    fn dbus_activation(&mut self, msg: dbus_activation::Message) -> Task<Self::Message> {
        if matches!(msg.msg, dbus_activation::Details::Activate) {
            self.activate()
        } else {
            Task::none()
        }
    }

    fn view(&self) -> Element<Message> {
        unimplemented!()
    }

    fn view_window(&self, id: SurfaceId) -> Element<Message> {
        let Spacing {
            space_none,
            space_xxs,
            space_xs,
            space_s,
            space_m,
            space_l,
            space_xxl,
            ..
        } = theme::active().cosmic().spacing;

        if id == MENU_ID.clone() {
            let Some((menu, i)) = self
                .menu
                .as_ref()
                .and_then(|i| self.entry_path_input.get(*i).map(|e| (e, i)))
            else {
                return container(horizontal_space())
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
                        menu_button(body(format!(
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
                    menu_button(body(RUN.clone()))
                        .on_press(Message::ActivateApp(*i, None))
                        .into(),
                );
            }

            if menu.desktop_actions.len() > 0 {
                list_column.push(divider::horizontal::light().into());
                for action in menu.desktop_actions.iter() {
                    list_column.push(
                        menu_button(body(&action.name))
                            .on_press(Message::SelectAction(
                                MenuAction::DesktopAction(action.exec.clone()).into(),
                            ))
                            .into(),
                    );
                }
            }

            // add to pinned
            let svg_accent = Rc::new(|theme: &cosmic::Theme| {
                let color = theme.cosmic().accent_color().into();
                svg::Style { color: Some(color) }
            });
            let is_pinned = self.app_list_config.favorites.iter().any(|p| p == &menu.id);
            let pin_to_app_tray = menu_button(
                if is_pinned {
                    row![
                        icon::icon(icon::from_name("checkbox-checked-symbolic").size(16).into())
                            .class(cosmic::theme::Svg::Custom(svg_accent.clone())),
                        body(fl!("pin-to-app-tray"))
                    ]
                } else {
                    row![horizontal_space().width(16.0), body(fl!("pin-to-app-tray"))]
                }
                .spacing(space_xxs),
            )
            .on_press(if is_pinned {
                Message::UnPinFromAppTray(*i)
            } else {
                Message::PinToAppTray(*i)
            });
            list_column.push(divider::horizontal::light().into());
            list_column.push(pin_to_app_tray.into());

            if self.cur_group > 0 {
                list_column.push(divider::horizontal::light().into());
                list_column.push(
                    menu_button(body(REMOVE.clone()))
                        .on_press(Message::SelectAction(MenuAction::Remove))
                        .into(),
                );
            }

            return autosize(
                container(scrollable(Column::with_children(list_column)))
                    .padding([8, 0])
                    .class(theme::Container::Custom(Box::new(|theme| {
                        container::Style {
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
                    .align_y(Vertical::Top),
                MENU_AUTOSIZE_ID.clone(),
            )
            .max_height(800.)
            .max_width(300.)
            .into();
        }
        if id == NEW_GROUP_WINDOW_ID.clone() {
            let Some(group_name) = self.new_group.as_ref() else {
                return container(horizontal_space())
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(1.0))
                    .into();
            };
            let dialog = column![
                container(text(CREATE_NEW.as_str()).size(24))
                    .align_x(Horizontal::Left)
                    .width(Length::Fixed(432.0)),
                text_input("", group_name)
                    .label(&*NEW_GROUP_PLACEHOLDER)
                    .on_input(Message::NewGroup)
                    .on_submit(|_| Message::SubmitNewGroup)
                    .width(Length::Fixed(432.0))
                    .size(14)
                    .id(NEW_GROUP_ID.clone()),
                container(
                    row![
                        button::custom(
                            container(text(CANCEL.to_string()).size(14.0))
                                .width(Length::Shrink)
                                .align_x(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .on_press(Message::CancelNewGroup)
                        .padding([space_xxs, space_s])
                        .width(142),
                        button::custom(
                            container(text(SAVE.to_string()).size(14.0))
                                .width(Length::Shrink)
                                .align_x(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .class(Button::Suggested)
                        .on_press(Message::SubmitNewGroup)
                        .padding([space_xxs, space_s])
                        .width(142),
                    ]
                    .spacing(space_s)
                )
                .width(Length::Fixed(432.0))
                .align_x(Horizontal::Right)
            ]
            .align_x(Alignment::Center)
            .spacing(space_s);
            return autosize(
                container(dialog)
                    .class(theme::Container::Custom(Box::new(|theme| {
                        container::Style {
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
                    .padding(space_s),
                NEW_GROUP_AUTOSIZE_ID.clone(),
            )
            .into();
        }
        if id == DELETE_GROUP_WINDOW_ID.clone() {
            let dialog = column![
                row![
                    container(
                        icon::icon(icon::from_name("edit-delete-symbolic").into())
                            .width(Length::Fixed(48.0))
                            .height(Length::Fixed(48.0))
                    )
                    .padding(8),
                    column![
                        text(fl!("delete-folder")).size(24),
                        text(fl!("delete-folder", "msg"))
                    ]
                    .spacing(8)
                    .width(Length::Fixed(360.0))
                ]
                .spacing(16),
                container(
                    row![
                        button::custom(
                            container(text(CANCEL.to_string()).size(14.0))
                                .width(Length::Shrink)
                                .align_x(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .on_press(Message::CancelDelete)
                        .padding([space_xxs, space_m])
                        .width(142),
                        button::custom(
                            container(text(fl!("delete")).size(14.0))
                                .width(Length::Shrink)
                                .align_x(Horizontal::Center)
                                .width(Length::Fill)
                        )
                        .id(SUBMIT_DELETE_ID.clone())
                        .class(Button::Destructive)
                        .on_press(Message::ConfirmDelete)
                        .padding([space_xxs, space_m])
                        .width(142),
                    ]
                    .spacing(space_s)
                )
                .width(Length::Fixed(432.0))
                .align_x(Horizontal::Right)
            ]
            .align_x(Alignment::Center)
            .spacing(space_l);
            return autosize(
                container(dialog)
                    .class(theme::Container::Custom(Box::new(|theme| {
                        container::Style {
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
                    .padding(space_m),
                DELETE_GROUP_AUTOSIZE_ID.clone(),
            )
            .into();
        }

        let cur_group = self.config.groups()[self.cur_group];
        let top_row = if self.cur_group == 0 {
            row![
                container(
                    search_input(SEARCH_PLACEHOLDER.as_str(), self.search_value.as_str())
                        .on_input(Message::InputChanged)
                        .on_paste(Message::InputChanged)
                        .on_submit(|_| Message::StartCurAppFocus)
                        .style(TextInput::Search)
                        .width(Length::Fixed(400.0))
                        .size(14)
                        .id(SEARCH_ID.clone())
                )
                .align_y(Vertical::Center)
                .height(Length::Fixed(96.0))
            ]
            .align_y(Alignment::Center)
            .spacing(space_xxs)
        } else {
            row![
                horizontal_space().width(Length::FillPortion(1)),
                if let Some(edit_name) = self.edit_name.as_ref() {
                    container(
                        text_input(cur_group.name(), edit_name)
                            .on_input(Message::EditName)
                            .on_paste(Message::EditName)
                            .on_clear(Message::EditName(String::new()))
                            .on_submit(|_| Message::SubmitName)
                            .id(EDIT_GROUP_ID.clone())
                            .width(Length::Fixed(200.0))
                            .size(14),
                    )
                } else {
                    container(text(cur_group.name()).size(24))
                },
                row![
                    horizontal_space(),
                    tooltip(
                        {
                            let mut b = button::custom(
                                icon::icon(icon::from_name("edit-symbolic").into())
                                    .width(Length::Fixed(32.0))
                                    .height(Length::Fixed(32.0)),
                            )
                            .padding(space_xs)
                            .class(Button::Icon);
                            if self.edit_name.is_none() {
                                b = b.on_press(Message::StartEditName(cur_group.name()));
                            }
                            container(b)
                                .height(Length::Fixed(96.0))
                                .align_y(Vertical::Center)
                        },
                        text(fl!("rename")),
                        tooltip::Position::Bottom
                    ),
                    tooltip(
                        container(
                            button::custom(
                                icon::icon(icon::from_name("edit-delete-symbolic").into())
                                    .width(Length::Fixed(32.0))
                                    .height(Length::Fixed(32.0)),
                            )
                            .padding(space_xs)
                            .class(Button::Icon)
                            .on_press(Message::Delete(self.cur_group))
                        )
                        .height(Length::Fixed(96.0))
                        .align_y(Vertical::Center),
                        text(fl!("delete")),
                        tooltip::Position::Bottom
                    )
                ]
                .spacing(space_xxs)
                .width(Length::FillPortion(1))
            ]
            .padding([0, space_l])
            .align_y(Alignment::Center)
        };

        // TODO grid widget in libcosmic
        let app_grid_list: Vec<_> = self
            .entry_path_input
            .iter()
            .zip(self.entry_ids.iter())
            .enumerate()
            .map(|(i, (entry, id))| {
                let gpu_idx = self.gpus.as_ref().map(|gpus| {
                    if entry.prefers_dgpu {
                        gpus.iter().position(|gpu| !gpu.default).unwrap_or(0)
                    } else {
                        gpus.iter().position(|gpu| gpu.default).unwrap_or(0)
                    }
                });
                let dup = entry
                    .path
                    .as_ref()
                    .and_then(|path| self.duplicates.get(path));
                let selected = self.menu.is_some_and(|m| m == i);

                let b = ApplicationButton::new(
                    id.clone(),
                    &entry,
                    move |rect| Message::OpenContextMenu(rect, i),
                    if self.menu.is_none() {
                        Some(Message::ActivateApp(i, gpu_idx))
                    } else if selected {
                        Some(Message::CloseContextMenu)
                    } else {
                        None
                    },
                    // TODO add icon and text if duplicated
                    dup,
                    selected,
                    self.menu.is_none().then_some(Message::StartDrag(i)),
                    self.menu.is_none().then_some(Message::FinishDrag(false)),
                    self.menu.is_none().then_some(Message::CancelDrag),
                );

                b.into()
            })
            .chunks(7)
            .into_iter()
            .map(|row_chunk| {
                let mut new_row = row_chunk.collect_vec();
                let missing = 7 - new_row.len();
                if missing > 0 {
                    new_row.push(
                        iced::widget::horizontal_space()
                            .width(Length::FillPortion(missing.try_into().unwrap()))
                            .into(),
                    );
                }
                row(new_row).spacing(space_xxs).into()
            })
            .collect();

        let app_scrollable = container(
            scrollable(
                column(app_grid_list)
                    .width(Length::Fill)
                    .spacing(space_xxs)
                    .padding([space_none, space_xxl, space_xxs, space_xxl]),
            )
            .on_scroll(|viewport| Message::ScrollYOffset(viewport.absolute_offset().y))
            .id(self.scrollable_id.clone())
            .height(Length::Fill),
        )
        .max_height(444.0);

        // TODO use the spacing variables from the theme
        let (group_icon_size, h_padding, group_width, chunks) = if self.config.groups().len() > 15 {
            (16.0, space_xxs, 96.0, 11)
        } else {
            (32.0, space_s, 128.0, 8)
        };
        let group_height =
            group_icon_size + 20.0 + (space_none as f32) + (space_xxs as f32) + (space_s as f32);

        let mut add_group_btn = Some(
            button::custom(
                column![
                    container(
                        icon::icon(icon::from_name("folder-new-symbolic").into())
                            .width(Length::Fixed(group_icon_size))
                            .height(Length::Fixed(group_icon_size))
                    )
                    .padding(space_xxs),
                    text(fl!("add-group")).size(14.0).width(Length::Shrink)
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .height(Length::Fixed(group_height))
            .width(Length::Fixed(group_width))
            .class(theme::Button::IconVertical)
            .padding([space_none, h_padding, space_xxs, h_padding])
            .on_press(Message::StartNewGroup),
        );
        let mut group_rows: Vec<_> = self
            .config
            .groups()
            .chunks(chunks)
            .enumerate()
            .map(|(chunk, groups)| {
                let mut group_row = row![]
                    .spacing(space_xxs)
                    .padding([space_s, space_none])
                    .align_y(Alignment::Center);
                for (i, group) in groups.iter().enumerate() {
                    let i = i + chunk * chunks;
                    let group_button = dnd_destination_for_data::<AppletString, Message>(
                        button::custom(
                            column![
                                container(
                                    icon::icon(from_name(group.icon.clone()).into())
                                        .width(Length::Fixed(group_icon_size))
                                        .height(Length::Fixed(group_icon_size))
                                )
                                .padding(space_xxs),
                                text(group.name()).size(14).width(Length::Shrink)
                            ]
                            .align_x(Alignment::Center)
                            .width(Length::Fill),
                        )
                        .height(Length::Fixed(group_height))
                        .width(Length::Fixed(group_width))
                        .class(
                            if self.offer_group == Some(i)
                                || (self.cur_group == i && self.offer_group.is_none())
                            {
                                // TODO customize the IconVertical to highlight in the way we need
                                Button::Custom {
                                    active: Box::new(|focused, theme| {
                                        let s =
                                            theme.pressed(focused, false, &Button::IconVertical);
                                        s
                                    }),
                                    disabled: Box::new(|theme| {
                                        let s = theme.disabled(&Button::IconVertical);
                                        s
                                    }),
                                    hovered: Box::new(|focused, theme| {
                                        let s =
                                            theme.hovered(focused, false, &Button::IconVertical);
                                        s
                                    }),
                                    pressed: Box::new(|focused, theme| {
                                        let s =
                                            theme.pressed(focused, false, &Button::IconVertical);
                                        s
                                    }),
                                }
                            } else {
                                Button::IconVertical
                            },
                        )
                        .padding([space_none, h_padding, space_xxs, h_padding])
                        .on_press_maybe(self.menu.is_none().then_some(Message::SelectGroup(i))),
                        move |data, _| {
                            Message::FinishDndOffer(
                                i,
                                data.and_then(|data| load_desktop_file(&[], data.0)),
                            )
                        },
                    )
                    .on_enter(move |_, _, _| Message::StartDndOffer(i))
                    .on_leave(move || Message::LeaveDndOffer(i));

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
                    .padding([space_s, space_none])
                    .align_y(Alignment::Center),
            );
        };
        let group_rows =
            Column::with_children(group_rows.into_iter().map(|r| r.into()).collect_vec());

        let content = column![
            top_row,
            app_scrollable,
            container(horizontal_rule(1))
                .padding([space_none, space_xxl])
                .width(Length::Fill),
            group_rows
        ]
        .align_x(Alignment::Center);

        let window = container(content)
            .height(Length::Fill)
            .max_height(685)
            .max_width(1200.0)
            .class(theme::Container::Custom(Box::new(|theme| {
                container::Style {
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
            .center_x(Length::Fill);
        row![
            mouse_area(
                container(horizontal_space().width(Length::Fixed(1.0)))
                    .width(Length::Fill)
                    .height(Length::Fill)
            )
            .on_press(Message::Hide),
            container(
                column![
                    mouse_area(
                        container(vertical_space())
                            .width(Length::Fill)
                            .height(Length::Fixed(self.margin + 16.))
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
                        container(vertical_space())
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
                container(horizontal_space().width(Length::Fixed(1.0)))
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
                listen_with(|e, status, id| match e {
                    cosmic::iced::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::Layer(e, _, id),
                    )) => Some(Message::Layer(e, id)),
                    cosmic::iced::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::OverlapNotify(event),
                    )) => Some(Message::Overlap(event)),
                    cosmic::iced::Event::Keyboard(cosmic::iced::keyboard::Event::KeyReleased {
                        key: Key::Named(Named::Escape),
                        modifiers: _mods,
                        ..
                    }) => Some(Message::Hide),
                    cosmic::iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
                        if id == WINDOW_ID.clone() =>
                    {
                        Some(Message::CloseContextMenu)
                    }
                    cosmic::iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key,
                        text: _,
                        modifiers,
                        ..
                    }) => match key {
                        Key::Character(c) if modifiers.control() && (c == "p" || c == "k") => {
                            Some(Message::PrevRow)
                        }
                        Key::Character(c) if modifiers.control() && (c == "n" || c == "j") => {
                            Some(Message::NextRow)
                        }
                        Key::Character(c) if modifiers.control() && (c == "f" || c == "l") => {
                            Some(Message::KeyboardNav(keyboard_nav::Action::FocusNext))
                        }
                        Key::Character(c) if modifiers.control() && (c == "b" || c == "h") => {
                            Some(Message::KeyboardNav(keyboard_nav::Action::FocusPrevious))
                        }
                        Key::Named(Named::ArrowUp)
                            if matches!(status, iced::event::Status::Ignored) =>
                        {
                            Some(Message::PrevRow)
                        }
                        Key::Named(Named::ArrowDown)
                            if matches!(status, iced::event::Status::Ignored) =>
                        {
                            Some(Message::NextRow)
                        }
                        Key::Named(Named::ArrowLeft)
                            if matches!(status, iced::event::Status::Ignored) =>
                        {
                            Some(Message::KeyboardNav(keyboard_nav::Action::FocusPrevious))
                        }
                        Key::Named(Named::ArrowRight)
                            if matches!(status, iced::event::Status::Ignored) =>
                        {
                            Some(Message::KeyboardNav(keyboard_nav::Action::FocusNext))
                        }
                        _ => None,
                    },
                    cosmic::iced::Event::Window(WindowEvent::Opened { position: _, size }) => {
                        Some(Message::Opened(size, id))
                    }
                    _ => None,
                }),
                keyboard_nav::subscription().map(|a| Message::KeyboardNav(a)),
                self.core
                    .watch_config::<cosmic_app_list_config::AppListConfig>(
                        cosmic_app_list_config::APP_ID,
                    )
                    .map(|config| Message::AppListConfig(config.config)),
            ]
            .into_iter(),
        )
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, _flags: Args) -> (Self, iced::Task<cosmic::Action<Self::Message>>) {
        core.set_keyboard_nav(false);
        let helper = AppLibraryConfig::helper();

        let mut config: AppLibraryConfig = helper
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
        config.groups.sort();
        let scrollable_id = Id::new(
            config
                .groups()
                .get(0)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "unknown-group".to_string()),
        );
        let self_ = Self {
            locale: std::env::var("LANG")
                .ok()
                .and_then(|l| l.split(".").next().map(str::to_string)),
            config,
            core,
            helper,
            last_hide: None,
            margin: 0.,
            overlap: HashMap::new(),
            height: 100.,
            scrollable_id,
            ..Default::default()
        };

        (self_, Task::none())
    }
}
