use std::cell::RefCell;
use std::rc::Rc;

use crate::hardware::mem::abstract_mem::{AbstractMemoryInterface, MemoryReqType};

pub(super) struct LowerMemoryTransaction {
    state: LowerMemoryState,
}

enum LowerMemoryState {
    Sending(MemoryReqType),
    Waiting(MemoryReqType),
}

impl LowerMemoryTransaction {
    pub(super) fn new(req: MemoryReqType) -> Self {
        Self {
            state: LowerMemoryState::Sending(req),
        }
    }

    pub(super) fn progress<M>(&mut self, mem_ref: &Rc<RefCell<M>>) -> bool
    where
        M: AbstractMemoryInterface,
    {
        match &self.state {
            LowerMemoryState::Sending(req) => {
                if mem_ref.borrow_mut().try_register_req(req).is_ok() {
                    self.state = LowerMemoryState::Waiting(req.clone());
                }
                false
            }
            LowerMemoryState::Waiting(req) => req.is_done(),
        }
    }

    pub(super) fn load_data(&self) -> Box<[u8]> {
        self.req().load_data()
    }

    fn req(&self) -> &MemoryReqType {
        match &self.state {
            LowerMemoryState::Sending(req) | LowerMemoryState::Waiting(req) => req,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::clock::Clocked;

    struct ImmediateMemory;

    impl Clocked for ImmediateMemory {
        fn tick(&mut self) {}
    }

    impl AbstractMemoryInterface for ImmediateMemory {
        fn try_register_req(&mut self, req: &MemoryReqType) -> Result<(), ()> {
            req.complete_store();
            Ok(())
        }
    }

    #[test]
    fn accepted_request_completes_on_the_following_progress_check() {
        let memory = Rc::new(RefCell::new(ImmediateMemory));
        let mut transaction = LowerMemoryTransaction::new(MemoryReqType::store(0x40, vec![1]));

        assert!(!transaction.progress(&memory));
        assert!(transaction.progress(&memory));
    }
}
