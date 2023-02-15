use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct Ballot {
    pub leader: String,
    pub number: usize,
}

// treat ballot with number = usize::MIN as invalid
impl Ballot {
    pub(crate) fn new(node: String, number: usize) -> Self {
        Ballot {
            leader: node,
            number,
        }
    }
}

/// Ordering of ballots:
/// firstly compare against number, whichever number is higher the ballot is greater
/// if both are equal base on the node in ballot

impl Ord for Ballot {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.number.cmp(&other.number) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.leader.cmp(&other.leader)
    }
}

impl PartialOrd for Ballot {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.number.partial_cmp(&other.number) {
            Some(std::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.leader.partial_cmp(&other.leader)
    }
}

impl std::fmt::Display for Ballot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.leader, self.number)
    }
}
