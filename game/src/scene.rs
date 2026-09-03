//! Everything one frame draws, in sim units. Plain data: the draw converts
//! to the engine's `f32` and nothing here decides what is drawn.

use std::collections::BTreeMap;

use probe_sim::orbit::Body;
use probe_sim::roster::Roster;
use probe_sim::state::view::{MassClass, View};
use probe_sim::{Band, Place, RockId, RowId, SeatId, Vec3};

use crate::fights::Fights;
use crate::glyph::Glyph;
use crate::send::Sending;

/// How a glyph on a ring is drawn: the state of the unit it stands for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    /// A unit present.
    Solid,
    /// A unit wanted but absent.
    Hollow,
    /// A frame in progress, filled from the bottom to this fraction of its
    /// work, in `0..=1`.
    Filling(f32),
    /// A shortfall with no builder at the rock.
    Dashed,
}

/// The band of a wheel slot under the cursor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelBand {
    /// The slot's outer band: a click adds one want.
    Plus,
    /// The slot's inner band: a click removes one want.
    Minus,
}

/// What the pointer previews, drawn dim until the click lands it.
#[derive(Clone, Debug, PartialEq)]
pub enum Hover {
    /// A wheel band, previewing the one want it edits.
    Wheel {
        place: Place,
        row: RowId,
        band: WheelBand,
    },
    /// A drag from one ring to another, previewing the units it moves.
    Send(Sending),
}

/// One seat's fight arc on a ring, drawn just inside its run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub seat: SeatId,
    /// The arc still filled: the seat's HP at the rock over its HP when the
    /// fight began, in `0..=1`.
    pub fraction: f32,
    /// The upper end of the red segment trailing the drain, in `0..=1`; at
    /// `fraction` when no damage landed in the last second and a half.
    pub trailing: f32,
}

/// One radar contact: a dot with the velocity streak of a body the seat
/// cannot see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blip {
    /// In meters, in the inertial frame.
    pub pos: Vec3,
    /// Velocity relative to the nearest rock, in meters per second, which
    /// is the motion the belt's own turn does not account for.
    pub drift: Vec3,
    pub mass: MassClass,
}

/// One entity drawn where the sim has it.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityView {
    pub seat: SeatId,
    /// The row it is a copy of, by the three rules; computed once when the
    /// scene is built.
    pub glyph: Glyph,
    /// In meters, in the inertial frame.
    pub pos: Vec3,
}

/// The line ahead of a ship in flight to its destination rock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightLine {
    /// The ship's position, in meters, in the inertial frame.
    pub from: Vec3,
    pub to: RockId,
}

/// One glyph of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The row it stands for, by the three rules; computed once when the
    /// scene is built.
    pub glyph: Glyph,
    pub fill: Fill,
    /// Drawn dim: a hover preview, or a unit the pointer says is leaving.
    pub dim: bool,
}

/// One ring: a place's runs in seat order and the fight arcs on it.
#[derive(Clone, Debug, PartialEq)]
pub struct RingView {
    pub place: Place,
    pub runs: Vec<Run>,
    pub arcs: Vec<Arc>,
}

/// One rock drawn where the sim has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RockView {
    pub id: RockId,
    /// In meters, in the inertial frame.
    pub pos: Vec3,
    /// In meters.
    pub radius: f64,
}

/// One seat's glyphs on a ring, in run order: rows by cost descending, a
/// row's glyphs consecutive.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub seat: SeatId,
    pub marks: Vec<Mark>,
}

/// What the client, not the sim, decides about a frame.
pub struct Client<'a> {
    /// The ring the wheel is open on.
    pub selection: Option<Place>,
    pub hover: Option<Hover>,
    pub fights: &'a Fights,
}

/// One frame's drawing. Rings and flights are listed only where they draw
/// something; the selection and hover are the client's own state.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub rocks: Vec<RockView>,
    pub entities: Vec<EntityView>,
    pub rings: Vec<RingView>,
    pub flights: Vec<FlightLine>,
    pub blips: Vec<Blip>,
    /// The ring the wheel is open on.
    pub selection: Option<Place>,
    pub hover: Option<Hover>,
}

impl Scene {
    /// What `view`'s tick draws, over `roster`, the match's own, with what
    /// `client` says the pointer is doing.
    ///
    /// Every rock draws its inner ring; an outer ring draws only where
    /// that band holds something or its rock is selected. A flying unit of
    /// the seat's carries a flight line to its destination rock and no
    /// glyph on a ring; a seen enemy in flight carries neither.
    pub fn from_view(view: &View, roster: &Roster, client: Client<'_>) -> Scene {
        let bodies: Vec<(RockId, Body)> = view
            .terrain
            .iter()
            .map(|rock| (rock.rock, rock.orbit.at(view.tick, view.gravity)))
            .collect();
        let mut runs = Runs::of(view, roster);
        runs.preview(view, roster, client.hover.as_ref());
        let drawn =
            view.terrain
                .iter()
                .map(|rock| Place {
                    rock: rock.rock,
                    band: Band::Inner,
                })
                .chain(client.selection.into_iter().flat_map(|place| {
                    [Band::Inner, Band::Outer].map(|band| Place { band, ..place })
                }));
        Scene {
            rocks: view
                .terrain
                .iter()
                .zip(&bodies)
                .map(|(rock, (_, body))| RockView {
                    id: rock.rock,
                    pos: body.pos,
                    radius: rock.radius,
                })
                .collect(),
            entities: view
                .seen
                .iter()
                .map(|seen| EntityView {
                    seat: seen.seat,
                    glyph: Glyph::of(&roster[seen.row]),
                    pos: seen.body.pos,
                })
                .collect(),
            rings: runs.rings(roster, client.fights, drawn),
            flights: view
                .seen
                .iter()
                .filter(|seen| seen.seat == view.seat && seen.flying)
                .filter_map(|seen| {
                    Some(FlightLine {
                        from: seen.body.pos,
                        to: seen.home?.rock,
                    })
                })
                .collect(),
            blips: view
                .blips
                .iter()
                .filter_map(|blip| {
                    Some(Blip {
                        pos: blip.body.pos,
                        drift: blip.body.vel - nearest_body(&bodies, blip.body.pos)?.vel,
                        mass: blip.mass,
                    })
                })
                .collect(),
            selection: client.selection,
            hover: client.hover,
        }
    }
}

/// The rows of one seat's run at one place, before the run is laid in cost
/// order.
type Rows = BTreeMap<RowId, Vec<Mark>>;

/// Every ring's runs, keyed so a place and seat is found once.
struct Runs {
    rows: BTreeMap<(Place, SeatId), Rows>,
}

impl Runs {
    /// The runs `view` states: one solid glyph per unit present, then, for
    /// the seat's own compositions, a filling glyph per frame, a dashed one
    /// where the rock has no builder, and a hollow one per unit wanted and
    /// absent.
    fn of(view: &View, roster: &Roster) -> Runs {
        let mut rows: BTreeMap<(Place, SeatId), Rows> = BTreeMap::new();
        for seen in view.seen.iter().filter(|seen| !seen.flying) {
            let Some(place) = seen.home else {
                continue;
            };
            rows.entry((place, seen.seat))
                .or_default()
                .entry(seen.row)
                .or_default()
                .push(mark(roster, seen.row, Fill::Solid));
        }
        let builders = builders(view, roster);
        for composition in &view.compositions {
            let place = composition.place;
            let marks = rows.entry((place, view.seat)).or_default();
            let unbuilt = builders.binary_search(&place.rock).is_err();
            for wanted in &composition.rows {
                let row = marks.entry(wanted.row).or_default();
                for progress in &wanted.frames {
                    let fill = match unbuilt {
                        true => Fill::Dashed,
                        false => Fill::Filling(*progress as f32),
                    };
                    row.push(mark(roster, wanted.row, fill));
                }
                let held = wanted.present + wanted.flying + wanted.frames.len() as u32;
                for _ in held..wanted.want {
                    row.push(mark(roster, wanted.row, Fill::Hollow));
                }
            }
        }
        Runs { rows }
    }

    /// Dims what the pointer says is leaving and appends what it says is
    /// coming, at half alpha, by the same rules as everything else.
    fn preview(&mut self, view: &View, roster: &Roster, hover: Option<&Hover>) {
        match hover {
            None => {}
            Some(Hover::Wheel { place, row, band }) => {
                let rows = self.rows.entry((*place, view.seat)).or_default();
                let marks = rows.entry(*row).or_default();
                match band {
                    WheelBand::Plus => marks.push(Mark {
                        dim: true,
                        ..mark(roster, *row, Fill::Hollow)
                    }),
                    WheelBand::Minus => {
                        if let Some(last) = marks.last_mut() {
                            last.dim = true;
                        }
                    }
                }
            }
            Some(Hover::Send(sending)) => {
                for (row, count) in sending.rows(view, roster) {
                    if let Some(marks) = self
                        .rows
                        .get_mut(&(sending.from, view.seat))
                        .and_then(|rows| rows.get_mut(&row))
                    {
                        for mark in marks
                            .iter_mut()
                            .rev()
                            .filter(|mark| mark.fill == Fill::Solid)
                            .take(count as usize)
                        {
                            mark.dim = true;
                        }
                    }
                    let arriving = self
                        .rows
                        .entry((sending.to, view.seat))
                        .or_default()
                        .entry(row)
                        .or_default();
                    for _ in 0..count {
                        arriving.push(Mark {
                            dim: true,
                            ..mark(roster, row, Fill::Hollow)
                        });
                    }
                }
            }
        }
    }

    /// The rings, in place order, each seat's rows flattened into a run by
    /// cost descending, with the fight arcs `fights` remembers. `drawn`
    /// names the rings that draw whether or not a seat is on them, so a
    /// rock holding nothing can still be aimed at and selected.
    fn rings(
        self,
        roster: &Roster,
        fights: &Fights,
        drawn: impl Iterator<Item = Place>,
    ) -> Vec<RingView> {
        let cost = |row: RowId| roster[row].cost.total();
        let mut rings: BTreeMap<Place, RingView> = BTreeMap::new();
        for place in drawn {
            rings.entry(place).or_insert_with(|| RingView {
                place,
                runs: Vec::new(),
                arcs: Vec::new(),
            });
        }
        for ((place, seat), rows) in self.rows {
            let mut rows: Vec<(RowId, Vec<Mark>)> = rows.into_iter().collect();
            rows.sort_by(|(a, _), (b, _)| cost(*b).total_cmp(&cost(*a)));
            rings
                .entry(place)
                .or_insert_with(|| RingView {
                    place,
                    runs: Vec::new(),
                    arcs: Vec::new(),
                })
                .runs
                .push(Run {
                    seat,
                    marks: rows.into_iter().flat_map(|(_, marks)| marks).collect(),
                });
        }
        for (place, arc) in fights.arcs() {
            let ring = rings.entry(place).or_insert_with(|| RingView {
                place,
                runs: Vec::new(),
                arcs: Vec::new(),
            });
            if !ring.runs.iter().any(|run| run.seat == arc.seat) {
                ring.runs.push(Run {
                    seat: arc.seat,
                    marks: Vec::new(),
                });
            }
            ring.arcs.push(arc);
        }
        for ring in rings.values_mut() {
            ring.runs.sort_by_key(|run| run.seat);
        }
        rings.into_values().collect()
    }
}

/// The rocks where the seat has a builder, sorted: a shortfall at any other
/// rock is dashed, since a build weapon reaches only its own rock.
fn builders(view: &View, roster: &Roster) -> Vec<RockId> {
    let mut rocks: Vec<RockId> = view
        .seen
        .iter()
        .filter(|seen| seen.seat == view.seat && !seen.flying)
        .filter(|seen| roster[seen.row].builds().next().is_some())
        .filter_map(|seen| Some(seen.home?.rock))
        .collect();
    rocks.sort_unstable();
    rocks.dedup();
    rocks
}

/// One mark of `row`, its glyph by the three rules.
fn mark(roster: &Roster, row: RowId, fill: Fill) -> Mark {
    Mark {
        glyph: Glyph::of(&roster[row]),
        fill,
        dim: false,
    }
}

/// The body of the rock nearest `pos`, of a belt that has one.
fn nearest_body(bodies: &[(RockId, Body)], pos: Vec3) -> Option<Body> {
    bodies
        .iter()
        .min_by(|(_, a), (_, b)| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)))
        .map(|(_, body)| *body)
}

#[cfg(test)]
mod tests {
    use probe_sim::roster::{CONSTRUCTOR, FRIGATE, SHIPYARD, STORAGE};
    use probe_sim::session::Session;
    use probe_sim::state::{Command, Issued, State};
    use probe_sim::{TICKS_PER_SECOND, Tick};

    use super::*;
    use crate::glyph::Frame;

    /// The seat every test plays.
    const PLAYER: SeatId = SeatId(0);

    /// A clock no test reaches.
    const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    fn outer(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Outer,
        }
    }

    fn start() -> Session {
        Session::new(State::start(
            CLOCK,
            probe_sim::belt::Belt::GRAVITY,
            probe_sim::belt::Belt::fixed(probe_sim::belt::Belt::GRAVITY),
            &[probe_sim::TeamId(0), probe_sim::TeamId(1)],
        ))
    }

    /// One tick with the player's wants.
    fn want(session: &mut Session, wants: &[(Place, RowId, u32)]) {
        let issued = wants
            .iter()
            .map(|(place, row, count)| Issued {
                seat: PLAYER,
                command: Command::Want {
                    place: *place,
                    row: *row,
                    count: *count,
                },
            })
            .collect();
        assert!(session.advance(issued).is_empty(), "a want was rejected");
    }

    /// `ticks` ticks with no wants.
    fn run(session: &mut Session, ticks: u64) {
        for _ in 0..ticks {
            session.advance(Vec::new());
        }
    }

    fn view(session: &Session) -> View {
        View::of(session.state(), PLAYER, session.shots())
    }

    /// The scene of `session`'s tick, with nothing selected or hovered.
    fn scene(session: &Session, fights: &Fights) -> Scene {
        drawn(session, fights, None, None)
    }

    fn drawn(
        session: &Session,
        fights: &Fights,
        selection: Option<Place>,
        hover: Option<Hover>,
    ) -> Scene {
        Scene::from_view(
            &view(session),
            session.state().roster(),
            Client {
                selection,
                hover,
                fights,
            },
        )
    }

    /// The player's run at `place`, or none where the ring draws nothing.
    fn run_at(scene: &Scene, place: Place) -> Option<Vec<Mark>> {
        scene
            .rings
            .iter()
            .find(|ring| ring.place == place)?
            .runs
            .iter()
            .find(|run| run.seat == PLAYER)
            .map(|run| run.marks.clone())
    }

    #[test]
    fn every_rock_draws_an_inner_ring_and_an_empty_outer_ring_draws_nothing() {
        let session = start();
        let scene = scene(&session, &Fights::default());

        assert_eq!(scene.rocks.len(), session.state().rocks().len());
        assert_eq!(
            scene.rings.len(),
            scene.rocks.len(),
            "one inner ring per rock and no outer ring"
        );
        assert!(
            scene
                .rings
                .iter()
                .all(|ring| ring.place.band == Band::Inner)
        );
        assert!(
            scene.rings.iter().all(|ring| ring.runs.is_empty()),
            "an empty ring draws no run"
        );
    }

    #[test]
    fn a_placed_structure_draws_one_solid_square_on_its_ring() {
        let mut session = start();
        want(&mut session, &[(inner(0), SHIPYARD, 1)]);

        let marks = run_at(&scene(&session, &Fights::default()), inner(0)).expect("its ring");

        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].fill, Fill::Solid);
        assert_eq!(marks[0].glyph.frame, Frame::Square);
        assert!(!marks[0].dim);
    }

    #[test]
    fn a_frame_fills_where_a_builder_stands_and_is_dashed_where_none_does() {
        let mut session = start();
        want(&mut session, &[(inner(0), SHIPYARD, 1)]);
        want(
            &mut session,
            &[(inner(0), STORAGE, 1), (inner(5), FRIGATE, 1)],
        );
        run(&mut session, 60);

        let scene = scene(&session, &Fights::default());

        let built = run_at(&scene, inner(0)).expect("the shipyard's ring");
        let filling = built
            .iter()
            .find(|mark| matches!(mark.fill, Fill::Filling(_)))
            .expect("the storage is building");
        assert!(
            matches!(filling.fill, Fill::Filling(progress) if progress > 0.0),
            "{:?} has not started",
            filling.fill
        );

        let unbuilt = run_at(&scene, inner(5)).expect("the far ring");
        assert_eq!(unbuilt.len(), 1);
        assert_eq!(
            unbuilt[0].fill,
            Fill::Dashed,
            "a frame with no builder at the rock is dashed"
        );
    }

    #[test]
    fn a_selected_rock_draws_both_its_rings_even_holding_nothing() {
        let session = start();

        let scene = drawn(&session, &Fights::default(), Some(inner(3)), None);

        for place in [inner(3), outer(3)] {
            let ring = scene
                .rings
                .iter()
                .find(|ring| ring.place == place)
                .expect("a selected rock draws both bands");
            assert!(ring.runs.is_empty(), "with no run on either");
        }
        assert_eq!(scene.selection, Some(inner(3)));
    }

    #[test]
    fn a_flying_unit_carries_a_flight_line_and_no_glyph_on_a_ring() {
        let mut session = start();
        want(&mut session, &[(inner(0), CONSTRUCTOR, 1)]);
        want(
            &mut session,
            &[(inner(0), CONSTRUCTOR, 0), (inner(1), CONSTRUCTOR, 1)],
        );
        assert_eq!(
            session.state().flights().count(),
            1,
            "the send is one flight"
        );

        let scene = scene(&session, &Fights::default());

        assert_eq!(scene.flights.len(), 1);
        assert_eq!(scene.flights[0].to, RockId(1));
        assert_eq!(scene.entities.len(), 1, "the ship is drawn on the belt");
        assert!(
            run_at(&scene, inner(1)).is_none_or(|marks| marks.is_empty()),
            "a flying unit's glyph rides the ship, not the ring"
        );
    }

    #[test]
    fn the_plus_band_previews_one_dim_hollow_glyph_at_the_end_of_the_run() {
        let mut session = start();
        want(&mut session, &[(inner(0), SHIPYARD, 1)]);
        let hover = Hover::Wheel {
            place: inner(0),
            row: SHIPYARD,
            band: WheelBand::Plus,
        };

        let marks = run_at(
            &drawn(&session, &Fights::default(), Some(inner(0)), Some(hover)),
            inner(0),
        )
        .expect("its ring");

        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].fill, Fill::Solid);
        assert!(!marks[0].dim);
        assert_eq!(marks[1].fill, Fill::Hollow);
        assert!(marks[1].dim, "the preview is dim");
    }

    #[test]
    fn the_minus_band_dims_the_last_glyph_of_its_row() {
        let mut session = start();
        want(&mut session, &[(inner(0), SHIPYARD, 1)]);
        let hover = Hover::Wheel {
            place: inner(0),
            row: SHIPYARD,
            band: WheelBand::Minus,
        };

        let marks = run_at(
            &drawn(&session, &Fights::default(), Some(inner(0)), Some(hover)),
            inner(0),
        )
        .expect("its ring");

        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].fill, Fill::Solid, "a solid one sends a ship away");
        assert!(marks[0].dim);
    }

    #[test]
    fn a_send_drag_dims_the_source_and_shows_the_destination_hollow() {
        let mut session = start();
        want(&mut session, &[(inner(0), CONSTRUCTOR, 1)]);
        let sending = Sending {
            from: inner(0),
            to: inner(1),
            count: 1,
        };

        let scene = drawn(
            &session,
            &Fights::default(),
            None,
            Some(Hover::Send(sending)),
        );

        let source = run_at(&scene, inner(0)).expect("the source ring");
        assert_eq!(source.len(), 1);
        assert_eq!(source[0].fill, Fill::Solid);
        assert!(source[0].dim, "the glyph that would go is dimmed");

        let destination = run_at(&scene, inner(1)).expect("the destination ring");
        assert_eq!(destination.len(), 1);
        assert_eq!(destination[0].fill, Fill::Hollow);
        assert!(destination[0].dim);
    }
}
