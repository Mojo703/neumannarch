#![cfg(feature = "host")]

use std::collections::BTreeMap;
use std::net::SocketAddr;

use neumannarch_agents::{Seated, Shipped};
use neumannarch_game::net::connection::Connection;
use neumannarch_game::net::listener::Listener;
use neumannarch_game::net::machine::Machine;
use neumannarch_game::net::transport::Transport;
use neumannarch_protocol::{
    Bot, CLOCK_RANGE, Lobby, LobbyEdit, Notice, PlayerId, Relayed, Request, Started,
};
use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::state::{Command, Issued};
use neumannarch_sim::{AsteroidId, SeatId, Stamped, Tick};

const TICKS: u64 = 360;

const WITHHELD: u64 = 24;

const PATIENCE: usize = 100_000;

struct Delaying<'a> {
    inner: &'a mut dyn Transport,
    holding: Vec<(Tick, Relayed)>,
    at: Tick,
    withheld: u64,
}

impl Transport for Delaying<'_> {
    fn acknowledge(&mut self, seat: SeatId, up_to: Tick) {
        self.inner.acknowledge(seat, up_to);
    }

    fn received(&mut self) -> Vec<Relayed> {
        let at = self.at;
        let received = self.inner.received();
        if self.withheld == 0 {
            return received;
        }
        self.holding
            .extend(received.into_iter().map(|message| (at, message)));
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

type Hashes = BTreeMap<Tick, u64>;

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

fn notices_until(room: &mut Connection, ready: impl Fn(&[Notice]) -> bool) -> Vec<Notice> {
    let mut notices = Vec::new();
    for _ in 0..PATIENCE {
        notices.extend(room.notices());
        if ready(&notices) {
            return notices;
        }
        assert!(!room.closed(), "the room closed: {notices:?}");
        std::thread::yield_now();
    }
    panic!("the room never answered: {notices:?}");
}

fn started(notices: &[Notice]) -> Option<Started> {
    notices.iter().find_map(|notice| match notice {
        Notice::Started(started) => Some(started.clone()),
        _ => None,
    })
}

fn played(withheld: u64) -> (Hashes, Hashes, usize) {
    let hosted = Listener::serving(SocketAddr::from(([127, 0, 0, 1], 0)))
        .expect("a room binds on the loopback");
    let mut host = Connection::joining(&hosted.address());
    let me = welcomed(&notices_until(&mut host, |notices| {
        welcomed(notices).is_some()
    }))
    .expect("the room welcomed the machine that opened it");
    let mut guest = Connection::joining(&hosted.address());
    let other = welcomed(&notices_until(&mut guest, |notices| {
        welcomed(notices).is_some()
    }))
    .expect("the room welcomed the machine that joined it");
    assert_eq!((me, other), (PlayerId::HOST, PlayerId(1)));

    guest.request(Request::Edit(LobbyEdit::SetReady { ready: true }));
    let lobby = notices_until(&mut host, |notices| {
        notices.iter().any(|message| ready_lobby(message).is_some())
    })
    .iter()
    .rev()
    .find_map(ready_lobby)
    .expect("the room sent the readied lobby");
    lobby.freeze().expect("both seats are held and ready");
    host.request(Request::Start);

    let frozen = started(&notices_until(&mut host, |notices| {
        started(notices).is_some()
    }))
    .expect("the room started the match");
    notices_until(&mut guest, |notices| started(notices).is_some());

    let mine = frozen.seating().run_by(me).expect("the host holds a seat");
    let theirs = frozen
        .seating()
        .run_by(other)
        .expect("the guest holds a seat");
    let mut playing = Machine::of(frozen.clone(), &mine, &mut host);
    let mut joined = Machine::of(frozen, &theirs, &mut guest);
    for _ in 0..PATIENCE {
        joined.receive(&mut guest);
        playing.receive(&mut host);
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
        inner: &mut guest,
        holding: Vec::new(),
        at: Tick::ZERO,
        withheld,
    };
    let mut rewinds = 0;
    while playing.session().state().tick().0 < TICKS {
        decide(&mut playing, &mut agents[0]);
        decide(&mut joined, &mut agents[1]);
        advance(&mut playing, &mut host, &mut theirs);
        delaying.at = joined.session().state().tick();
        rewinds += usize::from(advance(&mut joined, &mut delaying, &mut ours));
        std::thread::yield_now();
    }

    for _ in 0..PATIENCE {
        delaying.at = joined.session().state().tick();
        advance(&mut joined, &mut delaying, &mut ours);
        advance(&mut playing, &mut host, &mut theirs);
        if joined.session().state().tick().0 >= TICKS {
            break;
        }
        std::thread::yield_now();
    }
    (theirs, ours, rewinds)
}

fn welcomed(notices: &[Notice]) -> Option<PlayerId> {
    notices.iter().find_map(|notice| match notice {
        Notice::Welcome { player, .. } => Some(*player),
        _ => None,
    })
}

fn ready_lobby(notice: &Notice) -> Option<Lobby> {
    match notice {
        Notice::Lobby(lobby) if lobby.freeze().is_ok() => Some(lobby.clone()),
        _ => None,
    }
}

fn decide(machine: &mut Machine, agent: &mut Seated) {
    let commands = agent.issue(machine.session());
    let Some(human) = machine.human() else {
        return;
    };
    for stamped in commands {
        human.want(stamped.issued.command);
    }
}

fn seated(seat: SeatId, bot: Bot) -> Seated {
    Seated::new(seat, Shipped::of(bot).seated())
}

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

fn sent() -> Stamped {
    Stamped {
        tick: Tick::ZERO,
        issued: Issued {
            seat: SeatId(0),
            seq: 0,
            command: Command::Want {
                asteroid: AsteroidId(0),
                pattern: EntityPattern::Shipyard,
                count: 1,
            },
        },
    }
}

#[test]
fn a_match_message_that_arrives_while_a_screen_reads_the_room_reaches_the_machine() {
    let hosted = Listener::serving(SocketAddr::from(([127, 0, 0, 1], 0)))
        .expect("a room binds on the loopback");
    let mut host = Connection::joining(&hosted.address());
    notices_until(&mut host, |notices| welcomed(notices).is_some());
    let mut guest = Connection::joining(&hosted.address());
    notices_until(&mut guest, |notices| welcomed(notices).is_some());

    guest.request(Request::Edit(LobbyEdit::SetReady { ready: true }));
    notices_until(&mut host, |notices| {
        notices.iter().any(|notice| ready_lobby(notice).is_some())
    });
    host.request(Request::Start);
    notices_until(&mut host, |notices| started(notices).is_some());
    notices_until(&mut guest, |notices| started(notices).is_some());

    host.send(sent());

    let mut relayed = Vec::new();
    for _ in 0..PATIENCE {
        guest.notices();
        relayed.extend(guest.received());
        if !relayed.is_empty() {
            break;
        }
        assert!(!guest.closed(), "the room closed: {relayed:?}");
        std::thread::yield_now();
    }
    assert_eq!(relayed, vec![Relayed::Command(sent())]);
}
