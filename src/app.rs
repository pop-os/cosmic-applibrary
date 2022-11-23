use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::PathBuf;

use cosmic::button;
use cosmic::iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, text_input, vertical_space,
    Image,
};
use cosmic::iced::{alignment::Horizontal, executor, Alignment, Application, Command, Length};
use cosmic::iced_native::window::Id as SurfaceId;
use cosmic::iced_style::application::{self, Appearance};
use cosmic::theme::{Button, Container};
use cosmic::widget::widget::svg;
use cosmic::widget::{icon, image_icon};
use cosmic::{settings, Element, Theme};
use freedesktop_desktop_entry::DesktopEntry;
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

#[derive(Default)]
struct CosmicAppLibrary {
    id_ctr: u64,
    input_value: String,
    entry_path_input: Vec<(PathBuf, String)>,
    groups: Vec<AppGroup>,
    cur_group: Option<usize>,
    selected_app: Option<usize>,
    selected_group: Option<usize>,
    active_surface: Option<SurfaceId>,
    theme: Theme,
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

impl Application for CosmicAppLibrary {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            CosmicAppLibrary {
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
            Message::InputChanged(value) => self.input_value = value,
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
                // TODO reset application list based on group
            }
            Message::ActivateApp(i) => {
                // TODO activate the app at index i
            }
            Message::SelectGroup(i) => {
                // TODO select the group at index i
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
                self.entry_path_input = freedesktop_desktop_entry::Iter::new(
                    freedesktop_desktop_entry::default_paths(),
                )
                .filter_map(|path| {
                    std::fs::read_to_string(&path)
                        .ok()
                        .map(|input| (path, input))
                })
                .collect();
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

                let clear_button = button!("X").padding(10).on_press(Message::Clear);

                // TODO grid widget in libcosmic
                let app_grid_list: Vec<_> = self
                    .entry_path_input
                    .iter()
                    .filter_map(|(path, input)| {
                        DesktopEntry::decode(path, input)
                            .ok()
                            .filter(|de| !de.no_display())
                            .and_then(|de| {
                                if let Some(image) =
                                    freedesktop_icons::lookup(de.icon().unwrap_or(de.appid))
                                        .with_size(72)
                                        .with_cache()
                                        .find()
                                {
                                    let name = &de.name(None).unwrap_or(Cow::Borrowed(de.appid));
                                    let name = if name.len() > 10 {
                                        format!("{:.10}...", name)
                                    } else {
                                        name.to_string()
                                    };
                                    let mut btn_column = column![];
                                    btn_column = if image.extension() == Some(&OsStr::new("svg")) {
                                        btn_column.push(
                                            svg::Svg::from_path(image)
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
                                        .push(text(name).horizontal_alignment(Horizontal::Center));
                                    Some(
                                        button!(btn_column
                                            .spacing(8)
                                            .align_items(Alignment::Center)
                                            .width(Length::Fill))
                                        .width(Length::FillPortion(1))
                                        .style(Button::Secondary)
                                        .padding(16)
                                        .into(),
                                    )
                                } else {
                                    // TODO maybe want to log the missing icons somewhere
                                    None
                                }
                            })
                    })
                    .chunks(7)
                    .into_iter()
                    .map(|row_chunk| {
                        row(row_chunk.collect_vec())
                            .spacing(8)
                            .padding([0, 16, 0, 0])
                            .into()
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
                    for group in &self.groups {
                        group_row = group_row.push(
                            button!(
                                column![
                                    icon(&group.icon, 32),
                                    text(&group.name).horizontal_alignment(Horizontal::Center)
                                ]
                                .spacing(8)
                                .align_items(Alignment::Center)
                                .width(Length::Fill)
                            )
                            .height(Length::Fill)
                            .width(Length::Units(128))
                            .style(Button::Secondary)
                            .padding([16, 8]),
                        );
                    }
                    group_row
                };

                let content = column![
                    row![text_input, clear_button].spacing(8),
                    column![app_scrollable, horizontal_rule(1), group_row]
                        .spacing(16)
                        .align_items(Alignment::Center),
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
