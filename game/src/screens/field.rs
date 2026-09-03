//! The one element the player types into, painted and hit-tested by hand
//! like every other control.

use mirage_engine::egui::{self, Pos2, Rect};

use crate::screens::panel::{self, Panel};

/// The most characters an address holds. An address is a host and a port;
/// anything longer is not one, and the field refuses it rather than the
/// socket.
pub const MAX_ADDRESS: usize = 64;

/// The caret, drawn after the text while the field holds the focus. The
/// screens are monospaced, so a block after the text needs no measuring.
const CARET: char = '_';

/// A line of text the player types.
pub struct Field {
    text: String,
    /// Whether what is typed lands here.
    focused: bool,
}

/// What one frame typed, off the UI layer's own events.
pub struct Typed {
    /// The characters typed, in order.
    text: String,
    /// How many characters were deleted.
    deleted: usize,
    /// Whether the line was entered.
    entered: bool,
}

impl Field {
    /// A field holding `text`, unfocused.
    pub fn holding(text: &str) -> Field {
        Field {
            text: text.to_string(),
            focused: false,
        }
    }

    /// Paints the field over `rect` and takes what `typed` holds while it
    /// has the focus. True on the frame the line was entered.
    ///
    /// A click takes the focus where it lands and gives it up where it does
    /// not, so one click both aims and answers.
    pub fn frame(&mut self, panel: &Panel<'_>, rect: Rect, typed: &Typed) -> bool {
        if panel.clicked() {
            self.focused = panel.picked(rect);
        }
        if self.focused {
            for _ in 0..typed.deleted {
                self.text.pop();
            }
            for typed in typed.text.chars().filter(|typed| !typed.is_control()) {
                if self.text.chars().count() < MAX_ADDRESS {
                    self.text.push(typed);
                }
            }
        }
        panel.outline(rect);
        panel.label(
            &match self.focused {
                true => format!("{}{CARET}", self.text),
                false => self.text.clone(),
            },
            Pos2::new(rect.left() + panel::ROW_HEIGHT / 2.0, rect.center().y),
            panel::INK,
        );
        self.focused && typed.entered
    }

    /// What the player has typed.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Typed {
    /// What was typed into `ctx` this frame.
    pub fn of(ctx: &egui::Context) -> Typed {
        let mut typed = Typed {
            text: String::new(),
            deleted: 0,
            entered: false,
        };
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Text(text) => typed.text.push_str(text),
                    egui::Event::Key {
                        key: egui::Key::Backspace,
                        pressed: true,
                        ..
                    } => typed.deleted += 1,
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        ..
                    } => typed.entered = true,
                    _ => continue,
                }
            }
        });
        typed
    }
}
