//! The line a value control is typed into: the text it holds, and what one
//! frame typed.

use mirage_engine::egui;

/// The most characters an address holds. An address is a host and a port;
/// anything longer is not one, and the field refuses it rather than the
/// socket.
pub const MAX_ADDRESS: usize = 64;

/// The most digits a seed holds, which is every value a `u64` takes.
pub const MAX_SEED: usize = 20;

/// The caret, drawn after the text while the field holds the focus. The
/// screens are monospaced, so a block after the text needs no measuring.
const CARET: char = '_';

/// What a field takes, and how much of it.
#[derive(Clone, Copy)]
pub enum Allow {
    /// Any printable character, up to this many.
    Text(usize),
    /// Decimal digits, up to this many.
    Digits(usize),
}

/// A line of text the player types.
pub struct Field {
    text: String,
    allow: Allow,
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
    /// A field holding `text`, unfocused, taking what `allow` names.
    pub fn holding(text: &str, allow: Allow) -> Field {
        Field {
            text: text.to_string(),
            allow,
            focused: false,
        }
    }

    /// What the player has typed.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Shows `text` instead of what it holds, which is how a field over a
    /// value the room owns follows that value. It does nothing while the
    /// field has the focus, so it never overwrites what is being typed.
    pub fn shows(&mut self, text: &str) {
        if !self.focused {
            self.text = text.to_string();
        }
    }

    /// Takes the focus, or gives it up.
    pub(crate) fn focus(&mut self, on: bool) {
        self.focused = on;
    }

    /// Takes what `typed` holds while the field has the focus. True on the
    /// frame the line was entered.
    pub(crate) fn take_typing(&mut self, typed: &Typed) -> bool {
        if !self.focused {
            return false;
        }
        for _ in 0..typed.deleted {
            self.text.pop();
        }
        for taken in typed.text.chars().filter(|taken| self.allow.allows(*taken)) {
            if self.text.chars().count() < self.allow.limit() {
                self.text.push(taken);
            }
        }
        typed.entered
    }

    /// The line as it is drawn: the text, with the caret while it has the
    /// focus.
    pub(crate) fn line(&self) -> String {
        match self.focused {
            true => format!("{}{CARET}", self.text),
            false => self.text.clone(),
        }
    }
}

impl Allow {
    /// Whether the field takes `typed`.
    fn allows(self, typed: char) -> bool {
        match self {
            Allow::Text(_) => !typed.is_control(),
            Allow::Digits(_) => typed.is_ascii_digit(),
        }
    }

    /// The most characters the field holds.
    fn limit(self) -> usize {
        match self {
            Allow::Text(limit) | Allow::Digits(limit) => limit,
        }
    }
}

impl Typed {
    /// What was typed into `ctx` this frame.
    pub fn this_frame(ctx: &egui::Context) -> Typed {
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
