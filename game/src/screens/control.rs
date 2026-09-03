//! The three kinds of control every screen is built from: an action, a
//! choice and a value, each painted and hit tested by hand in the panel's
//! register.

use mirage_engine::egui::{self, Align2, Color32, Pos2, Rect, Stroke, Vec2};

use crate::screens::field::{Field, Typed};
use crate::screens::panel::{self, Panel};

/// How far a hover sentence stands off the control it explains, in points.
const NOTE_GAP: f32 = 12.0;

/// How far a choice's arrow stands in from its right edge, in points.
const ARROW_INSET: f32 = 14.0;

/// Half the width of a choice's arrow, in points.
const ARROW_HALF: f32 = 4.0;

/// How far the text of a control stands in from its left edge, in points.
const TEXT_INSET: f32 = 12.0;

/// How wide an action inside a value field stands, in points: Random Seed,
/// the longest of them, and its insets.
const INSIDE_WIDTH: f32 = 108.0;

/// What a control's own rectangle brightens by while the pointer is over
/// it.
const HOVER_FILL: Color32 = Color32::from_rgba_premultiplied(30, 34, 42, 255);

/// Whether a control will work, and the one sentence a disabled one shows
/// while the pointer is over it.
///
/// It is computed from the same function the control's action will call, so
/// a control is never enabled and then refused.
pub enum Rule {
    /// The action behind it succeeds.
    Allows,
    /// It does not, for this reason.
    Refuses(String),
}

/// What a click on a choice did.
pub enum Chose<T> {
    /// The list opened, or closed again.
    Toggled,
    /// This value was picked out of the open list.
    Value(T),
}

/// One value a choice offers, with the rule for picking it.
pub struct Value<T> {
    pub value: T,
    pub label: String,
    pub rule: Rule,
}

/// One value control: where it stands, whether it takes typing, the action
/// inside it, and the line under it.
pub struct Valued<'a> {
    pub rect: Rect,
    pub rule: &'a Rule,
    /// An action at the field's right edge, as Random Seed sits inside the
    /// seed.
    pub inside: Option<(&'a str, &'a Rule)>,
    /// One line shown under the value, inside the field, which is where a
    /// join says how it went.
    pub state: Option<&'a str>,
}

/// What one frame did to a value field.
pub struct Typing {
    /// The line was entered.
    pub entered: bool,
    /// The action inside the field was taken.
    pub acted: bool,
}

/// One frame's controls over one panel: what they paint, what the pointer
/// is over, and what a click took.
///
/// An open list and a hover sentence are painted by [`Controls::finish`],
/// after every control, so neither is drawn over.
pub struct Controls<'a> {
    panel: &'a Panel<'a>,
    /// An open list's rectangle, which covers the controls under it.
    covered: Option<Rect>,
    list: Option<List>,
    note: Option<Note>,
    /// Every control shown this frame, which a sentence is kept clear of.
    shown: Vec<Rect>,
    claimed: bool,
}

/// A choice's open list, kept until every control is painted.
struct List {
    rect: Rect,
    /// One row per value: where it stands, what it reads, and whether
    /// picking it works.
    rows: Vec<(Rect, String, bool)>,
}

/// One sentence beside one control.
struct Note {
    beside: Rect,
    sentence: String,
}

impl Rule {
    /// Whether the control works.
    pub fn allows(&self) -> bool {
        matches!(self, Rule::Allows)
    }

    /// A rule that refuses, for `why`.
    pub fn refuses(why: impl Into<String>) -> Rule {
        Rule::Refuses(why.into())
    }

    /// Allows where `why` is `None`.
    pub fn unless(why: Option<String>) -> Rule {
        match why {
            None => Rule::Allows,
            Some(why) => Rule::Refuses(why),
        }
    }

    /// Allows where `held`, and refuses for `why` otherwise.
    pub fn only_if(held: bool, why: impl Into<String>) -> Rule {
        match held {
            true => Rule::Allows,
            false => Rule::refuses(why),
        }
    }

    /// The reason it refuses, where it refuses.
    pub fn why(&self) -> Option<&str> {
        match self {
            Rule::Allows => None,
            Rule::Refuses(why) => Some(why),
        }
    }
}

impl<'a> Controls<'a> {
    /// The controls of one frame over `panel`.
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

    /// Paints one action over `rect` and answers whether the player took
    /// it. A disabled action answers false however it is clicked, and
    /// shows its reason while the pointer is over it.
    pub fn action(&mut self, rect: Rect, label: &str, rule: &Rule) -> bool {
        let taken = self.paint_action(rect, label, rule);
        self.show_reason(rect, rule);
        taken
    }

    /// Paints the screen's own way forward — Start, Ready, Join — whose
    /// reason stands beside it whenever it is disabled, so the way forward
    /// is never hidden.
    pub fn main_action(&mut self, rect: Rect, label: &str, rule: &Rule) -> bool {
        let taken = self.paint_action(rect, label, rule);
        if let Some(why) = rule.why() {
            let sentence = why.to_string();
            self.note(rect, sentence);
        }
        taken
    }

    /// Paints one choice showing `current`, its list open where `open`, and
    /// answers what a click did.
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

    /// Paints `valued`'s field, taking what `typed` holds while it has the
    /// focus.
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

    /// One plain sentence beside `beside`, painted over every control.
    /// The last one shown wins, so a frame shows one sentence.
    pub fn note(&mut self, beside: Rect, sentence: String) {
        self.note = Some(Note { beside, sentence });
    }

    /// Paints the open list and the hover sentence, and answers whether a
    /// click this frame landed on no control, which is how a list closes.
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

    /// Paints one action's box and label and answers whether the player
    /// took it.
    fn paint_action(&mut self, rect: Rect, label: &str, rule: &Rule) -> bool {
        if self.is_over(rect) && rule.allows() {
            self.panel.painter().rect_filled(rect, 0.0, HOVER_FILL);
        }
        self.panel.outline(rect);
        self.paint_label(label, rect, rule.allows());
        self.shown.push(rect);
        self.clicked_on(rect) && rule.allows()
    }

    /// Whether the pointer is over `rect` and no open list covers it.
    fn is_over(&self, rect: Rect) -> bool {
        self.panel.pointing_at(rect) && !self.is_covered(rect)
    }

    /// Whether a click this frame landed on `rect`, which claims it.
    fn clicked_on(&mut self, rect: Rect) -> bool {
        let clicked = self.panel.picked(rect) && !self.is_covered(rect);
        self.claimed |= clicked;
        clicked
    }

    /// Whether an open list stands over `rect`.
    fn is_covered(&self, rect: Rect) -> bool {
        self.covered.is_some_and(|covered| covered.intersects(rect))
    }

    /// Shows the sentence a disabled control shows while the pointer is
    /// over it.
    fn show_reason(&mut self, rect: Rect, rule: &Rule) {
        if let Some(why) = rule.why()
            && self.is_over(rect)
        {
            let sentence = why.to_string();
            self.note(rect, sentence);
        }
    }

    /// Paints one control's label, dim where it is disabled.
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

    /// Paints the mark at a choice's right edge, which says a click opens
    /// a list.
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

    /// Paints one sentence beside its control, on the side with room for
    /// it and clear of every other control; the side with more room where
    /// neither is clear.
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

/// Where a choice's open list stands: under its head, one row per value.
pub fn list_rect(head: Rect, values: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(head.left(), head.bottom()),
        Vec2::new(head.width(), head.height() * values as f32),
    )
}

/// Where the action inside a value field stands: at the field's right
/// edge, wide enough for its label but never past the field's half.
pub fn inside_rect(field: Rect) -> Rect {
    let width = INSIDE_WIDTH
        .max(field.width() / 3.0)
        .min(field.width() / 2.0);
    Rect::from_min_size(
        Pos2::new(field.right() - width, field.top()),
        Vec2::new(width, field.height()),
    )
}

/// Where the value at `index` of a choice's open list under `head` stands.
pub fn list_row(head: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(head.left(), head.bottom() + index as f32 * head.height()),
        head.size(),
    )
}

/// Each row of a choice's open list under `head`, in the order the values
/// were given.
fn list_rows(head: Rect, values: usize) -> impl Iterator<Item = Rect> {
    (0..values).map(move |index| list_row(head, index))
}
