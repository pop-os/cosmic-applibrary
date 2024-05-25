//! A widget that can be dragged and dropped.

use std::cell::RefCell;
use std::path::PathBuf;
use std::{iter, mem};

use cosmic::cctk::sctk::reexports::client::protocol::wl_data_device_manager::DndAction;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::wayland::actions::data_device::{DataFromMimeType, DndIcon};
use cosmic::iced::{Size, Vector};
use cosmic::iced_core::alignment::Horizontal;
use cosmic::iced_core::event::{wayland, PlatformSpecific};
use cosmic::iced_runtime::command::platform_specific;

use cosmic::iced_core::{
    event, layout, mouse, overlay, renderer, touch, Alignment, Clipboard, Event, Length, Point,
    Rectangle, Shell, Widget,
};

use cosmic::desktop::DesktopEntryData;
use cosmic::iced_core::widget::{operation::OperationOutputWrapper, tree, Operation, Tree};
use cosmic::widget::container;
use cosmic::Element;
use cosmic::{
    iced::widget::{column, text},
    theme,
    widget::button,
};

use crate::app::{AppSource, DND_ICON_ID, WINDOW_ID};

pub const MIME_TYPE: &str = "text/uri-list";
const DRAG_THRESHOLD: f32 = 25.0;
/// A widget that can be dragged and dropped.
#[allow(missing_debug_implementations)]
pub struct ApplicationButton<'a, Message> {
    path: PathBuf,

    content: Element<'a, Message>,

    on_right_release: Box<dyn Fn(Rectangle) -> Message + 'a>,

    on_create_dnd_source: Option<Message>,

    on_finish: Option<Box<dyn Fn(bool) -> Message + 'a>>,

    on_cancel: Option<Message>,

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

    // Optional icon, and text
    source_icon: Option<Element<'a, Message>>,
}

impl<'a, Message: Clone + 'static> ApplicationButton<'a, Message> {
    /// Creates a new [`ApplicationButton`].
    #[must_use]
    pub fn new(
        DesktopEntryData {
            name,
            icon: image,
            path,
            ..
        }: &'a DesktopEntryData,
        on_right_release: impl Fn(Rectangle) -> Message + 'a,
        on_pressed: Option<Message>,
        spacing: &Spacing,
        source: Option<&AppSource>,
    ) -> Self {
        let (source_icon, source_suffix_len) = match source {
            Some(source) => {
                let source_name = source.to_string();
                (
                    source.as_icon().map(|i| {
                        Element::from(
                            container(i)
                                .style(cosmic::theme::Container::Card)
                                .width(Length::Fixed(24.0))
                                .height(Length::Fixed(24.0))
                                .align_x(Horizontal::Center)
                                .align_y(Vertical::Center),
                        )
                    }),
                    source_name.len().saturating_add(3), // 3 for the parentheses
                )
            }
            None => (None, 0),
        };
        let max_name_len = 27 - source_suffix_len;
        let name = if name.len() > max_name_len {
            if let Some(source) = source {
                format!("{name:.17}... ({source})")
            } else {
                format!("{name:.24}...")
            }
        } else {
            if let Some(source) = source {
                format!("{name} ({source})")
            } else {
                name.to_string()
            }
        };
        let content = button(
            column![
                image
                    .as_cosmic_icon()
                    .width(Length::Fixed(72.0))
                    .height(Length::Fixed(72.0)),
                text(name)
                    .horizontal_alignment(Horizontal::Center)
                    .size(14)
                    .height(Length::Fixed(40.0))
            ]
            .width(Length::Fixed(120.0))
            .height(Length::Fixed(120.0))
            .spacing(spacing.space_xxs)
            .align_items(Alignment::Center)
            .width(Length::Fill),
        )
        .width(Length::FillPortion(1))
        .style(theme::Button::IconVertical)
        .padding(spacing.space_s);
        let content = if on_pressed.is_some() {
            content.on_press_maybe(on_pressed.clone())
        } else {
            content
        }
        .into();
        Self {
            path: path.clone().unwrap(),
            content,
            on_right_release: Box::new(on_right_release),
            on_create_dnd_source: None,
            on_dnd_command_produced: None,
            on_finish: None,
            on_cancel: None,
            source_icon,
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

    pub fn on_finish(self, message: impl Fn(bool) -> Message + 'a) -> Self {
        Self {
            on_finish: Some(Box::new(message)),
            ..self
        }
    }

    pub fn on_cancel(self, message: Message) -> Self {
        Self {
            on_cancel: Some(message),
            ..self
        }
    }

    pub fn on_create_dnd_source(self, message: Message) -> Self {
        Self {
            on_create_dnd_source: Some(message),
            ..self
        }
    }
}

impl<'a, Message> From<ApplicationButton<'a, Message>> for Element<'a, Message>
where
    Message: Clone + 'a,
{
    fn from(dnd_source: ApplicationButton<'a, Message>) -> Element<'a, Message> {
        Element::new(dnd_source)
    }
}

impl<'a, Message> Widget<Message, cosmic::Theme, cosmic::Renderer>
    for ApplicationButton<'a, Message>
where
    Message: Clone,
{
    fn children(&self) -> Vec<Tree> {
        iter::once(Tree::new(&self.content))
            .chain(self.source_icon.as_ref().map(|i| Tree::new(i)))
            .collect()
    }

    fn diff(&mut self, tree: &mut Tree) {
        let mut children: Vec<_> = iter::once(&mut self.content)
            .chain(self.source_icon.as_mut())
            .collect();
        tree.diff_children(children.as_mut_slice());
    }

    fn size(&self) -> cosmic::iced_core::Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &cosmic::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = self.size();
        let tree = RefCell::new(tree);
        layout(
            renderer,
            limits,
            size.width,
            size.height,
            u32::MAX,
            u32::MAX,
            |renderer, limits| {
                let content_state = &mut tree.borrow_mut().children[0];
                self.content
                    .as_widget()
                    .layout(content_state, renderer, limits)
            },
            self.source_icon.as_ref(),
            |renderer, limits, icon| {
                let icon_state = &mut tree.borrow_mut().children[1];
                icon.as_widget().layout(icon_state, renderer, limits)
            },
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

        if let Some(icon) = self.source_icon.as_ref() {
            icon.as_widget().draw(
                &tree.children[1],
                renderer,
                theme,
                renderer_style,
                layout.children().nth(1).unwrap(),
                cursor_position,
                viewport,
            );
        }
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
    ) -> Option<overlay::Element<'b, Message, cosmic::Theme, cosmic::Renderer>> {
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
        let mut ret = self.content.as_widget_mut().on_event(
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

        if cursor_position.is_over(layout.bounds()) {
            match &event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                    state.right_press = true;
                    return event::Status::Captured;
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                    if state.right_press {
                        shell.publish(self.on_right_release.as_ref()(layout.bounds()));
                        state.right_press = false;
                        return event::Status::Captured;
                    }
                }
                _ => {}
            }
        }

        if let (
            Some(on_dnd_command_produced),
            Some(on_create_dnd_source),
            Some(on_cancel),
            Some(on_finish),
        ) = (
            self.on_dnd_command_produced.as_ref(),
            self.on_create_dnd_source.as_ref(),
            self.on_cancel.as_ref(),
            self.on_finish.as_ref(),
        ) {
            state.dragging_state = match mem::take(&mut state.dragging_state) {
                DraggingState::None => {
                    // if no dragging state, listen for press events
                    match &event {
                        event::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                        | event::Event::Touch(touch::Event::FingerPressed { .. })
                            if cursor_position.is_over(layout.bounds()) =>
                        {
                            ret = event::Status::Captured;
                            DraggingState::Pressed(cursor_position.position().unwrap_or_default())
                        }
                        _ => DraggingState::None,
                    }
                }
                DraggingState::Dragging(applet, copy) => match &event {
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DataSource(wayland::DataSourceEvent::DndFinished),
                    )) => {
                        ret = event::Status::Captured;
                        shell.publish(on_finish.as_ref()(copy));
                        DraggingState::None
                    }
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DataSource(wayland::DataSourceEvent::Cancelled),
                    )) => {
                        ret = event::Status::Captured;
                        shell.publish(on_cancel.clone());

                        DraggingState::None
                    }
                    event::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::DataSource(wayland::DataSourceEvent::DndActionAccepted(a)),
                    )) => {
                        ret = event::Status::Captured;
                        DraggingState::Dragging(applet, a.contains(DndAction::Copy))
                    }
                    _ => DraggingState::Dragging(applet, copy),
                },
                DraggingState::Pressed(start) => {
                    // if dragging state is pressed, listen for motion events or release events
                    match &event {
                        event::Event::Mouse(mouse::Event::CursorMoved { .. })
                        | event::Event::Touch(touch::Event::FingerMoved { .. }) => {
                            let pos = cursor_position.position().unwrap_or_default();
                            let d_y = pos.y - start.y;
                            let d_x = pos.x - start.x;
                            let distance_squared = d_y * d_y + d_x * d_x;

                            if distance_squared > DRAG_THRESHOLD {
                                state.dragging_state =
                                    DraggingState::Dragging(self.path.clone(), false);

                                // TODO emit a dnd command
                                shell.publish(on_create_dnd_source.clone());

                                let p = self.path.to_path_buf();
                                shell.publish((on_dnd_command_produced)(Box::new(move || {
                                    platform_specific::wayland::data_device::ActionInner::StartDnd {
                                        mime_types: vec![MIME_TYPE.to_string()],
                                        actions: DndAction::Copy.union(DndAction::Move),
                                        origin_id: WINDOW_ID.clone(),
                                        icon_id: Some((
                                            DndIcon::Custom(DND_ICON_ID.clone()),
                                            cosmic::iced::Vector::default(),
                                        )),
                                        data: Box::new(AppletString(p.clone())),
                                    }
                                })));
                                ret = event::Status::Captured;
                                DraggingState::Dragging(self.path.clone(), false)
                            } else {
                                DraggingState::Pressed(start)
                            }
                        }
                        _ => DraggingState::Pressed(start),
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
pub fn layout<'a, Renderer, M>(
    renderer: &Renderer,
    limits: &layout::Limits,
    width: Length,
    height: Length,
    max_height: u32,
    max_width: u32,
    layout_content: impl FnOnce(&Renderer, &layout::Limits) -> layout::Node,
    icon: Option<&Element<'a, M>>,
    layout_icon: impl FnOnce(&Renderer, &layout::Limits, &Element<'a, M>) -> layout::Node,
) -> layout::Node {
    let limits = limits
        .loose()
        .max_height(max_height as f32)
        .max_width(max_width as f32)
        .width(width)
        .height(height);

    let content = layout_content(renderer, &limits);
    let size = limits.resolve(width, height, content.size());
    let mut children = vec![content];
    let app_icon_node = &children[0].children()[0].children()[0];
    if let Some(icon) = icon {
        let app_icon_size = app_icon_node.size();
        let mut icon_node = layout_icon(
            renderer,
            &layout::Limits::new(Size::new(24., 24.), Size::new(24., 24.)),
            icon,
        );
        icon_node = icon_node.move_to(app_icon_node.bounds().position());
        // translate to the bottom right corner
        icon_node = icon_node.translate(Vector::new(app_icon_size.width, app_icon_size.height));

        children.push(icon_node);
    }

    layout::Node::with_children(size, children)
}

/// A string which can be sent to the clipboard or drag-and-dropped.
#[derive(Debug, Clone)]
pub struct AppletString(PathBuf);

impl DataFromMimeType for AppletString {
    fn from_mime_type(&self, mime_type: &str) -> Option<Vec<u8>> {
        if mime_type == MIME_TYPE {
            let data = Some(
                url::Url::from_file_path(self.0.clone())
                    .ok()?
                    .to_string()
                    .as_bytes()
                    .to_vec(),
            );
            data
        } else {
            None
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum DraggingState {
    #[default]
    /// No ongoing drag or press
    None,
    /// A draggable item was being pressed at the recorded point
    Pressed(Point),
    /// An item is being dragged
    Dragging(PathBuf, bool),
}

#[derive(Debug, Default, Clone)]
pub struct State {
    dragging_state: DraggingState,
    right_press: bool,
}
