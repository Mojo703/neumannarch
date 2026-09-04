use mirage_engine::egui;

pub const MAX_ADDRESS: usize = 64;

pub const MAX_SEED: usize = 20;

const CARET: char = '_';

#[derive(Clone, Copy)]
pub enum Allow {
    Text(usize),
    Digits(usize),
}

pub struct Field {
    text: String,
    allow: Allow,
    focused: bool,
}

pub struct Typed {
    text: String,
    deleted: usize,
    entered: bool,
}

impl Field {
    pub fn holding(text: &str, allow: Allow) -> Field {
        Field {
            text: text.to_string(),
            allow,
            focused: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn shows(&mut self, text: &str) {
        if !self.focused {
            self.text = text.to_string();
        }
    }

    pub(crate) fn focus(&mut self, on: bool) {
        self.focused = on;
    }

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

    pub(crate) fn line(&self) -> String {
        match self.focused {
            true => format!("{}{CARET}", self.text),
            false => self.text.clone(),
        }
    }
}

impl Allow {
    fn allows(self, typed: char) -> bool {
        match self {
            Allow::Text(_) => !typed.is_control(),
            Allow::Digits(_) => typed.is_ascii_digit(),
        }
    }

    fn limit(self) -> usize {
        match self {
            Allow::Text(limit) | Allow::Digits(limit) => limit,
        }
    }
}

impl Typed {
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
