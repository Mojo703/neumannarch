use core::ops::Add;

use crate::ids::{EntityId, RockId, SeatId};
use crate::materials::{Material, Materials, Stockpile};
use crate::state::{Entity, State};
use crate::time::Tick;

pub struct Construction<'a> {
    state: &'a State,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Repair {
    pub entity: EntityId,
    pub hp: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub spends: Vec<Spend>,
    pub repairs: Vec<Repair>,
}

impl<'a> Construction<'a> {
    pub fn of(state: &'a State) -> Construction<'a> {
        Construction { state }
    }

    pub fn run(self) -> Progress {
        let mut progress = Progress::default();
        let dt = Tick(1).seconds();
        for seat in self.seats() {
            let mut stockpile = *self.state[seat].stockpile();
            for rock in self.rocks_of(seat) {
                let rates = self.builders(seat, rock);
                let left = self.build(&mut progress, seat, rock, &rates, dt, &mut stockpile);
                self.repair(&mut progress, seat, rock, left);
            }
        }
        progress
    }

    fn seats(&self) -> Vec<SeatId> {
        let mut seats: Vec<SeatId> = self
            .state
            .entities()
            .filter(|entity| self.rate_of(entity) > 0.0)
            .map(Entity::seat)
            .collect();
        seats.sort_unstable();
        seats.dedup();
        seats
    }

    fn rocks_of(&self, seat: SeatId) -> Vec<RockId> {
        let mut rocks: Vec<RockId> = self
            .state
            .entities()
            .filter(|entity| entity.seat() == seat && self.rate_of(entity) > 0.0)
            .filter_map(|entity| entity.standing(self.state.tick()))
            .collect();
        rocks.sort_unstable();
        rocks.dedup();
        rocks
    }

    fn builders(&self, seat: SeatId, rock: RockId) -> Vec<f64> {
        self.state
            .standing_at(rock)
            .filter(|entity| entity.seat() == seat)
            .flat_map(|entity| self.state[entity.row()].builds())
            .collect()
    }

    fn rate_of(&self, entity: &Entity) -> f64 {
        if entity.is_flying(self.state.tick()) {
            return 0.0;
        }
        self.state[entity.row()].builds().sum()
    }

    fn build(
        &self,
        progress: &mut Progress,
        seat: SeatId,
        rock: RockId,
        rates: &[f64],
        dt: f64,
        stockpile: &mut Stockpile,
    ) -> f64 {
        let frames = self.frames_at(seat, rock);
        let works: Vec<Work> = frames
            .iter()
            .map(|at| {
                let frame = &self.state.frames()[*at];
                Work {
                    cost: self.state[frame.row()].cost,
                    progress: frame.progress(),
                }
            })
            .collect();
        let efforts = assign(rates, works.len(), dt);
        let unused = efforts
            .iter()
            .zip(&works)
            .map(|(effort, work)| effort.amount - effort.amount.min(work.left()))
            .sum::<f64>();
        let spare = rates.iter().sum::<f64>() * dt - efforts.iter().map(|e| e.amount).sum::<f64>();
        progress.spends.extend(
            spend(&works, &efforts, stockpile)
                .into_iter()
                .map(|s| Spend {
                    frame: frames[s.frame],
                    ..s
                }),
        );
        unused + spare
    }

    fn repair(&self, progress: &mut Progress, seat: SeatId, rock: RockId, effort: f64) {
        let damaged: Vec<&Entity> = self
            .state
            .standing_at(rock)
            .filter(|entity| entity.seat() == seat)
            .filter(|entity| entity.hp() < self.state[entity.row()].hp.0)
            .collect();
        if damaged.is_empty() || effort <= 0.0 {
            return;
        }
        let share = effort / damaged.len() as f64;
        for entity in damaged {
            let missing = self.state[entity.row()].hp.0 - entity.hp();
            progress.repairs.push(Repair {
                entity: entity.id(),
                hp: share.min(missing),
            });
        }
    }

    fn frames_at(&self, seat: SeatId, rock: RockId) -> Vec<usize> {
        self.state
            .frames()
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame.post().seat == seat && frame.post().rock == rock)
            .map(|(at, _)| at)
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Work {
    pub cost: Materials,
    pub progress: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effort {
    pub frame: usize,
    pub amount: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spend {
    pub frame: usize,
    pub materials: Materials,
    pub completed: bool,
    pub short: Option<Material>,
}

struct Want {
    frame: usize,
    work: Work,
    units: f64,
}

impl Work {
    fn left(&self) -> f64 {
        (self.cost.total() - self.progress).max(0.0)
    }

    fn materials(&self, units: f64) -> Materials {
        self.cost * (units / self.cost.total())
    }
}

impl Want {
    fn of(works: &[Work], effort: &Effort) -> Option<Want> {
        let work = works[effort.frame];

        (work.cost.total() > 0.0).then(|| Want {
            frame: effort.frame,
            work,
            units: effort.amount.min(work.left()),
        })
    }

    fn demand(&self) -> Materials {
        self.work.materials(self.units)
    }

    fn spend(&self, ratios: Materials, stockpile: &mut Stockpile) -> Spend {
        let units = self.units * self.work.cost.bottleneck(ratios);
        let taken = stockpile.spend(self.work.materials(units));
        Spend {
            frame: self.frame,
            materials: taken,
            completed: self.work.progress + taken.total() >= self.work.cost.total(),
            short: match taken.total() > 0.0 {
                true => None,
                false => self.work.cost.binding_material(ratios),
            },
        }
    }
}

pub fn assign(rates: &[f64], frames: usize, dt: f64) -> Vec<Effort> {
    let combined = rates.iter().sum::<f64>() * dt;
    if frames == 0 || combined <= 0.0 {
        return Vec::new();
    }
    let amount = combined / frames as f64;
    (0..frames).map(|frame| Effort { frame, amount }).collect()
}

pub fn spend(works: &[Work], efforts: &[Effort], stockpile: &mut Stockpile) -> Vec<Spend> {
    let wants: Vec<Want> = efforts
        .iter()
        .filter_map(|effort| Want::of(works, effort))
        .collect();
    let demand = wants
        .iter()
        .map(Want::demand)
        .fold(Materials::ZERO, Add::add);
    let ratios = stockpile.stock().covers(demand);
    wants
        .iter()
        .map(|want| want.spend(ratios, stockpile))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMPLE: Materials = Materials::new(1000.0, 1000.0, 1000.0);

    fn stockpile(stock: Materials) -> Stockpile {
        Stockpile::new(stock, AMPLE)
    }

    #[test]
    fn builders_combine_and_split_evenly_across_frames() {
        let dt = 0.5;
        let efforts = assign(&[1.0, 2.0, 3.0], 2, dt);
        assert_eq!(
            efforts,
            vec![
                Effort {
                    frame: 0,
                    amount: 3.0 * dt
                },
                Effort {
                    frame: 1,
                    amount: 3.0 * dt
                },
            ]
        );
    }

    #[test]
    fn no_effort_is_assigned_without_frames_or_rate() {
        assert!(assign(&[1.0], 0, 0.5).is_empty());
        assert!(assign(&[0.0, 0.0], 3, 0.5).is_empty());
        assert!(assign(&[], 3, 0.5).is_empty());
    }

    #[test]
    fn a_short_stockpile_pays_only_what_it_has_and_credits_only_that() {
        let works = [Work {
            cost: Materials::new(40.0, 0.0, 10.0),
            progress: 49.5,
        }];
        let efforts = [Effort {
            frame: 0,
            amount: 1.5,
        }];
        let mut pile = stockpile(Materials::new(0.2, 0.0, 1.0));
        let spends = spend(&works, &efforts, &mut pile);
        assert_eq!(
            spends,
            vec![Spend {
                frame: 0,
                materials: Materials::new(0.2, 0.0, 0.05),
                completed: false,
                short: None,
            }]
        );
        assert_eq!(pile.stock(), Materials::new(0.0, 0.0, 0.95));
    }

    #[test]
    fn nothing_is_spent_or_completed_without_stock() {
        let works = [Work {
            cost: Materials::new(4.0, 0.0, 4.0),
            progress: 7.0,
        }];
        let efforts = [Effort {
            frame: 0,
            amount: 10.0,
        }];
        let mut pile = stockpile(Materials::ZERO);
        let spends = spend(&works, &efforts, &mut pile);
        assert_eq!(
            spends,
            vec![Spend {
                frame: 0,
                materials: Materials::ZERO,
                completed: false,
                short: Some(Material::Metals),
            }]
        );
        assert_eq!(pile.stock(), Materials::ZERO);
    }

    #[test]
    fn a_frame_completes_exactly_when_its_progress_reaches_its_cost_total() {
        let works = [Work {
            cost: Materials::new(4.0, 0.0, 4.0),
            progress: 6.0,
        }];
        let effort = |amount| [Effort { frame: 0, amount }];
        let mut pile = stockpile(AMPLE);
        assert!(!spend(&works, &effort(1.0), &mut pile)[0].completed);
        assert!(spend(&works, &effort(2.0), &mut pile)[0].completed);
        let overshoot = spend(&works, &effort(5.0), &mut pile)[0];
        assert!(overshoot.completed);
        assert_eq!(overshoot.materials, Materials::new(1.0, 0.0, 1.0));
    }

    #[test]
    fn a_metals_only_row_is_not_slowed_by_an_energy_shortage() {
        let works = [
            Work {
                cost: Materials::new(10.0, 0.0, 0.0),
                progress: 0.0,
            },
            Work {
                cost: Materials::new(0.0, 0.0, 10.0),
                progress: 0.0,
            },
        ];
        let efforts = [
            Effort {
                frame: 0,
                amount: 1.0,
            },
            Effort {
                frame: 1,
                amount: 1.0,
            },
        ];
        let mut pile = stockpile(Materials::new(100.0, 0.0, 0.5));
        let spends = spend(&works, &efforts, &mut pile);
        assert_eq!(spends[0].materials, Materials::new(1.0, 0.0, 0.0));
        assert_eq!(spends[1].materials, Materials::new(0.0, 0.0, 0.5));
    }

    #[test]
    fn a_frame_costing_nothing_is_never_built() {
        let works = [Work {
            cost: Materials::ZERO,
            progress: 0.0,
        }];
        let efforts = [Effort {
            frame: 0,
            amount: 1.0,
        }];
        let mut pile = stockpile(AMPLE);
        assert!(spend(&works, &efforts, &mut pile).is_empty());
        assert_eq!(pile.stock(), AMPLE);
    }
}
