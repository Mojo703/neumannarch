//! The register every screen outside a match is drawn in: the HUD's
//! palette, thin lines, no window chrome, and hit tests of its own.

use mirage_engine::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use mirage_engine::math::UVec2;

/// What a screen paints over, opaque where it replaces the belt.
pub const BACKDROP: Color32 = Color32::from_rgb(8, 10, 14);

/// What a screen paints over the belt, so the belt still reads under it.
pub const SCRIM: Color32 = Color32::from_rgba_premultiplied(6, 8, 11, 200);

/// Text a screen states plainly.
pub const INK: Color32 = Color32::from_gray(220);

/// Text a screen states but the player cannot act on.
pub const DIM_INK: Color32 = Color32::from_gray(96);

/// A thin line, the only rule a screen draws with.
pub const LINE: Color32 = Color32::from_gray(70);

/// A heading's text size, in points.
pub const HEADING_SIZE: f32 = 26.0;

/// Body text's size, in points.
pub const BODY_SIZE: f32 = 15.0;

/// How tall one action or one row stands, in points.
pub const ROW_HEIGHT: f32 = 30.0;

/// How far a screen's content stands off the window's edge, in points.
pub const MARGIN: f32 = 40.0;

/// A thin line's width, in points.
const LINE_WIDTH: f32 = 1.0;

/// What an action's own rectangle brightens by while the pointer is over
/// it, over [`BACKDROP`].
const HOVER_FILL: Color32 = Color32::from_rgba_premultiplied(30, 34, 42, 255);

/// One frame of one screen: what it paints on, and where the pointer is.
pub struct Panel<'a> {
    painter: &'a egui::Painter,
    window: Rect,
    pointer: Pos2,
    /// Whether the select button went down this frame.
    clicked: bool,
}

impl<'a> Panel<'a> {
    /// A screen painted over `window`, in points, with the pointer at
    /// `pointer` and `clicked` true on the frame the select button went
    /// down.
    pub fn new(
        painter: &'a egui::Painter,
        window: Rect,
        pointer: Pos2,
        clicked: bool,
    ) -> Panel<'a> {
        Panel {
            painter,
            window,
            pointer,
            clicked,
        }
    }

    /// The whole window, in points.
    pub fn window(&self) -> Rect {
        self.window
    }

    /// What this screen paints on, for a glyph drawn through a
    /// [`crate::display::stencil::Stencil`].
    pub fn painter(&self) -> &egui::Painter {
        self.painter
    }

    /// Fills the window, hiding whatever was drawn under it.
    pub fn backdrop(&self) {
        self.painter.rect_filled(self.window, 0.0, BACKDROP);
    }

    /// Fills `rect`, leaving what is under it readable.
    pub fn scrim(&self, rect: Rect) {
        self.painter.rect_filled(rect, 0.0, SCRIM);
    }

    /// A thin outline around `rect`.
    pub fn outline(&self, rect: Rect) {
        self.painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(LINE_WIDTH, LINE),
            egui::StrokeKind::Inside,
        );
    }

    /// `text` at `at`, anchored by `align`, in `colour`, at `size` points.
    pub fn text(&self, text: &str, at: Pos2, align: Align2, colour: Color32, size: f32) {
        self.painter
            .text(at, align, text, FontId::monospace(size), colour);
    }

    /// `text` as the screen's heading, centred on `at`.
    pub fn heading(&self, text: &str, at: Pos2) {
        self.text(text, at, Align2::CENTER_CENTER, INK, HEADING_SIZE);
    }

    /// `text` in a row of its own at `at`, left aligned.
    pub fn label(&self, text: &str, at: Pos2, colour: Color32) {
        self.text(text, at, Align2::LEFT_CENTER, colour, BODY_SIZE);
    }

    /// Paints one action and answers whether the player picked it. An
    /// action the screen states as unavailable is drawn dim and answers
    /// false however it is clicked.
    pub fn action(&self, rect: Rect, label: &str, enabled: bool) -> bool {
        let over = enabled && rect.contains(self.pointer);
        if over {
            self.painter.rect_filled(rect, 0.0, HOVER_FILL);
        }
        self.outline(rect);
        self.text(
            label,
            rect.center(),
            Align2::CENTER_CENTER,
            match enabled {
                true => INK,
                false => DIM_INK,
            },
            BODY_SIZE,
        );
        over && self.clicked
    }
}

/// The whole window in the painter's own measure, from `size` in physical
/// pixels and `points_per_pixel`, the inverse of egui's own
/// `pixels_per_point`.
///
/// A screen paints to the window's edges, where the engine's UI layer is
/// inset, so it takes its rectangle from the window and not from the
/// layer.
pub fn window_of(size: UVec2, points_per_pixel: f32) -> Rect {
    Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(
            size.x as f32 * points_per_pixel,
            size.y as f32 * points_per_pixel,
        ),
    )
}

/// A column of `count` rows `width` points wide, the first at `top`,
/// [`ROW_HEIGHT`] tall with a quarter of that between them.
pub fn column(top: Pos2, width: f32, count: usize) -> impl Iterator<Item = Rect> {
    let step = ROW_HEIGHT * 1.25;
    (0..count).map(move |at| {
        let corner = Pos2::new(top.x, top.y + at as f32 * step);
        Rect::from_min_size(corner, Vec2::new(width, ROW_HEIGHT))
    })
}
