//! Which screen is live, and what each one hands over to the next.

use mirage_engine::egui;
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_protocol::{Lobby, PlayerId};

use crate::controls::Button;
use crate::display::glyph_quad::GlyphQuad;
use crate::screens::Playable;
use crate::screens::loading::Loading;
use crate::screens::lobby::LobbyScreen;
use crate::screens::panel::{self, Panel};
use crate::screens::play::Play;
use crate::screens::results::Results;
use crate::screens::title::Title;

/// What a screen asks the flow to show next. No screen floats over
/// another: each replaces the last, and the pause screen is the match's
/// own.
pub enum Step {
    /// The title.
    Title,
    /// A skirmish lobby, every slot of it this machine's.
    Skirmish,
    /// The lobby the finished match was set up in, its shape kept.
    Rematch,
    /// This match, being built.
    Loading(Box<Loading>),
    /// This match, being played.
    Play(Box<Play>),
    /// The score of this finished match.
    Results(Box<Results>),
    /// The match under the pause screen, going on.
    Resume,
}

/// The screen the game is showing.
enum Live {
    Title(Title),
    Lobby(Box<LobbyScreen>),
    Loading(Box<Loading>),
    Play(Box<Play>),
    Results(Box<Results>),
}

/// The one value that owns which screen is live.
pub struct Flow {
    /// The person at this machine. Every lobby it opens is theirs to host
    /// until Join lands.
    me: PlayerId,
    live: Live,
}

impl Flow {
    /// The game as it opens: the title.
    pub fn opening() -> Flow {
        Flow {
            me: PlayerId::HOST,
            live: Live::Title(Title),
        }
    }

    /// The match being played or built, where one is.
    pub fn play(&self) -> Option<&Play> {
        match &self.live {
            Live::Play(play) => Some(play),
            Live::Title(_) | Live::Lobby(_) | Live::Loading(_) | Live::Results(_) => None,
        }
    }

    /// The lobby on screen, where it is the lobby.
    pub fn lobby(&self) -> Option<&Lobby> {
        match &self.live {
            Live::Lobby(lobby) => Some(lobby.lobby()),
            Live::Title(_) | Live::Loading(_) | Live::Play(_) | Live::Results(_) => None,
        }
    }

    /// The finished match's score, where it is on screen.
    pub fn results(&self) -> Option<&Results> {
        match &self.live {
            Live::Results(results) => Some(results),
            Live::Title(_) | Live::Lobby(_) | Live::Loading(_) | Live::Play(_) => None,
        }
    }

    /// One sim tick of the live screen.
    pub fn tick(&mut self) {
        match &mut self.live {
            Live::Loading(loading) => loading.tick(),
            Live::Play(play) => play.tick(),
            Live::Title(_) | Live::Lobby(_) | Live::Results(_) => {}
        }
    }

    /// One frame of the live screen, and the screen it hands over to.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>)
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let step = match &mut self.live {
            Live::Title(title) => {
                let pixel = ctx.pointer();
                let size = ctx.window_size();
                let clicked = ctx.pressed(Button::Select);
                let mut step = None;
                ctx.ui(|ui| {
                    let points = 1.0 / ui.ctx().pixels_per_point();
                    let panel = Panel::new(
                        ui.painter(),
                        panel::window_of(size, points),
                        egui::pos2(pixel.x * points, pixel.y * points),
                        clicked,
                    );
                    step = title.frame(&panel);
                });
                step
            }
            Live::Lobby(lobby) => lobby.frame(ctx),
            Live::Loading(loading) => loading.frame(ctx),
            Live::Play(play) => play.frame(ctx),
            Live::Results(results) => results.frame(ctx),
        };
        if let Some(step) = step {
            self.take(step);
        }
    }

    /// Shows what `step` names.
    fn take(&mut self, step: Step) {
        self.live = match step {
            Step::Title => Live::Title(Title),
            Step::Skirmish => {
                Live::Lobby(Box::new(LobbyScreen::of(Lobby::skirmish(self.me), self.me)))
            }
            Step::Rematch => match &self.live {
                Live::Results(results) => {
                    Live::Lobby(Box::new(LobbyScreen::of(results.lobby().clone(), self.me)))
                }
                // Only the results screen offers a rematch.
                Live::Title(_) | Live::Lobby(_) | Live::Loading(_) | Live::Play(_) => {
                    Live::Title(Title)
                }
            },
            Step::Loading(loading) => Live::Loading(loading),
            Step::Play(play) => Live::Play(play),
            Step::Results(results) => Live::Results(results),
            // The match itself closes its pause screen; the flow never
            // sees a resume.
            Step::Resume => return,
        };
    }
}
