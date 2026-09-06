use mirage_engine::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke};
use neumannarch_sim::{Material, Time};

use crate::display::bars::{ICON_SLOT, ICON_WIDTH};
use crate::display::hue;
use crate::display::icon;
use crate::display::scene::StripView;
use crate::display::stencil::Cell;
use crate::display::wheel;
use crate::screens::panel;

pub const HEIGHT: f32 = wheel::SECTION_HEIGHT;

pub const BAR_LENGTH: f32 = 3.0 * wheel::SECTION_HEIGHT;

pub const OVERRUN_ROOM: f32 = BAR_LENGTH / 4.0;

pub const PROJECTION_SECONDS: f64 = 10.0;

const TOP_MARGIN: f32 = wheel::ROW_GAP;

const BAR_HEIGHT: f32 = wheel::LINE_HEIGHT;

const STOCK_WIDTH: f32 = 4.0 * wheel::CHARACTER_WIDTH;

const NET_WIDTH: f32 = 4.0 * wheel::CHARACTER_WIDTH;

const NUMERAL_INSET: f32 = wheel::PAD;

const FILL_ALPHA: f32 = 0.75;

const INCOME_ALPHA: f32 = 0.25;

const SPEND_ALPHA: f32 = 0.45;

const CLOCK_FILL: Color32 = panel::DIM_INK;

const PHRASE_GAP: f32 = wheel::GAP;

pub struct Strip {
    view: StripView,
    frame: Rect,
    cells: Vec<StripCell>,
}

struct StripCell {
    frame: Rect,
    stock: Rect,
    track: Rect,
    reading: Reading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reading {
    Material(Material),
    Clock,
}

impl Strip {
    pub fn across(window: Rect, view: StripView) -> Strip {
        let material_width = ICON_SLOT
            + wheel::GAP
            + STOCK_WIDTH
            + wheel::GAP
            + BAR_LENGTH
            + OVERRUN_ROOM
            + wheel::GAP
            + NET_WIDTH;
        let clock_width = BAR_LENGTH;
        let total = 3.0 * material_width + clock_width + 3.0 * wheel::CELL_GAP;
        let top = window.top() + TOP_MARGIN + wheel::PAD;
        let height = HEIGHT - 2.0 * wheel::PAD;
        let mut left = window.center().x - total / 2.0;
        let readings = Material::EVERY
            .into_iter()
            .map(Reading::Material)
            .chain([Reading::Clock]);
        let cells: Vec<StripCell> = readings
            .map(|reading| {
                let (width, stock_left) = match reading {
                    Reading::Material(_) => (material_width, left + ICON_SLOT + wheel::GAP),
                    Reading::Clock => (clock_width, left),
                };
                let frame = Rect::from_min_size(egui::pos2(left, top), egui::vec2(width, height));
                let line_top = frame.center().y - BAR_HEIGHT / 2.0;
                let (stock, track_left) = match reading {
                    Reading::Material(_) => (
                        Rect::from_min_size(
                            egui::pos2(stock_left, line_top),
                            egui::vec2(STOCK_WIDTH, BAR_HEIGHT),
                        ),
                        stock_left + STOCK_WIDTH + wheel::GAP,
                    ),
                    Reading::Clock => (Rect::NOTHING, stock_left),
                };
                let track = Rect::from_min_size(
                    egui::pos2(track_left, line_top),
                    egui::vec2(BAR_LENGTH, BAR_HEIGHT),
                );
                left += width + wheel::CELL_GAP;
                StripCell {
                    frame,
                    stock,
                    track,
                    reading,
                }
            })
            .collect();
        let frame = cells
            .iter()
            .map(|cell| cell.frame)
            .reduce(|frame, cell| frame.union(cell))
            .unwrap_or(Rect::NOTHING)
            .expand(wheel::PAD);
        Strip { view, frame, cells }
    }

    pub fn frame(&self) -> Rect {
        self.frame
    }

    pub fn paint(&self, painter: &egui::Painter, pointer: Option<Pos2>) {
        painter.rect_filled(self.frame, 0.0, panel::BACKDROP);
        painter.rect_stroke(
            self.frame,
            0.0,
            Stroke::new(1.0, panel::LINE),
            egui::StrokeKind::Inside,
        );
        for cell in &self.cells {
            match cell.reading {
                Reading::Material(material) => self.paint_material(painter, cell, material),
                Reading::Clock => self.paint_clock(painter, cell),
            }
        }
        if let Some((cell, phrase)) = pointer.and_then(|at| self.spoken_at(at)) {
            painter.text(
                egui::pos2(
                    cell.left(),
                    self.frame.bottom() + PHRASE_GAP + wheel::LINE_HEIGHT / 2.0,
                ),
                Align2::LEFT_CENTER,
                phrase,
                FontId::monospace(wheel::LINE_HEIGHT),
                panel::INK,
            );
        }
    }

    pub fn spoken_at(&self, at: Pos2) -> Option<(Rect, String)> {
        self.cells
            .iter()
            .find(|cell| cell.frame.contains(at))
            .and_then(|cell| match cell.reading {
                Reading::Material(material) => Some((cell.frame, self.phrase(material))),
                Reading::Clock => None,
            })
    }

    pub fn phrase(&self, material: Material) -> String {
        format!("of {}", whole(self.view.stockpile.capacity()[material]))
    }

    pub fn net(&self, material: Material) -> String {
        signed(self.view.income[material] - self.view.spend[material])
    }

    pub fn elapsed(&self) -> String {
        clock(self.view.elapsed)
    }

    fn paint_material(&self, painter: &egui::Painter, cell: &StripCell, material: Material) {
        let hue = hue::of(material);
        let stock = self.view.stockpile.stock()[material];
        let capacity = self.view.stockpile.capacity()[material];
        let share = |amount: f64| match capacity > 0.0 {
            true => (amount / capacity) as f32 * BAR_LENGTH,
            false => 0.0,
        };
        Cell {
            centre: egui::pos2(cell.frame.left() + ICON_SLOT / 2.0, cell.frame.center().y),
            half: wheel::GLYPH_HALF,
        }
        .paint(
            painter,
            &icon::of(material).placed(icon::CENTRE, ICON_WIDTH),
            hue,
        );
        painter.text(
            egui::pos2(cell.stock.right(), cell.stock.center().y),
            Align2::RIGHT_CENTER,
            whole(stock).to_string(),
            FontId::monospace(wheel::LINE_HEIGHT),
            panel::INK,
        );
        let track = cell.track;
        paint_track(painter, track);
        let tip = track.left() + share(stock).min(BAR_LENGTH);
        let income = self.view.income[material];
        let spend = self.view.spend[material];
        let stalled = stock <= 0.0 && income < spend;
        let reach = match stalled {
            true => tip,
            false => tip + share(income * PROJECTION_SECONDS),
        };
        let past_the_end = (reach - track.right()).clamp(0.0, OVERRUN_ROOM);
        let overrun = match stock >= capacity {
            true => past_the_end,
            false => 0.0,
        };
        if !stalled {
            span(
                painter,
                track,
                tip,
                reach.min(track.right() + OVERRUN_ROOM),
                panel::BACKDROP.lerp_to_gamma(hue, INCOME_ALPHA),
            );
        }
        span(
            painter,
            track,
            track.left(),
            tip + overrun,
            panel::BACKDROP.lerp_to_gamma(hue, FILL_ALPHA),
        );
        if !stalled {
            let drain = tip - share(spend * PROJECTION_SECONDS);
            span(
                painter,
                track,
                drain.max(track.left()),
                tip,
                panel::BACKDROP.lerp_to_gamma(hue, SPEND_ALPHA),
            );
        }
        painter.text(
            egui::pos2(track.right() + overrun + wheel::GAP, track.center().y),
            Align2::LEFT_CENTER,
            self.net(material),
            FontId::monospace(wheel::LINE_HEIGHT),
            panel::INK,
        );
    }

    fn paint_clock(&self, painter: &egui::Painter, cell: &StripCell) {
        let track = cell.track;
        paint_track(painter, track);
        let left = match self.view.clock.0 > 0 {
            true => (self.view.clock.0.saturating_sub(self.view.elapsed.0) as f32
                / self.view.clock.0 as f32)
                .clamp(0.0, 1.0),
            false => 0.0,
        };
        span(
            painter,
            track,
            track.left(),
            track.left() + left * BAR_LENGTH,
            CLOCK_FILL,
        );
        painter.text(
            egui::pos2(track.left() + NUMERAL_INSET, track.center().y),
            Align2::LEFT_CENTER,
            self.elapsed(),
            FontId::monospace(wheel::LINE_HEIGHT),
            panel::INK,
        );
    }
}

fn paint_track(painter: &egui::Painter, track: Rect) {
    painter.rect_stroke(
        track,
        0.0,
        Stroke::new(1.0, panel::LINE),
        egui::StrokeKind::Inside,
    );
}

fn span(painter: &egui::Painter, track: Rect, from: f32, to: f32, colour: Color32) {
    if to <= from {
        return;
    }
    painter.rect_filled(
        Rect::from_min_max(
            egui::pos2(from, track.top()),
            egui::pos2(to, track.bottom()),
        ),
        0.0,
        colour,
    );
}

fn whole(amount: f64) -> u64 {
    amount.max(0.0).round() as u64
}

fn signed(net: f64) -> String {
    let net = net.round() as i64;
    format!("{net:+}")
}

fn clock(elapsed: Time) -> String {
    let seconds = elapsed.seconds() as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use neumannarch_sim::{Materials, Stockpile};

    use super::*;

    const WINDOW: Rect = Rect::from_min_max(Pos2::ZERO, egui::pos2(1280.0, 720.0));

    fn view() -> StripView {
        StripView {
            stockpile: Stockpile::new(
                Materials::new(120.0, 300.0, 0.0),
                Materials::new(300.0, 300.0, 300.0),
            ),
            income: Materials::new(12.0, 8.0, 0.0),
            spend: Materials::new(9.0, 0.0, 4.0),
            elapsed: Time(neumannarch_sim::TICKS_PER_SECOND as u64 * 247),
            clock: Time(neumannarch_sim::TICKS_PER_SECOND as u64 * 900),
        }
    }

    #[test]
    fn the_strip_is_one_box_across_the_top_centre_a_cell_per_material_then_the_clock() {
        let strip = Strip::across(WINDOW, view());

        assert_eq!(strip.cells.len(), 4);
        assert_eq!(
            strip
                .cells
                .iter()
                .map(|cell| cell.reading)
                .collect::<Vec<_>>(),
            vec![
                Reading::Material(Material::Metals),
                Reading::Material(Material::Volatiles),
                Reading::Material(Material::Energy),
                Reading::Clock,
            ]
        );
        assert!((strip.frame.center().x - WINDOW.center().x).abs() < 1.0);
        assert!(strip.frame.top() < HEIGHT);
        assert!((strip.frame.height() - HEIGHT).abs() < 1e-3);
        assert!(
            strip
                .cells
                .iter()
                .all(|cell| strip.frame.contains_rect(cell.frame)),
            "the box holds every cell"
        );
        assert!(
            strip.cells.windows(2).all(|pair| {
                let apart = pair[1].frame.left() - pair[0].frame.right();
                (apart - wheel::CELL_GAP).abs() < 1e-3
            }),
            "cells run left to right the wheel's cell gap apart"
        );
        assert!(
            strip
                .cells
                .iter()
                .all(|cell| (cell.track.width() - BAR_LENGTH).abs() < 1e-3),
            "every bar is one length"
        );
    }

    #[test]
    fn a_material_cell_runs_icon_then_stock_then_bar_then_net_and_the_numeral_is_off_the_bar() {
        let strip = Strip::across(WINDOW, view());
        let cell = &strip.cells[0];

        assert!(cell.frame.left() + ICON_SLOT <= cell.stock.left());
        assert!(
            cell.stock.right() < cell.track.left(),
            "the stock stands before the bar"
        );
        assert!(
            cell.track.right() + OVERRUN_ROOM + NET_WIDTH <= cell.frame.right() + 1e-3,
            "the net has its room past the overrun"
        );
        assert!(!cell.track.intersects(cell.stock));
    }

    #[test]
    fn a_cell_says_its_capacity_and_carries_its_net() {
        let strip = Strip::across(WINDOW, view());

        assert_eq!(strip.phrase(Material::Metals), "of 300");
        assert_eq!(strip.net(Material::Metals), "+3");
        assert_eq!(strip.net(Material::Energy), "-4");
        assert_eq!(strip.net(Material::Volatiles), "+8");
        assert_eq!(signed(0.0), "+0");
        assert_eq!(signed(-0.4), "+0");
    }

    #[test]
    fn the_clock_reads_the_elapsed_minutes_and_seconds() {
        let strip = Strip::across(WINDOW, view());

        assert_eq!(strip.elapsed(), "4:07");
        assert_eq!(clock(Time::ZERO), "0:00");
    }

    #[test]
    fn a_material_cell_under_the_pointer_speaks_and_the_clock_does_not() {
        let strip = Strip::across(WINDOW, view());

        let metals = strip.cells[0].frame;
        assert_eq!(
            strip.spoken_at(metals.center()),
            Some((metals, "of 300".to_string()))
        );
        assert_eq!(strip.spoken_at(strip.cells[3].frame.center()), None);
        assert_eq!(strip.spoken_at(WINDOW.center()), None);
    }
}
