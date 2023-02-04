use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet, VecDeque},
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use super::ballot::Ballot;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct ChosenSlot {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, PartialOrd, Ord, Default)]
pub struct DataHash(u64);

impl<T: Hash> From<T> for DataHash {
    fn from(value: T) -> Self {
        DataHash(hash(&value))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, PartialOrd, Default)]
pub struct SlotVoteValue(Vec<u8>, BTreeSet<String>, Ballot);
impl Ord for SlotVoteValue {
    // using this would need a lot of debugging, also need to know if an election on slot is won
    // may need to handwrite this logic elsewhere
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.2.cmp(&other.2) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.1.len().cmp(&other.1.len())
    }
}

pub type SlotVote = BTreeMap<DataHash, SlotVoteValue>;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct UnchosenSlot {
    vote: SlotVote,
}

impl Into<ChosenSlot> for UnchosenSlot {
    fn into(self) -> ChosenSlot {
        match self.vote.len() {
            0 => {
                return ChosenSlot {
                    data: Vec::from("INVALID"),
                };
            }
            1 => {
                let data = self.vote.into_iter().next().unwrap().1 .0;
                return ChosenSlot { data };
            }
            _ => {}
        };

        let best =
            self.vote
                .into_iter()
                .fold(
                    (DataHash::default(), SlotVoteValue::default()),
                    |a, b| match a.1.cmp(&b.1) {
                        std::cmp::Ordering::Less => b,
                        std::cmp::Ordering::Equal => a,
                        std::cmp::Ordering::Greater => a,
                    },
                );
        // destructure to get data
        let (_, SlotVoteValue(data, _, _)) = best;
        ChosenSlot { data }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Slot {
    Chosen(ChosenSlot),
    Unchosen(UnchosenSlot),
    Empty,
    Cleared,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct PaxosLog {
    slot_offset: u64,
    slots: VecDeque<Slot>,
}

impl PaxosLog {
    pub fn slot(&self, slot_num: u64) -> &Slot {
        if slot_num < self.slot_offset {
            return &Slot::Cleared;
        }
        if slot_num >= self.slot_offset + self.slots.len() as u64 {
            return &Slot::Empty;
        }
        let idx: usize = (slot_num - self.slot_offset) as usize;
        &self.slots.get(idx).unwrap()
    }

    pub(crate) fn merge(&self, _log: &PaxosLog) {
        //todo!()
    }
}

pub struct ChosenValue(u64, Vec<u8>);

impl Iterator for PaxosLog {
    type Item = ChosenValue;

    fn next(&mut self) -> Option<Self::Item> {
        if self.slots.is_empty() {
            return None;
        }
        match self.slots.front().unwrap() {
            Slot::Chosen(_) => {}
            _ => return None,
        };
        let slot_offset_res = self.slot_offset;
        self.slot_offset += 1;
        if let Slot::Chosen(chosen) = self.slots.pop_front().unwrap() {
            Some(ChosenValue(slot_offset_res, chosen.data))
        } else {
            // should never occur as previously checked this value is chosen
            // pre removal
            None
        }
    }
}

/// utils

fn hash<T: Hash>(t: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    t.hash(&mut hasher);
    hasher.finish()
}
