//! What the player can do, over the engine's action vocabularies. The
//! gamepad bindings DISPLAY.md states are a later unit; every action here
//! is keyboard and mouse.

use mirage_engine::prelude::*;

/// How far one turn of the mouse wheel counts as, in notches.
const WHEEL_NOTCH: f32 = 1.0;

/// How far the zoom keys count as per frame, in notches, so a held key
/// zooms smoothly where a wheel jumps.
const ZOOM_KEY_NOTCH: f32 = 0.06;

/// The zoom axis, which zooms the belt or, during a send, sets how many go.
#[derive(Axes, Clone, Copy)]
pub enum Axis {
    Zoom,
}

/// The pan axis: the keys that drag the belt.
#[derive(Axes2, Clone, Copy)]
pub enum Axis2 {
    Pan,
}

/// The buttons a screen or the belt reads.
#[derive(Buttons, Clone, Copy)]
pub enum Button {
    /// Pick a screen's action, select a ring, edit a wheel band, or start a
    /// send.
    Select,
    /// Drag the belt under the pointer.
    Pan,
    /// Open the pause screen, and close it again from inside it.
    Pause,
}

/// Every action the playable reads.
pub struct Controls;

impl ActionSet for Controls {
    type Axis = Axis;
    type Axis2 = Axis2;
    type Button = Button;
}

impl BindAxis for Axis {
    fn bindings(&self) -> Vec<AxisBinding> {
        match self {
            Axis::Zoom => vec![
                AxisBinding::motion(Motion::Wheel).scale(WHEEL_NOTCH),
                AxisBinding::from(ButtonAxis {
                    negative: Key::Q,
                    positive: Key::E,
                })
                .scale(ZOOM_KEY_NOTCH),
            ],
        }
    }
}

impl BindAxis2 for Axis2 {
    fn bindings(&self) -> Vec<Axis2Binding> {
        match self {
            Axis2::Pan => vec![
                ButtonAxis2 {
                    left: Key::A,
                    right: Key::D,
                    down: Key::S,
                    up: Key::W,
                }
                .into(),
                ButtonAxis2 {
                    left: Key::Left,
                    right: Key::Right,
                    down: Key::Down,
                    up: Key::Up,
                }
                .into(),
            ],
        }
    }
}

impl BindButton for Button {
    fn bindings(&self) -> Vec<ButtonBinding> {
        match self {
            Button::Select => vec![MouseButton::Left.into()],
            Button::Pan => vec![MouseButton::Right.into(), MouseButton::Middle.into()],
            Button::Pause => vec![Key::Escape.into()],
        }
    }
}
