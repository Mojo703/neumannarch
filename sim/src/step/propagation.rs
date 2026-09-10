use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::state::{Entity, State};
use crate::step::holding::Thrusts;
use crate::time::RunningSpan;
use crate::transfer::Transfer;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Move {
    pub(crate) entity: EntityId,
    pub body: Body,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Moved {
    steps: Vec<Move>,
    arrived: Vec<EntityId>,
}

pub(crate) struct Propagation<'a> {
    state: &'a State,
    thrusts: &'a Thrusts,
    over: RunningSpan,
}

impl<'a> Propagation<'a> {
    pub(crate) fn of(state: &'a State, thrusts: &'a Thrusts, over: RunningSpan) -> Propagation<'a> {
        Propagation {
            state,
            thrusts,
            over,
        }
    }

    pub(crate) fn run(self) -> Moved {
        let mut steps = Vec::new();
        let mut arrived = Vec::new();
        for entity in self.state.entities.steered() {
            let Some(step) = self.moved(entity) else {
                continue;
            };
            if entity.is_flying() && self.landed(entity, step.body) {
                arrived.push(entity.id());
            }
            steps.push(step);
        }
        Moved { steps, arrived }
    }

    fn moved(&self, entity: Entity) -> Option<Move> {
        let body = entity.steered()?;
        Some(Move {
            entity: entity.id(),
            body: body.after_tick(self.thrusts.of(entity.id()), self.state.gravity()),
        })
    }

    fn landed(&self, entity: Entity, body: Body) -> bool {
        let gravity = self.state.gravity();
        let destination = self.state[entity.home()]
            .orbit()
            .at(self.over.ends_at(self.state.time()), gravity);
        Transfer::of(body, destination, self.state.roster().movement_limit().0).arrived()
    }
}

impl Moved {
    pub(crate) fn iter(&self) -> impl Iterator<Item = Move> + '_ {
        self.steps.iter().copied()
    }

    pub(crate) fn arrived(&self) -> &[EntityId] {
        &self.arrived
    }
}
