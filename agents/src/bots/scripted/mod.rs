use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::View;

use self::commitments::Commitments;
use self::dice::Dice;
use self::personality::Personality;
use self::plan::Plan;
use self::roles::Roles;
use self::survey::Survey;
use crate::Agent;

pub struct Scripted {
    personality: Personality,
    roster: Roster,
    roles: Roles,
    commitments: Commitments,
    dice: Dice,
}

impl Scripted {
    pub fn new(personality: Personality, roster: Roster) -> Scripted {
        Scripted {
            roles: Roles::of(&roster),
            dice: Dice::new(personality.seed),
            personality,
            roster,
            commitments: Commitments::default(),
        }
    }

    pub fn personality(&self) -> &Personality {
        &self.personality
    }
}

impl Agent for Scripted {
    fn decide(&mut self, view: &View) -> Vec<Command> {
        let survey = Survey::of(view, &self.roster, &self.roles);
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
pub(crate) mod roles;
pub(crate) mod survey;

#[cfg(test)]
mod tests;
