#[derive(Default, Clone, Copy)]
pub struct BranchPredictResult {
    pub direction: bool,
    pub addr: u32,
}

pub trait BranchPredict {
    fn branch_predict(&mut self, pc: u32) -> BranchPredictResult;
    fn mispredict_recovery(&mut self, correct_direction: bool, pc: u32, target_addr: u32);
}

// Dummp Predictor which always predicts branch not-taken
#[allow(unused)]
pub mod dummy {
    use super::*;

    pub struct Predictor;
    impl Predictor {
        pub fn new() -> Self {
            Self
        }
    }

    impl BranchPredict for Predictor {
        #[allow(unused)]
        fn branch_predict(&mut self, pc: u32) -> BranchPredictResult {
            BranchPredictResult {
                direction: false,
                addr: 0u32,
            }
        }
        #[allow(unused)]
        fn mispredict_recovery(&mut self, correct_direction: bool, pc: u32, target_addr: u32) {}
    }
}

// other predictor implementations
pub mod bimodal;
pub mod two_level_adaptive;
