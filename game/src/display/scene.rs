//! Everything one frame draws, in sim units. Plain data: the draw converts
//! to the engine's `f32` and nothing here decides what is drawn.

use std::collections::BTreeMap;

use probe_sim::orbit::{Body, Gravity};
use probe_sim::roster::MassClass;
use probe_sim::roster::Roster;
use probe_sim::state::Rock;
use probe_sim::state::view::View;
use probe_sim::{Band, Material, Materials, Place, RockId, RowId, SeatId, Tick, Vec3};

use crate::display::fights::Fights;
use crate::display::glyph::Glyph;
use crate::display::label::titled;
use crate::display::send::Sending;

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
    /// What the glyph is and why it is in this state, which hovering it
    /// shows.
    pub reason: Reason,
}

/// Why a glyph is in the state it is drawn in: one sentence, shown while
/// the pointer is over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reason {
    /// A unit of this row is at the place.
    Here(RowId),
    /// A unit of this row is wanted and absent, with no frame open for it.
    Wanted(RowId),
    /// A frame of this row is being built, and the material it has spent
    /// nothing on this second for want of.
    Building(RowId, Option<Material>),
    /// A frame of this row waits on a builder at the rock.
    NoBuilder(RowId),
    /// A unit of this row is flying in from this rock.
    Arriving(RowId, RockId),
    /// A unit of this row is flying out to this rock.
    Leaving(RowId, RockId),
}

/// One ring: a place's runs in seat order and the fight arcs on it.
#[derive(Clone, Debug, PartialEq)]
pub struct RingView {
    pub place: Place,
    /// Its rock's caps, which its stroke is tinted by.
    pub caps: Materials,
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
    /// The most of each material extractable per second, which is what the
    /// rock is tinted by.
    pub caps: Materials,
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
    /// The belt alone at `tick`: every rock where its orbit puts it, tinted
    /// by its caps, with its inner ring and nothing on it.
    ///
    /// What the lobby and the loading screen draw, before a match exists to
    /// have a view of.
    pub fn of_belt(rocks: &[Rock], gravity: Gravity, tick: Tick) -> Scene {
        Scene {
            rocks: rocks
                .iter()
                .enumerate()
                .map(|(at, rock)| RockView {
                    id: RockId(at as u32),
                    pos: rock.orbit().at(tick, gravity).pos,
                    radius: rock.radius(),
                    caps: rock.caps(),
                })
                .collect(),
            entities: Vec::new(),
            rings: rocks
                .iter()
                .enumerate()
                .map(|(at, rock)| RingView {
                    place: Place {
                        rock: RockId(at as u32),
                        band: Band::Inner,
                    },
                    caps: rock.caps(),
                    runs: Vec::new(),
                    arcs: Vec::new(),
                })
                .collect(),
            flights: Vec::new(),
            blips: Vec::new(),
            selection: None,
            hover: None,
        }
    }

    /// The middle of the rocks this scene draws, in meters: where a camera
    /// looks to frame the whole belt.
    ///
    /// A belt with no rock has no middle, and this answers the origin,
    /// where the central mass is. A non-empty belt type in `sim` would
    /// delete the case; `Belt::fixed` returns a plain `Vec<Rock>` today.
    pub fn centre(&self) -> Vec3 {
        let rocks = self.rocks.len().max(1) as f64;
        self.rocks
            .iter()
            .fold(Vec3::ZERO, |sum, rock| sum + rock.pos)
            * (1.0 / rocks)
    }

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
        let caps: Caps = view
            .terrain
            .iter()
            .map(|rock| (rock.rock, rock.caps))
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
                    caps: rock.caps,
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
            rings: runs.rings(roster, client.fights, &caps, drawn),
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

impl RingView {
    /// `place`'s ring in `rings`, empty until something is put on it, its
    /// stroke tinted by what `caps` holds for its rock.
    fn at<'a>(
        rings: &'a mut BTreeMap<Place, RingView>,
        caps: &Caps,
        place: Place,
    ) -> &'a mut RingView {
        rings.entry(place).or_insert_with(|| RingView {
            place,
            // No caps is no material leading, which is a plain stroke.
            caps: caps.get(&place.rock).copied().unwrap_or(Materials::ZERO),
            runs: Vec::new(),
            arcs: Vec::new(),
        })
    }
}

/// Every rock's caps, by rock: what a ring's stroke is tinted by.
type Caps = BTreeMap<RockId, Materials>;

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
        let mut push = |place: Place, seat: SeatId, row: RowId, mark: Mark| {
            rows.entry((place, seat))
                .or_default()
                .entry(row)
                .or_default()
                .push(mark);
        };
        for seen in view.seen.iter().filter(|seen| !seen.flying) {
            let Some(place) = seen.home else {
                continue;
            };
            let reason = Reason::Here(seen.row);
            push(
                place,
                seen.seat,
                seen.row,
                mark(roster, seen.row, Fill::Solid, reason),
            );
        }
        // A unit in flight stands on both runs: dimmed where it left, and
        // hollow where it is arriving.
        for seen in view
            .seen
            .iter()
            .filter(|seen| seen.flying && seen.seat == view.seat)
        {
            if let Some(to) = seen.home
                && let Some(from) = seen.from
            {
                push(
                    from,
                    seen.seat,
                    seen.row,
                    Mark {
                        dim: true,
                        ..mark(
                            roster,
                            seen.row,
                            Fill::Solid,
                            Reason::Leaving(seen.row, to.rock),
                        )
                    },
                );
                push(
                    to,
                    seen.seat,
                    seen.row,
                    mark(
                        roster,
                        seen.row,
                        Fill::Hollow,
                        Reason::Arriving(seen.row, from.rock),
                    ),
                );
            }
        }
        let builders = builders(view, roster);
        for composition in &view.compositions {
            let place = composition.place;
            let unbuilt = builders.binary_search(&place.rock).is_err();
            for wanted in &composition.rows {
                for frame in &wanted.frames {
                    let (fill, reason) = match unbuilt {
                        true => (Fill::Dashed, Reason::NoBuilder(wanted.row)),
                        false => (
                            Fill::Filling(frame.progress as f32),
                            Reason::Building(wanted.row, frame.starved_of),
                        ),
                    };
                    push(
                        place,
                        view.seat,
                        wanted.row,
                        mark(roster, wanted.row, fill, reason),
                    );
                }
                let held = wanted.present + wanted.flying + wanted.frames.len() as u32;
                for _ in held..wanted.want {
                    push(
                        place,
                        view.seat,
                        wanted.row,
                        mark(roster, wanted.row, Fill::Hollow, Reason::Wanted(wanted.row)),
                    );
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
                        ..mark(roster, *row, Fill::Hollow, Reason::Wanted(*row))
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
                            ..mark(
                                roster,
                                row,
                                Fill::Hollow,
                                Reason::Arriving(row, sending.from.rock),
                            )
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
        caps: &Caps,
        drawn: impl Iterator<Item = Place>,
    ) -> Vec<RingView> {
        let cost = |row: RowId| roster[row].cost.total();
        let mut rings: BTreeMap<Place, RingView> = BTreeMap::new();
        for place in drawn {
            RingView::at(&mut rings, caps, place);
        }
        for ((place, seat), rows) in self.rows {
            let mut rows: Vec<(RowId, Vec<Mark>)> = rows.into_iter().collect();
            rows.sort_by(|(a, _), (b, _)| cost(*b).total_cmp(&cost(*a)));
            RingView::at(&mut rings, caps, place).runs.push(Run {
                seat,
                marks: rows.into_iter().flat_map(|(_, marks)| marks).collect(),
            });
        }
        for (place, arc) in fights.arcs() {
            let ring = RingView::at(&mut rings, caps, place);
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

impl Reason {
    /// The material the frame this glyph stands for has spent nothing on
    /// this second for want of, where it is a starved frame.
    pub fn starved(self) -> Option<Material> {
        match self {
            Reason::Building(_, starved) => starved,
            Reason::Here(_)
            | Reason::Wanted(_)
            | Reason::NoBuilder(_)
            | Reason::Arriving(..)
            | Reason::Leaving(..) => None,
        }
    }

    /// The one plain sentence hovering the glyph shows, naming the row and
    /// the state it is in. Rocks are numbered from one, as every number
    /// the player reads is.
    pub fn sentence(self, roster: &Roster) -> String {
        let row = |row: RowId| titled(roster[row].name);
        let rock = |rock: RockId| format!("Rock {}", rock.0 as u64 + 1);
        match self {
            Reason::Here(of) => format!("{}, here", row(of)),
            Reason::Wanted(of) => format!("{}, wanted", row(of)),
            Reason::Building(of, None) => format!("{}, building", row(of)),
            Reason::Building(of, Some(material)) => {
                format!("{}, building, short of {}", row(of), short_of(material))
            }
            Reason::NoBuilder(of) => format!("{}, no builder here", row(of)),
            Reason::Arriving(of, from) => format!("{}, arriving from {}", row(of), rock(from)),
            Reason::Leaving(of, to) => format!("{}, leaving for {}", row(of), rock(to)),
        }
    }
}

/// A material as a sentence names it.
fn short_of(material: Material) -> &'static str {
    match material {
        Material::Metals => "metals",
        Material::Volatiles => "volatiles",
        Material::Energy => "energy",
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
fn mark(roster: &Roster, row: RowId, fill: Fill, reason: Reason) -> Mark {
    Mark {
        glyph: Glyph::of(&roster[row]),
        fill,
        dim: false,
        reason,
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

    use super::*;
    use crate::display::glyph::Frame;
    use crate::display::local::{Local, PLAYER};

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

    /// The scene of the match's tick, with nothing selected or hovered.
    fn scene(local: &Local, fights: &Fights) -> Scene {
        drawn(local, fights, None, None)
    }

    fn drawn(
        local: &Local,
        fights: &Fights,
        selection: Option<Place>,
        hover: Option<Hover>,
    ) -> Scene {
        Scene::from_view(
            &local.view(),
            local.session().state().roster(),
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
        let local = Local::start(2);
        let scene = scene(&local, &Fights::default());

        assert_eq!(scene.rocks.len(), local.session().state().rocks().len());
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
        let mut local = Local::start(2);
        local.want(&[(inner(0), SHIPYARD, 1)]);

        let marks = run_at(&scene(&local, &Fights::default()), inner(0)).expect("its ring");

        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].fill, Fill::Solid);
        assert_eq!(marks[0].glyph.frame, Frame::Square);
        assert!(!marks[0].dim);
    }

    #[test]
    fn a_frame_fills_where_a_builder_stands_and_is_dashed_where_none_does() {
        let mut local = Local::start(2);
        local.want(&[(inner(0), SHIPYARD, 1)]);
        local.want(&[(inner(0), STORAGE, 1), (inner(5), FRIGATE, 1)]);
        local.run(60);

        let scene = scene(&local, &Fights::default());

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
        let local = Local::start(2);

        let scene = drawn(&local, &Fights::default(), Some(inner(3)), None);

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
    fn a_unit_in_flight_stays_dimmed_on_the_ring_it_left_and_arrives_on_the_ring_it_flies_to() {
        let mut local = Local::start(2);
        local.want(&[(inner(0), CONSTRUCTOR, 1)]);
        local.want(&[(inner(0), CONSTRUCTOR, 0), (inner(1), CONSTRUCTOR, 1)]);
        assert_eq!(
            local.session().state().flights().count(),
            1,
            "the send is one flight"
        );

        let scene = scene(&local, &Fights::default());

        assert_eq!(scene.flights.len(), 1);
        assert_eq!(scene.flights[0].to, RockId(1));
        assert_eq!(scene.entities.len(), 1, "the ship is drawn on the belt");
        let left = run_at(&scene, inner(0)).expect("the ring it left draws a run");
        assert_eq!(left.len(), 1);
        assert!(left[0].dim && left[0].fill == Fill::Solid);
        assert_eq!(left[0].reason, Reason::Leaving(CONSTRUCTOR, RockId(1)));
        let arriving = run_at(&scene, inner(1)).expect("the ring it flies to draws a run");
        assert_eq!(arriving.len(), 1);
        assert_eq!(arriving[0].fill, Fill::Hollow);
        assert_eq!(arriving[0].reason, Reason::Arriving(CONSTRUCTOR, RockId(0)));
    }

    #[test]
    fn the_plus_band_previews_one_dim_hollow_glyph_at_the_end_of_the_run() {
        let mut local = Local::start(2);
        local.want(&[(inner(0), SHIPYARD, 1)]);
        let hover = Hover::Wheel {
            place: inner(0),
            row: SHIPYARD,
            band: WheelBand::Plus,
        };

        let marks = run_at(
            &drawn(&local, &Fights::default(), Some(inner(0)), Some(hover)),
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
        let mut local = Local::start(2);
        local.want(&[(inner(0), SHIPYARD, 1)]);
        let hover = Hover::Wheel {
            place: inner(0),
            row: SHIPYARD,
            band: WheelBand::Minus,
        };

        let marks = run_at(
            &drawn(&local, &Fights::default(), Some(inner(0)), Some(hover)),
            inner(0),
        )
        .expect("its ring");

        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].fill, Fill::Solid, "a solid one sends a ship away");
        assert!(marks[0].dim);
    }

    #[test]
    fn a_send_drag_dims_the_source_and_shows_the_destination_hollow() {
        let mut local = Local::start(2);
        local.want(&[(inner(0), CONSTRUCTOR, 1)]);
        let sending = Sending {
            from: inner(0),
            to: inner(1),
            count: 1,
        };

        let scene = drawn(&local, &Fights::default(), None, Some(Hover::Send(sending)));

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
