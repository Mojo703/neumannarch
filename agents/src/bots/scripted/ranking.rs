pub struct Ranking<Id> {
    scored: Vec<(Id, f64)>,
}

impl<Id: Copy + Ord> Ranking<Id> {
    pub fn by(
        candidates: impl IntoIterator<Item = Id>,
        score: impl Fn(Id) -> Option<f64>,
    ) -> Ranking<Id> {
        Ranking {
            scored: candidates
                .into_iter()
                .filter_map(|id| score(id).map(|score| (id, score)))
                .collect(),
        }
    }

    pub fn best(&self) -> Option<Id> {
        self.scored
            .iter()
            .max_by(|one, other| one.1.total_cmp(&other.1).then(other.0.cmp(&one.0)))
            .map(|(id, _)| *id)
    }

    pub fn order(self) -> Vec<Id> {
        let mut scored = self.scored;
        scored.sort_by(|one, other| other.1.total_cmp(&one.1).then(one.0.cmp(&other.0)));
        scored.into_iter().map(|(id, _)| id).collect()
    }
}
