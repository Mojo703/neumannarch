use mirage_engine::prelude::*;

const WHEEL_NOTCH: f32 = 1.0;

const ZOOM_KEY_NOTCH: f32 = 0.06;

#[derive(Axes, Clone, Copy)]
pub enum Axis {
    Zoom,
}

#[derive(Axes2, Clone, Copy)]
pub enum Axis2 {
    Pan,
}

#[derive(Buttons, Clone, Copy)]
pub enum Button {
    Select,
    Pan,
    Pause,
}

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
