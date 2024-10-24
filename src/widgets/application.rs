//! A widget that can be dragged and dropped.

use core::str;
use std::{borrow::Cow, cell::RefCell, iter, path::PathBuf, str::FromStr};

use cosmic::{
    iced::{
        alignment::Vertical,
        clipboard::mime::{AllowedMimeTypes, AsMimeTypes},
        Size, Vector,
    },
    iced_core::alignment::Horizontal,
    widget::dnd_source,
};

use cosmic::iced_core::{
    event, layout, mouse, overlay, renderer, Alignment, Clipboard, Event, Length, Rectangle, Shell,
    Widget,
};

use cosmic::{
    desktop::DesktopEntryData,
    iced::widget::{column, text},
    iced_core::widget::{tree, Operation, Tree},
    theme,
    widget::{button, container},
    Element,
};

use crate::app::AppSource;

pub const MIME_TYPE: &str = "text/uri-list";
const DRAG_THRESHOLD: f32 = 25.0;
/// A widget that can be dragged and dropped.
#[allow(missing_debug_implementations)]
pub struct ApplicationButton<'a, Message> {
    path: PathBuf,

    content: Element<'a, Message>,

    on_right_release: Box<dyn Fn(Rectangle) -> Message + 'a>,

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
        source: Option<&AppSource>,
        selected: bool,
        on_start: Option<Message>,
        on_finish: Option<Message>,
        on_cancel: Option<Message>,
    ) -> Self {
        let cosmic::cosmic_theme::Spacing {
            space_xxs, space_s, ..
        } = theme::active().cosmic().spacing;

        let (source_icon, source_suffix_len) = match source {
            Some(source) => {
                let source_name = source.to_string();
                (
                    source.as_icon().map(|i| {
                        Element::from(
                            container(i)
                                .class(cosmic::theme::Container::Card)
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
        let path_ = path.clone();
        let image_clone = image.clone();
        let content = dnd_source(
            button::custom(
                column![
                    image
                        .as_cosmic_icon()
                        .width(Length::Fixed(72.0))
                        .height(Length::Fixed(72.0)),
                    text(name)
                        .align_x(Horizontal::Center)
                        .size(14)
                        .height(Length::Fixed(40.0))
                ]
                .width(Length::Fixed(120.0))
                .height(Length::Fixed(120.0))
                .spacing(space_xxs)
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .selected(selected)
            .width(Length::FillPortion(1))
            .class(theme::Button::IconVertical)
            .padding(space_s)
            .on_press_maybe(on_pressed.clone()),
        )
        .drag_icon(move || {
            (
                image_clone
                    .as_cosmic_icon()
                    .width(Length::Fixed(72.0))
                    .height(Length::Fixed(72.0))
                    .into(),
                tree::State::None,
            )
        })
        .drag_content(move || AppletString(path_.clone().unwrap()))
        .on_start(on_start)
        .on_cancel(on_cancel)
        .on_finish(on_finish)
        .into();
        Self {
            path: path.clone().unwrap(),
            content,
            on_right_release: Box::new(on_right_release),

            source_icon,
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
        operation: &mut dyn Operation<()>,
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
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, cosmic::Theme, cosmic::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            translation,
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
pub struct AppletString(pub PathBuf);

impl AllowedMimeTypes for AppletString {
    fn allowed() -> std::borrow::Cow<'static, [String]> {
        std::borrow::Cow::Owned(vec![MIME_TYPE.to_string()])
    }
}

impl TryFrom<(Vec<u8>, String)> for AppletString {
    type Error = anyhow::Error;

    fn try_from((value, mime): (Vec<u8>, String)) -> Result<Self, Self::Error> {
        if mime == MIME_TYPE {
            Ok(AppletString(
                url::Url::from_str(str::from_utf8(&value)?)?
                    .to_file_path()
                    .map_err(|_| anyhow::anyhow!("Invalid file path"))?,
            ))
        } else {
            Err(anyhow::anyhow!("Invalid mime"))
        }
    }
}

impl AsMimeTypes for AppletString {
    fn available(&self) -> std::borrow::Cow<'static, [String]> {
        std::borrow::Cow::Owned(vec![MIME_TYPE.to_string()])
    }

    fn as_bytes(&self, mime_type: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
        if mime_type != MIME_TYPE {
            return None;
        }
        Some(Cow::Owned(
            url::Url::from_file_path(self.0.clone())
                .ok()?
                .to_string()
                .into_bytes(),
        ))
    }
}

#[derive(Debug, Default, Clone)]
pub struct State {
    right_press: bool,
}
