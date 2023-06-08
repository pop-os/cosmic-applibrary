//! A widget that can be dragged and dropped.

use std::mem;
use std::path::PathBuf;

use cosmic::iced::wayland::actions::data_device::{DataFromMimeType, DndIcon};
use cosmic::iced_core::alignment::Horizontal;
use cosmic::iced_core::event::{wayland, PlatformSpecific};
use cosmic::iced_runtime::command::platform_specific;
use cosmic::sctk::reexports::client::protocol::wl_data_device_manager::DndAction;

use cosmic::iced_core::{
    event, layout, mouse, overlay, renderer, touch, Alignment, Clipboard, Element, Event, Length,
    Point, Rectangle, Shell, Widget,
};

use cosmic::iced_core::widget::{operation::OperationOutputWrapper, tree, Operation, Tree};
use cosmic::{
    iced::{
        self,
        widget::{column, text},
    },
    theme,
    widget::icon,
};

use crate::app::{DND_ICON_ID, WINDOW_ID};
use crate::app_group::DesktopEntryData;

const MIME_TYPE: &str = "text/uri-list";
const DRAG_THRESHOLD: f32 = 25.0;
/// A widget that can be dragged and dropped.
#[allow(missing_debug_implementations)]
pub struct ApplicationButton<'a, Message> {
    path: PathBuf,

    content: Element<'a, Message, cosmic::Renderer>,

    on_right_release: Box<dyn Fn(Rectangle) -> Message + 'a>,

    on_pressed: Option<Message>,

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
        ignore_button: Message,
        on_right_release: impl Fn(Rectangle) -> Message + 'a,
        on_pressed: Option<Message>,
    ) -> Self {
        let name = if name.len() > 27 {
            format!("{:.24}...", name)
        } else {
            name.to_string()
        };
        let content = iced::widget::button(
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
        .style(theme::Button::Text)
        .padding(16);
        let content = if on_pressed.is_some() {
            content.on_press(ignore_button.clone())
        } else {
            content
        }
        .into();
        Self {
            path: path.clone(),
            content,
            on_right_release: Box::new(on_right_release),
            on_pressed,
            on_create_dnd_source: None,
            on_dnd_command_produced: None,
            on_finish: None,
            on_cancel: None,
        }
    }

    pub fn on_pressed(self, on_pressed: Message) -> Self {
        Self {
            on_pressed: Some(on_pressed),
            ..self
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

impl<'a, Message> From<ApplicationButton<'a, Message>> for Element<'a, Message, cosmic::Renderer>
where
    Message: Clone + 'a,
{
    fn from(dnd_source: ApplicationButton<'a, Message>) -> Element<'a, Message, cosmic::Renderer> {
        Element::new(dnd_source)
    }
}

impl<'a, Message> Widget<Message, cosmic::Renderer> for ApplicationButton<'a, Message>
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
        cursor_position: Point,
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
        operation.container(None, &mut |operation| {
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
        cursor_position: Point,
        renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) -> event::Status {
        let mut ret = self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event.clone(),
            layout.children().next().unwrap(),
            cursor_position,
            renderer,
            clipboard,
            shell,
        );

        let mut state = tree.state.downcast_mut::<State>();

        if layout.bounds().contains(cursor_position) {
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
                            if layout.bounds().contains(cursor_position) =>
                        {
                            ret = event::Status::Captured;
                            DraggingState::Pressed(cursor_position)
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
                            let d_y = cursor_position.y - start.y;
                            let d_x = cursor_position.x - start.x;
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
                                        origin_id: WINDOW_ID,
                                        icon_id: Some(DndIcon::Custom(DND_ICON_ID)),
                                        data: Box::new(AppletString(p.clone())),
                                    }
                                })));
                                ret = event::Status::Captured;
                                DraggingState::Dragging(self.path.clone(), false)
                            } else {
                                DraggingState::Pressed(start)
                            }
                        }
                        event::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                        | event::Event::Touch(
                            touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. },
                        ) => {
                            ret = event::Status::Captured;
                            if let Some(on_pressed) = self.on_pressed.clone() {
                                dbg!("pressed");
                                shell.publish(on_pressed);
                            }
                            DraggingState::None
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
        cursor_position: Point,
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
