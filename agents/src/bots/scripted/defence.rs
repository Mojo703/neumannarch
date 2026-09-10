use super::personality::Personality;
use super::proposal::{Proposal, Reason};
use super::survey::Survey;

pub struct Defence;

impl Defence {
    pub fn proposals(survey: &Survey, personality: &Personality) -> Vec<Proposal> {
        let weights = personality.shares(survey);
        let unit_cost = Personality::damage_unit_cost(&weights);
        if unit_cost <= 0.0 {
            return Vec::new();
        }
        let mut proposals = Vec::new();
        for asteroid in survey.developed() {
            let threat = survey.threat_at(asteroid);
            if threat <= 0.0 {
                continue;
            }
            let units = personality.garrison(threat, unit_cost) / unit_cost;
            for (pattern, share) in &weights {
                proposals.push(Proposal::rounded(
                    survey,
                    Reason::Defence,
                    asteroid,
                    *pattern,
                    share * units,
                ));
            }
        }
        proposals
    }
}
