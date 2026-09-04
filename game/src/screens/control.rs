use mirage_engine::egui::{self, Align2, Pos2, Rect, Stroke, Vec2};

use crate::screens::field::{Field, Typed};
use crate::screens::panel::{self, HOVER_FILL, Panel};

pub(crate) const HOST_ONLY: &str = "Host only";

const NOTE_GAP: f32 = 12.0;

const ARROW_INSET: f32 = 14.0;

const ARROW_HALF: f32 = 4.0;

const TEXT_INSET: f32 = 12.0;

const INSIDE_WIDTH: f32 = 108.0;

pub enum Rule {
    Allows,
    Refuses(String),
}

pub enum Chose<T> {
    Toggled,
    Value(T),
}

pub struct Value<T> {
    pub value: T,
    pub label: String,
    pub rule: Rule,
}

pub struct Valued<'a> {
    pub rect: Rect,
    pub rule: &'a Rule,
    pub inside: Option<(&'a str, &'a Rule)>,
    pub state: Option<&'a str>,
}

pub struct Typing {
    pub entered: bool,
    pub acted: bool,
}

pub struct Controls<'a> {
    panel: &'a Panel<'a>,
    covered: Option<Rect>,
    list: Option<List>,
    note: Option<Note>,
    shown: Vec<Rect>,
    claimed: bool,
}

struct List {
    rect: Rect,
    rows: Vec<(Rect, String, bool)>,
}

struct Note {
    beside: Rect,
    sentence: String,
}

impl Rule {
    pub fn allows(&self) -> bool {
        matches!(self, Rule::Allows)
    }

    pub fn refuses(why: impl Into<String>) -> Rule {
        Rule::Refuses(why.into())
    }

    pub fn unless(why: Option<String>) -> Rule {
        match why {
            None => Rule::Allows,
            Some(why) => Rule::Refuses(why),
        }
    }

    pub fn only_if(held: bool, why: impl Into<String>) -> Rule {
        match held {
            true => Rule::Allows,
            false => Rule::refuses(why),
        }
    }

    pub fn why(&self) -> Option<&str> {
        match self {
            Rule::Allows => None,
            Rule::Refuses(why) => Some(why),
        }
    }
}

impl<'a> Controls<'a> {
    pub fn over(panel: &'a Panel<'a>) -> Controls<'a> {
        Controls {
            panel,
            covered: None,
            list: None,
            note: None,
            shown: Vec::new(),
            claimed: false,
        }
    }

    pub fn action(&mut self, rect: Rect, label: &str, rule: &Rule) -> bool {
        let taken = self.paint_action(rect, label, rule);
        self.show_reason(rect, rule);
        taken
    }

    pub fn main_action(&mut self, rect: Rect, label: &str, rule: &Rule) -> bool {
        let taken = self.paint_action(rect, label, rule);
        if let Some(why) = rule.why() {
            let sentence = why.to_string();
            self.note(rect, sentence);
        }
        taken
    }

    pub fn choice<T: Copy>(
        &mut self,
        rect: Rect,
        current: &str,
        values: &[Value<T>],
        rule: &Rule,
        open: bool,
    ) -> Option<Chose<T>> {
        let over = self.is_over(rect);
        if over && rule.allows() {
            self.panel.painter().rect_filled(rect, 0.0, HOVER_FILL);
        }
        self.panel.outline(rect);
        self.paint_label(current, rect, rule.allows());
        self.paint_arrow(rect, rule.allows());
        self.show_reason(rect, rule);
        self.shown.push(rect);
        let toggled = self.clicked_on(rect) && rule.allows();
        if !open || !rule.allows() {
            return toggled.then_some(Chose::Toggled);
        }

        let mut picked = None;
        let mut rows = Vec::new();
        for (row, value) in list_rows(rect, values.len()).zip(values) {
            if self.is_over(row) {
                if value.rule.allows() {
                    self.panel.painter().rect_filled(row, 0.0, HOVER_FILL);
                }
                self.show_reason(row, &value.rule);
            }
            if self.panel.picked(row) {
                self.claimed = true;
                if value.rule.allows() {
                    picked = Some(Chose::Value(value.value));
                }
            }
            rows.push((row, value.label.clone(), value.rule.allows()));
        }
        let list = list_rect(rect, values.len());
        self.covered = Some(list);
        self.shown.push(list);
        self.list = Some(List { rect: list, rows });
        picked.or(toggled.then_some(Chose::Toggled))
    }

    pub fn value(&mut self, valued: Valued<'_>, field: &mut Field, typed: &Typed) -> Typing {
        let Valued {
            rect,
            rule,
            inside,
            state,
        } = valued;
        let inner = inside.map(|_| inside_rect(rect));
        let right = inner.map_or(rect.right(), |inner| inner.left());
        let line = Rect::from_min_max(rect.min, egui::pos2(right, rect.top() + panel::ROW_HEIGHT));
        if self.panel.clicked() {
            let taken = self.clicked_on(line);
            field.focus(rule.allows() && taken);
        }
        let entered = rule.allows() && field.take_typing(typed);
        self.panel.outline(rect);
        self.paint_label(&field.line(), line, rule.allows());
        self.show_reason(line, rule);
        if let Some(state) = state {
            let under = Rect::from_min_max(egui::pos2(rect.left(), line.bottom()), rect.max);
            self.panel.text(
                state,
                Pos2::new(under.left() + TEXT_INSET, under.center().y),
                Align2::LEFT_CENTER,
                panel::DIM_INK,
                panel::BODY_SIZE,
            );
        }
        self.shown.push(rect);
        let acted = match (inner, inside) {
            (Some(inner), Some((label, inside))) => self.main_action(inner, label, inside),
            _ => false,
        };
        Typing { entered, acted }
    }

    pub fn note(&mut self, beside: Rect, sentence: String) {
        self.note = Some(Note { beside, sentence });
    }

    pub fn finish(self) -> bool {
        if let Some(list) = &self.list {
            self.panel
                .painter()
                .rect_filled(list.rect, 0.0, panel::BACKDROP);
            self.panel.outline(list.rect);
            for (row, label, allowed) in &list.rows {
                self.paint_label(label, *row, *allowed);
            }
        }
        if let Some(note) = &self.note {
            self.paint_note(note);
        }
        self.panel.clicked() && !self.claimed
    }

    fn paint_action(&mut self, rect: Rect, label: &str, rule: &Rule) -> bool {
        if self.is_over(rect) && rule.allows() {
            self.panel.painter().rect_filled(rect, 0.0, HOVER_FILL);
        }
        self.panel.outline(rect);
        self.paint_label(label, rect, rule.allows());
        self.shown.push(rect);
        self.clicked_on(rect) && rule.allows()
    }

    fn is_over(&self, rect: Rect) -> bool {
        self.panel.pointing_at(rect) && !self.is_covered(rect)
    }

    fn clicked_on(&mut self, rect: Rect) -> bool {
        let clicked = self.panel.picked(rect) && !self.is_covered(rect);
        self.claimed |= clicked;
        clicked
    }

    fn is_covered(&self, rect: Rect) -> bool {
        self.covered.is_some_and(|covered| covered.intersects(rect))
    }

    fn show_reason(&mut self, rect: Rect, rule: &Rule) {
        if let Some(why) = rule.why()
            && self.is_over(rect)
        {
            let sentence = why.to_string();
            self.note(rect, sentence);
        }
    }

    fn paint_label(&self, label: &str, rect: Rect, allowed: bool) {
        self.panel.text(
            label,
            Pos2::new(rect.left() + TEXT_INSET, rect.center().y),
            Align2::LEFT_CENTER,
            match allowed {
                true => panel::INK,
                false => panel::DIM_INK,
            },
            panel::BODY_SIZE,
        );
    }

    fn paint_arrow(&self, rect: Rect, allowed: bool) {
        let at = Pos2::new(rect.right() - ARROW_INSET, rect.center().y);
        let colour = match allowed {
            true => panel::INK,
            false => panel::DIM_INK,
        };
        self.panel.painter().add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(at.x - ARROW_HALF, at.y - ARROW_HALF / 2.0),
                Pos2::new(at.x + ARROW_HALF, at.y - ARROW_HALF / 2.0),
                Pos2::new(at.x, at.y + ARROW_HALF),
            ],
            colour,
            Stroke::NONE,
        ));
    }

    fn paint_note(&self, note: &Note) {
        let window = self.panel.window();
        let width = note.sentence.chars().count() as f32 * panel::CHARACTER_WIDTH;
        let height = panel::ROW_HEIGHT;
        let beside = |left: f32| {
            Rect::from_min_size(
                Pos2::new(left, note.beside.center().y - height / 2.0),
                Vec2::new(width, height),
            )
        };
        let right = beside(note.beside.right() + NOTE_GAP);
        let left = beside(note.beside.left() - NOTE_GAP - width);
        let clear = |at: &Rect| {
            window.contains_rect(*at)
                && !self
                    .shown
                    .iter()
                    .any(|shown| *shown != note.beside && shown.intersects(*at))
        };
        let at = match (clear(&right), clear(&left)) {
            (true, _) => right,
            (false, true) => left,
            (false, false) => match window.right() - note.beside.right() >= note.beside.left() {
                true => right,
                false => left,
            },
        };
        self.panel.text(
            &note.sentence,
            Pos2::new(at.left(), at.center().y),
            Align2::LEFT_CENTER,
            panel::INK,
            panel::BODY_SIZE,
        );
    }
}

pub fn list_rect(head: Rect, values: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(head.left(), head.bottom()),
        Vec2::new(head.width(), head.height() * values as f32),
    )
}

pub fn inside_rect(field: Rect) -> Rect {
    let width = INSIDE_WIDTH
        .max(field.width() / 3.0)
        .min(field.width() / 2.0);
    Rect::from_min_size(
        Pos2::new(field.right() - width, field.top()),
        Vec2::new(width, field.height()),
    )
}

pub fn list_row(head: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(head.left(), head.bottom() + index as f32 * head.height()),
        head.size(),
    )
}

fn list_rows(head: Rect, values: usize) -> impl Iterator<Item = Rect> {
    (0..values).map(move |index| list_row(head, index))
}
