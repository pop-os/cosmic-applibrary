use std::borrow::Cow;
use std::path::PathBuf;

use cosmic::iced::id::Id;
use cosmic::iced::subscription::events_with;
use cosmic::iced::wayland::actions::layer_surface::SctkLayerSurfaceSettings;
use cosmic::iced::wayland::layer_surface::{
    destroy_layer_surface, get_layer_surface, Anchor, KeyboardInteractivity,
};
use cosmic::iced::wayland::InitialSurface;
use cosmic::iced::widget::{column, container, horizontal_rule, row, scrollable, text, text_input};
use cosmic::iced::{alignment::Horizontal, executor, Alignment, Application, Command, Length};
use cosmic::iced::{Color, Subscription};
use cosmic::iced_runtime::core::event::wayland::LayerEvent;
use cosmic::iced_runtime::core::event::{wayland, PlatformSpecific};
use cosmic::iced_runtime::core::keyboard::KeyCode;
use cosmic::iced_runtime::core::window::Id as SurfaceId;
use cosmic::iced_style::application::{self, Appearance};
use cosmic::iced_widget::text_input::{Icon, Side};
use cosmic::theme::{Button, Container, TextInput};
use cosmic::widget::icon;
use cosmic::{iced, settings, Element, Theme};
use freedesktop_desktop_entry::DesktopEntry;
use iced::wayland::actions::layer_surface::IcedMargin;

use itertools::Itertools;
use once_cell::sync::Lazy;

use crate::app_group::{AppGroup, FilterType};
use crate::subscriptions::desktop_files::desktop_files;
use crate::subscriptions::toggle_dbus::dbus_toggle;
use crate::{config, fl};

static INPUT_ID: Lazy<Id> = Lazy::new(Id::unique);

const WINDOW_ID: SurfaceId = SurfaceId(1);

pub fn run() -> cosmic::iced::Result {
    let mut settings = settings();
    settings.exit_on_close_request = false;
    settings.initial_surface = InitialSurface::None;
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
    input_value: String,
    entry_path_input: Vec<MyDesktopEntryData>,
    groups: Vec<AppGroup>,
    cur_group: usize,
    active_surface: bool,
    theme: Theme,
    locale: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    InputChanged(String),
    Closed(SurfaceId),
    Layer(LayerEvent),
    Toggle,
    Hide,
    Clear,
    ActivateApp(usize),
    SelectGroup(usize),
    LoadApps,
    Ignore,
}

impl CosmicAppLibrary {
    pub fn load_apps(&mut self) {
        self.entry_path_input =
            freedesktop_desktop_entry::Iter::new(freedesktop_desktop_entry::default_paths())
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
            Command::none(),
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
            }
            Message::Closed(id) => {
                if self.active_surface && id == WINDOW_ID {
                    self.active_surface = false;
                    return Command::perform(async {}, |_| Message::Clear);
                }
                // TODO handle popups closed
            }
            Message::Layer(e) => match e {
                LayerEvent::Focused => {
                    return text_input::focus(INPUT_ID.clone());
                }
                LayerEvent::Unfocused => {
                    if self.active_surface {
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
                if self.active_surface {
                    self.active_surface = false;
                    return Command::batch(vec![
                        destroy_layer_surface(WINDOW_ID),
                        Command::perform(async {}, |_| Message::Clear),
                    ]);
                }
            }
            Message::Clear => {
                self.input_value.clear();
                self.cur_group = 0;
                self.load_apps();
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
                if self.active_surface {
                    self.active_surface = false;
                    return destroy_layer_surface(WINDOW_ID);
                } else {
                    let mut cmds = Vec::new();

                    self.input_value = "".to_string();
                    self.active_surface = true;
                    cmds.push(text_input::focus(INPUT_ID.clone()));
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
        }
        Command::none()
    }

    fn view(&self, _id: SurfaceId) -> Element<Message> {
        let text_input = text_input("Type to search apps...", &self.input_value)
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

                    iced::widget::button(
                        column![
                            icon(image.as_path(), 72)
                                .width(Length::Fixed(72.0))
                                .height(Length::Fixed(72.0)),
                            text(name)
                                .horizontal_alignment(Horizontal::Center)
                                .size(11)
                                .height(Length::Fixed(40.0))
                        ]
                        .width(Length::Fixed(120.0))
                        .height(Length::Fixed(120.0))
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
            .height(Length::Fixed(600.0));

        let group_row = {
            let mut group_row = row![]
                .height(Length::Fixed(100.0))
                .spacing(8)
                .align_items(Alignment::Center);
            for (i, group) in self.groups.iter().enumerate() {
                let mut group_button = iced::widget::button(
                    column![
                        icon(&*group.icon, 32),
                        text(&group.name).horizontal_alignment(Horizontal::Center)
                    ]
                    .spacing(8)
                    .align_items(Alignment::Center)
                    .width(Length::Fill),
                )
                .height(Length::Fill)
                .width(Length::Fixed(128.0))
                .style(Button::Primary)
                .padding([16, 8]);
                if i != self.cur_group {
                    group_button = group_button
                        .on_press(Message::SelectGroup(i))
                        .style(Button::Secondary);
                } else {
                    group_button = group_button.on_press(Message::Ignore);
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
            .style(Container::Custom(Box::new(|theme| container::Appearance {
                text_color: Some(theme.cosmic().on_bg_color().into()),
                background: Some(Color::from(theme.cosmic().background.base).into()),
                border_radius: 16.0.into(),
                border_width: 1.0,
                border_color: theme.cosmic().bg_divider().into(),
            })))
            .center_x()
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
