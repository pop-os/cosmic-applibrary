//! A widget that can be dragged and dropped.

use std::mem;

use std::str::FromStr;

use cosmic::iced_core::alignment::Horizontal;
use cosmic::iced_core::event::{wayland, PlatformSpecific};
use cosmic::iced_runtime::command::platform_specific;
use cosmic::iced_widget::graphics::image::image_rs::EncodableLayout;
use cosmic::sctk::reexports::client::protocol::wl_data_device_manager::DndAction;

use cosmic::iced_core::{
    event, layout, mouse, overlay, renderer, Alignment, Clipboard, Element, Event, Length, Point,
    Rectangle, Shell, Widget,
};

use cosmic::iced_core::widget::{operation::OperationOutputWrapper, tree, Operation, Tree};
use cosmic::widget::icon::from_name;
use cosmic::{
    iced::{
        self,
        widget::{column, text},
    },
    theme,
    widget::{button, icon},
};

use crate::app_group::DesktopEntryData;

use super::application::MIME_TYPE;

/// A widget that can be dragged and dropped.
#[allow(missing_debug_implementations)]
pub struct GroupButton<'a, Message> {
    content: Element<'a, Message, cosmic::Renderer>,

    on_offer: Option<Message>,

    on_finish: Option<Box<dyn Fn(DesktopEntryData) -> Message + 'a>>,

    on_leave: Option<Message>,

    on_dnd_command_produced: Option<
        Box<
            dyn Fn(
                    Box<
                        dyn Send
                            + Sync
                            + Fn() -> platform_specific::wayland::data_device::ActionInner,
                    >,
                ) -> Message
                + 'a,
        >,
    >,
}

impl<'a, Message: Clone + 'static> GroupButton<'a, Message> {
    /// Creates a new [`ApplicationButton`].
    #[must_use]
    pub fn new(
        name: String,
        icon_name: &'a str,
        on_pressed: Option<Message>,
        style: theme::Button,
    ) -> Self {
        let content = button(
            column![
                icon(from_name(icon_name).into()),
                text(name).horizontal_alignment(Horizontal::Center)
            ]
            .spacing(8)
            .align_items(Alignment::Center)
            .width(Length::Fill),
        )
        .height(Length::Fill)
        .width(Length::Fixed(128.0))
        .style(style)
        .padding([16, 8]);

        let content = if let Some(on_pressed) = on_pressed {
            content.on_press(on_pressed)
        } else {
            content
        }
        .into();

        Self {
            content,
            on_offer: None,
            on_dnd_command_produced: None,
            on_finish: None,
            on_leave: None,
        }
    }

    pub fn on_dnd_command_produced(
        self,
        message: impl Fn(
                Box<dyn Send + Sync + Fn() -> platform_specific::wayland::data_device::ActionInner>,
            ) -> Message
            + 'a,
    ) -> Self {
        Self {
            on_dnd_command_produced: Some(Box::new(message)),
            ..self
        }
    }

    pub fn on_finish(self, message: impl Fn(DesktopEntryData) -> Message + 'a) -> Self {
        Self {
            on_finish: Some(Box::new(message)),
            ..self
        }
    }

    pub fn on_cancel(self, message: Message) -> Self {
        Self {
            on_leave: Some(message),
            ..self
        }
    }

    pub fn on_offer(self, message: Message) -> Self {
        Self {
            on_offer: Some(message),
            ..self
        }
    }
}

impl<'a, Message> From<GroupButton<'a, Message>> for Element<'a, Message, cosmic::Renderer>
where
    Message: Clone + 'a,
{
    fn from(dnd_source: GroupButton<'a, Message>) -> Element<'a, Message, cosmic::Renderer> {
        Element::new(dnd_source)
    }
}

impl<'a, Message> Widget<Message, cosmic::Renderer> for GroupButton<'a, Message>
where
    Message: Clone,
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn width(&self) -> Length {
        self.content.as_widget().width()
    }

    fn height(&self) -> Length {
        self.content.as_widget().height()
    }

    fn layout(&self, renderer: &cosmic::Renderer, limits: &layout::Limits) -> layout::Node {
        layout(
            renderer,
            limits,
            Widget::<Message, cosmic::Renderer>::width(self),
            Widget::<Message, cosmic::Renderer>::height(self),
            u32::MAX,
            u32::MAX,
            |renderer, limits| self.content.as_widget().layout(renderer, limits),
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut cosmic::Renderer,
        theme: &cosmic::theme::Theme,
        renderer_style: &renderer::Style,
        layout: layout::Layout<'_>,
        cursor_position: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            renderer_style,
            layout.children().next().unwrap(),
            cursor_position,
            viewport,
        );
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: layout::Layout<'_>,
        renderer: &cosmic::Renderer,
        operation: &mut dyn Operation<OperationOutputWrapper<Message>>,
    ) {
        operation.container(None, layout.bounds(), &mut |operation| {
            self.content.as_widget().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: layout::Layout<'_>,
        renderer: &cosmic::Renderer,
    ) -> Option<overlay::Element<'b, Message, cosmic::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
        )
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: layout::Layout<'_>,
        cursor_position: mouse::Cursor,
        renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        let ret = self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event.clone(),
            layout.children().next().unwrap(),
            cursor_position,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state = tree.state.downcast_mut::<State>();

        if let (Some(on_dnd_command_produced), Some(on_offer), Some(on_cancel), Some(on_finish)) = (
            self.on_dnd_command_produced.as_ref(),
            self.on_offer.as_ref(),
            self.on_leave.as_ref(),
            self.on_finish.as_ref(),
        ) {
            state.dnd_offer = match mem::take(&mut state.dnd_offer) {
                DndOfferState::None => match &event {
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::SourceActions(actions)),
                    )) => DndOfferState::OutsideWidget(Vec::new(), *actions),
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::Enter {
                            x,
                            y,
                            mime_types,
                        }),
                    )) => {
                        if mime_types.iter().any(|m| m.as_str() == MIME_TYPE) {
                            let point = Point::new(*x as f32, *y as f32);

                            if layout.bounds().contains(point) {
                                shell.publish((on_dnd_command_produced.as_ref())(Box::new(
                                move || {
                                    platform_specific::wayland::data_device::ActionInner::SetActions {
                                        preferred: DndAction::Move,
                                        accepted: DndAction::Move,
                                    }
                                },
                                )));
                                shell.publish((on_dnd_command_produced.as_ref())(Box::new(
                                move || {
                                    platform_specific::wayland::data_device::ActionInner::Accept(
                                        Some(MIME_TYPE.to_string()),
                                    )
                                },
                                )));
                                shell.publish(on_offer.clone());

                                DndOfferState::HandlingOffer(mime_types.clone(), DndAction::empty())
                            } else {
                                DndOfferState::OutsideWidget(mime_types.clone(), DndAction::empty())
                            }
                        } else {
                            DndOfferState::None
                        }
                    }
                    _ => DndOfferState::None,
                },
                DndOfferState::OutsideWidget(mime_types, action) => match &event {
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::SourceActions(actions)),
                    )) => DndOfferState::OutsideWidget(mime_types, *actions),
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::Motion { x, y }),
                    )) => {
                        let point = Point::new(*x as f32, *y as f32);

                        if layout.bounds().contains(point) {
                            shell.publish((on_dnd_command_produced.as_ref())(Box::new(
                                move || {
                                    platform_specific::wayland::data_device::ActionInner::SetActions {
                                        preferred: DndAction::Move,
                                        accepted: DndAction::Move,
                                    }
                                },
                            )));
                            shell.publish((on_dnd_command_produced.as_ref())(Box::new(
                                move || {
                                    platform_specific::wayland::data_device::ActionInner::Accept(
                                        Some(MIME_TYPE.to_string()),
                                    )
                                },
                            )));
                            shell.publish(on_offer.clone());

                            // TODO maybe keep track of data and request here if we don't have it
                            // also maybe just refactor DND Targets to allow easier handling...

                            DndOfferState::HandlingOffer(mime_types, DndAction::empty())
                        } else {
                            DndOfferState::OutsideWidget(mime_types, DndAction::empty())
                        }
                    }
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(
                            wayland::DndOfferEvent::DropPerformed | wayland::DndOfferEvent::Leave,
                        ),
                    )) => DndOfferState::None,
                    _ => DndOfferState::OutsideWidget(mime_types, action),
                },
                DndOfferState::HandlingOffer(mime_types, action) => match &event {
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::Motion { x, y }),
                    )) => {
                        let point = Point::new(*x as f32, *y as f32);
                        if layout.bounds().contains(point) {
                            shell.publish((on_dnd_command_produced.as_ref())(Box::new(
                            move || {
                                platform_specific::wayland::data_device::ActionInner::SetActions {
                                    preferred: DndAction::Move,
                                    accepted: DndAction::Move,
                                }
                            },
                            )));

                            DndOfferState::HandlingOffer(mime_types, DndAction::empty())
                        } else {
                            shell.publish(on_cancel.clone());

                            shell.publish((on_dnd_command_produced.as_ref())(Box::new(
                                move || {
                                    platform_specific::wayland::data_device::ActionInner::Accept(
                                        None,
                                    )
                                },
                            )));
                            DndOfferState::OutsideWidget(mime_types, DndAction::empty())
                        }
                    }
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::Leave),
                    )) => {
                        shell.publish(on_cancel.clone());
                        DndOfferState::None
                    }
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::SourceActions(actions)),
                    )) => {
                        shell.publish((on_dnd_command_produced.as_ref())(Box::new(move || {
                            platform_specific::wayland::data_device::ActionInner::SetActions {
                                preferred: DndAction::Move,
                                accepted: DndAction::Move,
                            }
                        })));
                        DndOfferState::HandlingOffer(mime_types, *actions)
                    }

                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DndOffer(wayland::DndOfferEvent::DropPerformed),
                    )) => {
                        shell.publish((on_dnd_command_produced.as_ref())(Box::new(move || {
                            platform_specific::wayland::data_device::ActionInner::SetActions {
                                preferred: DndAction::Move,
                                accepted: DndAction::Move,
                            }
                        })));
                        shell.publish((on_dnd_command_produced.as_ref())(Box::new(move || {
                            platform_specific::wayland::data_device::ActionInner::Accept(Some(
                                MIME_TYPE.to_string(),
                            ))
                        })));
                        shell.publish((on_dnd_command_produced.as_ref())(Box::new(move || {
                            platform_specific::wayland::data_device::ActionInner::RequestDndData(
                                MIME_TYPE.to_string(),
                            )
                        })));
                        DndOfferState::Dropped
                    }
                    _ => DndOfferState::HandlingOffer(mime_types, action),
                },
                DndOfferState::Dropped => {
                    match &event {
                        event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                            wayland::Event::DndOffer(wayland::DndOfferEvent::DndData {
                                data,
                                mime_type,
                            }),
                        )) => {
                            if mime_type.as_str() == MIME_TYPE {
                                if let Some(data) = std::str::from_utf8(data.as_bytes())
                                    .ok()
                                    .and_then(|s| url::Url::from_str(s).ok())
                                    .and_then(|url| url.to_file_path().ok())
                                    .and_then(|p| DesktopEntryData::try_from(p).ok())
                                {
                                    shell.publish(on_finish(data));

                                    shell.publish((on_dnd_command_produced.as_ref())(Box::new(
                                        move || platform_specific::wayland::data_device::ActionInner::DndFinished,
                                    )));
                                }
                            }

                            DndOfferState::None
                        }
                        event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                            wayland::Event::DndOffer(wayland::DndOfferEvent::Leave),
                        )) => {
                            // already applied the offer, so we can just finish
                            if let Some(on_cancel) = self.on_leave.clone() {
                                shell.publish(on_cancel);
                            }

                            DndOfferState::None
                        }
                        _ => DndOfferState::Dropped,
                    }
                }
            };
        }

        ret
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: layout::Layout<'_>,
        cursor_position: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &cosmic::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor_position,
            viewport,
            renderer,
        )
    }
}

/// Computes the layout of a [`ApplicationButton`].
pub fn layout<Renderer>(
    renderer: &Renderer,
    limits: &layout::Limits,
    width: Length,
    height: Length,
    max_height: u32,
    max_width: u32,
    layout_content: impl FnOnce(&Renderer, &layout::Limits) -> layout::Node,
) -> layout::Node {
    let limits = limits
        .loose()
        .max_height(max_height as f32)
        .max_width(max_width as f32)
        .width(width)
        .height(height);

    let content = layout_content(renderer, &limits);
    let size = limits.resolve(content.size());

    layout::Node::with_children(size, vec![content])
}

#[derive(Debug, Clone, Default)]
pub(crate) enum DndOfferState {
    #[default]
    None,
    OutsideWidget(Vec<String>, DndAction),
    HandlingOffer(Vec<String>, DndAction),
    Dropped,
}

#[derive(Debug, Default, Clone)]
pub struct State {
    dnd_offer: DndOfferState,
}
