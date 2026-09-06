use std::collections::BTreeMap;

use mirage_engine::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use neumannarch_sim::roster::{Kind, Roster, Row};
use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::Building;
use neumannarch_sim::{RockId, RowId, SeatId};

use crate::display::glyph::{self, Glyph};
use crate::display::glyph_quad::seat_color32;
use crate::display::hue;
use crate::display::scene::{Arc, Entry, Fill, SectorView, Shown, WheelBand, WheelView};
use crate::display::stencil::{self, Cell, Stencil};
use crate::display::wheels::Spoken;
use crate::screens::panel;

pub const PICK_RADIUS: f32 = 36.0;

const INNER: f32 = 80.0;

const DEPTH: f32 = 420.0;

const SMALL_SCALE: f32 = 0.72;

pub(crate) const GLYPH_HALF: f32 = 13.0;

pub(crate) const GLYPH_SLOT: f32 = 2.0 * GLYPH_HALF * glyph::WIDEST_SCALE;

pub(crate) const SECTION_HEIGHT: f32 = 36.0;

pub(crate) const LINE_HEIGHT: f32 = 13.0;

pub(crate) const CHARACTER_WIDTH: f32 = LINE_HEIGHT * 0.6;

pub(crate) const MARK: f32 = 10.0;

pub(crate) const GAP: f32 = 4.0;

pub(crate) const CELL_GAP: f32 = 6.0;

pub(crate) const PAD: f32 = 3.0;

pub(crate) const ROW_GAP: f32 = 3.0;

const SECTOR_GAP: f32 = 12.0;

const STRIPS_PER_COLUMN: usize = 4;

const COLUMN_GAP: f32 = 8.0;

const STEP_WIDTH: f32 = GAP + 2.0 * CHARACTER_WIDTH + GAP;

const BAND_WIDTH: f32 = 24.0;

const BAR_LENGTH: f32 = 1.5 * SECTION_HEIGHT;

const SPINE_INSET: f32 = 4.0;

const SPINE_WIDTH: f32 = 1.0;

const SPINE_ALPHA: f32 = 0.6;

const BAR_WIDTH: f32 = 3.0;

const BAR_TRAIL: Color32 = Color32::from_gray(235);

const SEGMENTS: usize = 12;

pub(crate) const SCRIM: Color32 = Color32::from_rgba_premultiplied(5, 6, 14, 150);

pub(crate) const CORNER: f32 = 2.0;

const SIGN_SIZE: f32 = 15.0;

const STEP_SIZE: f32 = 12.0;

const PREVIEW_ALPHA: f32 = 0.5;

pub(crate) const MARK_STROKE: f32 = 1.2;

const DASH_LENGTH: f32 = 2.0;

const DASH_GAP: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Full,
    Small,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bands {
    pub step: u32,
    pub wants: BTreeMap<RowId, u32>,
    pub refused: BTreeMap<RowId, String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizing {
    pub detail: Detail,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub rock: RockId,
    pub centre: Pos2,
    pub sizing: Sizing,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub rock: RockId,
    pub centre: Pos2,
    full: Rect,
    small: Rect,
}

pub struct Wheel {
    rock: RockId,
    centre: Pos2,
    scale: f32,
    alpha: f32,
    sectors: Vec<Sector>,
    bands: Option<Bands>,
}

struct Sector {
    seat: SeatId,
    top: f32,
    bottom: f32,
    arc: Option<Arc>,
    edits: bool,
    slots: Vec<Slot>,
}

struct Slot {
    row: RowId,
    glyph: Glyph,
    frame: Rect,
    lines: Vec<Line>,
}

struct Line {
    mark: Mark,
    count: u32,
    cell: Rect,
    entries: Vec<Shown>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Here,
    Surplus,
    Leaving,
    Arriving,
    Wanted,
}

impl Footprint {
    pub fn of(
        rock: RockId,
        centre: Pos2,
        view: &WheelView,
        roster: &Roster,
        seat: SeatId,
    ) -> Footprint {
        let bounds = |detail: Detail| {
            let edits = detail == Detail::Full;
            stacked(centre, view, roster, seat, Sizing::settled(detail), edits)
                .iter()
                .flat_map(|sector| &sector.slots)
                .fold(pick_square(centre), |bounds, slot| bounds.union(slot.frame))
        };
        Footprint {
            rock,
            centre,
            full: bounds(Detail::Full),
            small: bounds(Detail::Small),
        }
    }

    pub fn at(&self, detail: Detail) -> Rect {
        match detail {
            Detail::Full => self.full,
            Detail::Small => self.small,
        }
    }
}

impl Wheel {
    pub fn over(
        placed: Placed,
        view: &WheelView,
        roster: &Roster,
        seat: SeatId,
        bands: Option<Bands>,
    ) -> Wheel {
        let Placed {
            rock,
            centre,
            sizing,
            alpha,
        } = placed;
        Wheel {
            rock,
            centre,
            scale: sizing.scale,
            alpha,
            sectors: stacked(centre, view, roster, seat, sizing, bands.is_some()),
            bands,
        }
    }

    pub fn rock(&self) -> RockId {
        self.rock
    }

    pub fn centre(&self) -> Pos2 {
        self.centre
    }

    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    pub fn holds(&self, at: Pos2) -> bool {
        self.slots()
            .fold(pick_square(self.centre), |bounds, slot| {
                bounds.union(slot.frame)
            })
            .contains(at)
    }

    pub fn draws(&self) -> bool {
        !self.sectors.is_empty()
    }

    pub fn clear_of(&mut self, rect: Rect) {
        for sector in &mut self.sectors {
            sector.slots.retain(|slot| !slot.frame.intersects(rect));
        }
        self.sectors
            .retain(|sector| !sector.slots.is_empty() || sector.arc.is_some());
    }

    pub fn frames(&self) -> impl Iterator<Item = Rect> + '_ {
        self.slots().map(|slot| slot.frame)
    }

    pub fn band(&self, row: RowId, band: WheelBand) -> Option<Pos2> {
        let slot = self
            .sectors
            .iter()
            .filter(|sector| sector.edits)
            .flat_map(|sector| &sector.slots)
            .find(|slot| slot.row == row)?;
        Some(slot.band(band, self.scale).center())
    }

    pub fn band_at(&self, at: Pos2) -> Option<(RowId, WheelBand)> {
        let bands = self.bands.as_ref()?;
        self.band_under(at)
            .filter(|(row, _)| !bands.refused.contains_key(row))
    }

    pub fn spoken_at(&self, at: Pos2) -> Option<(Pos2, Spoken)> {
        self.slots()
            .find_map(|slot| {
                let line = slot.lines.iter().find(|line| line.cell.contains(at));
                match line {
                    Some(line) => Some((
                        line.cell.center(),
                        Spoken::Row {
                            row: slot.row,
                            shown: line.speaks(),
                        },
                    )),
                    None => slot.glyph_rect(self.scale).contains(at).then(|| {
                        (
                            slot.glyph_rect(self.scale).center(),
                            Spoken::Row {
                                row: slot.row,
                                shown: None,
                            },
                        )
                    }),
                }
            })
            .or_else(|| self.refusal_at(at))
    }

    fn refusal_at(&self, at: Pos2) -> Option<(Pos2, Spoken)> {
        let bands = self.bands.as_ref()?;
        let (row, _) = self.band_under(at)?;
        let why = bands.refused.get(&row)?;
        let slot = self.slots().find(|slot| slot.row == row)?;
        Some((
            egui::pos2(slot.frame.right(), slot.frame.center().y),
            Spoken::Refused { why: why.clone() },
        ))
    }

    fn band_under(&self, at: Pos2) -> Option<(RowId, WheelBand)> {
        let step = self.bands.as_ref()?.step;
        self.sectors
            .iter()
            .filter(|sector| sector.edits)
            .flat_map(|sector| &sector.slots)
            .find_map(|slot| {
                [WheelBand::Plus(step), WheelBand::Minus(step)]
                    .into_iter()
                    .find(|band| slot.band(*band, self.scale).contains(at))
                    .map(|band| (slot.row, band))
            })
    }

    pub fn paint(&self, painter: &egui::Painter, hovered: Option<(RowId, WheelBand)>) {
        for sector in &self.sectors {
            self.paint_spine(painter, sector);
            if let Some(arc) = sector.arc {
                self.paint_bar(painter, sector, arc);
            }
            for slot in &sector.slots {
                self.paint_slot(painter, slot, sector.seat);
                if sector.edits {
                    self.paint_bands(painter, slot, hovered);
                }
            }
        }
    }

    fn slots(&self) -> impl Iterator<Item = &Slot> {
        self.sectors.iter().flat_map(|sector| &sector.slots)
    }

    fn faded(&self, colour: Color32) -> Color32 {
        colour.gamma_multiply(self.alpha)
    }

    fn spine(&self, from: f32, to: f32) -> Vec<Pos2> {
        spine_points(self.centre, self.scale, from, to, Side::Right)
    }

    fn paint_spine(&self, painter: &egui::Painter, sector: &Sector) {
        painter.add(Shape::line(
            self.spine(sector.top, sector.bottom),
            spine_stroke(seat_color32(sector.seat), self.alpha),
        ));
    }

    fn paint_bar(&self, painter: &egui::Painter, sector: &Sector, arc: Arc) {
        let length = BAR_LENGTH * self.scale;
        let at = |fraction: f32| sector.top + length * fraction.clamp(0.0, 1.0);
        self.paint_bar_segment(
            painter,
            sector.top,
            at(arc.fraction),
            self.faded(seat_color32(sector.seat)),
        );
        self.paint_bar_segment(
            painter,
            at(arc.fraction),
            at(arc.trailing),
            self.faded(BAR_TRAIL),
        );
    }

    fn paint_bar_segment(&self, painter: &egui::Painter, from: f32, to: f32, colour: Color32) {
        if to <= from {
            return;
        }
        painter.add(Shape::line(
            self.spine(from, to),
            Stroke::new(BAR_WIDTH, colour),
        ));
    }

    fn paint_slot(&self, painter: &egui::Painter, slot: &Slot, seat: SeatId) {
        let colour = seat_color32(seat);
        painter.rect_filled(slot.frame, CORNER, self.faded(SCRIM));
        let (fill, alpha) = match slot.lines.iter().any(|line| !line.previewed()) {
            true => (Fill::Solid, self.alpha),
            false => (Fill::Hollow, self.alpha * stencil::DIM_ALPHA),
        };
        Stencil {
            glyph: &slot.glyph,
            cell: Cell {
                centre: slot.glyph_rect(self.scale).center(),
                half: GLYPH_HALF * slot.glyph.size.scale() * self.scale,
            },
            colour,
            outline: Color32::WHITE,
            fill,
            alpha,
            starved: None,
        }
        .paint(painter);
        for line in &slot.lines {
            self.paint_line(painter, line, colour);
        }
    }

    fn paint_line(&self, painter: &egui::Painter, line: &Line, colour: Color32) {
        let colour = match line.previewed() {
            true => self.faded(colour.gamma_multiply(PREVIEW_ALPHA)),
            false => self.faded(colour),
        };
        self.paint_mark(painter, line, colour);
        painter.text(
            egui::pos2(
                line.cell.left() + (MARK + GAP) * self.scale,
                line.cell.center().y,
            ),
            Align2::LEFT_CENTER,
            line.count.to_string(),
            FontId::monospace(LINE_HEIGHT * self.scale),
            colour,
        );
    }

    fn paint_mark(&self, painter: &egui::Painter, line: &Line, colour: Color32) {
        let scale = self.scale;
        let half = MARK * scale / 2.0;
        let at = egui::pos2(line.cell.left() + half, line.cell.center().y);
        let stroke = Stroke::new(MARK_STROKE * scale, colour);
        match line.mark {
            Mark::Here => {
                painter.circle_filled(at, half * 0.7, colour);
            }
            Mark::Surplus => {
                painter.circle_stroke(at, half * 0.7, stroke);
            }
            Mark::Leaving | Mark::Arriving => {
                let out = match line.mark {
                    Mark::Arriving => -half,
                    _ => half,
                };
                painter.add(Shape::convex_polygon(
                    vec![
                        egui::pos2(at.x + out, at.y),
                        egui::pos2(at.x - out, at.y - half),
                        egui::pos2(at.x - out, at.y + half),
                    ],
                    colour,
                    Stroke::NONE,
                ));
            }
            Mark::Wanted => {
                let square = Rect::from_center_size(at, Vec2::splat(2.0 * half));
                if let Some(building) = line.building() {
                    self.paint_frame(painter, square, building, colour);
                }
                match line.dashed() {
                    true => {
                        let corners = [
                            square.left_top(),
                            square.right_top(),
                            square.right_bottom(),
                            square.left_bottom(),
                            square.left_top(),
                        ];
                        painter.extend(Shape::dashed_line(
                            &corners,
                            stroke,
                            DASH_LENGTH * scale,
                            DASH_GAP * scale,
                        ));
                    }
                    false => {
                        painter.rect_stroke(square, 0.0, stroke, egui::StrokeKind::Middle);
                    }
                }
            }
        }
    }

    fn paint_frame(
        &self,
        painter: &egui::Painter,
        square: Rect,
        building: Building,
        colour: Color32,
    ) {
        let filled = Rect::from_min_max(
            egui::pos2(
                square.left(),
                square.bottom() - square.height() * building.progress as f32,
            ),
            square.max,
        );
        painter.rect_filled(filled, 0.0, colour);
        if let Some(material) = building.starved_of {
            painter.line_segment(
                [square.left_bottom(), square.right_bottom()],
                Stroke::new(
                    MARK_STROKE * self.scale * 2.0,
                    self.faded(hue::of(material)),
                ),
            );
        }
    }

    fn paint_bands(
        &self,
        painter: &egui::Painter,
        slot: &Slot,
        hovered: Option<(RowId, WheelBand)>,
    ) {
        let Some(bands) = &self.bands else {
            return;
        };
        let want = bands.wants.get(&slot.row).copied().unwrap_or(0);
        let refused = bands.refused.contains_key(&slot.row);
        for band in [WheelBand::Plus(bands.step), WheelBand::Minus(bands.step)] {
            let at = slot.band(band, self.scale);
            let spent = refused || band.wanted(want) == want;
            let over = hovered == Some((slot.row, band));
            if over && !spent {
                painter.rect_filled(at, 0.0, self.faded(panel::HOVER_FILL));
                self.paint_delta(painter, slot, band);
            }
            painter.rect_stroke(
                at,
                0.0,
                Stroke::new(1.0, self.faded(panel::LINE)),
                egui::StrokeKind::Inside,
            );
            let ink = match (spent, over) {
                (true, _) => panel::DIM_INK,
                (false, true) => Color32::WHITE,
                (false, false) => panel::INK,
            };
            let size = match band.step() {
                1 => SIGN_SIZE,
                _ => STEP_SIZE,
            };
            painter.text(
                at.center(),
                Align2::CENTER_CENTER,
                band.label(),
                FontId::monospace(size * self.scale),
                self.faded(ink),
            );
        }
    }

    fn paint_delta(&self, painter: &egui::Painter, slot: &Slot, band: WheelBand) {
        painter.text(
            egui::pos2(slot.frame.right() + GAP * self.scale, slot.frame.center().y),
            Align2::LEFT_CENTER,
            band.delta(),
            FontId::monospace(LINE_HEIGHT * self.scale),
            self.faded(Color32::WHITE),
        );
    }
}

impl Sizing {
    pub fn settled(detail: Detail) -> Sizing {
        Sizing {
            detail,
            scale: detail.scale(),
        }
    }
}

impl Detail {
    pub fn scale(self) -> f32 {
        match self {
            Detail::Full => 1.0,
            Detail::Small => SMALL_SCALE,
        }
    }

    fn shows(self, mark: Mark, entries: &[Shown]) -> bool {
        match self {
            Detail::Full => true,
            Detail::Small => {
                mark != Mark::Wanted
                    || entries
                        .iter()
                        .all(|shown| matches!(shown.entry, Entry::Placed))
            }
        }
    }
}

impl Slot {
    fn glyph_rect(&self, scale: f32) -> Rect {
        Rect::from_min_size(
            egui::pos2(self.frame.left() + PAD * scale, self.frame.top()),
            egui::vec2(GLYPH_SLOT * scale, self.frame.height()),
        )
    }

    fn band(&self, band: WheelBand, scale: f32) -> Rect {
        let left = self.frame.left() + (PAD + GLYPH_SLOT + GAP) * scale;
        let pad = PAD * scale;
        let (top, bottom) = match band {
            WheelBand::Plus(_) => (self.frame.top() + pad, self.frame.center().y),
            WheelBand::Minus(_) => (self.frame.center().y, self.frame.bottom() - pad),
        };
        Rect::from_min_max(
            egui::pos2(left, top),
            egui::pos2(left + BAND_WIDTH * scale, bottom),
        )
    }
}

impl Line {
    fn previewed(&self) -> bool {
        self.entries.iter().any(|shown| shown.previewed)
    }

    fn building(&self) -> Option<Building> {
        self.entries.iter().find_map(|shown| match shown.entry {
            Entry::Building(building) => Some(building),
            _ => None,
        })
    }

    fn dashed(&self) -> bool {
        self.entries
            .iter()
            .any(|shown| matches!(shown.entry, Entry::Wanted { dashed: true, .. }))
    }

    fn speaks(&self) -> Option<Shown> {
        self.entries.iter().copied().max_by_key(says)
    }
}

impl WheelBand {
    pub fn edit(self, rock: RockId, row: RowId, want: u32) -> Command {
        Command::Want {
            rock,
            row,
            count: self.wanted(want),
        }
    }

    pub fn delta(self) -> String {
        match self {
            WheelBand::Plus(step) => format!("+{step}"),
            WheelBand::Minus(step) => format!("-{step}"),
        }
    }
}

fn says(shown: &Shown) -> u8 {
    match shown.entry {
        Entry::Building(Building {
            starved_of: Some(_),
            ..
        }) => 3,
        Entry::Wanted { dashed: true, .. } => 2,
        Entry::Leaving { .. } | Entry::Arriving { .. } => 1,
        _ => 0,
    }
}

fn to_come(entry: Entry) -> u32 {
    match entry {
        Entry::Building(_) => 1,
        other => other.count().unwrap_or(0),
    }
}

fn digits(count: u32) -> f32 {
    count.to_string().len() as f32
}

fn pick_square(centre: Pos2) -> Rect {
    Rect::from_center_size(centre, Vec2::splat(2.0 * PICK_RADIUS))
}

pub(crate) fn x_at(scale: f32, y: f32) -> f32 {
    let depth = DEPTH * scale;
    let radius = depth + INNER * scale;
    (radius * radius - y * y).max(0.0).sqrt() - depth
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Right,
    Left,
}

pub(crate) fn spine_points(centre: Pos2, scale: f32, from: f32, to: f32, side: Side) -> Vec<Pos2> {
    let inset = SPINE_INSET * scale;
    (0..=SEGMENTS)
        .map(|step| {
            let y = from + (to - from) * step as f32 / SEGMENTS as f32;
            let out = x_at(scale, y) - inset;
            let x = match side {
                Side::Right => centre.x + out,
                Side::Left => centre.x - out,
            };
            egui::pos2(x, centre.y + y)
        })
        .collect()
}

pub(crate) fn spine_stroke(colour: Color32, alpha: f32) -> Stroke {
    Stroke::new(
        SPINE_WIDTH,
        colour.gamma_multiply(SPINE_ALPHA).gamma_multiply(alpha),
    )
}

fn stacked(
    centre: Pos2,
    view: &WheelView,
    roster: &Roster,
    seat: SeatId,
    sizing: Sizing,
    edits: bool,
) -> Vec<Sector> {
    let Sizing { detail, scale } = sizing;
    let rows: Vec<(&SectorView, bool, Vec<Unlaid>)> = view
        .sectors
        .iter()
        .map(|sector| {
            let edits = edits && sector.seat == seat;
            (sector, edits, rows_of(sector, roster, detail, edits))
        })
        .filter(|(sector, _, rows)| !rows.is_empty() || sector.arc.is_some())
        .collect();
    let height = |rows: &[Unlaid], arc: bool| {
        let tall = rows.len().min(STRIPS_PER_COLUMN);
        let stacked =
            tall as f32 * SECTION_HEIGHT * scale + tall.saturating_sub(1) as f32 * ROW_GAP * scale;
        match arc {
            true => stacked.max(BAR_LENGTH * scale),
            false => stacked,
        }
    };
    let total: f32 = rows
        .iter()
        .map(|(sector, _, rows)| height(rows, sector.arc.is_some()))
        .sum::<f32>()
        + rows.len().saturating_sub(1) as f32 * SECTOR_GAP * scale;
    let mut top = -total / 2.0;
    rows.into_iter()
        .map(|(sector, edits, rows)| {
            let bottom = top + height(&rows, sector.arc.is_some());
            let mut across = 0.0;
            let slots = rows
                .chunks(STRIPS_PER_COLUMN)
                .flat_map(|column| {
                    let left = across;
                    let widest = column
                        .iter()
                        .map(|unlaid| unlaid.reserved(scale, edits))
                        .fold(0.0, f32::max);
                    across += widest + COLUMN_GAP * scale;
                    column.iter().enumerate().map(move |(index, unlaid)| {
                        let y = top + index as f32 * (SECTION_HEIGHT + ROW_GAP) * scale;
                        unlaid.laid(centre, scale, y, left, edits)
                    })
                })
                .collect();
            let laid = Sector {
                seat: sector.seat,
                top,
                bottom,
                arc: sector.arc,
                edits,
                slots,
            };
            top = bottom + SECTOR_GAP * scale;
            laid
        })
        .collect()
}

struct Unlaid {
    row: RowId,
    glyph: Glyph,
    lines: Vec<(Mark, Vec<Shown>)>,
}

impl Unlaid {
    fn lead(scale: f32, edits: bool) -> f32 {
        let bands = match edits {
            true => BAND_WIDTH + GAP,
            false => 0.0,
        };
        (PAD + GLYPH_SLOT + GAP + bands) * scale
    }

    fn cells(&self) -> impl Iterator<Item = (Mark, &[Shown], u32, f32)> {
        self.lines.iter().map(|(mark, entries)| {
            let count = entries.iter().map(|shown| to_come(shown.entry)).sum();
            let width = MARK + GAP + digits(count) * CHARACTER_WIDTH + CELL_GAP;
            (*mark, entries.as_slice(), count, width)
        })
    }

    fn width(&self, scale: f32, edits: bool) -> f32 {
        let cells: f32 = self.cells().map(|(_, _, _, width)| width).sum();
        let trailing = match self.lines.is_empty() {
            true => -GAP,
            false => 0.0,
        };
        Self::lead(scale, edits) + (cells + trailing + PAD) * scale
    }

    fn reserved(&self, scale: f32, edits: bool) -> f32 {
        let grown: f32 = match self.lines.is_empty() {
            true => MARK + GAP + CHARACTER_WIDTH + CELL_GAP - GAP,
            false => self
                .cells()
                .map(|(_, _, _, width)| width + CHARACTER_WIDTH)
                .sum(),
        };
        let step = match edits {
            true => STEP_WIDTH,
            false => 0.0,
        };
        Self::lead(scale, edits) + (grown + step + PAD) * scale
    }

    fn laid(&self, centre: Pos2, scale: f32, top: f32, across: f32, edits: bool) -> Slot {
        let height = SECTION_HEIGHT * scale;
        let left = centre.x + x_at(scale, top + height / 2.0) + across;
        let width = self.width(scale, edits);
        let top = centre.y + top;
        let cell_top = top + (height - LINE_HEIGHT * scale) / 2.0;
        let mut right = left + Self::lead(scale, edits);
        let lines: Vec<Line> = self
            .cells()
            .map(|(mark, entries, count, width)| {
                let cell = Rect::from_min_size(
                    egui::pos2(right, cell_top),
                    egui::vec2(width * scale, LINE_HEIGHT * scale),
                );
                right += width * scale;
                Line {
                    mark,
                    count,
                    cell,
                    entries: entries.to_vec(),
                }
            })
            .collect();
        Slot {
            row: self.row,
            glyph: self.glyph.clone(),
            frame: Rect::from_min_size(egui::pos2(left, top), egui::vec2(width, height)),
            lines,
        }
    }
}

fn rows_of(view: &SectorView, roster: &Roster, detail: Detail, edits: bool) -> Vec<Unlaid> {
    let mut structures: Vec<(RowId, &Row)> = Vec::new();
    let mut units: Vec<(RowId, &Row)> = Vec::new();
    for (row, data) in roster.iter() {
        match data.kind() {
            Kind::Structure => structures.push((row, data)),
            Kind::Unit => units.push((row, data)),
        }
    }
    by_cost_descending(&mut structures);
    by_cost_descending(&mut units);
    structures
        .into_iter()
        .chain(units)
        .map(|(row, data)| {
            let entries: &[Shown] = view
                .rows
                .iter()
                .find(|shown| shown.row == row)
                .map_or(&[], |shown| &shown.entries);
            Unlaid {
                row,
                glyph: Glyph::of(data),
                lines: lines(entries)
                    .into_iter()
                    .filter(|(mark, entries)| !entries.is_empty() && detail.shows(*mark, entries))
                    .collect(),
            }
        })
        .filter(|unlaid| edits || !unlaid.lines.is_empty())
        .collect()
}

fn lines(entries: &[Shown]) -> [(Mark, Vec<Shown>); 4] {
    let of = |wanted: &[Mark]| -> Vec<Shown> {
        entries
            .iter()
            .filter(|shown| wanted.contains(&marked(shown.entry)))
            .copied()
            .collect()
    };
    let moving = of(&[Mark::Leaving, Mark::Arriving]);
    let mark = match moving.first().map(|shown| marked(shown.entry)) {
        Some(mark) => mark,
        None => Mark::Arriving,
    };
    [
        (Mark::Here, of(&[Mark::Here])),
        (Mark::Surplus, of(&[Mark::Surplus])),
        (mark, moving),
        (Mark::Wanted, of(&[Mark::Wanted])),
    ]
}

fn marked(entry: Entry) -> Mark {
    match entry {
        Entry::Present(_) => Mark::Here,
        Entry::Surplus(_) => Mark::Surplus,
        Entry::Leaving { .. } => Mark::Leaving,
        Entry::Arriving { .. } => Mark::Arriving,
        Entry::Building(_) | Entry::Wanted { .. } | Entry::Placed => Mark::Wanted,
    }
}

fn by_cost_descending(rows: &mut [(RowId, &Row)]) {
    rows.sort_by(|(_, a), (_, b)| b.cost.total().total_cmp(&a.cost.total()));
}

const _: () = assert!(GLYPH_SLOT < SECTION_HEIGHT);

#[cfg(test)]
mod tests {
    use neumannarch_sim::roster::{FRIGATE, SHIPYARD};
    use neumannarch_sim::state::MAX_WANT;

    use super::*;
    use crate::display::scene::RowView;

    const ROCK: RockId = RockId(0);

    const MINE: SeatId = SeatId(0);

    const THEIRS: SeatId = SeatId(1);

    const CENTRE: Pos2 = egui::pos2(400.0, 300.0);

    fn shown(entry: Entry) -> Shown {
        Shown {
            entry,
            previewed: false,
        }
    }

    fn row(row: RowId, entries: Vec<Entry>) -> RowView {
        RowView {
            row,
            entries: entries.into_iter().map(shown).collect(),
        }
    }

    fn sector(seat: SeatId, rows: Vec<RowView>) -> SectorView {
        SectorView {
            seat,
            rows,
            arc: None,
        }
    }

    fn wheel(sectors: Vec<SectorView>, detail: Detail, bands: Option<Bands>) -> Wheel {
        let roster = Roster::shipped();
        Wheel::over(
            Placed {
                rock: ROCK,
                centre: CENTRE,
                sizing: Sizing::settled(detail),
                alpha: 1.0,
            },
            &WheelView {
                rock: ROCK,
                sectors,
            },
            &roster,
            MINE,
            bands,
        )
    }

    fn selected() -> Bands {
        Bands {
            step: 1,
            wants: BTreeMap::from([(FRIGATE, 2)]),
            refused: BTreeMap::new(),
        }
    }

    fn held() -> Vec<SectorView> {
        vec![sector(
            MINE,
            vec![row(
                FRIGATE,
                vec![
                    Entry::Present(2),
                    Entry::Arriving {
                        count: 3,
                        from: RockId(4),
                    },
                    Entry::Wanted {
                        count: 1,
                        dashed: false,
                    },
                ],
            )],
        )]
    }

    fn slots(wheel: &Wheel) -> impl Iterator<Item = (&Sector, &Slot)> {
        wheel
            .sectors
            .iter()
            .flat_map(|sector| sector.slots.iter().map(move |slot| (sector, slot)))
    }

    fn slot_of(wheel: &Wheel, row: RowId) -> &Slot {
        slots(wheel)
            .find(|(_, slot)| slot.row == row)
            .map(|(_, slot)| slot)
            .expect("the row has a section")
    }

    #[test]
    fn a_selected_wheel_stands_a_section_per_row_to_the_rocks_right_structures_first() {
        let roster = Roster::shipped();
        let wheel = wheel(
            vec![sector(MINE, Vec::new())],
            Detail::Full,
            Some(selected()),
        );

        let slots: Vec<&Slot> = slots(&wheel).map(|(_, slot)| slot).collect();
        assert_eq!(slots.len(), roster.iter().count());
        assert!(
            slots.iter().all(|slot| slot.frame.left() > CENTRE.x),
            "every section stands right of the rock"
        );
        assert!(
            slots.chunks(STRIPS_PER_COLUMN).all(|column| {
                column
                    .windows(2)
                    .all(|pair| pair[0].frame.bottom() < pair[1].frame.top())
            }),
            "sections stack down a column without touching"
        );
        let last_structure = slots
            .iter()
            .rposition(|slot| roster[slot.row].kind() == Kind::Structure)
            .expect("a structure");
        let first_unit = slots
            .iter()
            .position(|slot| roster[slot.row].kind() == Kind::Unit)
            .expect("a unit");
        assert!(last_structure < first_unit, "structures stand above units");
        let stack = slots
            .iter()
            .fold(slots[0].frame, |stack, slot| stack.union(slot.frame));
        assert!(
            (stack.center().y - CENTRE.y).abs() < 1.0,
            "the stack is centred on the rock's height"
        );
    }

    #[test]
    fn sections_stand_on_an_arc_that_bows_away_from_the_rock() {
        let wheel = wheel(
            vec![sector(MINE, Vec::new())],
            Detail::Full,
            Some(selected()),
        );

        let slots: Vec<&Slot> = slots(&wheel)
            .map(|(_, slot)| slot)
            .take(STRIPS_PER_COLUMN)
            .collect();
        let middle = slots[slots.len() / 2];
        for end in [slots[0], slots[slots.len() - 1]] {
            assert!(
                end.frame.left() < middle.frame.left(),
                "the ends of the stack stand nearer the rock than its middle"
            );
        }
    }

    #[test]
    fn a_section_is_one_strip_with_its_counts_in_a_row_beside_its_glyph() {
        let full = wheel(held(), Detail::Full, None);

        let slot = slot_of(&full, FRIGATE);
        assert_eq!(slot.lines.len(), 3, "here, arriving and wanted");
        assert_eq!(slot.lines[0].count, 2);
        assert_eq!(slot.lines[1].count, 3);
        assert_eq!(slot.lines[2].count, 1);
        assert!(
            slot.lines
                .windows(2)
                .all(|pair| pair[0].cell.right() <= pair[1].cell.left()),
            "the counts run left to right without overlapping"
        );
        assert!(
            slot.lines
                .windows(2)
                .all(|pair| pair[0].cell.center().y == pair[1].cell.center().y),
            "on one line"
        );
        assert!(
            slot.glyph_rect(1.0).right() <= slot.lines[0].cell.left(),
            "the glyph stands left of the counts"
        );
        assert!(
            slot.frame.contains_rect(slot.lines[2].cell),
            "and the strip holds every count"
        );

        let wanted_alone = wheel(
            vec![sector(
                MINE,
                vec![row(
                    FRIGATE,
                    vec![Entry::Wanted {
                        count: 1,
                        dashed: false,
                    }],
                )],
            )],
            Detail::Full,
            None,
        );
        assert_eq!(
            slot_of(&wanted_alone, FRIGATE).lines[0].cell.left(),
            slot.lines[0].cell.left(),
            "a lone count stands where the first count stands"
        );
    }

    #[test]
    fn a_wide_count_widens_its_cell_and_the_strip_with_it() {
        let counted = |count| {
            let wheel = wheel(
                vec![sector(
                    MINE,
                    vec![row(FRIGATE, vec![Entry::Present(count)])],
                )],
                Detail::Full,
                None,
            );
            let slot = slot_of(&wheel, FRIGATE);
            (slot.lines[0].cell.width(), slot.frame.width())
        };

        let (one, narrow) = counted(7);
        let (three, wide) = counted(MAX_WANT);
        assert!(three > one);
        assert!(wide > narrow);
    }

    #[test]
    fn a_row_at_rest_stands_as_its_glyph_alone_and_only_where_it_takes_an_edit() {
        let open = wheel(
            vec![sector(MINE, Vec::new())],
            Detail::Full,
            Some(selected()),
        );
        assert!(slot_of(&open, FRIGATE).lines.is_empty());

        let shut = wheel(vec![sector(MINE, Vec::new())], Detail::Full, None);
        assert!(
            !shut.draws(),
            "a wheel that takes no edit shows what stands there alone"
        );
    }

    #[test]
    fn a_small_wheel_shows_what_stands_and_moves_and_never_a_want() {
        let small = wheel(held(), Detail::Small, None);

        assert_eq!(slots(&small).count(), 1, "only the row it holds");
        let slot = slot_of(&small, FRIGATE);
        assert_eq!(slot.lines.len(), 2);
        assert_eq!(slot.lines[0].count, 2, "what stands there");
        assert_eq!(slot.lines[1].count, 3, "and what is on its way");
        assert!(
            slot.frame.height()
                < slot_of(&wheel(held(), Detail::Full, None), FRIGATE)
                    .frame
                    .height()
        );

        let placed = wheel(
            vec![sector(THEIRS, vec![row(SHIPYARD, vec![Entry::Placed])])],
            Detail::Small,
            None,
        );
        let slot = slot_of(&placed, SHIPYARD);
        assert_eq!(
            slot.lines.len(),
            1,
            "a draft placement is the one want it shows"
        );
        assert_eq!(slot.lines[0].mark, Mark::Wanted);
        assert_eq!(slot.lines[0].count, 1);
    }

    #[test]
    fn several_seats_stack_from_the_top_in_seat_order_each_in_its_own_sector() {
        let theirs = vec![
            row(FRIGATE, vec![Entry::Present(1)]),
            row(SHIPYARD, vec![Entry::Present(1)]),
        ];
        let wheel = wheel(
            vec![sector(MINE, Vec::new()), sector(THEIRS, theirs)],
            Detail::Full,
            Some(selected()),
        );

        assert_eq!(wheel.sectors.len(), 2);
        assert_eq!(wheel.sectors[0].seat, MINE);
        assert_eq!(wheel.sectors[1].seat, THEIRS);
        assert!(
            wheel.sectors[0].bottom < wheel.sectors[1].top,
            "the first seat stands above the second"
        );
        for sector in &wheel.sectors {
            for slot in &sector.slots {
                let (top, bottom) = (CENTRE.y + sector.top, CENTRE.y + sector.bottom);
                assert!(
                    slot.frame.top() >= top - 0.5 && slot.frame.bottom() <= bottom + 0.5,
                    "every section stands in its own seat's sector"
                );
            }
        }
    }

    #[test]
    fn a_sector_taller_than_the_column_wraps_into_the_next_column_beside_it() {
        let roster = Roster::shipped();
        let every = roster
            .iter()
            .map(|(id, _)| row(id, vec![Entry::Present(1)]))
            .collect();
        let wheel = wheel(vec![sector(THEIRS, every)], Detail::Full, None);

        let slots: Vec<&Slot> = slots(&wheel).map(|(_, slot)| slot).collect();
        assert!(
            slots.len() > STRIPS_PER_COLUMN,
            "the roster overflows one column"
        );
        let (first, second) = slots.split_at(STRIPS_PER_COLUMN);
        let first_right = first
            .iter()
            .map(|slot| slot.frame.right())
            .fold(0.0, f32::max);
        assert!(
            second.iter().all(|slot| slot.frame.left() >= first_right),
            "the overflow stands beside the first column"
        );
        assert_eq!(second[0].frame.top(), first[0].frame.top(), "from its top");
        let sector = &wheel.sectors[0];
        assert!(
            sector.bottom - sector.top
                < (STRIPS_PER_COLUMN as f32 + 0.5) * (SECTION_HEIGHT + ROW_GAP),
            "the sector is no taller than one column"
        );
    }

    #[test]
    fn a_strip_that_gains_a_digit_and_a_signed_step_stays_clear_of_the_next_column() {
        let counted = |count| {
            wheel(
                vec![sector(
                    MINE,
                    vec![row(FRIGATE, vec![Entry::Present(count)])],
                )],
                Detail::Full,
                Some(selected()),
            )
        };
        let nine = counted(9);
        let ten = counted(10);
        let mine = slot_of(&nine, FRIGATE).frame.left();
        let next = slots(&nine)
            .map(|(_, slot)| slot.frame.left())
            .filter(|left| *left > mine + 1.0)
            .fold(f32::INFINITY, f32::min);
        assert!(
            next.is_finite(),
            "the roster wraps past the frigate's column"
        );

        let grown = slot_of(&ten, FRIGATE).frame.right() + STEP_WIDTH;
        assert!(
            grown <= next,
            "{grown} runs under the next column at {next}"
        );
    }

    #[test]
    fn a_seat_in_a_fight_holds_room_for_its_bar_though_it_holds_nothing() {
        let arc = Arc {
            seat: THEIRS,
            fraction: 0.5,
            trailing: 0.6,
        };
        let wheel = wheel(
            vec![
                sector(MINE, vec![row(FRIGATE, vec![Entry::Present(1)])]),
                SectorView {
                    seat: THEIRS,
                    rows: Vec::new(),
                    arc: Some(arc),
                },
            ],
            Detail::Full,
            None,
        );

        assert_eq!(wheel.sectors.len(), 2);
        assert_eq!(wheel.sectors[1].arc, Some(arc));
        assert!(wheel.sectors[1].slots.is_empty());
        assert!(
            (wheel.sectors[1].bottom - wheel.sectors[1].top - BAR_LENGTH).abs() < 1e-3,
            "the empty sector is one bar tall"
        );
    }

    #[test]
    fn only_a_selected_wheels_own_sections_take_a_band() {
        let sectors = || {
            vec![
                sector(MINE, Vec::new()),
                sector(THEIRS, vec![row(FRIGATE, vec![Entry::Present(1)])]),
            ]
        };
        let open = wheel(sectors(), Detail::Full, Some(selected()));

        let plus = open
            .band(FRIGATE, WheelBand::Plus(1))
            .expect("the frigate's own section takes a band");
        let minus = open
            .band(FRIGATE, WheelBand::Minus(1))
            .expect("and both bands");
        assert!(plus.y < minus.y, "plus stands above minus");
        assert_eq!(open.band_at(plus), Some((FRIGATE, WheelBand::Plus(1))));
        assert_eq!(open.band_at(minus), Some((FRIGATE, WheelBand::Minus(1))));

        let slot = slot_of(&open, FRIGATE);
        assert_eq!(
            open.band_at(slot.glyph_rect(1.0).center()),
            None,
            "the glyph itself is no band"
        );
        let theirs = slots(&open)
            .find(|(sector, slot)| slot.row == FRIGATE && sector.seat == THEIRS)
            .map(|(_, slot)| slot)
            .expect("the rival holds the same rows");
        assert_eq!(
            open.band_at(theirs.band(WheelBand::Plus(1), 1.0).center()),
            None,
            "a rival's section takes no edit"
        );

        let shut = wheel(sectors(), Detail::Full, None);
        assert_eq!(shut.band_at(plus), None, "an unselected wheel takes none");
        assert_eq!(shut.band(FRIGATE, WheelBand::Plus(1)), None);
    }

    #[test]
    fn a_band_bears_its_step_and_a_band_that_would_change_nothing_wants_the_same() {
        assert_eq!(WheelBand::Plus(1).label(), "+");
        assert_eq!(WheelBand::Minus(1).label(), "-");
        assert_eq!(WheelBand::Plus(5).label(), "+5");
        assert_eq!(WheelBand::Minus(5).label(), "-5");
        assert_eq!(WheelBand::Plus(1).delta(), "+1");
        assert_eq!(WheelBand::Minus(5).delta(), "-5");

        assert_eq!(WheelBand::Plus(5).wanted(2), 7);
        assert_eq!(WheelBand::Minus(5).wanted(2), 0);
        assert_eq!(WheelBand::Plus(1).wanted(MAX_WANT), MAX_WANT);
        assert_eq!(WheelBand::Minus(1).wanted(0), 0);
        assert_eq!(
            WheelBand::Plus(1).edit(ROCK, FRIGATE, 2),
            Command::Want {
                rock: ROCK,
                row: FRIGATE,
                count: 3
            }
        );
    }

    #[test]
    fn what_is_under_the_pointer_says_what_it_is_and_why() {
        let wheel = wheel(held(), Detail::Full, None);
        let slot = slot_of(&wheel, FRIGATE);
        let at = |line: usize| slot.lines[line].cell.center();

        let said = |at: Pos2| wheel.spoken_at(at).map(|(_, spoken)| spoken);

        assert_eq!(
            said(at(0)),
            Some(Spoken::Row {
                row: FRIGATE,
                shown: Some(shown(Entry::Present(2)))
            })
        );
        assert_eq!(
            said(at(1)).map(|spoken| spoken.phrase(&Roster::shipped())),
            Some("Frigate arriving from Rock 5".to_string())
        );
        assert_eq!(
            said(slot.glyph_rect(1.0).center()),
            Some(Spoken::Row {
                row: FRIGATE,
                shown: None
            }),
            "the glyph names its row alone"
        );
        assert_eq!(said(CENTRE), None, "the middle carries nothing");
    }

    #[test]
    fn a_refused_band_takes_no_click_and_says_why_beside_its_strip() {
        let refused = Bands {
            refused: BTreeMap::from([(FRIGATE, "Not yet".to_string())]),
            ..selected()
        };
        let wheel = wheel(vec![sector(MINE, Vec::new())], Detail::Full, Some(refused));
        let slot = slot_of(&wheel, FRIGATE);
        let plus = slot.band(WheelBand::Plus(1), 1.0).center();

        assert_eq!(
            wheel.band_at(plus),
            None,
            "a refused band is no band to click"
        );
        assert_eq!(
            wheel.spoken_at(plus),
            Some((
                egui::pos2(slot.frame.right(), slot.frame.center().y),
                Spoken::Refused {
                    why: "Not yet".to_string()
                }
            ))
        );

        let live = slot_of(&wheel, SHIPYARD)
            .band(WheelBand::Plus(1), 1.0)
            .center();
        assert_eq!(wheel.band_at(live), Some((SHIPYARD, WheelBand::Plus(1))));
        assert_eq!(wheel.spoken_at(live), None, "a live band says nothing");
    }

    #[test]
    fn a_starved_frame_speaks_before_the_want_it_fills() {
        let starved = Building {
            progress: 0.5,
            starved_of: Some(neumannarch_sim::Material::Metals),
        };
        let wheel = wheel(
            vec![sector(
                MINE,
                vec![row(
                    FRIGATE,
                    vec![
                        Entry::Building(starved),
                        Entry::Wanted {
                            count: 1,
                            dashed: false,
                        },
                    ],
                )],
            )],
            Detail::Full,
            None,
        );
        let line = &slot_of(&wheel, FRIGATE).lines[0];

        assert_eq!(line.mark, Mark::Wanted);
        assert_eq!(line.count, 2, "the frame counts among what is to come");
        assert_eq!(line.building(), Some(starved));
        assert_eq!(
            line.speaks().map(|shown| shown.entry.phrase("Frigate")),
            Some("Frigate short of metals".to_string())
        );
    }

    #[test]
    fn a_section_of_a_rival_seat_shows_what_it_holds_and_takes_no_band() {
        let wheel = wheel(
            vec![sector(THEIRS, vec![row(SHIPYARD, vec![Entry::Present(1)])])],
            Detail::Small,
            Some(selected()),
        );

        assert_eq!(slots(&wheel).count(), 1);
        assert!(!wheel.sectors[0].edits);
    }

    #[test]
    fn a_footprint_covers_the_rock_and_every_section_at_each_size() {
        let roster = Roster::shipped();
        let view = WheelView {
            rock: ROCK,
            sectors: held(),
        };
        let footprint = Footprint::of(ROCK, CENTRE, &view, &roster, MINE);

        for detail in [Detail::Full, Detail::Small] {
            assert!(footprint.at(detail).contains(CENTRE), "the rock is on it");
        }
        let full = wheel(held(), Detail::Full, Some(selected()));
        let slot = slot_of(&full, FRIGATE);
        assert!(footprint.at(Detail::Full).contains_rect(slot.frame));
        assert!(
            !footprint.at(Detail::Small).contains_rect(slot.frame),
            "the small footprint is not the full one"
        );
        assert!(
            full.holds(slot.band(WheelBand::Plus(1), 1.0).center()),
            "a wheel holds its own bands"
        );
    }
}
