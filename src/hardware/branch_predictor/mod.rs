#[derive(Default)]
pub struct BranchPredictResult {
    pub direction: bool,
    pub addr: Option<u32>,
}

pub trait BranchPredict {
    fn branch_predict(&mut self, current_pc: u32) -> BranchPredictResult;
    fn mispredict_recovery(&mut self, correct_direction: bool, target_addr: u32);
}

// Dummp Predictor which always predicts branch not-taken
pub mod dummy {
    use super::*;
    pub struct Predictor;
    impl BranchPredict for Predictor {
        #[allow(unused)]
        fn branch_predict(&mut self, current_pc: u32) -> BranchPredictResult {
            BranchPredictResult {
                direction: false,
                addr: None,
            }
        }
        #[allow(unused)]
        fn mispredict_recovery(&mut self, correct_direction: bool, target_addr: u32) {}
    }
}

// other predictor implementations
pub mod bimodal;
pub mod two_level_adaptive;
