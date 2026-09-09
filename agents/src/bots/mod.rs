use neumannarch_protocol::Bot;
use neumannarch_sim::roster::Roster;

use self::scripted::Scripted;
use self::scripted::personality::Personality;
use crate::Agent;

pub struct Shipped {
    pub bot: Bot,
    pub name: &'static str,
    pub agent: fn(&Roster) -> Box<dyn Agent>,
}

pub fn shipped() -> Vec<Shipped> {
    vec![
        Shipped {
            bot: Bot::Turtle,
            name: "turtle",
            agent: |roster| Box::new(Scripted::new(Personality::turtle(), roster.clone())),
        },
        Shipped {
            bot: Bot::Expand,
            name: "expand",
            agent: |roster| Box::new(Scripted::new(Personality::expand(), roster.clone())),
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

    pub fn seated(&self, roster: &Roster) -> Box<dyn Agent> {
        (self.agent)(roster)
    }
}

pub mod scripted;
