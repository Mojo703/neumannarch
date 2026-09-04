use std::collections::BTreeMap;

use crate::ids::SeatId;
use crate::materials::Materials;
use crate::state::State;
use crate::time::Tick;

pub struct Extraction<'a> {
    state: &'a State,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Income(Vec<(SeatId, Materials)>);

impl<'a> Extraction<'a> {
    pub fn of(state: &'a State) -> Extraction<'a> {
        Extraction { state }
    }

    pub fn run(self) -> Income {
        let dt = Tick(1).seconds();
        let mut income: BTreeMap<SeatId, Materials> = BTreeMap::new();
        for (id, rock) in self.state.rocks().iter().enumerate() {
            let rock_id = crate::RockId(id as u32);
            let extractors: Vec<Extractor> = self
                .state
                .standing_at(rock_id)
                .flat_map(|entity| {
                    self.state[entity.row()]
                        .extracts()
                        .map(move |rate| Extractor {
                            seat: entity.seat(),
                            rate,
                        })
                        .collect::<Vec<Extractor>>()
                })
                .collect();
            for (seat, taken) in extract(rock.caps(), &extractors, dt) {
                *income.entry(seat).or_default() += taken;
            }
        }
        Income(income.into_iter().collect())
    }
}

impl Income {
    pub fn iter(&self) -> impl Iterator<Item = (SeatId, Materials)> + '_ {
        self.0.iter().copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extractor {
    pub seat: SeatId,
    pub rate: f64,
}

pub fn extract(caps: Materials, extractors: &[Extractor], dt: f64) -> Vec<(SeatId, Materials)> {
    let rates: Vec<f64> = extractors.iter().map(|e| e.rate).collect();
    let metals = split(caps.metals, &rates);
    let volatiles = split(caps.volatiles, &rates);
    let energy = split(caps.energy, &rates);
    let mut income: BTreeMap<SeatId, Materials> = BTreeMap::new();
    for (((e, metals), volatiles), energy) in
        extractors.iter().zip(metals).zip(volatiles).zip(energy)
    {
        *income.entry(e.seat).or_default() += Materials::new(metals, volatiles, energy) * dt;
    }
    income.into_iter().collect()
}

pub fn split(cap: f64, rates: &[f64]) -> Vec<f64> {
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

    fn sum(shares: &[f64]) -> f64 {
        shares.iter().sum()
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
    fn extract_sums_a_seats_extractors_and_orders_seats() {
        let extractors = [
            Extractor {
                seat: SeatId(2),
                rate: 1.0,
            },
            Extractor {
                seat: SeatId(1),
                rate: 2.0,
            },
            Extractor {
                seat: SeatId(2),
                rate: 3.0,
            },
        ];
        let income = extract(Materials::new(100.0, 3.0, 0.0), &extractors, 0.5);
        assert_eq!(
            income,
            vec![
                (SeatId(1), Materials::new(1.0, 0.5, 0.0)),
                (SeatId(2), Materials::new(2.0, 1.0, 0.0)),
            ]
        );
    }

    #[test]
    fn no_extractors_yield_no_income() {
        assert!(extract(Materials::new(1.0, 1.0, 1.0), &[], 1.0).is_empty());
    }
}
