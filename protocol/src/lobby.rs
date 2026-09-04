//! The lobby a match is set up in: who holds each seat, and who may say so.

use core::ops::RangeInclusive;

use probe_sim::{MAX_SEATS, SeatId, Setup, TICKS_PER_SECOND, TeamId, Tick};
use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use crate::seating::{Holder, Seating, Started};

/// Clocks a lobby allows, in ticks: a minute to half an hour, around
/// DESIGN.md's fifteen.
pub const CLOCK_RANGE: RangeInclusive<Tick> =
    Tick(60 * TICKS_PER_SECOND as u64)..=Tick(30 * 60 * TICKS_PER_SECOND as u64);

/// The clock a lobby opens at: fifteen minutes.
pub const DEFAULT_CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

/// The seed a lobby opens at.
pub const DEFAULT_SEED: u64 = 1;

/// Slots a lobby holds, one per seat a match can have.
pub const MAX_SLOTS: usize = MAX_SEATS;

/// What [`Lobby::next_seed`] steps the seed by: odd, so the seeds it walks
/// never repeat.
const SEED_STEP: u64 = 0x9E37_79B9_7F4A_7C15;

/// A frozen lobby is a match, so its held slots are a setup's seats.
const _: () = assert!(MAX_SLOTS == MAX_SEATS);

/// A shipped scripted opponent, by the way it plays. The machine that owns
/// the seat turns it into the agent's own constants.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Bot {
    /// Holds few rocks and keeps a heavy defensive army.
    Turtle,
    /// Claims rocks fast and commits early.
    Expand,
}

/// What holds one slot.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Control {
    /// A player may still take it; the match cannot start while one does.
    Open,
    /// Not in the match.
    Closed,
    /// A person, and whether they have readied. A guest's readiness gates
    /// the start; the host's does not, since the host starts the match.
    Player { player: PlayerId, ready: bool },
    /// A scripted opponent, run by the machine that added it.
    Bot(Bot),
}

/// Why an edit was not applied.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Refused {
    /// The shape is the host's, and this editor is not the host.
    NotHost,
    /// A guest edits its own slot and no other.
    NotYours,
    /// The editor holds no slot to edit.
    NotSeated,
    /// No slot of that index.
    NoSuchSlot,
    /// A team no slot could sit on: teams run to [`MAX_SLOTS`].
    BadTeam,
    /// A clock outside [`CLOCK_RANGE`].
    BadClock,
    /// That player already holds another slot, and holds one at most.
    AlreadySeated,
    /// The player named holds no guest's slot, so there is none to open.
    NotAGuest,
    /// A guest holds that slot, and a guest's slot is opened by a kick,
    /// which sends the guest away with it.
    HeldByAGuest,
}

/// Why a lobby is not yet a match.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum NotReady {
    /// Every slot is closed.
    NoSeats,
    /// This slot is open, so a player may still take it.
    OpenSeat { slot: usize },
    /// This slot's guest has not readied.
    Unready { slot: usize },
    /// The host runs none of the seats, so its own machine has no part in
    /// the match it would start.
    HostUnseated,
}

/// One change to a lobby. The host may make any of them; a guest may set
/// its own slot's team and its own readiness.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum LobbyEdit {
    /// What holds a slot.
    SetSlot { slot: usize, control: Control },
    /// Opens the slot a guest holds, which sends that guest away.
    Kick(PlayerId),
    /// A slot's team.
    SetTeam { slot: usize, team: TeamId },
    /// The map's seed.
    SetSeed(u64),
    /// The tick the match ends at.
    SetClock(Tick),
    /// The editor's own readiness.
    SetReady { ready: bool },
}

/// One seat of a lobby.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct SeatSlot {
    pub team: TeamId,
    pub control: Control,
}

/// A match being set up: [`MAX_SLOTS`] slots, the shape the host owns, and
/// the host. The same value on every machine of the lobby.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Lobby {
    slots: Vec<SeatSlot>,
    seed: u64,
    clock: Tick,
    host: PlayerId,
}

impl Lobby {
    /// A skirmish: the host in slot zero, a bot in slot one, the rest
    /// closed, so the one-against-one baseline starts with no edits.
    pub fn skirmish(host: PlayerId) -> Lobby {
        let mut slots = closed_slots();
        slots[0].control = Control::Player {
            player: host,
            ready: true,
        };
        slots[1].control = Control::Bot(Bot::Expand);
        Lobby {
            slots,
            seed: DEFAULT_SEED,
            clock: DEFAULT_CLOCK,
            host,
        }
    }

    /// The shape a room opens on: the host in slot zero, one open seat, the
    /// rest closed, so a guest has somewhere to join.
    pub fn room(host: PlayerId) -> Lobby {
        let mut lobby = Lobby::skirmish(host);
        lobby.slots[1].control = Control::Open;
        lobby
    }

    /// Every slot, in slot order.
    pub fn slots(&self) -> &[SeatSlot] {
        &self.slots
    }

    /// The map's seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The tick a match from this lobby ends at.
    pub fn clock(&self) -> Tick {
        self.clock
    }

    /// The player who owns the shape.
    pub fn host(&self) -> PlayerId {
        self.host
    }

    /// The slot `player` holds, if any.
    pub fn slot_of(&self, player: PlayerId) -> Option<usize> {
        self.slots.iter().position(|slot| slot.holds(player))
    }

    /// The seat a slot becomes once the lobby is frozen: its place among
    /// the slots that are not closed. `None` for a closed slot, which is
    /// not in the match, and for no such slot.
    pub fn seat_of(&self, slot: usize) -> Option<SeatId> {
        self.numbered()
            .find(|(_, at, _)| *at == slot)
            .map(|(seat, _, _)| seat)
    }

    /// Every machine with a seat to run once this lobby is frozen, in id
    /// order, each once.
    pub fn players(&self) -> Vec<PlayerId> {
        self.seating().players()
    }

    /// Whether `player` holds a slot and has readied.
    pub fn readied(&self, player: PlayerId) -> bool {
        self.slot_of(player)
            .map(|slot| self.slots[slot].control)
            .is_some_and(|control| matches!(control, Control::Player { ready: true, .. }))
    }

    /// Opens the slot a guest holds, so another machine may take it. False
    /// for the host's own slot, which stays its own, and for a player
    /// holding none.
    pub fn release(&mut self, who: PlayerId) -> bool {
        let Some(slot) = self.slot_of(who).filter(|_| who != self.host) else {
            return false;
        };
        self.slots[slot].control = Control::Open;
        true
    }

    /// Seats `player`: the slot it already holds, else the first open one.
    /// `None` where no slot is open, which is a lobby with nowhere to sit.
    pub fn admit(&mut self, player: PlayerId) -> Option<SeatId> {
        let slot = match self.slot_of(player) {
            Some(slot) => slot,
            None => {
                let open = self
                    .slots
                    .iter()
                    .position(|slot| slot.control == Control::Open)?;
                self.slots[open].control = Control::Player {
                    player,
                    ready: false,
                };
                open
            }
        };
        self.seat_of(slot)
    }

    /// Applies `edit` as `by`, or refuses it by name: the host owns the
    /// shape, and a guest owns its own slot's team and its own readiness.
    pub fn edit(&mut self, by: PlayerId, edit: LobbyEdit) -> Result<(), Refused> {
        match edit {
            LobbyEdit::SetSlot { slot, control } => {
                self.as_host(by)?;
                let held = self.slots.get(slot).ok_or(Refused::NoSuchSlot)?.control;
                if let Control::Player { player: guest, .. } = held
                    && guest != self.host
                    && !matches!(control, Control::Player { player, .. } if player == guest)
                {
                    return Err(Refused::HeldByAGuest);
                }
                if let Control::Player { player, .. } = control
                    && self.slot_of(player).is_some_and(|held| held != slot)
                {
                    return Err(Refused::AlreadySeated);
                }
                self.slots[slot].control = control;
            }
            LobbyEdit::Kick(who) => {
                self.as_host(by)?;
                if !self.release(who) {
                    return Err(Refused::NotAGuest);
                }
            }
            LobbyEdit::SetTeam { slot, team } => {
                if usize::from(team.0) >= MAX_SLOTS {
                    return Err(Refused::BadTeam);
                }
                self.own_slot(by, slot)?.team = team;
            }
            LobbyEdit::SetSeed(seed) => {
                self.as_host(by)?;
                self.seed = seed;
            }
            LobbyEdit::SetClock(clock) => {
                self.as_host(by)?;
                if !CLOCK_RANGE.contains(&clock) {
                    return Err(Refused::BadClock);
                }
                self.clock = clock;
            }
            LobbyEdit::SetReady { ready } => {
                let slot = self.slot_of(by).ok_or(Refused::NotSeated)?;
                self.slots[slot].control = Control::Player { player: by, ready };
            }
        }
        Ok(())
    }

    /// The seed the random_seed action asks for: this one stepped by
    /// [`SEED_STEP`]. The sim has no clock and no randomness to draw a
    /// fresh seed from, and a step needs neither.
    pub fn next_seed(&self) -> u64 {
        self.seed.wrapping_add(SEED_STEP)
    }

    /// The match this lobby starts, or why it is not one yet: every slot
    /// held or closed, every guest ready, at least one seat, and a seat the
    /// host's own machine runs.
    pub fn freeze(&self) -> Result<Started, NotReady> {
        for (at, slot) in self.slots.iter().enumerate() {
            match slot.control {
                Control::Open => return Err(NotReady::OpenSeat { slot: at }),
                Control::Player { player, ready } if !ready && player != self.host => {
                    return Err(NotReady::Unready { slot: at });
                }
                Control::Closed | Control::Player { .. } | Control::Bot(_) => {}
            }
        }
        let seating = self.seating();
        if seating.players().is_empty() {
            return Err(NotReady::NoSeats);
        }
        if seating.run_by(self.host).is_none() {
            return Err(NotReady::HostUnseated);
        }
        // At most `MAX_SLOTS` slots are held and `MAX_SLOTS` is `MAX_SEATS`,
        // so the only setup a lobby can refuse is the empty one, above.
        let setup = Setup::new(self.teams_seated(), self.seed, self.clock)
            .expect("held slots are a match's seats");
        Ok(Started::new(setup, seating).expect("one seat per slot that holds one"))
    }

    /// Every slot that holds a seat, with the seat it becomes and its own
    /// index, in slot order.
    fn numbered(&self) -> impl Iterator<Item = (SeatId, usize, &SeatSlot)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.control.holder().is_some())
            .zip(0u8..)
            .map(|((at, slot), seat)| (SeatId(seat), at, slot))
    }

    /// Who holds each seat this lobby would freeze into.
    fn seating(&self) -> Seating {
        Seating::new(
            self.numbered()
                .filter_map(|(_, _, slot)| slot.control.holder())
                .collect(),
            self.host,
        )
    }

    /// Each seat's team, in seat order, which is a match's seating.
    fn teams_seated(&self) -> Vec<TeamId> {
        self.numbered().map(|(_, _, slot)| slot.team).collect()
    }

    /// `slot` as its holder or as the host, or a refusal.
    fn own_slot(&mut self, by: PlayerId, slot: usize) -> Result<&mut SeatSlot, Refused> {
        let held = self.slots.get(slot).ok_or(Refused::NoSuchSlot)?;
        if by != self.host && !held.holds(by) {
            return Err(Refused::NotYours);
        }
        Ok(&mut self.slots[slot])
    }

    fn as_host(&self, by: PlayerId) -> Result<(), Refused> {
        (by == self.host).then_some(()).ok_or(Refused::NotHost)
    }
}

impl Control {
    /// What it holds as a seat of a match; `None` for a closed slot, which
    /// holds no seat.
    pub fn holder(self) -> Option<Holder> {
        match self {
            Control::Open => Some(Holder::Open),
            Control::Player { player, .. } => Some(Holder::Player(player)),
            Control::Bot(bot) => Some(Holder::Bot(bot)),
            Control::Closed => None,
        }
    }
}

impl SeatSlot {
    /// Whether `player` holds this slot.
    pub fn holds(&self, player: PlayerId) -> bool {
        matches!(self.control, Control::Player { player: held, .. } if held == player)
    }
}

/// [`MAX_SLOTS`] closed slots, each on its own team, which is the shape a
/// lobby edits from.
fn closed_slots() -> Vec<SeatSlot> {
    (0..MAX_SLOTS)
        .map(|at| SeatSlot {
            team: TeamId(at as u8),
            control: Control::Closed,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUEST: PlayerId = PlayerId(7);

    /// The skirmish lobby with slot two held by a guest who has not
    /// readied, which is the shape every rule below is read against.
    fn joined() -> Lobby {
        let mut lobby = Lobby::skirmish(PlayerId::HOST);
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 2,
                    control: Control::Player {
                        player: GUEST,
                        ready: false,
                    },
                },
            )
            .expect("the host seats a guest");
        lobby
    }

    #[test]
    fn the_host_edits_the_whole_shape() {
        let mut lobby = joined();
        let clock = *CLOCK_RANGE.start();

        for edit in [
            LobbyEdit::SetSlot {
                slot: 3,
                control: Control::Open,
            },
            LobbyEdit::SetTeam {
                slot: 2,
                team: TeamId(0),
            },
            LobbyEdit::SetSeed(42),
            LobbyEdit::SetClock(clock),
        ] {
            assert_eq!(lobby.edit(PlayerId::HOST, edit), Ok(()), "{edit:?}");
        }

        assert_eq!(lobby.slots()[3].control, Control::Open);
        assert_eq!(lobby.slots()[2].team, TeamId(0));
        assert_eq!(lobby.seed(), 42);
        assert_eq!(lobby.clock(), clock);
    }

    #[test]
    fn a_guest_edits_its_own_slots_team_and_its_own_readiness_and_nothing_else() {
        let mut lobby = joined();

        assert_eq!(
            lobby.edit(
                GUEST,
                LobbyEdit::SetTeam {
                    slot: 2,
                    team: TeamId(0)
                }
            ),
            Ok(())
        );
        assert_eq!(
            lobby.edit(GUEST, LobbyEdit::SetReady { ready: true }),
            Ok(())
        );

        assert_eq!(lobby.slots()[2].team, TeamId(0));
        assert_eq!(
            lobby.slots()[2].control,
            Control::Player {
                player: GUEST,
                ready: true
            }
        );
    }

    #[test]
    fn every_edit_a_player_does_not_own_is_refused_by_name() {
        let mut lobby = joined();
        let stranger = PlayerId(99);

        assert_eq!(
            lobby.edit(
                GUEST,
                LobbyEdit::SetSlot {
                    slot: 0,
                    control: Control::Open
                }
            ),
            Err(Refused::NotHost)
        );
        assert_eq!(
            lobby.edit(GUEST, LobbyEdit::SetSeed(3)),
            Err(Refused::NotHost)
        );
        assert_eq!(
            lobby.edit(
                GUEST,
                LobbyEdit::SetTeam {
                    slot: 0,
                    team: TeamId(1)
                }
            ),
            Err(Refused::NotYours)
        );
        assert_eq!(
            lobby.edit(stranger, LobbyEdit::SetReady { ready: true }),
            Err(Refused::NotSeated)
        );
        assert_eq!(
            lobby.edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: MAX_SLOTS,
                    control: Control::Open
                }
            ),
            Err(Refused::NoSuchSlot)
        );
        assert_eq!(
            lobby.edit(
                PlayerId::HOST,
                LobbyEdit::SetTeam {
                    slot: 0,
                    team: TeamId(MAX_SLOTS as u8)
                }
            ),
            Err(Refused::BadTeam)
        );
        assert_eq!(
            lobby.edit(
                PlayerId::HOST,
                LobbyEdit::SetClock(CLOCK_RANGE.end().next())
            ),
            Err(Refused::BadClock)
        );

        assert_eq!(lobby, joined(), "a refused edit changes nothing");
    }

    #[test]
    fn the_host_opens_a_guests_slot_and_nobody_else_opens_anyones() {
        let mut lobby = joined();

        assert_eq!(lobby.edit(PlayerId::HOST, LobbyEdit::Kick(GUEST)), Ok(()));

        assert_eq!(lobby.slots()[2].control, Control::Open);
        assert_eq!(lobby.slot_of(GUEST), None, "the guest holds no slot now");
        assert_eq!(
            joined().edit(GUEST, LobbyEdit::Kick(PlayerId::HOST)),
            Err(Refused::NotHost)
        );
        assert_eq!(
            joined().edit(PlayerId::HOST, LobbyEdit::Kick(PlayerId::HOST)),
            Err(Refused::NotAGuest),
            "the host does not remove itself"
        );
        assert_eq!(
            joined().edit(PlayerId::HOST, LobbyEdit::Kick(PlayerId(99))),
            Err(Refused::NotAGuest)
        );
    }

    #[test]
    fn a_player_holds_one_slot_at_most() {
        let mut lobby = joined();

        assert_eq!(
            lobby.edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 3,
                    control: Control::Player {
                        player: GUEST,
                        ready: false
                    }
                }
            ),
            Err(Refused::AlreadySeated)
        );

        assert_eq!(lobby, joined(), "a refused edit changes nothing");
        assert_eq!(
            lobby.edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 2,
                    control: Control::Player {
                        player: GUEST,
                        ready: true
                    }
                }
            ),
            Ok(()),
            "the slot a player already holds is still its own"
        );
    }

    #[test]
    fn a_lobby_freezes_only_once_every_slot_is_held_or_closed_and_every_guest_is_ready() {
        let mut lobby = joined();

        assert_eq!(lobby.freeze(), Err(NotReady::Unready { slot: 2 }));
        lobby
            .edit(GUEST, LobbyEdit::SetReady { ready: true })
            .expect("the guest readies");

        let started = lobby.freeze().expect("three held slots are a match");
        let setup = started.setup();
        assert_eq!(setup.teams(), [TeamId(0), TeamId(1), TeamId(2)]);
        assert_eq!(setup.seed(), lobby.seed());
        assert_eq!(setup.clock(), lobby.clock());

        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 3,
                    control: Control::Open,
                },
            )
            .expect("the host opens a seat");
        assert_eq!(lobby.freeze(), Err(NotReady::OpenSeat { slot: 3 }));

        lobby
            .edit(PlayerId::HOST, LobbyEdit::Kick(GUEST))
            .expect("the host removes the guest");
        for slot in 0..MAX_SLOTS {
            lobby
                .edit(
                    PlayerId::HOST,
                    LobbyEdit::SetSlot {
                        slot,
                        control: Control::Closed,
                    },
                )
                .expect("the host closes a seat");
        }
        assert_eq!(lobby.freeze(), Err(NotReady::NoSeats));
    }

    #[test]
    fn a_guests_slot_is_opened_by_a_kick_and_by_no_other_edit() {
        let mut lobby = joined();

        for control in [
            Control::Open,
            Control::Closed,
            Control::Bot(Bot::Turtle),
            Control::Player {
                player: PlayerId::HOST,
                ready: true,
            },
        ] {
            assert_eq!(
                lobby.edit(PlayerId::HOST, LobbyEdit::SetSlot { slot: 2, control }),
                Err(Refused::HeldByAGuest),
                "{control:?}"
            );
        }

        assert_eq!(lobby, joined(), "a refused edit changes nothing");
        assert_eq!(lobby.edit(PlayerId::HOST, LobbyEdit::Kick(GUEST)), Ok(()));
        assert_eq!(lobby.slots()[2].control, Control::Open);
    }

    #[test]
    fn a_lobby_the_host_runs_no_seat_of_is_not_a_match() {
        let mut lobby = joined();
        lobby
            .edit(GUEST, LobbyEdit::SetReady { ready: true })
            .expect("the guest readies");
        for slot in [0, 1] {
            lobby
                .edit(
                    PlayerId::HOST,
                    LobbyEdit::SetSlot {
                        slot,
                        control: Control::Closed,
                    },
                )
                .expect("the host closes a seat");
        }

        assert_eq!(lobby.freeze(), Err(NotReady::HostUnseated));

        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 1,
                    control: Control::Bot(Bot::Turtle),
                },
            )
            .expect("the host adds a bot");
        assert!(
            lobby.freeze().is_ok(),
            "a bot of the host's is a seat its machine runs"
        );
    }

    #[test]
    fn every_machine_of_a_frozen_lobby_runs_a_seat_of_it() {
        let mut lobby = joined();
        lobby
            .edit(GUEST, LobbyEdit::SetReady { ready: true })
            .expect("the guest readies");
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 0,
                    control: Control::Closed,
                },
            )
            .expect("the host closes its own seat");

        let started = lobby.freeze().expect("a bot and a ready guest");
        let seating = started.seating();

        assert_eq!(seating.players(), [PlayerId::HOST, GUEST]);
        for player in seating.players() {
            assert!(
                seating.run_by(player).is_some(),
                "{player:?} runs no seat of the match it is in"
            );
        }
        let host = seating.run_by(PlayerId::HOST).expect("the host's bot");
        assert_eq!(host.seats(), [SeatId(0)], "the bot is the host's to run");
        assert_eq!(
            host.watched(),
            SeatId(0),
            "a host holding no seat watches the one it runs"
        );
        assert_eq!(seating.seat(PlayerId::HOST), None);
        assert_eq!(seating.owner(SeatId(0)), Some(PlayerId::HOST));
        assert_eq!(seating.owner(SeatId(1)), Some(GUEST));
        assert_eq!(seating.owner(SeatId(2)), None, "the match holds two seats");
    }

    #[test]
    fn a_slot_that_is_not_closed_takes_the_seat_its_place_among_the_held_slots_names() {
        let mut lobby = Lobby::skirmish(PlayerId::HOST);
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 0,
                    control: Control::Closed,
                },
            )
            .expect("the host closes its own seat");
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 3,
                    control: Control::Bot(Bot::Turtle),
                },
            )
            .expect("the host adds a bot");

        let seats: Vec<Option<probe_sim::SeatId>> =
            (0..MAX_SLOTS + 1).map(|slot| lobby.seat_of(slot)).collect();

        assert_eq!(
            seats,
            [
                None,
                Some(probe_sim::SeatId(0)),
                None,
                Some(probe_sim::SeatId(1)),
                None
            ]
        );
        assert_eq!(
            lobby
                .freeze()
                .expect("two bots are a match")
                .setup()
                .teams()
                .len(),
            2,
            "the seats a freeze names are the slots that are not closed"
        );
    }

    #[test]
    fn regenerating_the_seed_never_repeats_a_seed_it_has_walked() {
        let mut lobby = Lobby::skirmish(PlayerId::HOST);
        let mut walked = vec![lobby.seed()];

        for _ in 0..MAX_SLOTS * 8 {
            lobby
                .edit(PlayerId::HOST, LobbyEdit::SetSeed(lobby.next_seed()))
                .expect("the host sets the seed");
            walked.push(lobby.seed());
        }

        let mut sorted = walked.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), walked.len());
    }
}
