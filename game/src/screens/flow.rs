//! Which screen is live, the room this machine is in, and what each screen
//! hands over to the next.

use mirage_engine::egui;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_protocol::{Lobby, LobbyEdit, Message, PlayerId, Refusal};
use probe_sim::Setup;

use crate::controls::Button;
use crate::display::glyph_quad::GlyphQuad;
use crate::net::hosting::Hosting;
use crate::net::local::Local;
use crate::net::room::Room;
use crate::net::transport::Transport;
use crate::screens::Playable;
use crate::screens::field::Typed;
use crate::screens::loading::Loading;
use crate::screens::lobby::{LobbyScreen, Wants};
use crate::screens::panel::{self, Panel};
use crate::screens::play::Play;
use crate::screens::results::Results;
use crate::screens::title::Title;

/// What a screen asks the flow to show next. No screen floats over
/// another: each replaces the last, and the pause screen is the match's
/// own.
pub enum Step {
    /// Serve a room on this machine and open its lobby as the host.
    Host,
    /// Join the room at this address, as `host:port`.
    Join(String),
    /// This match, being built.
    Loading(Box<Loading>),
    /// This match, being played.
    Play(Box<Play>),
    /// The lobby the finished match was set up in, its shape kept.
    Rematch,
    /// The match under the pause screen, going on.
    Resume,
    /// The score of this finished match.
    Results(Box<Results>),
    /// A skirmish lobby, every slot of it this machine's.
    Skirmish,
    /// The title.
    Title,
}

/// The one value that owns which screen is live.
pub struct Flow {
    live: Live,
    /// The person at this machine: the host of every lobby it opens itself,
    /// and whoever a room welcomed it as.
    me: PlayerId,
    /// The room this machine is in, where it is in one.
    room: Option<Room>,
    /// The room this machine serves, where it is hosting one.
    hosting: Option<Hosting>,
    /// The transport of a match with no peers.
    local: Local,
    /// What the title says under its actions, where there is something.
    note: Option<String>,
}

/// The screen the game is showing.
enum Live {
    Loading(Box<Loading>),
    Lobby(Box<LobbyScreen>),
    Play(Box<Play>),
    Results(Box<Results>),
    Title(Title),
}

impl Flow {
    /// The lobby on screen, where it is the lobby.
    pub fn lobby(&self) -> Option<&Lobby> {
        match &self.live {
            Live::Lobby(lobby) => Some(lobby.lobby()),
            Live::Title(_) | Live::Loading(_) | Live::Play(_) | Live::Results(_) => None,
        }
    }

    /// The game as it opens: the title.
    pub fn opening() -> Flow {
        Flow {
            live: Live::Title(Title::opening()),
            me: PlayerId::HOST,
            room: None,
            hosting: None,
            local: Local,
            note: None,
        }
    }

    /// The match being played, where one is.
    pub fn play(&self) -> Option<&Play> {
        match &self.live {
            Live::Play(play) => Some(play),
            Live::Title(_) | Live::Lobby(_) | Live::Loading(_) | Live::Results(_) => None,
        }
    }

    /// The finished match's score, where it is on screen.
    pub fn results(&self) -> Option<&Results> {
        match &self.live {
            Live::Results(results) => Some(results),
            Live::Title(_) | Live::Lobby(_) | Live::Loading(_) | Live::Play(_) => None,
        }
    }

    /// One frame of the live screen, and the screen it hands over to.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>)
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        self.hears();
        let step = match &mut self.live {
            Live::Title(title) => {
                let pixel = ctx.pointer();
                let size = ctx.window_size();
                let clicked = ctx.pressed(Button::Select);
                let note = self.note.clone();
                let mut step = None;
                ctx.ui(|ui| {
                    let points = 1.0 / ui.ctx().pixels_per_point();
                    let panel = Panel::new(
                        ui.painter(),
                        panel::window_of(size, points),
                        egui::pos2(pixel.x * points, pixel.y * points),
                        clicked,
                    );
                    step = title.frame(&panel, &Typed::of(ui.ctx()), note.as_deref());
                });
                step
            }
            Live::Lobby(lobby) => {
                let asked = lobby.frame(ctx);
                let picked = asked.picked;
                self.asks(asked.edits);
                self.wants(picked)
            }
            Live::Loading(loading) => loading.frame(ctx),
            Live::Play(play) => play.frame(ctx),
            // A rematch of a match played in a room would need the room to
            // open its lobby again, which no message asks for yet.
            Live::Results(results) => results.frame(ctx, self.room.is_none()),
        };
        if let Some(step) = step {
            self.take(step);
        }
    }

    /// One sim tick of the live screen.
    pub fn tick(&mut self) {
        match &mut self.live {
            Live::Loading(loading) => {
                loading.tick(transport(&mut self.room, &mut self.local));
            }
            Live::Play(play) => play.tick(transport(&mut self.room, &mut self.local)),
            Live::Title(_) | Live::Lobby(_) | Live::Results(_) => {}
        }
    }

    /// Asks the room for `edits`, or applies them where this machine's
    /// lobby is its own.
    fn asks(&mut self, edits: Vec<LobbyEdit>) {
        let Live::Lobby(lobby) = &mut self.live else {
            return;
        };
        for edit in edits {
            match &mut self.room {
                Some(room) => room.say(Message::Edit(edit)),
                // A skirmish has no room, so the screen's own lobby is the
                // authority; every control it draws is its viewer's.
                None => {
                    let mut own = lobby.lobby().clone();
                    if own.edit(self.me, edit).is_ok() {
                        lobby.takes(own);
                    }
                }
            }
        }
    }

    /// Takes in what the room has said, which is the lobby's authority and
    /// the start of every match.
    ///
    /// Only up to the start: a match reads the same socket itself, so the
    /// flow stops reading it the moment one is being built.
    fn hears(&mut self) {
        match self.live {
            Live::Title(_) | Live::Lobby(_) => {}
            Live::Loading(_) | Live::Play(_) | Live::Results(_) => return,
        }
        let (closed, heard, me) = match &mut self.room {
            Some(room) => (room.closed(), room.heard(), room.me()),
            None => return,
        };
        if closed {
            self.left(Some("The room closed"));
            return;
        }
        self.me = me.unwrap_or(self.me);
        for message in heard {
            self.hear(message);
        }
    }

    /// Takes one of the room's messages.
    fn hear(&mut self, message: Message) {
        match (&mut self.live, message) {
            (Live::Title(_), Message::Welcome { player, lobby }) => {
                self.note = None;
                self.me = player;
                self.live = Live::Lobby(Box::new(LobbyScreen::of(lobby, player)));
            }
            (Live::Lobby(lobby), Message::Lobby(sent)) => lobby.takes(sent),
            (Live::Lobby(lobby), Message::Start(setup)) => {
                let held = lobby.lobby().clone();
                self.opens(held, setup);
            }
            (_, Message::Refused(why)) => self.note = Some(refusal(why)),
            (_, Message::Leave) => self.left(Some("The host left the room")),
            // A message the live screen has no use for: the match's own,
            // which the machine playing it reads through the transport, and
            // the room's words to a screen that is past them.
            (_, Message::Welcome { .. })
            | (_, Message::Lobby(_))
            | (_, Message::Start(_))
            | (_, Message::Join)
            | (_, Message::Edit(_))
            | (_, Message::Command(_))
            | (_, Message::Acknowledge { .. })
            | (_, Message::Hash { .. })
            | (_, Message::Desync { .. }) => {}
        }
    }

    /// Drops the room and shows the title, saying why where there is
    /// something to say.
    fn left(&mut self, why: Option<&str>) {
        self.leave();
        self.note = why.map(str::to_string);
        self.live = Live::Title(Title::opening());
    }

    /// Tells the room this machine is leaving and drops it, along with the
    /// room it was serving. The flow owns the socket, so this is how every
    /// screen leaves.
    fn leave(&mut self) {
        if let Some(room) = &mut self.room {
            room.leaving();
        }
        self.room = None;
        self.hosting = None;
        self.me = PlayerId::HOST;
    }

    /// Builds the match `setup` names, over the lobby it was frozen from.
    fn opens(&mut self, lobby: Lobby, setup: Setup) {
        let me = self.me;
        match Loading::of(lobby, setup, me, transport(&mut self.room, &mut self.local)) {
            Ok(loading) => self.live = Live::Loading(Box::new(loading)),
            Err(why) => self.left(Some(&format!("This machine holds no seat: {why:?}"))),
        }
    }

    /// Shows what `step` names.
    fn take(&mut self, step: Step) {
        match step {
            Step::Title => self.left(None),
            Step::Skirmish => {
                self.leave();
                self.live =
                    Live::Lobby(Box::new(LobbyScreen::of(Lobby::skirmish(self.me), self.me)));
            }
            Step::Host => match Hosting::opened() {
                Ok(hosting) => {
                    self.note = Some(format!("Joining {}", hosting.address()));
                    self.room = Some(Room::joining(&hosting.address()));
                    self.hosting = Some(hosting);
                }
                Err(why) => self.note = Some(why.to_string()),
            },
            Step::Join(address) => {
                self.note = Some(format!("Joining {address}"));
                self.room = Some(Room::joining(&address));
            }
            Step::Rematch => {
                if let Live::Results(results) = &self.live {
                    let lobby = results.lobby().clone();
                    self.live = Live::Lobby(Box::new(LobbyScreen::of(lobby, self.me)));
                }
            }
            Step::Loading(loading) => self.live = Live::Loading(loading),
            Step::Play(play) => self.live = Live::Play(play),
            Step::Results(results) => self.live = Live::Results(results),
            // The match itself closes its pause screen; the flow never
            // sees a resume.
            Step::Resume => {}
        }
    }

    /// What the lobby's own actions ask of the flow.
    fn wants(&mut self, picked: Option<Wants>) -> Option<Step> {
        let Live::Lobby(lobby) = &self.live else {
            return None;
        };
        let held = lobby.lobby().clone();
        match picked? {
            Wants::Leave => Some(Step::Title),
            Wants::Start => {
                let setup = held.freeze().ok()?;
                match &mut self.room {
                    // The room is the authority: it freezes its own copy
                    // and starts every machine, this one included.
                    Some(room) => {
                        room.say(Message::Start(setup));
                        None
                    }
                    None => {
                        self.opens(held, setup);
                        None
                    }
                }
            }
        }
    }
}

/// What this machine's peers hear it through: the room's socket where it is
/// in one, and nothing where the match is its own.
fn transport<'a>(room: &'a mut Option<Room>, local: &'a mut Local) -> &'a mut dyn Transport {
    match room {
        Some(room) => room.transport(),
        None => local,
    }
}

/// A refusal as the title states it.
fn refusal(why: Refusal) -> String {
    match why {
        Refusal::Full => "That room is full".to_string(),
        Refusal::Edit(_) => "The room refused that change".to_string(),
        Refusal::NotReady(_) => "The lobby is not a match yet".to_string(),
    }
}
