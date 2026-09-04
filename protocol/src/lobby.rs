use core::ops::RangeInclusive;

use neumannarch_sim::{MAX_SEATS, SeatId, Setup, TICKS_PER_SECOND, TeamId, Tick};
use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use crate::seating::{Occupant, Seating, Started};

pub const CLOCK_RANGE: RangeInclusive<Tick> =
    Tick(60 * TICKS_PER_SECOND as u64)..=Tick(30 * 60 * TICKS_PER_SECOND as u64);

pub const DEFAULT_CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

pub const DEFAULT_SEED: u64 = 1;

pub const MAX_SLOTS: usize = MAX_SEATS;

const SEED_STEP: u64 = 0x9E37_79B9_7F4A_7C15;

const _: () = assert!(MAX_SLOTS == MAX_SEATS);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Bot {
    Turtle,
    Expand,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Holder {
    Open,
    Closed,
    Player { player: PlayerId, ready: bool },
    Bot(Bot),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Refused {
    NotHost,
    NotYours,
    NotSeated,
    NoSuchSlot,
    BadTeam,
    BadClock,
    AlreadySeated,
    NotAGuest,
    HeldByAGuest,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum NotReady {
    NoSeats,
    OpenSeat { slot: usize },
    Unready { slot: usize },
    HostUnseated,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum LobbyEdit {
    SetSlot { slot: usize, holder: Holder },
    Kick(PlayerId),
    SetTeam { slot: usize, team: TeamId },
    SetSeed(u64),
    SetClock(Tick),
    SetReady { ready: bool },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct SeatSlot {
    pub team: TeamId,
    pub holder: Holder,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Lobby {
    slots: Vec<SeatSlot>,
    seed: u64,
    clock: Tick,
    host: PlayerId,
}

impl Lobby {
    pub fn skirmish(host: PlayerId) -> Lobby {
        let mut slots = closed_slots();
        slots[0].holder = Holder::Player {
            player: host,
            ready: true,
        };
        slots[1].holder = Holder::Bot(Bot::Expand);
        Lobby {
            slots,
            seed: DEFAULT_SEED,
            clock: DEFAULT_CLOCK,
            host,
        }
    }

    pub fn room(host: PlayerId) -> Lobby {
        let mut lobby = Lobby::skirmish(host);
        lobby.slots[1].holder = Holder::Open;
        lobby
    }

    pub fn slots(&self) -> &[SeatSlot] {
        &self.slots
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn clock(&self) -> Tick {
        self.clock
    }

    pub fn host(&self) -> PlayerId {
        self.host
    }

    pub fn slot_of(&self, player: PlayerId) -> Option<usize> {
        self.slots.iter().position(|slot| slot.holds(player))
    }

    pub fn seat_of(&self, slot: usize) -> Option<SeatId> {
        self.numbered()
            .find(|(_, at, _)| *at == slot)
            .map(|(seat, _, _)| seat)
    }

    pub fn players(&self) -> Vec<PlayerId> {
        self.seating().players()
    }

    pub fn readied(&self, player: PlayerId) -> bool {
        self.slot_of(player)
            .map(|slot| self.slots[slot].holder)
            .is_some_and(|holder| matches!(holder, Holder::Player { ready: true, .. }))
    }

    pub fn release(&mut self, who: PlayerId) -> bool {
        let Some(slot) = self.slot_of(who).filter(|_| who != self.host) else {
            return false;
        };
        self.slots[slot].holder = Holder::Open;
        true
    }

    pub fn admit(&mut self, player: PlayerId) -> bool {
        if self.slot_of(player).is_some() {
            return true;
        }
        let Some(open) = self
            .slots
            .iter()
            .position(|slot| slot.holder == Holder::Open)
        else {
            return false;
        };
        self.slots[open].holder = Holder::Player {
            player,
            ready: false,
        };
        true
    }

    pub fn edit(&mut self, by: PlayerId, edit: LobbyEdit) -> Result<(), Refused> {
        match edit {
            LobbyEdit::SetSlot { slot, holder } => {
                self.as_host(by)?;
                let held = self.slots.get(slot).ok_or(Refused::NoSuchSlot)?.holder;
                if let Holder::Player { player: guest, .. } = held
                    && guest != self.host
                    && !matches!(holder, Holder::Player { player, .. } if player == guest)
                {
                    return Err(Refused::HeldByAGuest);
                }
                if let Holder::Player { player, .. } = holder
                    && self.slot_of(player).is_some_and(|held| held != slot)
                {
                    return Err(Refused::AlreadySeated);
                }
                self.slots[slot].holder = holder;
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
                self.slots[slot].holder = Holder::Player { player: by, ready };
            }
        }
        Ok(())
    }

    pub fn next_seed(&self) -> u64 {
        self.seed.wrapping_add(SEED_STEP)
    }

    pub fn freeze(&self) -> Result<Started, NotReady> {
        for (at, slot) in self.slots.iter().enumerate() {
            match slot.holder {
                Holder::Open => return Err(NotReady::OpenSeat { slot: at }),
                Holder::Player { player, ready } if !ready && player != self.host => {
                    return Err(NotReady::Unready { slot: at });
                }
                Holder::Closed | Holder::Player { .. } | Holder::Bot(_) => {}
            }
        }
        let seating = self.seating();
        if seating.players().is_empty() {
            return Err(NotReady::NoSeats);
        }
        if seating.run_by(self.host).is_none() {
            return Err(NotReady::HostUnseated);
        }

        let setup = Setup::new(self.teams_seated(), self.seed, self.clock)
            .expect("held slots are a match's seats");
        Ok(Started::new(setup, seating).expect("one seat per slot that holds one"))
    }

    fn numbered(&self) -> impl Iterator<Item = (SeatId, usize, &SeatSlot)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| !matches!(slot.holder, Holder::Closed))
            .zip(0u8..)
            .map(|((at, slot), seat)| (SeatId(seat), at, slot))
    }

    fn seating(&self) -> Seating {
        Seating::new(
            self.slots
                .iter()
                .filter_map(|slot| slot.holder.occupant())
                .collect(),
            self.host,
        )
    }

    fn teams_seated(&self) -> Vec<TeamId> {
        self.numbered().map(|(_, _, slot)| slot.team).collect()
    }

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

impl Holder {
    fn occupant(self) -> Option<Occupant> {
        match self {
            Holder::Player { player, .. } => Some(Occupant::Player(player)),
            Holder::Bot(bot) => Some(Occupant::Bot(bot)),
            Holder::Open | Holder::Closed => None,
        }
    }
}

impl SeatSlot {
    pub fn holds(&self, player: PlayerId) -> bool {
        matches!(self.holder, Holder::Player { player: held, .. } if held == player)
    }
}

fn closed_slots() -> Vec<SeatSlot> {
    (0..MAX_SLOTS)
        .map(|at| SeatSlot {
            team: TeamId(at as u8),
            holder: Holder::Closed,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUEST: PlayerId = PlayerId(7);

    fn joined() -> Lobby {
        let mut lobby = Lobby::skirmish(PlayerId::HOST);
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 2,
                    holder: Holder::Player {
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
                holder: Holder::Open,
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

        assert_eq!(lobby.slots()[3].holder, Holder::Open);
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
            lobby.slots()[2].holder,
            Holder::Player {
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
                    holder: Holder::Open
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
                    holder: Holder::Open
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

        assert_eq!(lobby.slots()[2].holder, Holder::Open);
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
                    holder: Holder::Player {
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
                    holder: Holder::Player {
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
                    holder: Holder::Open,
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
                        holder: Holder::Closed,
                    },
                )
                .expect("the host closes a seat");
        }
        assert_eq!(lobby.freeze(), Err(NotReady::NoSeats));
    }

    #[test]
    fn a_guests_slot_is_opened_by_a_kick_and_by_no_other_edit() {
        let mut lobby = joined();

        for holder in [
            Holder::Open,
            Holder::Closed,
            Holder::Bot(Bot::Turtle),
            Holder::Player {
                player: PlayerId::HOST,
                ready: true,
            },
        ] {
            assert_eq!(
                lobby.edit(PlayerId::HOST, LobbyEdit::SetSlot { slot: 2, holder }),
                Err(Refused::HeldByAGuest),
                "{holder:?}"
            );
        }

        assert_eq!(lobby, joined(), "a refused edit changes nothing");
        assert_eq!(lobby.edit(PlayerId::HOST, LobbyEdit::Kick(GUEST)), Ok(()));
        assert_eq!(lobby.slots()[2].holder, Holder::Open);
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
                        holder: Holder::Closed,
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
                    holder: Holder::Bot(Bot::Turtle),
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
                    holder: Holder::Closed,
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
                    holder: Holder::Closed,
                },
            )
            .expect("the host closes its own seat");
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 3,
                    holder: Holder::Bot(Bot::Turtle),
                },
            )
            .expect("the host adds a bot");

        let seats: Vec<Option<neumannarch_sim::SeatId>> =
            (0..MAX_SLOTS + 1).map(|slot| lobby.seat_of(slot)).collect();

        assert_eq!(
            seats,
            [
                None,
                Some(neumannarch_sim::SeatId(0)),
                None,
                Some(neumannarch_sim::SeatId(1)),
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
