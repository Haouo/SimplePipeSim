use super::{BranchPredictResult, BranchPredictor};

pub struct Predictor {
    up_down_saturate_counter: u8,
    // @TODO: implement simple bimodal predictor with one global up-down saturated counter
    // history_table: TODO
}

impl Predictor {
    pub fn new() -> Self {
        Self {
            up_down_saturate_counter: 0,
        }
    }
}

impl BranchPredictor for Predictor {
    fn branch_predict(&mut self, current_pc: u32) -> BranchPredictResult {
        todo!();
    }
    fn mispredict_recovery(&mut self, correct_direction: bool, target_addr: u32) {
        todo!()
    }
}
