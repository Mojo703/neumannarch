use std::collections::BTreeMap;

use neumannarch_sim::pattern::{EntityPattern, Kind};
use neumannarch_sim::state::Held;
use neumannarch_sim::state::view::View;
use neumannarch_sim::{AsteroidId, Material, Materials, Posting};

use super::ranking::Ranking;

pub const REACH_METERS: f64 = 2_000.0;

pub struct Survey<'a> {
    pub view: &'a View,
    pub mine: BTreeMap<AsteroidId, BTreeMap<EntityPattern, Held>>,
    pub taken: Vec<AsteroidId>,
    pub enemy_asteroids: Vec<AsteroidId>,
    pub threats: BTreeMap<AsteroidId, f64>,
    pub enemy_plating: f64,
    pub enemy_range: f64,
}

impl<'a> Survey<'a> {
    pub fn of(view: &'a View) -> Survey<'a> {
        let mut mine: BTreeMap<AsteroidId, BTreeMap<EntityPattern, Held>> = BTreeMap::new();
        let mut taken: Vec<AsteroidId> = Vec::new();
        let mut enemy_asteroids: Vec<AsteroidId> = Vec::new();
        let mut threats: BTreeMap<AsteroidId, f64> = BTreeMap::new();
        let mut enemy_plating = 0.0f64;
        let mut enemy_range = 0.0f64;
        for (post, composition) in &view.compositions {
            let homed: u32 = composition
                .patterns
                .values()
                .map(|held| held.present + held.arriving)
                .sum();
            if homed > 0 {
                taken.push(post.asteroid);
            }
            if post.seat == view.seat {
                mine.insert(post.asteroid, composition.patterns.clone());
                continue;
            }
            if !view.is_enemy(post.seat) {
                continue;
            }
            for (pattern, held) in &composition.patterns {
                let count = held.present + held.arriving;
                if count == 0 {
                    continue;
                }
                enemy_plating = enemy_plating.max(pattern.plating().0);
                enemy_range = enemy_range.max(pattern.max_damage_range());
                if pattern.kind() == Kind::Structure {
                    enemy_asteroids.push(post.asteroid);
                }
                if pattern.does_damage() {
                    *threats.entry(post.asteroid).or_default() +=
                        pattern.cost().total() * f64::from(count);
                }
            }
        }
        taken.sort_unstable();
        taken.dedup();
        enemy_asteroids.sort_unstable();
        enemy_asteroids.dedup();
        Survey {
            view,
            mine,
            taken,
            enemy_asteroids,
            threats,
            enemy_plating,
            enemy_range,
        }
    }

    pub fn posting(&self, asteroid: AsteroidId, pattern: EntityPattern) -> Posting {
        Posting::of(asteroid, self.view.seat, pattern)
    }

    pub fn count(&self, asteroid: AsteroidId, pattern: EntityPattern) -> u32 {
        self.holding(asteroid, pattern)
            .map_or(0, |held| held.present + held.arriving)
    }

    pub fn postings(&self) -> Vec<Posting> {
        let mut postings: Vec<Posting> = self
            .mine
            .iter()
            .flat_map(|(asteroid, patterns)| {
                patterns
                    .keys()
                    .map(|pattern| self.posting(*asteroid, *pattern))
            })
            .chain(self.view.plans.keys().copied())
            .collect();
        postings.sort_unstable();
        postings.dedup();
        postings
    }

    pub fn want(&self, asteroid: AsteroidId, pattern: EntityPattern) -> u32 {
        self.view.want_of(self.posting(asteroid, pattern))
    }

    pub fn standing(&self, asteroid: AsteroidId, pattern: EntityPattern) -> u32 {
        self.holding(asteroid, pattern)
            .map_or(0, |held| held.present)
    }

    pub fn arriving(&self, asteroid: AsteroidId, pattern: EntityPattern) -> u32 {
        self.holding(asteroid, pattern)
            .map_or(0, |held| held.arriving)
    }

    pub fn between(&self, from: AsteroidId, to: AsteroidId) -> f64 {
        match (self.view.asteroid_body(from), self.view.asteroid_body(to)) {
            (Some(from), Some(to)) => from.pos.distance(to.pos),
            _ => 0.0,
        }
    }

    pub fn held(&self) -> Vec<AsteroidId> {
        self.mine
            .keys()
            .copied()
            .filter(|asteroid| {
                self.patterns(*asteroid)
                    .any(|(pattern, held)| held.present > 0 && pattern.kind() == Kind::Structure)
            })
            .collect()
    }

    pub fn builders(&self, asteroid: AsteroidId) -> u32 {
        self.patterns(asteroid)
            .filter(|(pattern, _)| pattern.build_rate() > 0.0)
            .map(|(_, held)| held.present)
            .sum()
    }

    pub fn builder_arriving(&self, asteroid: AsteroidId) -> bool {
        self.patterns(asteroid)
            .any(|(pattern, held)| pattern.build_rate() > 0.0 && held.arriving > 0)
    }

    pub fn builds_at(&self, asteroid: AsteroidId) -> bool {
        self.builders(asteroid) > 0 || self.builder_arriving(asteroid)
    }

    pub fn building(&self) -> Vec<AsteroidId> {
        self.mine
            .keys()
            .copied()
            .filter(|asteroid| self.builders(*asteroid) > 0)
            .collect()
    }

    pub fn developed(&self) -> Vec<AsteroidId> {
        let mut asteroids = self.held();
        asteroids.extend(self.building());
        asteroids.sort_unstable();
        asteroids.dedup();
        asteroids
    }

    pub fn occupied(&self) -> Vec<AsteroidId> {
        self.mine.keys().copied().collect()
    }

    pub fn home(&self) -> Option<AsteroidId> {
        Ranking::by(self.building(), |asteroid| {
            Some(self.build_rate_at(asteroid))
        })
        .best()
    }

    pub fn build_rate(&self) -> f64 {
        self.mine
            .keys()
            .map(|asteroid| self.build_rate_at(*asteroid))
            .sum()
    }

    pub fn build_rate_at(&self, asteroid: AsteroidId) -> f64 {
        self.patterns(asteroid)
            .map(|(pattern, held)| pattern.build_rate() * f64::from(held.present))
            .sum()
    }

    pub fn army(&self) -> f64 {
        self.mine
            .keys()
            .map(|asteroid| self.damage_value(*asteroid))
            .sum()
    }

    pub fn damage_value(&self, asteroid: AsteroidId) -> f64 {
        self.patterns(asteroid)
            .filter(|(pattern, _)| pattern.does_damage())
            .map(|(pattern, held)| pattern.cost().total() * f64::from(held.present + held.arriving))
            .sum()
    }

    pub fn enemy(&self) -> f64 {
        self.threats.values().sum()
    }

    pub fn threat_at(&self, asteroid: AsteroidId) -> f64 {
        self.threats.get(&asteroid).copied().unwrap_or_default()
    }

    pub fn frame_open(&self, asteroid: AsteroidId, pattern: EntityPattern) -> bool {
        self.view
            .plan_of(self.posting(asteroid, pattern))
            .is_some_and(|plan| plan.building.is_some())
    }

    pub fn short_of(&self, asteroid: AsteroidId, patterns: &[EntityPattern]) -> bool {
        patterns
            .iter()
            .any(|pattern| self.want(asteroid, *pattern) > self.count(asteroid, *pattern))
    }

    pub fn frame_no_builder_fills(&self, posting: Posting) -> bool {
        self.frame_open(posting.asteroid(), posting.pattern())
            && !self.builds_at(posting.asteroid())
    }

    pub fn spare_cap_at(&self, asteroid: AsteroidId, material: Material) -> f64 {
        self.view
            .terrain_of(asteroid)
            .map_or(0.0, |terrain| {
                terrain.caps[material] - terrain.pull[material]
            })
            .max(0.0)
    }

    pub fn spare_cap(&self, material: Material) -> f64 {
        self.developed()
            .into_iter()
            .map(|asteroid| self.spare_cap_at(asteroid, material))
            .sum()
    }

    pub fn threatened(&self) -> bool {
        self.developed()
            .into_iter()
            .any(|asteroid| self.threat_at(asteroid) > 0.0)
    }

    pub fn short_of_room(&self, demand: Materials) -> bool {
        Material::EVERY
            .into_iter()
            .any(|material| demand[material] > 0.0 && self.spare_cap(material) < demand[material])
    }

    pub fn standing_cost(&self) -> Materials {
        self.mine
            .values()
            .flatten()
            .map(|(pattern, held)| pattern.cost() * f64::from(held.present + held.arriving))
            .fold(Materials::ZERO, |standing, cost| standing + cost)
    }

    pub fn fittest(&self, wanted: Materials) -> Option<AsteroidId> {
        let held = self.held();
        let nearest = |from: AsteroidId, among: &[AsteroidId]| -> Option<f64> {
            among
                .iter()
                .map(|to| self.between(from, *to))
                .min_by(f64::total_cmp)
        };
        Ranking::by(
            self.view
                .terrain
                .iter()
                .map(|terrain| terrain.asteroid)
                .filter(|asteroid| !self.taken.contains(asteroid)),
            |asteroid| {
                let terrain = self.view.terrain_of(asteroid)?;
                let fit: f64 = terrain
                    .caps
                    .amounts()
                    .map(|(material, cap)| cap * wanted[material])
                    .sum();
                let away = nearest(asteroid, &self.enemy_asteroids)
                    .map_or(1.0, |gap| gap / (gap + REACH_METERS));
                let near = nearest(asteroid, &held).unwrap_or_default();
                Some(fit * away / (1.0 + near / REACH_METERS))
            },
        )
        .best()
    }

    pub fn wanting_more_at(&self, asteroid: AsteroidId) -> bool {
        self.view
            .plans
            .iter()
            .filter(|(posting, _)| posting.asteroid() == asteroid)
            .any(|(posting, plan)| plan.want > self.standing(asteroid, posting.pattern()))
    }

    pub fn homed_at(&self, asteroid: AsteroidId) -> u32 {
        self.patterns(asteroid)
            .map(|(_, held)| held.present + held.arriving)
            .sum()
    }

    pub fn nearest(&self, from: AsteroidId, among: &[AsteroidId]) -> Option<AsteroidId> {
        Ranking::by(among.iter().copied(), |to| Some(-self.between(from, to))).best()
    }

    pub fn staging(&self) -> Option<AsteroidId> {
        let enemies = &self.enemy_asteroids;
        Ranking::by(self.building(), |asteroid| {
            let to = self.nearest(asteroid, enemies)?;
            Some(-self.between(asteroid, to))
        })
        .best()
        .or_else(|| self.home())
    }

    fn patterns(&self, asteroid: AsteroidId) -> impl Iterator<Item = (EntityPattern, &Held)> {
        self.mine
            .get(&asteroid)
            .into_iter()
            .flatten()
            .map(|(pattern, held)| (*pattern, held))
    }

    fn holding(&self, asteroid: AsteroidId, pattern: EntityPattern) -> Option<&Held> {
        self.mine
            .get(&asteroid)
            .and_then(|patterns| patterns.get(&pattern))
    }
}
