use std::collections::BTreeMap;

use probe_agents::{Mix, Personality, Scripted, Seated};
use probe_protocol::Record;
use probe_sim::roster::{FRIGATE, LANCER, RAIDER, Roster};
use probe_sim::state::State;
use probe_sim::state::standings::Standings;
use probe_sim::{
    Retention, RowId, SeatId, Session, Setup, Stamped, TICKS_PER_SECOND, TeamId, Tick,
    WINDOW_SECONDS,
};

const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

const MATRIX_CLOCK: Tick = Tick(5 * 60 * TICKS_PER_SECOND as u64);

const CHECK_CLOCK: Tick = Tick(60 * TICKS_PER_SECOND as u64);

const TRACE_INTERVAL: u64 = 60;

const SEATS: [SeatId; 2] = [SeatId(0), SeatId(1)];

const SEED: u64 = 1;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let named: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let passed = match named.as_slice() {
        ["match", rest @ ..] => {
            played(rest);
            true
        }
        ["replay"] => replayed(),
        ["rollback"] => rolled_back(),
        ["matrix"] => {
            matrix();
            true
        }
        _ => {
            println!(
                "usage: harness match [turtle|expand|none] [turtle|expand|none]\n       harness replay\n       harness rollback\n       harness matrix"
            );
            true
        }
    };
    if !passed {
        std::process::exit(1);
    }
}

fn played(named: &[&str]) {
    let sides = [
        *named.first().unwrap_or(&"turtle"),
        *named.get(1).unwrap_or(&"none"),
    ];
    let Some(seats) = seats(&sides) else {
        println!("unknown personality: one of turtle, expand, none");
        return;
    };
    let mut run = Match::new(CLOCK, seats);
    println!("match {} vs {}", sides[0], sides[1]);
    while !run.over() {
        run.tick();
        let tick = run.session.state().tick();
        if tick
            .0
            .is_multiple_of(TRACE_INTERVAL * u64::from(TICKS_PER_SECOND))
        {
            println!("  {}", line(run.session.state()));
        }
    }
    println!("  {}", line(run.session.state()));
    report(run.session.state());
}

fn replayed() -> bool {
    let mut run = Match::new(CHECK_CLOCK, both());
    while !run.over() {
        run.tick();
    }
    let live = run.session.state();
    let settled = run.session.settled();
    let record = Record::of(&run.session);
    let replay = record.replay(settled);
    println!(
        "ticks {}, settled {}, {} commands",
        live.tick().0,
        settled.0,
        record.commands()
    );
    println!("live hash    {:#018x}", live.hash());
    println!("replay hash  {:#018x}", replay.hash());
    let mut passed = check(
        "every tick a machine of its own plays settles as it is played",
        settled == live.tick(),
    );
    passed &= check(
        "the record reproduces the live hash",
        Some(replay.hash()) == run.session.hash_at(settled),
    );
    passed &= check("the record reproduces the whole state", &replay == live);

    let mut again = Match::new(CHECK_CLOCK, both());
    while !again.over() {
        again.tick();
    }
    println!("repeat hash  {:#018x}", again.session.state().hash());
    passed &= check(
        "the same match twice reproduces the hash",
        again.session.state().hash() == live.hash(),
    );
    println!("{}", line(live));
    passed
}

fn rolled_back() -> bool {
    let mut run = Match::new(CHECK_CLOCK, both());
    let mut hashes = vec![run.session.state().hash()];
    let mut issued: Vec<Stamped> = Vec::new();
    while !run.over() {
        issued.extend(run.tick());
        hashes.push(run.session.state().hash());
    }
    let end = run.session.state().tick();
    println!(
        "{} commands over {} ticks, on time {:#018x}",
        issued.len(),
        end.0,
        hashes[end.0 as usize]
    );

    let mut late = Session::new(setup(CHECK_CLOCK), Retention::shipped(), &[])
        .expect("a session owning no seat seats nothing to refuse");
    let mut deliveries = scrambled(&issued);
    let mut pending = issued.clone();
    let mut refused = 0;
    let mut settled = Tick::ZERO;
    while late.state().tick() < end {
        for stamped in deliveries.remove(&late.state().tick()).unwrap_or_default() {
            if late.insert(stamped).is_err() {
                refused += 1;
            }
            pending.retain(|held| held != &stamped);
        }
        acknowledge(&mut late, &pending);

        settled = late.settled().min(late.state().tick());
        if late.hash_at(settled) != Some(hashes[settled.0 as usize]) {
            println!(
                "FAIL: the settled tick {} hashes {:#018x} against {:#018x} on time",
                settled.0,
                late.hash_at(settled).unwrap_or_default(),
                hashes[settled.0 as usize]
            );
            return false;
        }
        late.advance();
    }
    for stamped in pending.clone() {
        if late.insert(stamped).is_err() {
            refused += 1;
        }
    }
    acknowledge(&mut late, &[]);
    println!("settled to tick {} on the way", settled.0);
    check("every command inside the window was taken", refused == 0)
        & check(
            "every command learned late hashes the same at the end",
            late.hash_at(end) == Some(hashes[end.0 as usize]),
        )
        & check("the whole match settled", late.settled() == end)
}

fn scrambled(issued: &[Stamped]) -> BTreeMap<Tick, Vec<Stamped>> {
    let spread = WINDOW_SECONDS * TICKS_PER_SECOND / 2;
    let mut deliveries: BTreeMap<Tick, Vec<Stamped>> = BTreeMap::new();
    for stamped in issued {
        let late = 1 + (stamped.issued.seq + u32::from(stamped.issued.seat.0)) % spread;
        deliveries
            .entry(stamped.tick.ahead(late))
            .or_default()
            .push(*stamped);
    }
    for due in deliveries.values_mut() {
        due.reverse();
    }
    deliveries
}

fn acknowledge(session: &mut Session, pending: &[Stamped]) {
    let latest = session.state().tick();
    for seat in SEATS {
        let unknown = pending
            .iter()
            .filter(|stamped| stamped.issued.seat == seat)
            .map(|stamped| stamped.tick)
            .min();
        session.acknowledge(seat, unknown.unwrap_or(latest));
    }
}

fn matrix() {
    let compositions = compositions();
    println!("matrix, {} minutes a cell", MATRIX_CLOCK.seconds() / 60.0);
    print!("{:<10}", "");
    for (name, _) in &compositions {
        print!("{name:>12}");
    }
    println!();
    for (ours, mine) in &compositions {
        print!("{ours:<10}");
        for (theirs, yours) in &compositions {
            let seats = vec![
                Seated::new(SeatId(0), Box::new(Scripted::new(mine.clone(), roster()))),
                Seated::new(SeatId(1), Box::new(Scripted::new(yours.clone(), roster()))),
            ];
            let mut run = Match::new(MATRIX_CLOCK, seats);
            while !run.over() {
                run.tick();
            }
            let standings = run.session.state().standings();
            let teams = standings.teams();
            let rocks = |at: usize| teams.get(at).map_or(0, |team| team.rocks);
            let verdict = match standings.leaders().as_slice() {
                [TeamId(0)] => "win",
                [TeamId(1)] => "loss",
                _ => "draw",
            };
            let _ = theirs;
            print!("{:>12}", format!("{verdict} {}-{}", rocks(0), rocks(1)));
        }
        println!();
    }
}

fn compositions() -> Vec<(&'static str, Personality)> {
    let pinned = |name: &'static str, mix: Vec<(RowId, f64)>| {
        (
            name,
            Personality {
                name,
                mix: Mix::Pinned(mix),
                ..Personality::expand()
            },
        )
    };
    vec![
        pinned("raiders", vec![(RAIDER, 1.0)]),
        pinned("frigates", vec![(FRIGATE, 1.0)]),
        pinned("lancers", vec![(LANCER, 1.0)]),
        pinned("mixed", vec![(RAIDER, 1.0), (FRIGATE, 1.0), (LANCER, 1.0)]),
        ("counters", Personality::expand()),
    ]
}

struct Match {
    session: Session,
    seated: Vec<Seated>,
}

impl Match {
    fn new(clock: Tick, seated: Vec<Seated>) -> Match {
        Match {
            session: Session::new(setup(clock), Retention::shipped(), &SEATS)
                .expect("one seat per team seats both checks"),
            seated,
        }
    }

    fn tick(&mut self) -> Vec<Stamped> {
        let session = &self.session;
        let issued: Vec<Stamped> = self
            .seated
            .iter_mut()
            .flat_map(|seated| seated.issue(session))
            .collect();
        for stamped in &issued {
            let taken = self.session.insert(*stamped);
            assert!(
                taken.is_ok(),
                "an agent's own command was refused: {taken:?}"
            );
        }
        self.session.advance();
        issued
    }

    fn over(&self) -> bool {
        self.session.state().standings().over()
    }
}

fn setup(clock: Tick) -> Setup {
    let teams = SEATS.iter().map(|seat| TeamId(seat.0)).collect();
    Setup::new(teams, SEED, clock).expect("one seat per team is a match")
}

fn both() -> Vec<Seated> {
    seats(&["turtle", "expand"]).expect("both ship")
}

fn roster() -> Roster {
    Roster::shipped()
}

fn seats(named: &[&str]) -> Option<Vec<Seated>> {
    let mut seated = Vec::new();
    for (at, name) in named.iter().enumerate() {
        let seat = SeatId(u8::try_from(at).ok()?);
        if *name == "none" {
            continue;
        }
        let personality = Personality::named(name)?;
        seated.push(Seated::new(
            seat,
            Box::new(Scripted::new(personality, roster())),
        ));
    }
    Some(seated)
}

fn line(state: &State) -> String {
    let standings = state.standings();
    let sides: Vec<String> = standings
        .teams()
        .iter()
        .map(|team| {
            let entities = state
                .entities()
                .filter(|entity| state[entity.seat()].team() == team.team)
                .count();
            format!(
                "team {} {} rocks, {} value, {entities} entities",
                team.team.0, team.rocks, team.value as u64
            )
        })
        .collect();
    let flying = state
        .entities()
        .filter(|entity| entity.is_flying(state.tick()))
        .count();
    format!(
        "{:>4}s  {flying} flying, {} frames, {} posts  |  {}",
        state.tick().seconds() as u64,
        state.frames().len(),
        state.posts().count(),
        sides.join(" | ")
    )
}

fn report(state: &State) {
    let standings: Standings = state.standings();
    for team in standings.teams() {
        println!(
            "team {}: {} rocks, {} army value, {}",
            team.team.0,
            team.rocks,
            team.value as u64,
            match team.alive {
                true => "in",
                false => "out",
            }
        );
    }
    let leaders: Vec<String> = standings
        .leaders()
        .iter()
        .map(|team| team.0.to_string())
        .collect();
    println!("leaders: {}", leaders.join(", "));
}

fn check(what: &str, passed: bool) -> bool {
    println!("{} {what}", if passed { "ok  " } else { "FAIL" });
    passed
}
