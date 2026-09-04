use mirage_engine::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use mirage_engine::math::UVec2;

pub const BACKDROP: Color32 = Color32::from_rgb(8, 10, 14);

pub const SCRIM: Color32 = Color32::from_rgba_premultiplied(6, 8, 11, 200);

pub const INK: Color32 = Color32::from_gray(220);

pub const DIM_INK: Color32 = Color32::from_gray(96);

pub const LINE: Color32 = Color32::from_gray(70);

pub const HOVER_FILL: Color32 = Color32::from_rgba_premultiplied(30, 34, 42, 255);

pub const HEADING_SIZE: f32 = 26.0;

pub const BODY_SIZE: f32 = 15.0;

pub const CHARACTER_WIDTH: f32 = BODY_SIZE * 0.6;

pub const ROW_HEIGHT: f32 = 30.0;

pub const MARGIN: f32 = 40.0;

const LINE_WIDTH: f32 = 1.0;

pub struct Panel<'a> {
    painter: &'a egui::Painter,
    window: Rect,
    pointer: Pos2,
    clicked: bool,
}

impl<'a> Panel<'a> {
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

    pub fn window(&self) -> Rect {
        self.window
    }

    pub fn painter(&self) -> &egui::Painter {
        self.painter
    }

    pub fn clicked(&self) -> bool {
        self.clicked
    }

    pub fn picked(&self, rect: Rect) -> bool {
        self.clicked && rect.contains(self.pointer)
    }

    pub fn pointing_at(&self, rect: Rect) -> bool {
        rect.contains(self.pointer)
    }

    pub fn backdrop(&self) {
        self.painter.rect_filled(self.window, 0.0, BACKDROP);
    }

    pub fn scrim(&self, rect: Rect) {
        self.painter.rect_filled(rect, 0.0, SCRIM);
    }

    pub fn outline(&self, rect: Rect) {
        self.painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(LINE_WIDTH, LINE),
            egui::StrokeKind::Inside,
        );
    }

    pub fn text(&self, text: &str, at: Pos2, align: Align2, colour: Color32, size: f32) {
        self.painter
            .text(at, align, text, FontId::monospace(size), colour);
    }

    pub fn heading(&self, text: &str, at: Pos2) {
        self.text(text, at, Align2::CENTER_CENTER, INK, HEADING_SIZE);
    }

    pub fn label(&self, text: &str, at: Pos2, colour: Color32) {
        self.text(text, at, Align2::LEFT_CENTER, colour, BODY_SIZE);
    }
}

pub fn window_of(size: UVec2, points_per_pixel: f32) -> Rect {
    Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(
            size.x as f32 * points_per_pixel,
            size.y as f32 * points_per_pixel,
        ),
    )
}

pub fn rows<const N: usize>(top: Pos2, width: f32) -> [Rect; N] {
    let mut rects = [Rect::ZERO; N];
    for (rect, laid) in rects.iter_mut().zip(column(top, width, N)) {
        *rect = laid;
    }
    rects
}

pub fn column(top: Pos2, width: f32, count: usize) -> impl Iterator<Item = Rect> {
    let step = ROW_HEIGHT * 1.25;
    (0..count).map(move |at| {
        let corner = Pos2::new(top.x, top.y + at as f32 * step);
        Rect::from_min_size(corner, Vec2::new(width, ROW_HEIGHT))
    })
}
