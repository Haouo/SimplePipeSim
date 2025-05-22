use super::{BranchPredict, BranchPredictResult};

pub struct Predictor {
    //
}

impl Predictor {
    pub fn new() -> Self {
        todo!();
    }
}

impl BranchPredict for Predictor {
    fn branch_predict(&mut self, current_pc: u32) -> BranchPredictResult {
        todo!();
    }
    fn mispredict_recovery(&mut self, correct_direction: bool, target_addr: u32) {
        todo!();
    }
}
