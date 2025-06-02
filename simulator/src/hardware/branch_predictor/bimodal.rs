use super::{BranchPredict, BranchPredictResult};

use std::collections::HashMap;

struct BimodalCounter(u8);
impl BimodalCounter {
    pub fn new() -> Self {
        Self(0u8)
    }

    pub fn get_direction(&self) -> bool {
        if self.0 <= 1 {
            false
        } else {
            true
        }
    }

    pub fn counter_increase(&mut self) {
        if self.0 < 3 {
            self.0 += 1;
        }
    }

    pub fn counter_decrease(&mut self) {
        if self.0 > 0 {
            self.0 -= 1;
        }
    }
}

struct TagPcPair {
    pub tag: u32,
    pub target_pc: u32,
}

pub struct Predictor {
    direction_precditor: BimodalCounter, // global shared counter for branch direction prediction
    history_table: HashMap<u32, TagPcPair>, // a small cache to record the branch history
}

impl Predictor {
    /// Constructor
    pub fn new() -> Self {
        Self {
            direction_precditor: BimodalCounter::new(),
            history_table: HashMap::new(),
        }
    }

    /// A associated function to hash to incoing PC
    pub fn pc_hash(pc: u32) -> u32 {
        pc & 0xffffffff // it means that the BHT has 2^8 = 256 entries
    }
}

impl BranchPredict for Predictor {
    fn branch_predict(&mut self, pc: u32) -> BranchPredictResult {
        // There are two requirements the predictor must meet to predict branch taken.
        // 1. The Direction Predictor predicts taken
        // 2. The Branch History Table is also indexed hit instead of miss
        if self.direction_precditor.get_direction() {
            if let Some(tag_pc) = self.history_table.get(&pc) {
                if tag_pc.tag == (pc) {
                    // predicts taken with predicted target PC
                    return BranchPredictResult {
                        direction: true,
                        addr: tag_pc.target_pc,
                    };
                }
            }
        }
        BranchPredictResult {
            direction: false,
            addr: 0,
        }
    }
    fn mispredict_recovery(&mut self, correct_direction: bool, pc: u32, target_addr: u32) {
        // update direction predictor
        if correct_direction {
            self.direction_precditor.counter_increase();
        } else {
            self.direction_precditor.counter_decrease();
        }
        // update BHT
        if let Some(old_tag_pc) = self.history_table.get_mut(&Predictor::pc_hash(pc)) {
            old_tag_pc.tag = pc;
            old_tag_pc.target_pc = target_addr;
        } else {
            self.history_table.insert(
                Predictor::pc_hash(pc),
                TagPcPair {
                    tag: pc,
                    target_pc: target_addr,
                },
            );
        }
    }
}
