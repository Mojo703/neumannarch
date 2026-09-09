use core::f64::consts::TAU;
use std::collections::BTreeMap;

use super::State;
use crate::belt::Belt;
use crate::ids::{AsteroidId, RowId, TeamId};
use crate::orbit::body::Body;
use crate::roster::Roster;
use crate::vec3::Vec3;

#[derive(Clone, Debug, PartialEq)]
pub struct FightStage {
    centre: Vec3,
    axis: Vec3,
    lateral: Vec3,
    normal: Vec3,
    stations: BTreeMap<(TeamId, RowId), Vec<Vec3>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub team: TeamId,
    pub row: RowId,
    pub standing: usize,
}

impl FightStage {
    const SIDES_FACING_ACROSS: usize = 2;

    pub fn of(body: Body, radius_meters: f64, roster: &Roster, lines: &[Line]) -> FightStage {
        let lateral = body.pos.normalized().unwrap_or(Vec3::ZERO);
        let axis = (body.vel - lateral * body.vel.dot(lateral))
            .normalized()
            .unwrap_or(Vec3::ZERO);
        let normal = lateral.cross(axis);
        let outward = 0.5 * roster.longest_damage_range();
        let lift = radius_meters + Belt::SPACING_METERS;
        let mut stage = FightStage {
            centre: body.pos + lateral * outward + normal * lift,
            axis,
            lateral,
            normal,
            stations: BTreeMap::new(),
        };
        let sides = Line::teams_holding_a_station(lines, roster);
        for line in lines {
            stage.place(line, roster, &sides);
        }
        stage
    }

    pub fn stations_of(&self, team: TeamId, row: RowId) -> &[Vec3] {
        self.stations
            .get(&(team, row))
            .map_or(&[], |stations| stations.as_slice())
    }

    fn place(&mut self, line: &Line, roster: &Roster, sides: &[TeamId]) {
        let Some(standoff) = roster[line.row].standoff() else {
            return;
        };
        let turn =
            TAU * sides.partition_point(|team| *team < line.team) as f64 / sides.len() as f64;
        let side = -self.axis * libm::cos(turn) + self.lateral * libm::sin(turn);
        let across = self.normal.cross(side);
        let in_a_rank = FightStage::stations_in_a_rank(standoff, sides.len());
        let on_the_stage = FightStage::stations_on_the_stage(standoff, sides.len());
        let stations = (0..line.standing).map(|place| {
            let taken = place % on_the_stage;
            let ranks_behind = (taken / in_a_rank) as f64;
            let out = standoff + ranks_behind * Belt::STATION_SPACING_METERS;
            let aside = FightStage::spread(taken % in_a_rank);
            self.centre + side * out + across * aside
        });
        self.stations
            .insert((line.team, line.row), stations.collect());
    }

    fn stations_each_way(standoff_meters: f64, sides: usize) -> usize {
        match sides > FightStage::SIDES_FACING_ACROSS {
            true => Belt::STATIONS_EACH_WAY
                .min((standoff_meters / Belt::STATION_SPACING_METERS) as usize),
            false => Belt::STATIONS_EACH_WAY,
        }
    }

    fn stations_in_a_rank(standoff_meters: f64, sides: usize) -> usize {
        2 * FightStage::stations_each_way(standoff_meters, sides) + 1
    }

    fn stations_on_the_stage(standoff_meters: f64, sides: usize) -> usize {
        FightStage::stations_in_a_rank(standoff_meters, sides) * (Belt::RANKS_BEHIND + 1)
    }

    fn spread(place: usize) -> f64 {
        let step = place.div_ceil(2) as f64 * Belt::STATION_SPACING_METERS;
        match place % 2 {
            1 => step,
            _ => -step,
        }
    }
}

impl Line {
    pub(crate) fn standing_at(state: &State, asteroid: AsteroidId) -> Vec<Line> {
        let mut standing: BTreeMap<(TeamId, RowId), usize> = BTreeMap::new();
        for entity in state.entities.standing_at(asteroid) {
            if entity.steered().is_none() {
                continue;
            }
            *standing
                .entry((state[entity.seat()].team(), entity.row()))
                .or_default() += 1;
        }
        standing
            .into_iter()
            .map(|((team, row), standing)| Line {
                team,
                row,
                standing,
            })
            .collect()
    }

    fn teams_holding_a_station(lines: &[Line], roster: &Roster) -> Vec<TeamId> {
        let mut teams: Vec<TeamId> = lines
            .iter()
            .filter(|line| roster[line.row].standoff().is_some())
            .map(|line| line.team)
            .collect();
        teams.sort_unstable();
        teams.dedup();
        teams
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::{AsteroidId, EntityId, SeatId};
    use crate::orbit::body::Gravity;
    use crate::roster::{CONSTRUCTOR, FRIGATE, Kind, LANCER, METALS_EXTRACTOR, RAIDER};
    use crate::state::Rolls;

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    fn world() -> World {
        World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)])
    }

    fn teamed(teams: u8) -> World {
        World::ring(GRAVITY, 2, &(0..teams).map(TeamId).collect::<Vec<TeamId>>())
    }

    fn staged_at(world: &World, asteroid: AsteroidId) -> FightStage {
        FightStage::of(
            world.state.asteroid_body(asteroid),
            world.state[asteroid].radius(),
            world.state.roster(),
            &Line::standing_at(&world.state, asteroid),
        )
    }

    fn staged(world: &World) -> FightStage {
        staged_at(world, HOME)
    }

    fn stationed_at(world: &World, asteroid: AsteroidId, force: &[EntityId]) -> Vec<Option<Vec3>> {
        let rolls = Rolls::called(&world.state);
        force
            .iter()
            .map(|one| rolls[asteroid].station(*one))
            .collect()
    }

    fn stationed(world: &World, force: &[EntityId]) -> Vec<Option<Vec3>> {
        stationed_at(world, HOME, force)
    }

    fn station(world: &World, unit: EntityId) -> Vec3 {
        stationed(world, &[unit])[0].expect("a unit that does damage is stationed")
    }

    fn floor(world: &World) -> f64 {
        world.state[HOME].floor_meters()
    }

    fn off_asteroid(world: &World, station: Vec3) -> f64 {
        station.distance(world.state.asteroid_body(HOME).pos)
    }

    #[test]
    fn the_stage_stands_out_along_the_radial_lifted_off_the_body_with_a_square_frame() {
        let mut world = world();
        world.hold(0, FRIGATE, HOME, 0.0);
        let asteroid = world.state.asteroid_body(HOME);

        let stage = staged(&world);

        let out = stage.centre - asteroid.pos;
        let half = 0.5 * world.state.roster().longest_damage_range();
        assert!(
            (out.dot(stage.lateral) - half).abs() < 1e-9,
            "the stage stands {} out along the radial, not {half}",
            out.dot(stage.lateral)
        );
        assert!(
            (out.dot(stage.normal) - floor(&world)).abs() < 1e-9,
            "the stage's plane stands {} off the body, not {}",
            out.dot(stage.normal),
            floor(&world)
        );
        assert!(
            out.dot(asteroid.pos) > 0.0,
            "the stage sits between the asteroid and the star"
        );
        assert!(
            stage.axis.dot(asteroid.vel) > 0.0,
            "the axis runs against the asteroid's motion"
        );
        assert!(
            (stage
                .lateral
                .distance(asteroid.pos.normalized().expect("a radius"))
                < 1e-12),
            "the lateral is not the radial"
        );
        for (one, other) in [
            (stage.axis, stage.lateral),
            (stage.lateral, stage.normal),
            (stage.normal, stage.axis),
        ] {
            assert!(one.dot(other).abs() < 1e-12, "{one:?} leans on {other:?}");
            assert!((one.length() - 1.0).abs() < 1e-12, "{one:?} is not a unit");
        }
    }

    #[test]
    fn the_frame_is_square_on_a_stirred_belt_asteroid() {
        let mut world = World::started(&[TeamId(0)]);
        let stirred = world
            .state
            .asteroids()
            .max_by(|one, other| {
                one.1
                    .orbit()
                    .eccentricity()
                    .total_cmp(&other.1.orbit().eccentricity())
            })
            .map(|(id, _)| id)
            .expect("the belt holds an asteroid");
        assert!(
            world.state[stirred].orbit().eccentricity() > 0.05,
            "the belt lays no eccentric orbit"
        );
        world.hold(0, LANCER, stirred, 0.0);

        let stage = staged_at(&world, stirred);

        assert!(
            stage.axis.dot(stage.lateral).abs() < 1e-12,
            "the axis leans {} on the lateral",
            stage.axis.dot(stage.lateral)
        );
        assert!((stage.axis.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn no_station_stands_on_the_asteroid_whatever_teams_stand_there() {
        for teams in 1..=4 {
            let mut world = teamed(teams);
            let force: Vec<EntityId> = (0..teams)
                .flat_map(|seat| {
                    [RAIDER, FRIGATE, LANCER]
                        .map(|row| world.hold(seat, row, HOME, f64::from(seat)))
                })
                .collect();
            let floor = floor(&world);

            let stations = stationed(&world, &force);

            for station in stations {
                let off = off_asteroid(
                    &world,
                    station.expect("a unit that does damage is stationed"),
                );
                assert!(
                    off >= floor - 1e-9,
                    "{teams} teams: a station stands {off} off an asteroid whose floor is {floor}"
                );
            }
        }
    }

    #[test]
    fn no_station_stands_outside_the_zone_however_long_a_row_grows() {
        for teams in 1..=4 {
            let seats: Vec<TeamId> = (0..teams).map(TeamId).collect();
            for (mut world, at) in [
                (World::ring(GRAVITY, 2, &seats), HOME),
                (World::started(&seats), AsteroidId(3)),
            ] {
                let standoff = world.state[LANCER]
                    .standoff()
                    .expect("a row that does damage");
                let deep = 3 * FightStage::stations_on_the_stage(standoff, usize::from(teams));
                let force: Vec<EntityId> = (0..teams)
                    .flat_map(|seat| {
                        (0..deep)
                            .map(|out| world.hold(seat, LANCER, at, out as f64 * 0.01))
                            .collect::<Vec<EntityId>>()
                    })
                    .collect();
                let radius = world.state[at].radius();

                let stations = stationed_at(&world, at, &force);

                for station in stations {
                    let station = station.expect("a unit that does damage is stationed");
                    let off = station.distance(world.state.asteroid_body(at).pos);
                    assert!(
                        off < Belt::ZONE_RADIUS_METERS,
                        "{teams} teams at an asteroid of {radius}: a station stands {off} out, past the zone"
                    );
                }
            }
        }
    }

    #[test]
    fn the_teams_standing_there_divide_the_circle_about_the_stage_in_team_order() {
        for teams in 1..=4 {
            let mut world = teamed(teams);
            let force: Vec<EntityId> = (0..teams)
                .map(|seat| world.hold(seat, FRIGATE, HOME, f64::from(seat)))
                .collect();

            let stage = staged(&world);

            let sides: Vec<Vec3> = stationed(&world, &force)
                .into_iter()
                .map(|station| {
                    (station.expect("a station") - stage.centre)
                        .normalized()
                        .expect("a side")
                })
                .collect();
            assert!(
                sides[0].distance(-stage.axis) < 1e-9,
                "the lowest team stands {:?}, not retrograde",
                sides[0]
            );
            for (at, side) in sides.iter().enumerate() {
                let turn = TAU * at as f64 / f64::from(teams);
                let wanted = -stage.axis * libm::cos(turn) + stage.lateral * libm::sin(turn);
                assert!(
                    side.distance(wanted) < 1e-9,
                    "{teams} teams: team {at} stands {side:?}, not {wanted:?}"
                );
            }
        }
    }

    #[test]
    fn a_team_that_holds_no_station_takes_no_side() {
        let mut world = teamed(3);
        world.hold(1, FRIGATE, HOME, 0.0);
        let alone = world.hold(2, FRIGATE, HOME, 1.0);

        let stage = staged(&world);

        let side = (station(&world, alone) - stage.centre)
            .normalized()
            .expect("a side");
        assert!(
            side.distance(stage.axis) < 1e-9,
            "the second of two teams present stands {side:?}, not prograde"
        );
    }

    #[test]
    fn a_station_stands_at_the_rows_standoff_from_the_centre_on_its_sides_side() {
        let mut world = world();
        let unit = world.hold(0, LANCER, HOME, 0.0);
        let standoff = world.state[LANCER]
            .standoff()
            .expect("a row that does damage");

        let stage = staged(&world);

        let out = station(&world, unit) - stage.centre;
        assert!((out.length() - standoff).abs() < 1e-9, "{out:?}");
        assert!(
            out.dot(-stage.axis) > 0.0,
            "it stands across the stage from its own side"
        );
        assert!(
            out.dot(stage.normal).abs() < 1e-9,
            "the station left the stage's plane: {out:?}"
        );
    }

    #[test]
    fn two_teams_lines_of_one_row_stand_a_stated_distance_inside_that_rows_reach() {
        for row in [RAIDER, FRIGATE, LANCER] {
            let mut world = world();
            let ours = world.hold(0, row, HOME, 0.0);
            let theirs = world.hold(1, row, HOME, 1.0);
            let reach = world.state[row].max_damage_range();
            let name = world.state[row].name;

            let apart = station(&world, ours).distance(station(&world, theirs));
            assert!(
                apart < reach,
                "two lines of {name} stand {apart} apart, past the {reach} they reach"
            );
            assert!(
                (reach - apart - Belt::LINES_INSIDE_RANGE_METERS).abs() < 1e-9,
                "two lines of {name} stand {apart} apart, not {} inside the {reach} they reach",
                Belt::LINES_INSIDE_RANGE_METERS
            );
        }
    }

    fn damage_units() -> Vec<RowId> {
        Roster::shipped()
            .iter()
            .filter(|(_, row)| row.does_damage() && row.kind() == Kind::Unit)
            .map(|(id, _)| id)
            .collect()
    }

    fn lined_up(teams: u8, row: RowId) -> World {
        let mut world = teamed(teams);
        let standoff = world.state[row].standoff().expect("a row that does damage");
        let deep = 2 * FightStage::stations_on_the_stage(standoff, usize::from(teams));
        for seat in 0..teams {
            for at in 0..deep {
                world.hold(seat, row, HOME, at as f64 * 0.01);
            }
        }
        world
    }

    fn nearest_across_seats(world: &World) -> f64 {
        let rolls = Rolls::called(&world.state);
        let stations: Vec<(SeatId, Vec3)> = world
            .state
            .entities()
            .filter_map(|entity| {
                rolls[HOME]
                    .station(entity.id())
                    .map(|station| (entity.seat(), station))
            })
            .collect();
        let mut nearest = f64::INFINITY;
        for (at, one) in stations.iter().enumerate() {
            for other in &stations[at + 1..] {
                if one.0 != other.0 {
                    nearest = nearest.min(one.1.distance(other.1));
                }
            }
        }
        nearest
    }

    #[test]
    fn no_two_seats_stations_stand_closer_than_the_spacing_however_many_teams_stand_there() {
        for teams in 1..=4 {
            for row in damage_units() {
                let world = lined_up(teams, row);
                let name = world.state[row].name;

                let apart = nearest_across_seats(&world);

                assert!(
                    apart >= Belt::SPACING_METERS,
                    "{teams} teams: two seats' {name} stations stand {apart} apart, \
                     inside the {} a pair settles at",
                    Belt::SPACING_METERS
                );
            }
        }
    }

    #[test]
    fn every_station_stands_on_its_own_teams_side_of_the_stage() {
        for teams in 2..=4 {
            for row in damage_units() {
                let world = lined_up(teams, row);
                let name = world.state[row].name;
                let stage = staged(&world);
                let sides: Vec<Vec3> = (0..teams)
                    .map(|at| {
                        let turn = TAU * f64::from(at) / f64::from(teams);
                        -stage.axis * libm::cos(turn) + stage.lateral * libm::sin(turn)
                    })
                    .collect();
                let rolls = Rolls::called(&world.state);

                for entity in world.state.entities() {
                    let Some(station) = rolls[HOME].station(entity.id()) else {
                        continue;
                    };
                    let out = (station - stage.centre).normalized().expect("a side");
                    let mine = usize::from(entity.seat().0);
                    for (at, side) in sides.iter().enumerate() {
                        assert!(
                            at == mine || sides[mine].dot(out) > side.dot(out),
                            "{teams} teams: a {name} of seat {mine} stands nearer \
                             the side of team {at} than its own"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn no_station_stands_further_off_the_body_than_the_roster_is_checked_against() {
        let mut world = world();
        let standoff = world.state[LANCER]
            .standoff()
            .expect("a row that does damage");
        let force: Vec<EntityId> = (0..FightStage::stations_on_the_stage(standoff, 1))
            .map(|at| world.hold(0, LANCER, HOME, at as f64 * 0.01))
            .collect();
        let checked =
            Belt::furthest_station_meters(world.state.roster().longest_damage_range(), standoff);

        let stations = stationed(&world, &force);

        for station in stations {
            let off = off_asteroid(&world, station.expect("a station"));
            assert!(
                off <= checked,
                "a station stands {off} off the body, past the {checked} the roster is built against"
            );
        }
    }

    #[test]
    fn a_long_range_row_stands_further_off_the_stage_than_a_short_range_one() {
        let mut world = world();
        let near = world.hold(0, RAIDER, HOME, 0.0);
        let far = world.hold(0, LANCER, HOME, 1.0);

        let stage = staged(&world);

        let out = |unit: EntityId| (station(&world, unit) - stage.centre).length();
        assert!(
            out(far) > out(near),
            "the lancer stands {} out, inside the raider at {}",
            out(far),
            out(near)
        );
    }

    #[test]
    fn a_row_straddles_its_standoff_point_in_id_order_at_the_station_spacing() {
        let mut world = World::ring(GRAVITY, 2, &[TeamId(0), TeamId(0)]);
        let force: Vec<EntityId> = (0..5)
            .map(|at| world.hold(at % 2, FRIGATE, HOME, f64::from(at) * 0.1))
            .collect();
        assert_ne!(
            world.state.entity(force[1]).seat(),
            SeatId(0),
            "the row is not split across two seats of one team"
        );

        let stage = staged(&world);

        let standoff = world.state[FRIGATE]
            .standoff()
            .expect("a row that does damage");
        let aside: Vec<f64> = stationed(&world, &force)
            .into_iter()
            .map(|station| {
                let out = station.expect("a station") - stage.centre;
                assert!(
                    (out.dot(-stage.axis) - standoff).abs() < 1e-9,
                    "a unit of the row stands off {}, not {standoff}",
                    out.dot(-stage.axis)
                );
                out.dot(stage.lateral)
            })
            .collect();
        assert!(
            aside[0].abs() < 1e-9,
            "the first of a row stands {} off its point",
            aside[0]
        );
        for (at, offset) in aside.iter().enumerate() {
            let step = at.div_ceil(2) as f64 * Belt::STATION_SPACING_METERS;
            let wanted = match at % 2 {
                1 => step,
                _ => -step,
            };
            assert!(
                (offset - wanted).abs() < 1e-9,
                "the {at} unit of the row stands {offset} aside, not {wanted}"
            );
        }
    }

    #[test]
    fn a_row_wider_than_the_stage_wraps_into_a_rank_behind() {
        let mut world = world();
        let standoff = world.state[FRIGATE]
            .standoff()
            .expect("a row that does damage");
        let across = FightStage::stations_in_a_rank(standoff, 1);
        let force: Vec<EntityId> = (0..across + 1)
            .map(|at| world.hold(0, FRIGATE, HOME, at as f64 * 0.1))
            .collect();

        let stage = staged(&world);

        let aside = |unit: EntityId| (station(&world, unit) - stage.centre).dot(stage.lateral);
        let widest = force[..across]
            .iter()
            .map(|one| aside(*one).abs())
            .fold(0.0_f64, f64::max);
        let half = Belt::STATIONS_EACH_WAY as f64 * Belt::STATION_SPACING_METERS;
        assert!(
            (widest - half).abs() < 1e-9,
            "a rank spreads {widest} either way, not {half}"
        );
        let wrapped = station(&world, force[across]) - stage.centre;
        assert!(
            (wrapped.dot(-stage.axis) - (standoff + Belt::STATION_SPACING_METERS)).abs() < 1e-9,
            "the wrapped unit stands {} out, not one spacing behind {standoff}",
            wrapped.dot(-stage.axis)
        );
        assert!(
            aside(force[across]).abs() < 1e-9,
            "it stands {} aside, not at the front of the rank behind",
            aside(force[across])
        );
    }

    #[test]
    fn a_row_longer_than_the_stage_fills_it_again_from_the_front() {
        let mut world = world();
        let standoff = world.state[RAIDER]
            .standoff()
            .expect("a row that does damage");
        let stations = FightStage::stations_on_the_stage(standoff, 1);
        let force: Vec<EntityId> = (0..stations + 1)
            .map(|at| world.hold(0, RAIDER, HOME, at as f64 * 0.01))
            .collect();

        let wrapped = station(&world, force[stations]);

        assert_eq!(
            wrapped,
            station(&world, force[0]),
            "the unit past the stage took a station of its own"
        );
    }

    #[test]
    fn a_unit_that_does_no_damage_and_any_structure_take_no_place_on_the_stage() {
        let mut world = world();
        world.hold(0, CONSTRUCTOR, HOME, 0.0);
        world.fix(0, METALS_EXTRACTOR, HOME);
        world.fix(0, FRIGATE, HOME);
        assert!(world.state[FRIGATE].does_damage());

        let stage = staged(&world);

        assert_eq!(
            stage.stations_of(TeamId(0), CONSTRUCTOR),
            &[],
            "a unit that does no damage took a place on the stage"
        );
        assert_eq!(
            stage.stations_of(TeamId(0), METALS_EXTRACTOR),
            &[],
            "a structure that does no damage took a place on the stage"
        );
        assert_eq!(
            stage.stations_of(TeamId(0), FRIGATE),
            &[],
            "a structure of a row that does damage took a place it can never move to"
        );
    }

    #[test]
    fn an_asteroid_nobody_stands_at_has_no_stations() {
        let world = world();

        let stage = staged(&world);

        assert_eq!(stage.stations, BTreeMap::new());
    }
}
