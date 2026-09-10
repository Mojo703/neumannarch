use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::View;

use self::commitments::Commitments;
use self::dice::Dice;
use self::personality::Personality;
use self::plan::Plan;
use self::survey::Survey;
use crate::Agent;

pub struct Scripted {
    personality: Personality,
    commitments: Commitments,
    dice: Dice,
}

impl Scripted {
    pub fn new(personality: Personality) -> Scripted {
        Scripted {
            dice: Dice::new(personality.seed),
            personality,
            commitments: Commitments::default(),
        }
    }

    pub fn personality(&self) -> &Personality {
        &self.personality
    }
}

impl Agent for Scripted {
    fn decide(&mut self, view: &View) -> Vec<Command> {
        let survey = Survey::of(view);
        self.commitments.settle(&survey);
        let plan = Plan::of(
            &survey,
            &self.personality,
            &mut self.commitments,
            &mut self.dice,
        );
        plan.commands(view)
    }
}

mod commitments;
mod defence;
mod dice;
mod economy;
mod expansion;
mod funding;
mod offence;
pub mod personality;
mod plan;
mod proposal;
mod ranking;
pub(crate) mod survey;

#[cfg(test)]
mod tests;
