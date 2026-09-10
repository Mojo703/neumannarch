use std::collections::BTreeMap;
use std::time::Instant;

use neumannarch_agents::{
    Guarantees, Mix, Personality, PlayedMatch, Scripted, Seated, Shipped, free_for_all, minutes,
};
use neumannarch_protocol::{Bot, Record};
use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::state::State;
use neumannarch_sim::state::standings::Standings;
use neumannarch_sim::{
    Retention, SeatId, Session, Setup, Stamped, TICKS_PER_SECOND, TeamId, Tick, Time,
    WINDOW_SECONDS,
};

const CLOCK: Time = Time(15 * 60 * TICKS_PER_SECOND as u64);

const GROWTH_CLOCK: Time = Time(15 * 60 * TICKS_PER_SECOND as u64);

const MATRIX_CLOCK: Time = Time(5 * 60 * TICKS_PER_SECOND as u64);

const CHECK_CLOCK: Time = Time(60 * TICKS_PER_SECOND as u64);

const TRACE_INTERVAL: u64 = 60;

const SEATS: [SeatId; 2] = [SeatId(0), SeatId(1)];

const SEED: u64 = 1;

const MIRRORS: u64 = 8;

const SETTLED: Time = Time(90 * TICKS_PER_SECOND as u64);

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
        ["verify"] => verified(CLOCK),
        ["verify", over] => match over.parse::<u64>() {
            Ok(over) => verified(minutes(over)),
            Err(_) => {
                println!("verify takes a whole number of minutes");
                false
            }
        },
        ["time"] => {
            timed(CLOCK);
            true
        }
        ["time", over] => match over.parse::<u64>() {
            Ok(over) => {
                timed(minutes(over));
                true
            }
            Err(_) => {
                println!("time takes a whole number of minutes");
                false
            }
        },
        ["matrix"] => {
            matrix();
            true
        }
        ["draft"] => {
            drafts();
            true
        }
        _ => {
            println!(
                "usage: harness match [turtle|expand|none] .. up to four\n       harness verify [minutes]\n       harness time [minutes]\n       harness replay\n       harness rollback\n       harness matrix\n       harness draft"
            );
            true
        }
    };
    if !passed {
        std::process::exit(1);
    }
}

fn played(named: &[&str]) {
    let sides: Vec<&str> = match named.is_empty() {
        true => vec!["turtle", "none"],
        false => named.to_vec(),
    };
    let Some(seated) = seats(&sides) else {
        println!("unknown personality: one of turtle, expand, none");
        return;
    };
    let mut run = PlayedMatch::new(setup(CLOCK, SEED, sides.len().max(SEATS.len())), seated);
    println!("match {}", sides.join(" vs "));
    while !run.over() {
        run.advance();
        let tick = run.state().tick();
        if tick
            .0
            .is_multiple_of(TRACE_INTERVAL * u64::from(TICKS_PER_SECOND))
        {
            println!("  {}", line(run.state()));
        }
    }
    println!("  {}", line(run.state()));
    report(run.state());
}

fn timed(clock: Time) {
    let seated = seats(&["expand", "expand"]).expect("both personalities ship");
    let mut run = PlayedMatch::new(setup(clock, SEED, SEATS.len()), seated);
    println!(
        "expand against expand over {} minutes, milliseconds a tick",
        clock.seconds() / 60.0
    );
    let mut spans: Vec<f64> = Vec::new();
    while !run.over() {
        let started = Instant::now();
        run.advance();
        spans.push(started.elapsed().as_secs_f64() * 1e3);
        let tick = run.state().tick();
        if tick
            .0
            .is_multiple_of(TRACE_INTERVAL * u64::from(TICKS_PER_SECOND))
        {
            spans.sort_by(f64::total_cmp);
            println!(
                "  {:>3}m  {:>4} entities  p50 {:.2}  p95 {:.2}  max {:.2}",
                (tick.seconds() / 60.0) as u64,
                run.state().entities().count(),
                at_percentile(&spans, 0.5),
                at_percentile(&spans, 0.95),
                spans.last().copied().unwrap_or_default()
            );
            spans.clear();
        }
    }
}

fn at_percentile(sorted: &[f64], share: f64) -> f64 {
    let at = ((sorted.len() as f64 * share) as usize).min(sorted.len().saturating_sub(1));
    sorted.get(at).copied().unwrap_or_default()
}

fn verified(clock: Time) -> bool {
    println!(
        "guarantees over {} minutes, expand against expand",
        clock.seconds() / 60.0
    );
    let guarantees = Guarantees::over(&[Bot::Expand, Bot::Expand], clock);
    let trading = guarantees.both_sides_arm_and_trade_shots();
    held(
        "no unit a bot wants outlives a decision building at an asteroid where no builder of its seat stands",
        guarantees.every_unit_builds_where_a_builder_of_its_seat_stands(),
    ) & held(
        "two bots field units that do damage early and trade shots by the middle of the clock",
        trading.as_deref(),
    ) & held(
        "no bot sits at its capacity for a minute with builders idle while a pattern that does damage is affordable",
        guarantees.no_stockpile_sits_full_with_builders_idle(),
    ) & grew()
}

fn grew() -> bool {
    println!(
        "growth over {} minutes, expand against turtle",
        GROWTH_CLOCK.seconds() / 60.0
    );
    let guarantees = Guarantees::over(&[Bot::Expand, Bot::Turtle], GROWTH_CLOCK);
    held(
        "an expand bot never idles while free asteroids are within reach: it never stands a minute below the most it ever pulled and still has a free asteroid within reach at the end of it, and it holds half the asteroids within reach where a turtle keeps its drafted two",
        guarantees.an_expand_bot_grows_where_a_turtle_sits(),
    )
}

fn held(what: &str, breach: Option<&str>) -> bool {
    let passed = check(what, breach.is_none());
    if let Some(breach) = breach {
        println!("     {breach}");
    }
    passed
}

fn replayed() -> bool {
    let mut run = played_over(CHECK_CLOCK, both());
    while !run.over() {
        run.advance();
    }
    let live = run.state();
    let settled = run.session().settled();
    let record = Record::of(run.session());
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
        Some(replay.hash()) == run.session().hash_at(settled),
    );
    passed &= check("the record reproduces the whole state", &replay == live);

    let mut again = played_over(CHECK_CLOCK, both());
    while !again.over() {
        again.advance();
    }
    println!("repeat hash  {:#018x}", again.state().hash());
    passed &= check(
        "the same match twice reproduces the hash",
        again.state().hash() == live.hash(),
    );
    println!("{}", line(live));
    passed
}

fn rolled_back() -> bool {
    let mut run = played_over(CHECK_CLOCK, both());
    let mut hashes = vec![run.state().hash()];
    let mut issued: Vec<Stamped> = Vec::new();
    while !run.over() {
        issued.extend(run.advance());
        hashes.push(run.state().hash());
    }
    let end = run.state().tick();
    println!(
        "{} commands over {} ticks, on time {:#018x}",
        issued.len(),
        end.0,
        hashes[end.0 as usize]
    );

    let mut late = Session::new(
        setup(CHECK_CLOCK, SEED, SEATS.len()),
        Retention::shipped(),
        &[],
    )
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
                Seated::new(SeatId(0), Box::new(Scripted::new(mine.clone()))),
                Seated::new(SeatId(1), Box::new(Scripted::new(yours.clone()))),
            ];
            let mut run = played_over(MATRIX_CLOCK, seats);
            while !run.over() {
                run.advance();
            }
            let standings = run.state().standings();
            let teams = standings.teams();
            let asteroids = |at: usize| teams.get(at).map_or(0, |team| team.asteroids);
            let verdict = match standings.leaders().as_slice() {
                [TeamId(0)] => "win",
                [TeamId(1)] => "loss",
                _ => "draw",
            };
            let _ = theirs;
            print!(
                "{:>12}",
                format!("{verdict} {}-{}", asteroids(0), asteroids(1))
            );
        }
        println!();
    }
}

fn drafts() {
    println!(
        "draft mirrors over {MIRRORS} seeds, {} minutes a match",
        MATRIX_CLOCK.seconds() / 60.0
    );
    let mut won = 0;
    let mut drawn = 0;
    let mut spread = 0;
    for seed in 0..MIRRORS {
        let mirrored = |seat| Seated::new(seat, Box::new(Scripted::new(Personality::expand())));
        let mut run = PlayedMatch::new(
            setup(MATRIX_CLOCK, seed, SEATS.len()),
            Vec::from(SEATS.map(mirrored)),
        );
        let mut settled = [0, 0];
        while !run.over() {
            run.advance();
            if run.state().time() == SETTLED {
                settled = standing(run.state());
            }
        }
        spread += usize::from(settled.iter().all(|asteroids| *asteroids >= 2));
        let state = run.state();
        let first = TeamId(state.draft().stages()[0].seat.0);
        let leaders = state.standings().leaders();
        let verdict = match leaders.as_slice() {
            [leader] if *leader == first => {
                won += 1;
                "the first picker".to_string()
            }
            [leader] => format!("team {}", leader.0),
            _ => {
                drawn += 1;
                "a draw".to_string()
            }
        };
        println!(
            "  seed {seed}: team {} picked first, {verdict} won; {} asteroids at 90s {}-{}",
            first.0,
            drafted(state),
            settled[0],
            settled[1]
        );
    }
    println!("  the first picker won {won} of {MIRRORS}, {drawn} drawn");
    println!("  both teams held two asteroids at ninety seconds in {spread} of {MIRRORS}");
}

fn standing(state: &State) -> [u32; 2] {
    let standings = state.standings();
    let asteroids = |at: usize| standings.teams().get(at).map_or(0, |team| team.asteroids);
    [asteroids(0), asteroids(1)]
}

fn drafted(state: &State) -> String {
    let seats: Vec<String> = SEATS
        .iter()
        .map(|seat| {
            let asteroids: Vec<String> = state
                .draft()
                .stages()
                .iter()
                .filter(|stage| stage.seat == *seat)
                .filter_map(|stage| stage.placed)
                .map(|asteroid| {
                    let caps = state[asteroid].caps();
                    format!("{} ({})", asteroid.0, caps.total() as u64)
                })
                .collect();
            format!("seat {} took {}", seat.0, asteroids.join(" and "))
        })
        .collect();
    seats.join(", ")
}

fn compositions() -> Vec<(&'static str, Personality)> {
    let pinned = |name: &'static str, mix: Vec<(EntityPattern, f64)>| {
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
        pinned("raiders", vec![(EntityPattern::Raider, 1.0)]),
        pinned("frigates", vec![(EntityPattern::Frigate, 1.0)]),
        pinned("lancers", vec![(EntityPattern::Lancer, 1.0)]),
        pinned(
            "mixed",
            vec![
                (EntityPattern::Raider, 1.0),
                (EntityPattern::Frigate, 1.0),
                (EntityPattern::Lancer, 1.0),
            ],
        ),
        ("counters", Personality::expand()),
    ]
}

fn setup(clock: Time, seed: u64, seats: usize) -> Setup {
    Setup::new(free_for_all(seats), seed, clock).expect("one seat per team is a match")
}

fn played_over(clock: Time, seated: Vec<Seated>) -> PlayedMatch {
    PlayedMatch::new(setup(clock, SEED, SEATS.len()), seated)
}

fn both() -> Vec<Seated> {
    seats(&["turtle", "expand"]).expect("both ship")
}

fn seats(named: &[&str]) -> Option<Vec<Seated>> {
    let mut seated = Vec::new();
    for (at, name) in named.iter().enumerate() {
        let seat = SeatId(u8::try_from(at).ok()?);
        if *name == "none" {
            continue;
        }
        let shipped = Shipped::named(name)?;
        seated.push(Seated::new(seat, shipped.seated()));
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
                "team {} {} asteroids, {} value, {entities} entities",
                team.team.0, team.asteroids, team.value as u64
            )
        })
        .collect();
    let flying = state.entities().filter(|entity| entity.is_flying()).count();
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
        let mut owned: BTreeMap<&str, u32> = BTreeMap::new();
        for entity in state
            .entities()
            .filter(|entity| state[entity.seat()].team() == team.team)
        {
            *owned.entry(entity.pattern().name()).or_default() += 1;
        }
        let owned: Vec<String> = owned
            .into_iter()
            .map(|(name, count)| format!("{count} {name}"))
            .collect();
        println!(
            "team {}: {} asteroids, {} army value, {}; {}",
            team.team.0,
            team.asteroids,
            team.value as u64,
            match team.alive {
                true => "in",
                false => "out",
            },
            owned.join(", ")
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
