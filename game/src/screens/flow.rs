use mirage_engine::egui;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use neumannarch_protocol::{Lobby, LobbyEdit, Notice, PlayerId, Request, Started};

use crate::controls::Button;
use crate::display::glyph_quad::GlyphQuad;
use crate::net::connection::Connection;
use crate::net::listener::{Listener, NoListener};
use crate::net::local::Local;
use crate::net::transport::Transport;
use crate::screens::Playable;
use crate::screens::control::{HOST_ONLY, Rule};
use crate::screens::field::Typed;
use crate::screens::loading::Loading;
use crate::screens::lobby::{self, LobbyScreen};
use crate::screens::panel::{self, Panel};
use crate::screens::play::{self, Play};
use crate::screens::results::{self, Results};
use crate::screens::title::{self, Outcome, Title};

pub enum Connect {
    Host,
    Join(String),
}

pub struct Flow {
    stage: Screen,
    title: Title,
}

pub enum Screen {
    Title {
        listener: Result<Listener, NoListener>,
        join: Join,
    },
    Lobby {
        screen: Box<LobbyScreen>,
        room: Room,
    },
    Loading {
        loading: Box<Loading>,
        room: Room,
    },
    Play {
        play: Box<Play>,
        room: Room,
    },
    Results {
        results: Box<Results>,
        room: Room,
    },
}

pub enum Join {
    Idle,
    Connecting {
        room: Connection,
        hosted: Option<Listener>,
    },
    Failed(Outcome),
}

pub enum Room {
    Local(Local),
    Guest {
        room: Connection,
        me: PlayerId,
    },
    Host {
        room: Connection,
        listener: Listener,
    },
}

impl Flow {
    pub fn opening() -> Flow {
        Flow {
            stage: Screen::Title {
                listener: Listener::opened(),
                join: Join::Idle,
            },
            title: Title::opening(),
        }
    }

    pub fn stage(&self) -> &Screen {
        &self.stage
    }

    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>)
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let stage = core::mem::replace(&mut self.stage, Screen::placeholder());
        self.stage = stage.updated().framed(ctx, &mut self.title);
    }

    pub fn tick(&mut self) {
        self.stage.tick();
    }
}

impl Screen {
    fn framed<G: Playable>(self, ctx: &mut FrameCtx<'_, G>, title: &mut Title) -> Screen
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        match self {
            Screen::Title { listener, join } => match title_frame(ctx, title, &listener, &join) {
                Some(title::Action::Skirmish) => Screen::opens(
                    Lobby::skirmish(PlayerId::HOST),
                    PlayerId::HOST,
                    Room::Local(Local),
                ),
                Some(title::Action::Host) => Screen::opening(listener, join, Connect::Host),
                Some(title::Action::Join(address)) => {
                    Screen::opening(listener, join, Connect::Join(address))
                }
                None => Screen::Title { listener, join },
            },
            Screen::Lobby {
                mut screen,
                mut room,
            } => {
                let asked = screen.frame(ctx);
                for edit in asked.edits {
                    if let Some(edited) = room.edits(screen.lobby(), edit) {
                        screen.takes(edited);
                    }
                }
                match asked.picked {
                    Some(lobby::Picked::Leave) => Screen::leaves(room, None),
                    Some(lobby::Picked::Start) => match room.starts(screen.lobby()) {
                        Some(started) => Screen::starts(screen.lobby(), started, room),
                        None => Screen::Lobby { screen, room },
                    },
                    None => Screen::Lobby { screen, room },
                }
            }
            Screen::Loading { mut loading, room } => {
                loading.frame(ctx);
                match loading.agreed() {
                    true => Screen::plays(*loading, room),
                    false => Screen::Loading { loading, room },
                }
            }
            Screen::Play { mut play, room } => match play.frame(ctx) {
                Some(play::Picked::Leave) => Screen::leaves(room, None),
                None => match play.over() {
                    true => Screen::ends(&play, room),
                    false => Screen::Play { play, room },
                },
            },
            Screen::Results { mut results, room } => {
                let rematch = Rule::only_if(room.hosts(results.lobby()), HOST_ONLY);
                match results.frame(ctx, &rematch) {
                    Some(results::Picked::Leave) => Screen::leaves(room, None),
                    Some(results::Picked::Rematch) => Screen::rematches(results, room),
                    None => Screen::Results { results, room },
                }
            }
        }
    }

    fn updated(self) -> Screen {
        match self {
            Screen::Title { listener, join } => after_welcome(listener, join),
            Screen::Lobby {
                mut screen,
                mut room,
            } => {
                if room.closed() {
                    return Screen::leaves(room, Some(Outcome::Closed));
                }
                for notice in room.notices() {
                    match notice {
                        Notice::Lobby(sent) => screen.takes(sent),
                        Notice::Started(started) => {
                            return Screen::starts(screen.lobby(), started, room);
                        }
                        Notice::Removed => return Screen::removed(room),
                        Notice::Left => return Screen::leaves(room, Some(Outcome::HostLeft)),
                        Notice::Refused(_)
                        | Notice::NotReady(_)
                        | Notice::Full
                        | Notice::Version
                        | Notice::Welcome { .. } => {}
                    }
                }
                Screen::Lobby { screen, room }
            }
            Screen::Results { results, mut room } => {
                if room.closed() {
                    return Screen::leaves(room, Some(Outcome::Closed));
                }
                for notice in room.notices() {
                    match notice {
                        Notice::Lobby(sent) => {
                            let me = room.me();
                            return Screen::opens(sent, me, room);
                        }
                        Notice::Removed => return Screen::removed(room),
                        Notice::Left => return Screen::leaves(room, Some(Outcome::HostLeft)),
                        Notice::Refused(_)
                        | Notice::NotReady(_)
                        | Notice::Full
                        | Notice::Version
                        | Notice::Welcome { .. }
                        | Notice::Started(_) => {}
                    }
                }
                Screen::Results { results, room }
            }
            Screen::Loading { .. } | Screen::Play { .. } => self,
        }
    }

    fn tick(&mut self) {
        match self {
            Screen::Loading { loading, room } => loading.tick(room.transport()),
            Screen::Play { play, room } => play.tick(room.transport()),
            Screen::Title { .. } | Screen::Lobby { .. } | Screen::Results { .. } => {}
        }
    }

    fn opens(lobby: Lobby, me: PlayerId, room: Room) -> Screen {
        Screen::Lobby {
            screen: Box::new(LobbyScreen::of(lobby, me)),
            room,
        }
    }

    fn starts(lobby: &Lobby, started: Started, mut room: Room) -> Screen {
        let Some(crew) = started.seating().run_by(room.me()) else {
            return Screen::leaves(room, Some(Outcome::Closed));
        };
        let loading = Loading::of(lobby.clone(), started, &crew, room.transport());
        Screen::Loading {
            loading: Box::new(loading),
            room,
        }
    }

    fn plays(loading: Loading, room: Room) -> Screen {
        Screen::Play {
            play: Box::new(loading.plays()),
            room,
        }
    }

    fn ends(play: &Play, room: Room) -> Screen {
        Screen::Results {
            results: Box::new(play.ends()),
            room,
        }
    }

    fn rematches(results: Box<Results>, mut room: Room) -> Screen {
        match room.rematches() {
            true => Screen::Results { results, room },
            false => {
                let me = room.me();
                Screen::opens(results.lobby().clone(), me, room)
            }
        }
    }

    fn leaves(mut room: Room, outcome: Option<Outcome>) -> Screen {
        room.leaving();

        drop(room);
        Screen::Title {
            listener: Listener::opened(),
            join: outcome.map_or(Join::Idle, Join::Failed),
        }
    }

    fn removed(room: Room) -> Screen {
        Screen::leaves(room, Some(Outcome::Removed))
    }

    fn opening(listener: Result<Listener, NoListener>, join: Join, connect: Connect) -> Screen {
        match connect {
            Connect::Host => match listener {
                Ok(hosted) => Screen::Title {
                    listener: Err(NoListener::PortHeld),
                    join: Join::Connecting {
                        room: Connection::joining(&hosted.address()),
                        hosted: Some(hosted),
                    },
                },
                Err(why) => Screen::Title {
                    listener: Err(why),
                    join,
                },
            },
            Connect::Join(address) => Screen::Title {
                listener,
                join: Join::Connecting {
                    room: Connection::joining(&address),
                    hosted: None,
                },
            },
        }
    }

    fn placeholder() -> Screen {
        Screen::Title {
            listener: Err(NoListener::PortHeld),
            join: Join::Idle,
        }
    }
}

impl Join {
    fn phrase(&self) -> Option<&'static str> {
        match self {
            Join::Idle => None,
            Join::Connecting { .. } => Some("Connecting"),
            Join::Failed(outcome) => Some(outcome.phrase()),
        }
    }
}

impl Room {
    pub fn me(&self) -> PlayerId {
        match self {
            Room::Local(_) | Room::Host { .. } => PlayerId::HOST,
            Room::Guest { me, .. } => *me,
        }
    }

    pub fn transport(&mut self) -> &mut dyn Transport {
        match self {
            Room::Local(local) => local,
            Room::Guest { room, .. } | Room::Host { room, .. } => room,
        }
    }

    fn hosts(&self, lobby: &Lobby) -> bool {
        match self {
            Room::Local(_) => true,
            Room::Guest { .. } | Room::Host { .. } => lobby.host() == self.me(),
        }
    }

    fn edits(&mut self, lobby: &Lobby, edit: LobbyEdit) -> Option<Lobby> {
        let me = self.me();
        match self {
            Room::Local(_) => {
                let mut own = lobby.clone();
                own.edit(me, edit)
                    .expect("a control is offered only where its own rule allows the edit");
                Some(own)
            }
            Room::Guest { room, .. } | Room::Host { room, .. } => {
                room.request(Request::Edit(edit));
                None
            }
        }
    }

    fn starts(&mut self, lobby: &Lobby) -> Option<Started> {
        let started = lobby.freeze().ok()?;
        match self {
            Room::Local(_) => Some(started),
            Room::Guest { room, .. } | Room::Host { room, .. } => {
                room.request(Request::Start);
                None
            }
        }
    }

    fn rematches(&mut self) -> bool {
        match self {
            Room::Local(_) => false,
            Room::Guest { room, .. } | Room::Host { room, .. } => {
                room.request(Request::Rematch);
                true
            }
        }
    }

    fn notices(&mut self) -> Vec<Notice> {
        match self {
            Room::Local(_) => Vec::new(),
            Room::Guest { room, .. } | Room::Host { room, .. } => room.notices(),
        }
    }

    fn closed(&self) -> bool {
        match self {
            Room::Local(_) => false,
            Room::Guest { room, .. } | Room::Host { room, .. } => room.closed(),
        }
    }

    fn leaving(&mut self) {
        match self {
            Room::Local(_) => {}
            Room::Guest { room, .. } | Room::Host { room, .. } => room.leaving(),
        }
    }
}

fn title_frame<G: Playable>(
    ctx: &mut FrameCtx<'_, G>,
    title: &mut Title,
    listener: &Result<Listener, NoListener>,
    join: &Join,
) -> Option<title::Action>
where
    G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
{
    let hosts = Rule::unless(listener.as_ref().err().map(|why| why.reason().to_string()));
    let outcome = join.phrase();
    let pixel = ctx.pointer();
    let size = ctx.window_size();
    let clicked = ctx.pressed(Button::Select);
    let mut picked = None;
    ctx.ui(|ui| {
        let points = 1.0 / ui.ctx().pixels_per_point();
        let panel = Panel::new(
            ui.painter(),
            panel::window_of(size, points),
            egui::pos2(pixel.x * points, pixel.y * points),
            clicked,
        );
        picked = title.frame(&panel, &Typed::this_frame(ui.ctx()), &hosts, outcome);
    });
    picked
}

fn after_welcome(listener: Result<Listener, NoListener>, join: Join) -> Screen {
    let Join::Connecting { mut room, hosted } = join else {
        return Screen::Title { listener, join };
    };
    if room.closed() {
        return Screen::Title {
            listener: kept_listener(listener, hosted),
            join: Join::Failed(Outcome::NoRoom),
        };
    }
    let mut outcome = None;
    for notice in room.notices() {
        match notice {
            Notice::Welcome { player, lobby } => {
                let room = match hosted {
                    Some(listener) => Room::Host { room, listener },
                    None => Room::Guest { room, me: player },
                };
                return Screen::opens(lobby, player, room);
            }
            Notice::Full => outcome = Some(Outcome::Full),
            Notice::Version => outcome = Some(Outcome::Version),
            Notice::Left | Notice::Removed => outcome = Some(Outcome::NoRoom),
            Notice::Refused(_) | Notice::NotReady(_) | Notice::Lobby(_) | Notice::Started(_) => {}
        }
    }
    match outcome {
        Some(outcome) => {
            drop(room);
            Screen::Title {
                listener: kept_listener(listener, hosted),
                join: Join::Failed(outcome),
            }
        }
        None => Screen::Title {
            listener,
            join: Join::Connecting { room, hosted },
        },
    }
}

fn kept_listener(
    listener: Result<Listener, NoListener>,
    hosted: Option<Listener>,
) -> Result<Listener, NoListener> {
    match hosted {
        Some(hosted) => Ok(hosted),
        None => listener,
    }
}

#[cfg(all(test, feature = "host", not(target_arch = "wasm32")))]
mod tests {
    use std::net::SocketAddr;

    use super::*;

    const PATIENCE: usize = 100_000;

    #[test]
    fn a_failed_host_join_leaves_host_enabled() {
        let hosted = Listener::serving(SocketAddr::from(([127, 0, 0, 1], 0)))
            .expect("a room binds on the loopback");
        let address = hosted.address();
        let mut stage = Screen::Title {
            listener: Err(NoListener::PortHeld),
            join: Join::Connecting {
                room: Connection::joining("127.0.0.1:1"),
                hosted: Some(hosted),
            },
        };
        for _ in 0..PATIENCE {
            stage = stage.updated();
            if matches!(
                &stage,
                Screen::Title {
                    join: Join::Failed(_),
                    ..
                }
            ) {
                break;
            }
            std::thread::yield_now();
        }
        let Screen::Title {
            listener,
            join: Join::Failed(_),
        } = stage
        else {
            panic!("the join never resolved");
        };

        assert_eq!(listener.map(|hosted| hosted.address()), Ok(address));
    }
}
