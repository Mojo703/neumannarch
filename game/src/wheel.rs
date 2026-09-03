//! The roster wheel: one slot per roster row of a band, as screen-space
//! sectors editing that band's wants.

use core::f32::consts::{PI, TAU};

use mirage_engine::egui::{self, Color32, Pos2, Shape, Stroke};
use probe_sim::roster::{Kind, Roster, Row};
use probe_sim::state::Command;
use probe_sim::{Band, Place, RowId, SeatId};

use crate::glyph::Glyph;
use crate::glyph_quad::seat_color32;
use crate::scene::{Fill, WheelBand};
use crate::stencil::Stencil;

/// A band's radial half-width around [`RADIUS`], in points: the span
/// [`Wheel::slot_at`] answers, split at the radius into plus (outer) and
/// minus (inner).
pub const BAND_WIDTH: f32 = 16.0;

/// How far the wheel's inner edge clears the outer ring, in points, so the
/// annulus reads as its own control and not a third ring.
pub const GAP: f32 = 40.0;

/// The wheel's own screen radius, in points: its annulus's mid radius,
/// [`GAP`] beyond the outer ring.
pub const RADIUS: f32 = crate::hud::OUTER_RADIUS + GAP + BAND_WIDTH;

/// The annulus's inner edge stands [`GAP`] clear of the outer ring.
const _: () = assert!(GAP > 0.0 && RADIUS - BAND_WIDTH == crate::hud::OUTER_RADIUS + GAP);

/// A slot's glyph half-width, in points, before its size class steps it.
/// Wider than a ring's, since a slot is a control and not a unit.
const GLYPH_HALF: f32 = 9.0;

/// The annulus's own stroke width, in points.
const EDGE_WIDTH: f32 = 1.0;

/// The annulus's own stroke colour.
const EDGE_COLOUR: Color32 = Color32::from_gray(120);

/// A hovered band's colour, which brightens that band's own segment of the
/// annulus and nothing else.
const HOVER_COLOUR: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 46);

/// The straight segments one band of a slot is drawn with.
const SECTOR_SEGMENTS: usize = 12;

/// The roster wheel open on one band: a sector per row, structures in the
/// left half and units in the right, each half ordered by cost from the
/// top down.
pub struct Wheel {
    place: Place,
    seat: SeatId,
    centre: Pos2,
    slots: Vec<Slot>,
}

/// One row's sector of the wheel.
struct Slot {
    row: RowId,
    glyph: Glyph,
    /// The sector's centre, in rad clockwise from twelve o'clock.
    angle: f32,
    /// Half the sector's angular width, in rad.
    half_share: f32,
}

impl Wheel {
    /// Opens a wheel over `roster`'s rows for `place`'s band, centred at
    /// `centre`, [`RADIUS`] points out, drawn in `seat`'s colour. The outer
    /// band excludes structures, since DESIGN.md forbids one there.
    pub fn open(place: Place, seat: SeatId, roster: &Roster, centre: Pos2) -> Wheel {
        let mut structures = Vec::new();
        let mut units = Vec::new();
        for (row, data) in roster.iter() {
            match data.kind() {
                Kind::Structure if place.band == Band::Inner => structures.push((row, data)),
                Kind::Structure => {}
                Kind::Unit => units.push((row, data)),
            }
        }
        by_cost_descending(&mut structures);
        by_cost_descending(&mut units);

        let mut slots: Vec<Slot> = half_slots(&structures, Side::Left)
            .chain(half_slots(&units, Side::Right))
            .collect();
        slots.sort_by_key(|slot| slot.row);

        Wheel {
            place,
            seat,
            centre,
            slots,
        }
    }

    /// The band this wheel edits.
    pub fn place(&self) -> Place {
        self.place
    }

    /// The row and band the point `pixel` names, or `None` off every slot.
    pub fn slot_at(&self, pixel: Pos2) -> Option<(RowId, WheelBand)> {
        let offset = pixel - self.centre;
        let length = offset.length();
        if (length - RADIUS).abs() > BAND_WIDTH {
            return None;
        }
        let angle = angle_of(offset);
        let slot = self
            .slots
            .iter()
            .find(|slot| angular_distance(angle, slot.angle) < slot.half_share)?;
        let band = match length >= RADIUS {
            true => WheelBand::Plus,
            false => WheelBand::Minus,
        };
        Some((slot.row, band))
    }

    /// Paints the annulus, its sector boundaries, and every slot's glyph,
    /// brightening `hover`'s own band where it names one of this wheel's
    /// slots.
    pub fn paint(&self, painter: &egui::Painter, hover: Option<(RowId, WheelBand)>) {
        for radius in [RADIUS - BAND_WIDTH, RADIUS + BAND_WIDTH] {
            painter.circle_stroke(self.centre, radius, Stroke::new(EDGE_WIDTH, EDGE_COLOUR));
        }

        for slot in &self.slots {
            for edge in [slot.angle - slot.half_share, slot.angle + slot.half_share] {
                painter.line_segment(
                    [
                        self.at(RADIUS - BAND_WIDTH, edge),
                        self.at(RADIUS + BAND_WIDTH, edge),
                    ],
                    Stroke::new(EDGE_WIDTH, EDGE_COLOUR),
                );
            }

            if let Some((row, band)) = hover
                && row == slot.row
            {
                self.brighten(painter, slot, band);
            }

            Stencil {
                glyph: &slot.glyph,
                centre: self.at(RADIUS, slot.angle),
                half: GLYPH_HALF * slot.glyph.size.scale(),
                colour: seat_color32(self.seat),
                fill: Fill::Solid,
                dim: false,
            }
            .paint(painter);
        }
    }

    /// The point `radius` points out from the centre at `angle`, in rad
    /// clockwise from twelve o'clock.
    fn at(&self, radius: f32, angle: f32) -> Pos2 {
        let (sin, cos) = angle.sin_cos();
        egui::pos2(self.centre.x + radius * sin, self.centre.y - radius * cos)
    }

    /// Brightens one band of one slot: the half of the annulus that band
    /// owns, over that slot's own sector, as one stroked arc.
    fn brighten(&self, painter: &egui::Painter, slot: &Slot, band: WheelBand) {
        let radius = match band {
            WheelBand::Plus => RADIUS + BAND_WIDTH / 2.0,
            WheelBand::Minus => RADIUS - BAND_WIDTH / 2.0,
        };
        let from = slot.angle - slot.half_share;
        let step = 2.0 * slot.half_share / SECTOR_SEGMENTS as f32;
        let arc: Vec<Pos2> = (0..=SECTOR_SEGMENTS)
            .map(|at| self.at(radius, from + at as f32 * step))
            .collect();
        painter.add(Shape::line(arc, Stroke::new(BAND_WIDTH, HOVER_COLOUR)));
    }
}

impl WheelBand {
    /// The want a click on this band lands: `current` plus one for
    /// [`WheelBand::Plus`], minus one for [`WheelBand::Minus`] and never
    /// below zero.
    pub fn edit(self, place: Place, row: RowId, current: u32) -> Command {
        let count = match self {
            WheelBand::Plus => current + 1,
            WheelBand::Minus => current.saturating_sub(1),
        };
        Command::Want { place, row, count }
    }
}

/// Which half of the wheel a row's slot sits in.
#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
}

/// Orders `rows` by cost from the top of the wheel down.
fn by_cost_descending(rows: &mut [(RowId, &Row)]) {
    rows.sort_by(|(_, a), (_, b)| b.cost.total().total_cmp(&a.cost.total()));
}

/// `rows`, ordered top to bottom, as slots over `side`'s half of the wheel.
fn half_slots<'a>(rows: &'a [(RowId, &'a Row)], side: Side) -> impl Iterator<Item = Slot> + 'a {
    let share = PI / rows.len().max(1) as f32;
    rows.iter().enumerate().map(move |(index, (row, data))| {
        let from_top = (index as f32 + 0.5) * share;
        let angle = match side {
            Side::Right => from_top,
            Side::Left => normalize(-from_top),
        };
        Slot {
            row: *row,
            glyph: Glyph::of(data),
            angle,
            half_share: share / 2.0,
        }
    })
}

/// `offset`'s angle, in rad clockwise from twelve o'clock, in `0..TAU`.
fn angle_of(offset: egui::Vec2) -> f32 {
    normalize(offset.x.atan2(-offset.y))
}

/// `angle` wrapped into `0..TAU`.
fn normalize(angle: f32) -> f32 {
    angle.rem_euclid(TAU)
}

/// The absolute angular gap between `a` and `b`, both in `0..TAU`, taking
/// the shorter way round.
fn angular_distance(a: f32, b: f32) -> f32 {
    let raw = (a - b).abs() % TAU;
    raw.min(TAU - raw)
}

#[cfg(test)]
mod tests {
    use probe_sim::RockId;
    use probe_sim::roster::Roster;

    use super::*;

    const CENTRE: Pos2 = egui::pos2(400.0, 300.0);

    const SEAT: SeatId = SeatId(0);

    fn inner_place() -> Place {
        Place {
            rock: RockId(0),
            band: Band::Inner,
        }
    }

    fn outer_place() -> Place {
        Place {
            rock: RockId(0),
            band: Band::Outer,
        }
    }

    #[test]
    fn every_slot_is_found_at_its_mid_radius_and_the_centre_finds_none() {
        let roster = Roster::shipped();
        let wheel = Wheel::open(inner_place(), SEAT, &roster, CENTRE);

        assert_eq!(wheel.slot_at(CENTRE), None);

        for slot in &wheel.slots {
            let at = wheel.at(RADIUS, slot.angle);
            assert_eq!(wheel.slot_at(at).map(|(row, _)| row), Some(slot.row));
        }
    }

    #[test]
    fn the_outer_half_of_a_slot_is_plus_and_the_inner_half_minus() {
        let roster = Roster::shipped();
        let wheel = Wheel::open(inner_place(), SEAT, &roster, CENTRE);
        let slot = &wheel.slots[0];

        let outside = wheel.at(RADIUS + BAND_WIDTH * 0.5, slot.angle);
        let inside = wheel.at(RADIUS - BAND_WIDTH * 0.5, slot.angle);
        let beyond = wheel.at(RADIUS + BAND_WIDTH * 1.5, slot.angle);

        assert_eq!(wheel.slot_at(outside), Some((slot.row, WheelBand::Plus)));
        assert_eq!(wheel.slot_at(inside), Some((slot.row, WheelBand::Minus)));
        assert_eq!(wheel.slot_at(beyond), None);
    }

    #[test]
    fn structures_sit_left_of_centre_and_units_right() {
        let roster = Roster::shipped();
        let wheel = Wheel::open(inner_place(), SEAT, &roster, CENTRE);

        for slot in &wheel.slots {
            let is_structure = roster[slot.row].kind() == Kind::Structure;
            let (sin, _) = slot.angle.sin_cos();
            if is_structure {
                assert!(sin <= 0.0, "a structure at angle {}", slot.angle);
            } else {
                assert!(sin >= 0.0, "a unit at angle {}", slot.angle);
            }
        }
    }

    #[test]
    fn an_outer_band_wheel_has_no_structure_slot() {
        let roster = Roster::shipped();
        let wheel = Wheel::open(outer_place(), SEAT, &roster, CENTRE);

        assert!(
            wheel
                .slots
                .iter()
                .all(|slot| roster[slot.row].kind() != Kind::Structure)
        );
    }

    #[test]
    fn plus_adds_one_and_minus_removes_one_and_never_below_zero() {
        let place = inner_place();
        let row = RowId(0);

        assert_eq!(
            WheelBand::Plus.edit(place, row, 3),
            Command::Want {
                place,
                row,
                count: 4
            }
        );
        assert_eq!(
            WheelBand::Minus.edit(place, row, 3),
            Command::Want {
                place,
                row,
                count: 2
            }
        );
        assert_eq!(
            WheelBand::Minus.edit(place, row, 0),
            Command::Want {
                place,
                row,
                count: 0
            }
        );
    }

    #[test]
    fn a_slot_glyph_sits_at_its_sectors_mid_angle_and_the_wheels_radius() {
        let roster = Roster::shipped();
        let wheel = Wheel::open(inner_place(), SEAT, &roster, CENTRE);

        for slot in &wheel.slots {
            let centre = wheel.at(RADIUS, slot.angle);
            assert!(
                (centre.distance(CENTRE) - RADIUS).abs() <= 1e-3,
                "the glyph sits off the wheel's radius"
            );
            let edges = [slot.angle - slot.half_share, slot.angle + slot.half_share];
            let [low, high] = edges.map(|edge| angular_distance(edge, slot.angle));
            assert!((low - high).abs() <= 1e-5, "the glyph sits off mid angle");
            assert_eq!(
                wheel.slot_at(centre).map(|(row, _)| row),
                Some(slot.row),
                "and inside its own sector"
            );
        }
    }

    #[test]
    fn every_slot_glyph_takes_the_wheels_own_size_by_its_cost_class() {
        let roster = Roster::shipped();
        let wheel = Wheel::open(inner_place(), SEAT, &roster, CENTRE);

        for slot in &wheel.slots {
            let half = GLYPH_HALF * slot.glyph.size.scale();
            assert_eq!(half, GLYPH_HALF * Glyph::of(&roster[slot.row]).size.scale());
            assert!(
                half <= BAND_WIDTH,
                "a slot's glyph stands within its band's width"
            );
        }
    }
}
