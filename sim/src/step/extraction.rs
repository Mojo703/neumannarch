use std::collections::BTreeMap;

use crate::ids::{AsteroidId, SeatId};
use crate::materials::{Material, Materials};
use crate::state::{Roll, Rolls, State};
use crate::time::Time;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Income(BTreeMap<(AsteroidId, SeatId), Materials>);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Extractor {
    seat: SeatId,
    material: Material,
    rate: f64,
}

impl Income {
    pub(crate) fn extracted(state: &State, rolls: &Rolls, ran: Time) -> Income {
        let mut taken = Income::default();
        for roll in rolls.iter() {
            let caps = state[roll.asteroid()].caps();
            let pulling = Extractor::pulling(state, roll);
            taken
                .0
                .extend(extract(roll.asteroid(), caps, &pulling, ran.seconds()).0);
        }
        taken
    }

    pub(crate) fn apply(&self, state: &mut State) {
        for ((asteroid, seat), taken) in &self.0 {
            state[*seat].earn(*taken);
            state[*asteroid].extract(*taken);
        }
    }
}

impl Extractor {
    fn pulling(state: &State, roll: &Roll) -> Vec<Extractor> {
        let mut extractors = Vec::new();
        for entity in roll.standing() {
            let seat = entity.seat();
            let pulls = state[entity.row()].extracts();
            extractors.extend(pulls.map(|(material, rate)| Extractor {
                seat,
                material,
                rate,
            }));
        }
        extractors
    }
}

fn extract(
    asteroid: AsteroidId,
    caps: Materials,
    extractors: &[Extractor],
    seconds: f64,
) -> Income {
    let mut income: BTreeMap<SeatId, Materials> = BTreeMap::new();
    for (material, cap) in caps.amounts() {
        let pulling: Vec<&Extractor> = extractors
            .iter()
            .filter(|extractor| extractor.material == material)
            .collect();
        let rates: Vec<f64> = pulling.iter().map(|extractor| extractor.rate).collect();
        for (extractor, share) in pulling.iter().zip(split(cap, &rates)) {
            income.entry(extractor.seat).or_default()[material] += share * seconds;
        }
    }
    Income(
        income
            .into_iter()
            .map(|(seat, taken)| ((asteroid, seat), taken))
            .collect(),
    )
}

fn split(cap: f64, rates: &[f64]) -> Vec<f64> {
    if rates.iter().sum::<f64>() <= cap {
        return rates.to_vec();
    }
    let mut ascending: Vec<usize> = (0..rates.len()).collect();
    ascending.sort_by(|&a, &b| rates[a].total_cmp(&rates[b]));
    let mut shares = vec![0.0; rates.len()];
    let mut remaining = cap;
    for (left, &i) in (1..=rates.len()).rev().zip(&ascending) {
        let share = rates[i].min(remaining / left as f64);
        shares[i] = share;
        remaining -= share;
    }
    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASTEROID: AsteroidId = AsteroidId(3);

    fn sum(shares: &[f64]) -> f64 {
        shares.iter().sum()
    }

    fn extractor(seat: u8, material: Material, rate: f64) -> Extractor {
        Extractor {
            seat: SeatId(seat),
            material,
            rate,
        }
    }

    fn income(taken: [(SeatId, Materials); 2]) -> Income {
        Income(taken.map(|(seat, taken)| ((ASTEROID, seat), taken)).into())
    }

    #[test]
    fn under_the_cap_each_takes_its_rate() {
        assert_eq!(split(10.0, &[1.0, 2.0, 3.0]), vec![1.0, 2.0, 3.0]);
        assert_eq!(split(6.0, &[1.0, 2.0, 3.0]), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn over_the_cap_equal_rates_split_the_cap_equally() {
        assert_eq!(split(6.0, &[4.0, 4.0, 4.0]), vec![2.0, 2.0, 2.0]);
    }

    #[test]
    fn a_small_extractor_takes_its_rate_and_the_large_split_the_rest() {
        assert_eq!(split(10.0, &[8.0, 1.0, 8.0]), vec![4.5, 1.0, 4.5]);
    }

    #[test]
    fn unused_shares_redistribute_until_none_is_left() {
        assert_eq!(
            split(10.0, &[100.0, 3.0, 1.0, 2.0]),
            vec![4.0, 3.0, 1.0, 2.0]
        );
    }

    #[test]
    fn shares_never_exceed_the_cap_or_their_rate() {
        let cases: [(f64, &[f64]); 5] = [
            (10.0, &[1.0, 2.0, 3.0]),
            (6.0, &[4.0, 4.0, 4.0]),
            (10.0, &[8.0, 1.0, 8.0]),
            (7.0, &[0.1, 0.2, 0.3, 5.0, 9.0]),
            (7.0, &[]),
        ];
        for (cap, rates) in cases {
            let shares = split(cap, rates);
            assert_eq!(shares.len(), rates.len());
            for (share, rate) in shares.iter().zip(rates) {
                assert!(share <= rate, "{share} exceeds {rate}");
            }
            let expected = cap.min(sum(rates));
            assert!(
                (sum(&shares) - expected).abs() <= 1e-12,
                "{shares:?} sums off {expected}"
            );
        }
    }

    #[test]
    fn a_zero_cap_yields_zeros() {
        assert_eq!(split(0.0, &[1.0, 2.0]), vec![0.0, 0.0]);
    }

    #[test]
    fn a_materials_cap_is_split_among_the_extractors_of_that_material_alone() {
        let extractors = [
            extractor(2, Material::Metals, 4.0),
            extractor(1, Material::Volatiles, 4.0),
            extractor(2, Material::Metals, 4.0),
        ];

        let taken = extract(ASTEROID, Materials::new(6.0, 1.0, 9.0), &extractors, 1.0);

        assert_eq!(
            taken,
            income([
                (SeatId(1), Materials::new(0.0, 1.0, 0.0)),
                (SeatId(2), Materials::new(6.0, 0.0, 0.0)),
            ]),
            "the metals pair split six, the lone volatiles took the cap, energy nobody"
        );
    }

    #[test]
    fn extract_sums_a_seats_extractors_at_the_asteroid_they_stand_at() {
        let extractors = [
            extractor(2, Material::Metals, 1.0),
            extractor(1, Material::Metals, 2.0),
            extractor(2, Material::Energy, 3.0),
        ];

        let taken = extract(
            ASTEROID,
            Materials::new(100.0, 3.0, 100.0),
            &extractors,
            0.5,
        );

        assert_eq!(
            taken,
            income([
                (SeatId(1), Materials::new(1.0, 0.0, 0.0)),
                (SeatId(2), Materials::new(0.5, 0.0, 1.5)),
            ])
        );
    }

    #[test]
    fn no_extractors_yield_no_income() {
        let bare = extract(ASTEROID, Materials::new(1.0, 1.0, 1.0), &[], 1.0);
        assert_eq!(bare, Income::default());
    }
}
