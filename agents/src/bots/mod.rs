use neumannarch_protocol::Bot;

use self::scripted::Scripted;
use self::scripted::personality::Personality;
use crate::Agent;

pub struct Shipped {
    pub bot: Bot,
    pub name: &'static str,
    pub agent: fn() -> Box<dyn Agent>,
}

pub fn shipped() -> Vec<Shipped> {
    vec![
        Shipped {
            bot: Bot::Turtle,
            name: "turtle",
            agent: || Box::new(Scripted::new(Personality::turtle())),
        },
        Shipped {
            bot: Bot::Expand,
            name: "expand",
            agent: || Box::new(Scripted::new(Personality::expand())),
        },
    ]
}

impl Shipped {
    pub fn of(bot: Bot) -> Shipped {
        shipped()
            .into_iter()
            .find(|shipped| shipped.bot == bot)
            .expect("every bot a lobby can seat ships")
    }

    pub fn named(name: &str) -> Option<Shipped> {
        shipped().into_iter().find(|shipped| shipped.name == name)
    }

    pub fn seated(&self) -> Box<dyn Agent> {
        (self.agent)()
    }
}

pub mod scripted;
