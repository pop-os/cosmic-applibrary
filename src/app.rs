use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::PathBuf;

use cosmic::iced::widget::{
    column, container, horizontal_rule, row, scrollable, text, text_input, Image,
};
use cosmic::iced::{alignment::Horizontal, executor, Alignment, Application, Command, Length};
use cosmic::iced_native::window::Id as SurfaceId;
use cosmic::iced_style::application::{self, Appearance};
use cosmic::theme::{Button, Container};
use cosmic::widget::icon;
use cosmic::{settings, Element, Theme, Renderer};
use freedesktop_desktop_entry::DesktopEntry;
use iced::widget::{horizontal_space, svg};
use iced_sctk::application::SurfaceIdWrapper;
use iced_sctk::command::platform_specific::wayland::layer_surface::SctkLayerSurfaceSettings;
use iced_sctk::commands::layer_surface::{Anchor, KeyboardInteractivity, Layer};
use iced_sctk::event::wayland::LayerEvent;
use iced_sctk::event::{wayland, PlatformSpecific};
use iced_sctk::keyboard::KeyCode;
use iced_sctk::settings::InitialSurface;
use iced_sctk::subscription::events_with;
use iced_sctk::{commands, Color, Subscription};
use itertools::Itertools;
use once_cell::sync::Lazy;

use crate::app_group::{AppGroup, FilterType};
use crate::subscriptions::desktop_files::{desktop_files, DesktopFileEvent};
use crate::subscriptions::toggle_dbus::{dbus_toggle, DbusEvent};
use crate::{config, fl};
static INPUT_ID: Lazy<text_input::Id> = Lazy::new(text_input::Id::unique);

pub fn run() -> cosmic::iced::Result {
    let mut settings = settings();
    settings.exit_on_close_request = false;
    settings.initial_surface = InitialSurface::LayerSurface(SctkLayerSurfaceSettings {
        keyboard_interactivity: KeyboardInteractivity::None,
        namespace: "ignore".into(),
        size: (Some(1), Some(1)),
        layer: Layer::Background,
        ..Default::default()
    });
    CosmicAppLibrary::run(settings)
}

#[derive(Debug, Clone)]
struct MyDesktopEntryData {
    desktop_entry_path: PathBuf,
    name: String,
    icon: PathBuf,
}

#[derive(Default)]
struct CosmicAppLibrary {
    id_ctr: u64,
    input_value: String,
    entry_path_input: Vec<MyDesktopEntryData>,
    groups: Vec<AppGroup>,
    cur_group: usize,
    active_surface: Option<SurfaceId>,
    theme: Theme,
    locale: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    InputChanged(String),
    Closed(SurfaceIdWrapper),
    Layer(LayerEvent),
    Toggle,
    Hide,
    Clear,
    ActivateApp(usize),
    SelectGroup(usize),
    LoadApps,
}

impl CosmicAppLibrary {
    pub fn load_apps(&mut self) {
        self.entry_path_input = freedesktop_desktop_entry::Iter::new(
            freedesktop_desktop_entry::default_paths(),
        )
        .filter_map(|path| {
            std::fs::read_to_string(&path).ok().and_then(|input| {
                DesktopEntry::decode(&path, &input).ok().and_then(|de| {
                    let name = de
                        .name(self.locale.as_ref().map(|x| &**x))
                        .unwrap_or(Cow::Borrowed(de.appid))
                        .to_string();
                    let group_filter = &self.groups[self.cur_group];
                    let mut keep_de = !de.no_display()
                        && match &group_filter.filter {
                            FilterType::AppNames(names) => names.contains(&name),
                            FilterType::Categories(categories) => {
                                categories.into_iter().any(|cat| {
                                    de.categories()
                                        .map(|cats| {
                                            cats.to_lowercase()
                                                .contains(&cat.to_lowercase())
                                        })
                                        .unwrap_or_default()
                                })
                            }
                            FilterType::None => true,
                        };
                    if keep_de && self.input_value.len() > 0 {
                        keep_de = name
                            .to_lowercase()
                            .contains(&self.input_value.to_lowercase())
                            || de
                                .categories()
                                .map(|cats| {
                                    cats.to_lowercase()
                                        .contains(&self.input_value.to_lowercase())
                                })
                                .unwrap_or_default()
                    }
                    if keep_de {
                        freedesktop_icons::lookup(de.icon().unwrap_or(de.appid))
                            .with_size(72)
                            .with_cache()
                            .find()
                            .map(|icon| MyDesktopEntryData {
                                desktop_entry_path: path.clone(),
                                name,
                                icon,
                            })
                    } else {
                        None
                    }
                })
            })
        })
        .collect();
    }
}

impl Application for CosmicAppLibrary {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            CosmicAppLibrary {
                locale: current_locale::current_locale().ok(),
                groups: vec![
                    AppGroup {
                        name: fl!("library-home"),
                        icon: "user-home-symbolic".to_string(),
                        mutable: false,
                        filter: FilterType::None,
                    },
                    AppGroup {
                        name: fl!("office"),
                        icon: "folder-symbolic".to_string(),
                        mutable: false,
                        filter: FilterType::Categories(vec!["Office".to_string()]),
                    },
                    AppGroup {
                        name: fl!("system"),
                        icon: "folder-symbolic".to_string(),
                        mutable: false,
                        filter: FilterType::Categories(vec!["System".to_string()]),
                    },
                    AppGroup {
                        name: fl!("utilities"),
                        icon: "folder-symbolic".to_string(),
                        mutable: false,
                        filter: FilterType::Categories(vec!["Utility".to_string()]),
                    },
                ],
                ..Default::default()
            },
            commands::layer_surface::destroy_layer_surface(SurfaceId::new(0)),
        )
    }

    fn title(&self) -> String {
        config::APP_ID.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::InputChanged(value) => {
                self.input_value = value;
                self.load_apps();
            },
            Message::Closed(id) => {
                if self
                    .active_surface
                    .map(|active_id| SurfaceIdWrapper::LayerSurface(active_id) == id)
                    .unwrap_or_default()
                {
                    self.active_surface.take();
                    // TODO reset app state
                }
                // TODO handle popups closed
            }
            Message::Layer(e) => match e {
                LayerEvent::Focused(_) => {
                    return text_input::focus(INPUT_ID.clone());
                }
                LayerEvent::Unfocused(_) => {
                    if let Some(id) = self.active_surface {
                        return commands::layer_surface::destroy_layer_surface(id);
                    }
                }
                _ => {}
            },
            Message::Hide => {
                if let Some(id) = self.active_surface {
                    return commands::layer_surface::destroy_layer_surface(id);
                }
            }
            Message::Clear => {
                self.input_value.clear();
                self.load_apps();
                // TODO reset application list based on group
            }
            Message::ActivateApp(i) => {
                if let Some(de) = self.entry_path_input.get(i).and_then(
                    |MyDesktopEntryData {
                         desktop_entry_path, ..
                     }| {
                        std::fs::read_to_string(&desktop_entry_path)
                            .ok()
                            .and_then(|input| {
                                DesktopEntry::decode(desktop_entry_path, &input)
                                    .ok()
                                    .map(|de| de.exec().map(|e| e.to_string()))
                            })
                    },
                ) {
                    let mut exec = match de.as_ref() {
                        Some(exec_str) => shlex::Shlex::new(exec_str),
                        _ => return Command::none(),
                    };
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
                self.cur_group = i;
                self.load_apps();
            }
            Message::Toggle => {
                if let Some(id) = self.active_surface {
                    return commands::layer_surface::destroy_layer_surface(id);
                } else {
                    self.id_ctr += 1;
                    let mut cmds = Vec::new();

                    self.input_value = "".to_string();
                    let id = SurfaceId::new(self.id_ctr);
                    self.active_surface.replace(id);
                    cmds.push(text_input::focus(INPUT_ID.clone()));
                    cmds.push(commands::layer_surface::get_layer_surface(
                        SctkLayerSurfaceSettings {
                            id,
                            keyboard_interactivity: KeyboardInteractivity::Exclusive,
                            anchor: Anchor::empty(),
                            namespace: "app-library".into(),
                            size: (Some(1200), Some(860)),
                            ..Default::default()
                        },
                    ));
                    return Command::batch(cmds);
                }
            }
            Message::LoadApps => {
                self.load_apps();
            }
        }
        Command::none()
    }

    fn view(&self, id: SurfaceIdWrapper) -> Element<Message> {
        match id {
            SurfaceIdWrapper::LayerSurface(_) => {
                let text_input = text_input(
                    "Type something...",
                    &self.input_value,
                    Message::InputChanged,
                )
                .width(Length::Units(400))
                .size(20)
                .id(INPUT_ID.clone());

                // TODO grid widget in libcosmic
                let app_grid_list: Vec<_> = self
                    .entry_path_input
                    .iter()
                    .enumerate()
                    .map(
                        |(
                            i,
                            MyDesktopEntryData {
                                name, icon: image, ..
                            },
                        )| {
                            let name = if name.len() > 27 {
                                format!("{:.24}...", name)
                            } else {
                                name.to_string()
                            };
                            let mut btn_column = column![];
                            btn_column = if image.extension() == Some(&OsStr::new("svg")) {
                                let handle = svg::Handle::from_path(image);
                                btn_column.push(
                                    svg::Svg::<Renderer>::new(handle)
                                        .width(Length::Units(72))
                                        .height(Length::Units(72)),
                                )
                            } else {
                                btn_column.push(
                                    Image::new(image)
                                        .width(Length::Units(72))
                                        .height(Length::Units(72)),
                                )
                            };
                            btn_column = btn_column
                                .push(text(name).horizontal_alignment(Horizontal::Center).size(16));

                            iced::widget::button(
                                btn_column
                                    .spacing(8)
                                    .align_items(Alignment::Center)
                                    .width(Length::Fill),
                            )
                            .width(Length::FillPortion(1))
                            .style(Button::Text)
                            .padding(16)
                            .on_press(Message::ActivateApp(i))
                            .into()
                        },
                    )
                    .chunks(7)
                    .into_iter()
                    .map(|row_chunk| {
                        let mut new_row = row_chunk.collect_vec();
                        let missing = 7 - new_row.len();
                        if missing > 0 {
                            new_row.push(
                                horizontal_space(Length::FillPortion(missing.try_into().unwrap()))
                                    .into(),
                            );
                        }
                        row(new_row).spacing(8).padding([0, 16, 0, 0]).into()
                    })
                    .collect();
                // let rows = app_grid_list.chunks(7).into_iter().map(|row_chunk| row(row_chunk));

                let app_scrollable =
                    scrollable(column(app_grid_list).width(Length::Fill).spacing(8))
                        .height(Length::Units(600));

                let group_row = {
                    let mut group_row = row![]
                        .height(Length::Units(100))
                        .spacing(8)
                        .align_items(Alignment::Center);
                    for (i, group) in self.groups.iter().enumerate() {
                        let mut group_button = iced::widget::button(
                            column![
                                icon(&group.icon, 32),
                                text(&group.name).horizontal_alignment(Horizontal::Center)
                            ]
                            .spacing(8)
                            .align_items(Alignment::Center)
                            .width(Length::Fill),
                        )
                        .height(Length::Fill)
                        .width(Length::Units(128))
                        .style(Button::Primary)
                        .padding([16, 8]);
                        if i != self.cur_group {
                            group_button = group_button
                                .on_press(Message::SelectGroup(i))
                                .style(Button::Secondary);
                        }
                        group_row = group_row.push(group_button);
                    }
                    group_row
                };

                let content = column![
                    row![text_input].spacing(8),
                    app_scrollable,
                    horizontal_rule(1),
                    group_row
                ]
                .spacing(16)
                .align_items(Alignment::Center)
                .padding([32, 64, 16, 64]);

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(Container::Custom(|theme| container::Appearance {
                        text_color: Some(theme.cosmic().on_bg_color().into()),
                        background: Some(theme.extended_palette().background.base.color.into()),
                        border_radius: 16.0,
                        border_width: 0.0,
                        border_color: Color::TRANSPARENT,
                    }))
                    .center_x()
                    .into()
            }
            SurfaceIdWrapper::Popup(_) => todo!(),
            SurfaceIdWrapper::Window(_) => unimplemented!(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(
            vec![
                dbus_toggle(0).map(|e| match e {
                    (_, DbusEvent::Toggle) => Message::Toggle,
                }),
                desktop_files(0).map(|e| match e {
                    (_, DesktopFileEvent::Changed) => Message::LoadApps,
                }),
                events_with(|e, _status| match e {
                    cosmic::iced::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::Layer(e),
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
        self.theme
    }

    fn style(&self) -> <Self::Theme as application::StyleSheet>::Style {
        <Self::Theme as application::StyleSheet>::Style::Custom(|theme| Appearance {
            background_color: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
            text_color: theme.cosmic().on_bg_color().into(),
        })
    }

    fn close_requested(&self, id: iced_sctk::application::SurfaceIdWrapper) -> Self::Message {
        Message::Closed(id)
    }
}
