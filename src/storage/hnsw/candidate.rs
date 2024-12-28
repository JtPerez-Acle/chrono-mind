use std::cmp::Ordering;

/// Candidate node during search
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    pub distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance.eq(&other.distance)
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl Candidate {
    pub fn new(id: String, distance: f32) -> Self {
        Self { id, distance }
    }
}
