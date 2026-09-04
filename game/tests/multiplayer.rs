//! Two machines of one match in one process, over real sockets to a room
//! this test serves, each seat played by a scripted agent.

#![cfg(feature = "host")]

use std::collections::BTreeMap;
use std::net::SocketAddr;

use probe_agents::{Personality, Scripted, Seated};
use probe_game::net::hosting::Hosting;
use probe_game::net::machine::Machine;
use probe_game::net::room::{Room, Word};
use probe_game::net::transport::Transport;
use probe_protocol::{Bot, CLOCK_RANGE, Lobby, LobbyEdit, Message, PlayerId, Started};
use probe_sim::{SeatId, Stamped, Tick};

/// How long the two machines play, in ticks: three seconds, which is
/// several acknowledgement and report cadences and more than one settling
/// window.
const TICKS: u64 = 360;

/// How long a withheld command is kept from the machine it was sent to, in
/// ticks: a fifth of a second, well inside the retention window and well
/// past the tick it was stamped at.
const WITHHELD: u64 = 24;

/// How many rounds of yielding the test waits on the room or the other
/// machine before it gives up.
const PATIENCE: usize = 100_000;

/// One machine's transport, holding everything it hears back by `withheld`
/// ticks so the commands in it are learned late.
///
/// It holds messages in the order they arrived, as a slow link does: a
/// seat's acknowledgement never overtakes the commands it covers.
struct Delaying<'a> {
    inner: &'a mut dyn Transport,
    /// Messages not yet handed over, with the tick they were heard at.
    holding: Vec<(Tick, Message)>,
    /// The tick the machine reading it is at.
    at: Tick,
    withheld: u64,
}

impl Transport for Delaying<'_> {
    fn acknowledge(&mut self, seat: SeatId, up_to: Tick) {
        self.inner.acknowledge(seat, up_to);
    }

    fn leave(&mut self) {
        self.inner.leave();
    }

    fn received(&mut self) -> Vec<Message> {
        let at = self.at;
        let heard = self.inner.received();
        if self.withheld == 0 {
            return heard;
        }
        self.holding
            .extend(heard.into_iter().map(|message| (at, message)));
        let due = at.back(self.withheld as u32);
        let ready = self.holding.partition_point(|(held, _)| *held <= due);
        self.holding
            .drain(..ready)
            .map(|(_, message)| message)
            .collect()
    }

    fn report(&mut self, tick: Tick, hash: u64) {
        self.inner.report(tick, hash);
    }

    fn send(&mut self, stamped: Stamped) {
        self.inner.send(stamped);
    }
}

/// The hash each machine held at every settled tick it passed.
type Hashes = BTreeMap<Tick, u64>;

/// Runs one machine's tick and records the hash of every tick that settled,
/// which is the tick machines are compared at.
fn advance(machine: &mut Machine, transport: &mut dyn Transport, hashes: &mut Hashes) -> bool {
    let before = machine.session().settled();
    let ticked = machine.tick(transport);
    let settled = machine.session().settled();
    for tick in before.0..=settled.0 {
        if let Some(hash) = machine.session().hash_at(Tick(tick)) {
            hashes.insert(Tick(tick), hash);
        }
    }
    ticked.rewound
}

/// Reads `room` until what it has said holds `ready`, and answers
/// everything it said. Panics on a room that never answers.
fn heard_until(room: &mut Room, ready: impl Fn(&[Word]) -> bool) -> Vec<Word> {
    let mut heard = Vec::new();
    for _ in 0..PATIENCE {
        heard.extend(room.heard());
        if ready(&heard) {
            return heard;
        }
        assert!(!room.closed(), "the room closed: {heard:?}");
        std::thread::yield_now();
    }
    panic!("the room never answered: {heard:?}");
}

/// The match a word starts, where one of them is a start.
fn started(heard: &[Word]) -> Option<Started> {
    heard.iter().find_map(|word| match word {
        Word::Started(started) => Some(started.clone()),
        _ => None,
    })
}

/// Plays a match of two machines over a room this test serves, holding the
/// guest's incoming commands back by `withheld` ticks.
///
/// Answers each machine's hash at every settled tick, and how many ticks
/// the guest rewound.
fn played(withheld: u64) -> (Hashes, Hashes, usize) {
    let hosted = Hosting::serving(SocketAddr::from(([127, 0, 0, 1], 0)))
        .expect("a room binds on the loopback");
    let mut host = Room::joining(&hosted.address());
    let me = welcomed(&heard_until(&mut host, |heard| welcomed(heard).is_some()))
        .expect("the room welcomed the machine that opened it");
    let mut guest = Room::joining(&hosted.address());
    let other = welcomed(&heard_until(&mut guest, |heard| welcomed(heard).is_some()))
        .expect("the room welcomed the machine that joined it");
    assert_eq!((me, other), (PlayerId::HOST, PlayerId(1)));

    guest.say(Message::Edit(LobbyEdit::SetReady { ready: true }));
    let lobby = heard_until(&mut host, |heard| {
        heard.iter().any(|message| ready_lobby(message).is_some())
    })
    .iter()
    .rev()
    .find_map(ready_lobby)
    .expect("the room sent the readied lobby");
    host.say(Message::Start(
        lobby.freeze().expect("both seats are held and ready"),
    ));

    // The room is the authority: both machines play the match it sent.
    let frozen = started(&heard_until(&mut host, |heard| started(heard).is_some()))
        .expect("the room started the match");
    heard_until(&mut guest, |heard| started(heard).is_some());

    let mine = frozen.seating().run_by(me).expect("the host holds a seat");
    let theirs = frozen
        .seating()
        .run_by(other)
        .expect("the guest holds a seat");
    let mut playing = Machine::of(frozen.clone(), &mine, host.transport());
    let mut joined = Machine::of(frozen, &theirs, guest.transport());
    for _ in 0..PATIENCE {
        joined.listen(guest.transport());
        playing.listen(host.transport());
        if playing.agreed() && joined.agreed() {
            break;
        }
        std::thread::yield_now();
    }
    assert!(
        playing.agreed() && joined.agreed(),
        "the machines never agreed the first hash"
    );

    let mut agents = [
        seated(SeatId(0), Bot::Expand),
        seated(SeatId(1), Bot::Turtle),
    ];
    let (mut theirs, mut ours) = (Hashes::new(), Hashes::new());
    let mut delaying = Delaying {
        inner: guest.transport(),
        holding: Vec::new(),
        at: Tick::ZERO,
        withheld,
    };
    let mut rewinds = 0;
    while playing.session().state().tick().0 < TICKS {
        decide(&mut playing, &mut agents[0]);
        decide(&mut joined, &mut agents[1]);
        advance(&mut playing, host.transport(), &mut theirs);
        delaying.at = joined.session().state().tick();
        rewinds += usize::from(advance(&mut joined, &mut delaying, &mut ours));
        std::thread::yield_now();
    }
    // The guest is behind by whatever the sockets and the hold cost, so the
    // ticks it has yet to settle are not yet its answer.
    for _ in 0..PATIENCE {
        delaying.at = joined.session().state().tick();
        advance(&mut joined, &mut delaying, &mut ours);
        advance(&mut playing, host.transport(), &mut theirs);
        if joined.session().state().tick().0 >= TICKS {
            break;
        }
        std::thread::yield_now();
    }
    (theirs, ours, rewinds)
}

/// The id a word carries, where one of them is a welcome.
fn welcomed(heard: &[Word]) -> Option<PlayerId> {
    heard.iter().find_map(|word| match word {
        Word::Welcome { player, .. } => Some(*player),
        _ => None,
    })
}

/// The lobby a word carries, where every seat of it is ready.
fn ready_lobby(word: &Word) -> Option<Lobby> {
    match word {
        Word::Lobby(lobby) if lobby.freeze().is_ok() => Some(lobby.clone()),
        _ => None,
    }
}

/// Asks `agent` for this tick's commands and gives them to the machine's
/// own seat, which is what a person's hands do.
fn decide(machine: &mut Machine, agent: &mut Seated) {
    let commands = agent.issue(machine.session());
    let Some(human) = machine.human() else {
        return;
    };
    for stamped in commands {
        human.want(stamped.issued.command);
    }
}

/// A scripted agent playing `seat` the way `bot` plays.
fn seated(seat: SeatId, bot: Bot) -> Seated {
    Seated::new(
        seat,
        Box::new(Scripted::new(
            Personality::of(bot),
            probe_sim::roster::Roster::shipped(),
        )),
    )
}

/// Every tick both machines settled, and the hash each held there.
fn agreed(theirs: &Hashes, ours: &Hashes) -> Vec<Tick> {
    let both: Vec<Tick> = theirs
        .keys()
        .filter(|tick| ours.contains_key(tick))
        .copied()
        .collect();
    for tick in &both {
        assert_eq!(
            theirs.get(tick),
            ours.get(tick),
            "the machines differ at settled tick {}",
            tick.0
        );
    }
    both
}

#[test]
fn two_machines_of_one_match_over_a_room_hold_the_same_state_at_every_settled_tick() {
    let (theirs, ours, _) = played(0);

    let both = agreed(&theirs, &ours);

    assert!(
        both.len() > 1,
        "only {} ticks settled on both machines, so nothing was compared",
        both.len()
    );
    assert!(
        both.iter().any(|tick| tick.0 > CLOCK_RANGE.start().0 / 60),
        "the machines settled nothing past the first second"
    );
}

#[test]
fn a_machine_that_learns_a_command_late_reaches_the_history_the_others_have() {
    let (theirs, ours, rewinds) = played(WITHHELD);

    let both = agreed(&theirs, &ours);

    assert!(
        rewinds > 0,
        "no command was learned late, so nothing rewound"
    );
    assert!(
        both.len() > 1,
        "only {} ticks settled on both machines, so nothing was compared",
        both.len()
    );
}
