//! The flow of screens as one state machine: which screen is on, who is
//! the authority on its lobby, and every transition between them.

use mirage_engine::egui;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_protocol::{Lobby, LobbyEdit, Message, PlayerId, Started};

use crate::controls::Button;
use crate::display::glyph_quad::GlyphQuad;
use crate::net::hosting::{Hosting, NoRoom};
use crate::net::local::Local;
use crate::net::room::{Room, Word};
use crate::net::transport::Transport;
use crate::screens::Playable;
use crate::screens::control::Rule;
use crate::screens::field::Typed;
use crate::screens::loading::Loading;
use crate::screens::lobby::{self, LobbyScreen};
use crate::screens::panel::{self, Panel};
use crate::screens::play::{self, Play};
use crate::screens::results::{self, Results};
use crate::screens::title::{self, Joining, Title};

/// What the title asks the flow for: each one opens a socket, which no
/// screen does itself.
pub enum Ask {
    /// Serve a room in this process and join it at the loopback.
    Host,
    /// Join the room at this address, as `host:port`.
    Join(String),
}

/// The one value that owns which screen is live.
pub struct Flow {
    stage: Stage,
    /// The title's own screen, which outlives every stage: DISPLAY.md keeps
    /// the address it holds for as long as the game runs.
    title: Title,
}

/// The screen the game is showing, with the room behind it.
pub enum Stage {
    /// The title: the room it serves for Host, and the join it is waiting
    /// on or the last one's outcome.
    Title {
        listener: Result<Hosting, NoRoom>,
        asking: Asking,
    },
    /// A match being set up.
    Lobby {
        screen: Box<LobbyScreen>,
        room: Authority,
    },
    /// A match being built, until every machine agrees its first hash.
    Loading {
        loading: Box<Loading>,
        room: Authority,
    },
    /// A match being played.
    Play { play: Box<Play>, room: Authority },
    /// The score of a match that has reached its clock.
    Results {
        results: Box<Results>,
        room: Authority,
    },
}

/// What the title is doing about a room.
pub enum Asking {
    /// No join has been tried.
    Idle,
    /// A join waiting on the room's welcome: the socket, and the room this
    /// machine serves where Host opened it.
    Connecting { room: Room, hosted: Option<Hosting> },
    /// What the join field says about the last join, or about the room this
    /// machine was in and is no longer.
    Said(Joining),
}

/// Who is the authority on the lobby a match is set up in, and what carries
/// that match between machines.
pub enum Authority {
    /// Every seat is on this machine, so its own lobby is the authority and
    /// nothing goes on the wire.
    Local(Local),
    /// A room another machine serves.
    Guest { room: Room, me: PlayerId },
    /// A room this machine serves and plays in.
    Host { room: Room, listener: Hosting },
}

impl Flow {
    /// The game as it opens: the title, serving the room Host would join.
    pub fn opening() -> Flow {
        Flow {
            stage: Stage::Title {
                listener: Hosting::opened(),
                asking: Asking::Idle,
            },
            title: Title::opening(),
        }
    }

    /// The screen on, which the headless drive reads.
    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    /// One frame of the screen on, and the screen the next frame shows.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>)
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        // A stage answers the next one out of its own parts, so it is taken
        // to be replaced; nothing reads the flow between these two lines.
        let stage = core::mem::replace(&mut self.stage, Stage::vacant());
        self.stage = stage.heard().framed(ctx, &mut self.title);
    }

    /// One sim tick of the screen on.
    pub fn tick(&mut self) {
        self.stage.tick();
    }
}

impl Stage {
    /// One frame of this screen, and the screen it hands over to.
    fn framed<G: Playable>(self, ctx: &mut FrameCtx<'_, G>, title: &mut Title) -> Stage
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        match self {
            Stage::Title { listener, asking } => match titled(ctx, title, &listener, &asking) {
                Some(title::Picked::Skirmish) => Stage::opens(
                    Lobby::skirmish(PlayerId::HOST),
                    PlayerId::HOST,
                    Authority::Local(Local),
                ),
                Some(title::Picked::Host) => Stage::asked(listener, asking, Ask::Host),
                Some(title::Picked::Join(address)) => {
                    Stage::asked(listener, asking, Ask::Join(address))
                }
                None => Stage::Title { listener, asking },
            },
            Stage::Lobby {
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
                    Some(lobby::Picked::Leave) => Stage::leaves(room, None),
                    Some(lobby::Picked::Start) => match room.starts(screen.lobby()) {
                        Some(started) => Stage::starts(screen.lobby(), started, room),
                        None => Stage::Lobby { screen, room },
                    },
                    None => Stage::Lobby { screen, room },
                }
            }
            Stage::Loading { mut loading, room } => {
                loading.frame(ctx);
                match loading.agreed() {
                    true => Stage::plays(*loading, room),
                    false => Stage::Loading { loading, room },
                }
            }
            Stage::Play { mut play, room } => match play.frame(ctx) {
                Some(play::Picked::Leave) => Stage::leaves(room, None),
                None => match play.over() {
                    true => Stage::ends(&play, room),
                    false => Stage::Play { play, room },
                },
            },
            Stage::Results { mut results, room } => {
                let rematch = Rule::only_if(
                    room.hosts(results.lobby()),
                    "Only the host starts a rematch",
                );
                match results.frame(ctx, &rematch) {
                    Some(results::Picked::Leave) => Stage::leaves(room, None),
                    Some(results::Picked::Rematch) => Stage::rematches(results, room),
                    None => Stage::Results { results, room },
                }
            }
        }
    }

    /// Takes in what the room has said, which is the authority on the
    /// lobby, the start of every match and the lobby a rematch opens.
    ///
    /// A match reads its own socket through its machine, so neither the
    /// loading screen nor the match takes anything here.
    fn heard(self) -> Stage {
        match self {
            Stage::Title { listener, asking } => welcomed(listener, asking),
            Stage::Lobby {
                mut screen,
                mut room,
            } => {
                if room.closed() {
                    return Stage::leaves(room, Some(Joining::Closed));
                }
                for word in room.heard() {
                    match word {
                        Word::Lobby(sent) => screen.takes(sent),
                        Word::Started(started) => {
                            return Stage::starts(screen.lobby(), started, room);
                        }
                        Word::Removed => return Stage::removed(room),
                        Word::Left => return Stage::leaves(room, Some(Joining::HostLeft)),
                        // The lobby the room broadcasts is the truth about
                        // what took, and a machine is welcomed once.
                        Word::Refused(_) | Word::Welcome { .. } => {}
                    }
                }
                Stage::Lobby { screen, room }
            }
            Stage::Results { results, mut room } => {
                if room.closed() {
                    return Stage::leaves(room, Some(Joining::Closed));
                }
                for word in room.heard() {
                    match word {
                        // A lobby sent to a machine at the results is the
                        // rematch the room has opened.
                        Word::Lobby(sent) => {
                            let me = room.me();
                            return Stage::opens(sent, me, room);
                        }
                        Word::Removed => return Stage::removed(room),
                        Word::Left => return Stage::leaves(room, Some(Joining::HostLeft)),
                        Word::Refused(_) | Word::Welcome { .. } | Word::Started(_) => {}
                    }
                }
                Stage::Results { results, room }
            }
            Stage::Loading { .. } | Stage::Play { .. } => self,
        }
    }

    /// One sim tick of the match, where a match is on.
    fn tick(&mut self) {
        match self {
            Stage::Loading { loading, room } => loading.tick(room.transport()),
            Stage::Play { play, room } => play.tick(room.transport()),
            Stage::Title { .. } | Stage::Lobby { .. } | Stage::Results { .. } => {}
        }
    }

    /// The lobby `lobby`, as `me` sees it, under `room`.
    fn opens(lobby: Lobby, me: PlayerId, room: Authority) -> Stage {
        Stage::Lobby {
            screen: Box::new(LobbyScreen::of(lobby, me)),
            room,
        }
    }

    /// The match `started`, being built over the belt `lobby` was set up
    /// on.
    ///
    /// A machine with no seat in the match the room started has no part in
    /// that room, which the join field reads as the room closing.
    fn starts(lobby: &Lobby, started: Started, mut room: Authority) -> Stage {
        let Some(crew) = started.seating().run_by(room.me()) else {
            return Stage::leaves(room, Some(Joining::Closed));
        };
        let loading = Loading::of(lobby.clone(), started, &crew, room.transport());
        Stage::Loading {
            loading: Box::new(loading),
            room,
        }
    }

    /// The match `loading` built, now that every machine has agreed it.
    fn plays(loading: Loading, room: Authority) -> Stage {
        Stage::Play {
            play: Box::new(loading.plays()),
            room,
        }
    }

    /// The score of the match `play` has reached the clock of.
    fn ends(play: &Play, room: Authority) -> Stage {
        Stage::Results {
            results: Box::new(play.ends()),
            room,
        }
    }

    /// The lobby the finished match was set up in, with its shape kept: the
    /// room's own where there is a room, which broadcasts it, and the one
    /// the results kept where the match was this machine's alone.
    fn rematches(results: Box<Results>, mut room: Authority) -> Stage {
        match room.rematches() {
            true => Stage::Results { results, room },
            false => {
                let me = room.me();
                Stage::opens(results.lobby().clone(), me, room)
            }
        }
    }

    /// The title, with `said` in its join field, telling the room this
    /// machine is leaving and dropping it.
    fn leaves(mut room: Authority, said: Option<Joining>) -> Stage {
        room.leaving();
        // A room this machine served holds the port until it is dropped, so
        // the title binds its own listener after that.
        drop(room);
        Stage::Title {
            listener: Hosting::opened(),
            asking: said.map_or(Asking::Idle, Asking::Said),
        }
    }

    /// The title, told that the host opened the seat this machine held.
    fn removed(room: Authority) -> Stage {
        Stage::leaves(room, Some(Joining::Removed))
    }

    /// The title with `ask` answered: a room served here and joined at the
    /// loopback, or a room joined at a typed address.
    fn asked(listener: Result<Hosting, NoRoom>, asking: Asking, ask: Ask) -> Stage {
        match ask {
            Ask::Host => match listener {
                Ok(hosted) => Stage::Title {
                    // The room is served until the welcome hands it to the
                    // authority, so Host is offered nowhere meanwhile.
                    listener: Err(NoRoom::Held),
                    asking: Asking::Connecting {
                        room: Room::joining(&hosted.address()),
                        hosted: Some(hosted),
                    },
                },
                Err(why) => Stage::Title {
                    listener: Err(why),
                    asking,
                },
            },
            Ask::Join(address) => Stage::Title {
                listener,
                asking: Asking::Connecting {
                    room: Room::joining(&address),
                    hosted: None,
                },
            },
        }
    }

    fn vacant() -> Stage {
        Stage::Title {
            listener: Err(NoRoom::Held),
            asking: Asking::Idle,
        }
    }
}

impl Asking {
    /// What the join field shows about it.
    fn sentence(&self) -> Option<&'static str> {
        match self {
            Asking::Idle => None,
            Asking::Connecting { .. } => Some("Connecting"),
            Asking::Said(said) => Some(said.sentence()),
        }
    }
}

impl Authority {
    /// The person at this machine, as the room's lobby names them.
    pub fn me(&self) -> PlayerId {
        match self {
            Authority::Local(_) | Authority::Host { .. } => PlayerId::HOST,
            Authority::Guest { me, .. } => *me,
        }
    }

    /// What this machine's peers hear its match through: the room's socket
    /// where it is in one, and nothing where the match is its own.
    pub fn transport(&mut self) -> &mut dyn Transport {
        match self {
            Authority::Local(local) => local,
            Authority::Guest { room, .. } | Authority::Host { room, .. } => room.transport(),
        }
    }

    /// Whether this machine holds the host's own controls.
    fn hosts(&self, lobby: &Lobby) -> bool {
        match self {
            Authority::Local(_) => true,
            Authority::Guest { .. } | Authority::Host { .. } => lobby.host() == self.me(),
        }
    }

    /// Applies `edit` to `lobby` and answers the lobby it made, or asks the
    /// room to apply it and answers nothing, since the room broadcasts the
    /// lobby that took.
    fn edits(&mut self, lobby: &Lobby, edit: LobbyEdit) -> Option<Lobby> {
        let me = self.me();
        match self {
            Authority::Local(_) => {
                let mut own = lobby.clone();
                own.edit(me, edit)
                    .expect("a control is offered only where its own rule allows the edit");
                Some(own)
            }
            Authority::Guest { room, .. } | Authority::Host { room, .. } => {
                room.say(Message::Edit(edit));
                None
            }
        }
    }

    /// The match `lobby` starts here, or nothing where the room is the
    /// authority: it freezes its own copy and starts every machine.
    fn starts(&mut self, lobby: &Lobby) -> Option<Started> {
        let started = lobby.freeze().ok()?;
        match self {
            Authority::Local(_) => Some(started),
            Authority::Guest { room, .. } | Authority::Host { room, .. } => {
                room.say(Message::Start(started));
                None
            }
        }
    }

    /// Asks the room to open its lobby again, and answers whether it was
    /// asked: a match this machine played alone opens its own.
    fn rematches(&mut self) -> bool {
        match self {
            Authority::Local(_) => false,
            Authority::Guest { room, .. } | Authority::Host { room, .. } => {
                room.say(Message::Rematch);
                true
            }
        }
    }

    /// Everything the room has said to this screen since the last call.
    fn heard(&mut self) -> Vec<Word> {
        match self {
            Authority::Local(_) => Vec::new(),
            Authority::Guest { room, .. } | Authority::Host { room, .. } => room.heard(),
        }
    }

    /// Whether the room's socket has closed.
    fn closed(&self) -> bool {
        match self {
            Authority::Local(_) => false,
            Authority::Guest { room, .. } | Authority::Host { room, .. } => room.closed(),
        }
    }

    /// Tells the room this machine is leaving it.
    fn leaving(&mut self) {
        match self {
            Authority::Local(_) => {}
            Authority::Guest { room, .. } | Authority::Host { room, .. } => room.leaving(),
        }
    }
}

/// One frame of the title, and what the player picked on it.
fn titled<G: Playable>(
    ctx: &mut FrameCtx<'_, G>,
    title: &mut Title,
    listener: &Result<Hosting, NoRoom>,
    asking: &Asking,
) -> Option<title::Picked>
where
    G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
{
    let hosts = Rule::unless(listener.as_ref().err().map(|why| why.reason().to_string()));
    let said = asking.sentence();
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
        picked = title.frame(&panel, &Typed::this_frame(ui.ctx()), &hosts, said);
    });
    picked
}

/// The title, or the lobby the room it is joining welcomed this machine
/// into.
fn welcomed(listener: Result<Hosting, NoRoom>, asking: Asking) -> Stage {
    let Asking::Connecting { mut room, hosted } = asking else {
        return Stage::Title { listener, asking };
    };
    if room.closed() {
        // A socket that closed before any welcome never reached a room,
        // which is what the address says.
        return Stage::Title {
            listener: Hosting::opened(),
            asking: Asking::Said(Joining::NoRoom),
        };
    }
    let mut said = None;
    for word in room.heard() {
        match word {
            Word::Welcome { player, lobby } => {
                let room = match hosted {
                    Some(listener) => Authority::Host { room, listener },
                    None => Authority::Guest { room, me: player },
                };
                return Stage::opens(lobby, player, room);
            }
            Word::Refused(why) => said = Joining::refused(why).or(said),
            // A room that ends before it welcomes this machine is a room it
            // was never in, and nothing else there names it yet.
            Word::Left | Word::Removed => said = Some(Joining::NoRoom),
            Word::Lobby(_) | Word::Started(_) => {}
        }
    }
    match said {
        Some(said) => {
            drop(room);
            Stage::Title {
                listener: Hosting::opened(),
                asking: Asking::Said(said),
            }
        }
        None => Stage::Title {
            listener,
            asking: Asking::Connecting { room, hosted },
        },
    }
}
