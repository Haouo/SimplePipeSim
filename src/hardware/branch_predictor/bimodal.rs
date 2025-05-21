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

    pub fn predict_correctly(&mut self) {
        if self.0 < 3 {
            self.0 += 1;
        }
    }

    pub fn predict_incorrectly(&mut self) {
        if self.0 > 0 {
            self.0 -= 1;
        }
    }
}

pub struct Predictor {
    direction_precditor: BimodalCounter, // global shared counter for branch direction prediction
    history_table: HashMap<u32, u32>,    // a small cache to record the branch history
}

impl Predictor {
    pub fn new() -> Self {
        Self {
            direction_precditor: BimodalCounter::new(),
            history_table: HashMap::new(),
        }
    }
}

impl BranchPredict for Predictor {
    fn branch_predict(&mut self, current_pc: u32) -> BranchPredictResult {
        // There are two requirements the predictor must meet to predict branch taken.
        // 1. The Direction Predictor predicts taken
        // 2. The Branch History Table is also indexed hit instead of miss
        todo!();
    }
    fn mispredict_recovery(&mut self, correct_direction: bool, target_addr: u32) {
        todo!()
    }
}
