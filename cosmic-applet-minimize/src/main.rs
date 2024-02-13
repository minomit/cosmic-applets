mod localize;
pub(crate) mod wayland_handler;
pub(crate) mod wayland_subscription;

use crate::localize::localize;
use cosmic::app::Command;
use cosmic::applet::cosmic_panel_config::PanelAnchor;
use cosmic::cctk::cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1;
use cosmic::cctk::sctk::reexports::calloop;
use cosmic::cctk::toplevel_info::ToplevelInfo;
use cosmic::desktop::DesktopEntryData;
use cosmic::iced::{widget::text, Length, Subscription};
use cosmic::iced_style::application;
use cosmic::iced_widget::{Column, Row};
use cosmic::theme::Button;
use cosmic::widget::tooltip;
use cosmic::{Element, Theme};
use wayland_subscription::{ToplevelRequest, ToplevelUpdate, WaylandRequest, WaylandUpdate};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    // Prepare i18n
    localize();

    tracing::info!("Starting minimize applet with version {VERSION}");

    cosmic::applet::run::<Minimize>(true, ())
}

#[derive(Default)]
struct Minimize {
    core: cosmic::app::Core,
    apps: Vec<(ZcosmicToplevelHandleV1, ToplevelInfo, DesktopEntryData)>,
    tx: Option<calloop::channel::Sender<WaylandRequest>>,
}

#[derive(Debug, Clone)]
enum Message {
    Wayland(WaylandUpdate),
    Activate(ZcosmicToplevelHandleV1),
}

impl cosmic::Application for Minimize {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = "com.system76.CosmicAppletMinimize";

    fn init(core: cosmic::app::Core, _flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                core,
                ..Default::default()
            },
            Command::none(),
        )
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<<Theme as application::StyleSheet>::Style> {
        Some(cosmic::applet::style())
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Wayland(update) => match update {
                WaylandUpdate::Init(tx) => {
                    self.tx = Some(tx);
                }
                WaylandUpdate::Finished => {
                    panic!("Wayland Subscription ended...")
                }
                WaylandUpdate::Toplevel(t) => match t {
                    ToplevelUpdate::Add(handle, info) | ToplevelUpdate::Update(handle, info) => {
                        let data = |id| {
                            cosmic::desktop::load_applications_for_app_ids(
                                None,
                                std::iter::once(id),
                                true,
                            )
                            .remove(0)
                        };
                        if let Some(pos) = self.apps.iter_mut().position(|a| a.0 == handle) {
                            if self.apps[pos].1.app_id != info.app_id {
                                self.apps[pos].2 = data(&info.app_id)
                            }
                            self.apps[pos].1 = info;
                        } else {
                            let data = data(&info.app_id);
                            self.apps.push((handle, info, data));
                        }
                    }
                    ToplevelUpdate::Remove(handle) => self.apps.retain(|a| a.0 != handle),
                },
            },
            Message::Activate(handle) => {
                if let Some(tx) = self.tx.as_ref() {
                    let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Activate(handle)));
                }
            }
        };
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        wayland_subscription::wayland_subscription().map(|e| Message::Wayland(e))
    }

    fn view(&self) -> Element<Message> {
        let (width, height) = self.core.applet.suggested_size();
        let theme = self.core.system_theme().cosmic();
        let space_xxs = theme.space_xxs();
        let icon_buttons = self.apps.iter().map(|(handle, _, data)| {
            tooltip(
                cosmic::widget::button::button(
                    data.icon
                        .as_cosmic_icon()
                        .width(Length::Fixed(width as f32))
                        .height(Length::Fixed(height as f32)),
                )
                .style(Button::AppletIcon)
                .padding(space_xxs)
                .width(Length::Shrink)
                .height(Length::Shrink)
                .on_press(Message::Activate(handle.clone())),
                data.name.clone(),
                // tooltip::Position::FollowCursor,
                // FIXME tooltip fails to appear when created as indicated in design
                match self.core.applet.anchor {
                    PanelAnchor::Left => tooltip::Position::Right,
                    PanelAnchor::Right => tooltip::Position::Left,
                    PanelAnchor::Top => tooltip::Position::Bottom,
                    PanelAnchor::Bottom => tooltip::Position::Top,
                },
            )
            .snap_within_viewport(false)
            .text_shaping(text::Shaping::Advanced)
            .into()
        });

        // TODO optional dividers on ends if detects app list neighbor

        if matches!(
            self.core.applet.anchor,
            PanelAnchor::Top | PanelAnchor::Bottom
        ) {
            Row::with_children(icon_buttons)
                .align_items(cosmic::iced_core::Alignment::Center)
                .height(Length::Shrink)
                .width(Length::Shrink)
                .spacing(space_xxs)
                .padding([0, space_xxs])
                .into()
        } else {
            Column::with_children(icon_buttons)
                .align_items(cosmic::iced_core::Alignment::Center)
                .height(Length::Shrink)
                .width(Length::Shrink)
                .spacing(space_xxs)
                .padding([space_xxs, 0])
                .into()
        }
    }
}
