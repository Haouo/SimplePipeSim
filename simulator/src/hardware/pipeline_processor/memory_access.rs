use crate::hardware::mem::abstract_mem::{AbstractMemoryInterface, MemoryReqType};
use crate::hardware::uop::MemoryAccess;

#[derive(Default)]
pub(super) struct MemoryTransaction {
    state: MemoryTransactionState,
}

#[derive(Default)]
enum MemoryTransactionState {
    #[default]
    Idle,
    Sending {
        req: MemoryReqType,
        access: MemoryAccess,
    },
    Waiting {
        req: MemoryReqType,
        access: MemoryAccess,
    },
    Complete {
        load_value: Option<u32>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MemoryTransactionStatus {
    Pending,
    Complete { load_value: Option<u32> },
}

impl MemoryTransaction {
    pub(super) fn progress<M>(
        &mut self,
        memory: &mut M,
        access: MemoryAccess,
    ) -> MemoryTransactionStatus
    where
        M: AbstractMemoryInterface,
    {
        match &self.state {
            MemoryTransactionState::Idle => {
                let req = request_for(access);
                self.send(memory, req, access)
            }
            MemoryTransactionState::Sending { req, access } => {
                self.send(memory, req.clone(), *access)
            }
            MemoryTransactionState::Waiting { req, access } => {
                if req.is_done() {
                    self.complete(req.clone(), *access)
                } else {
                    MemoryTransactionStatus::Pending
                }
            }
            MemoryTransactionState::Complete { load_value } => MemoryTransactionStatus::Complete {
                load_value: *load_value,
            },
        }
    }

    pub(super) fn reset(&mut self) {
        self.state = MemoryTransactionState::Idle;
    }

    fn send<M>(
        &mut self,
        memory: &mut M,
        req: MemoryReqType,
        access: MemoryAccess,
    ) -> MemoryTransactionStatus
    where
        M: AbstractMemoryInterface,
    {
        if memory.try_register_req(&req).is_err() {
            self.state = MemoryTransactionState::Sending { req, access };
            return MemoryTransactionStatus::Pending;
        }

        if req.is_done() {
            self.complete(req, access)
        } else {
            self.state = MemoryTransactionState::Waiting { req, access };
            MemoryTransactionStatus::Pending
        }
    }

    fn complete(&mut self, req: MemoryReqType, access: MemoryAccess) -> MemoryTransactionStatus {
        let load_value = access
            .is_load()
            .then(|| access.loaded_value(req.load_data().as_ref()));
        self.state = MemoryTransactionState::Complete { load_value };
        MemoryTransactionStatus::Complete { load_value }
    }
}

fn request_for(access: MemoryAccess) -> MemoryReqType {
    if let Some(store_value) = access.store_value() {
        MemoryReqType::store(
            access.addr(),
            store_value.to_le_bytes()[..access.len()].to_vec(),
        )
    } else {
        MemoryReqType::load(access.addr(), access.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::clock::Clocked;
    use crate::hardware::uop::{LoadExtension, MemoryAccessWidth};

    #[derive(Default)]
    struct RetryMemory {
        first_register_fails: bool,
        pending: Option<MemoryReqType>,
    }

    impl Clocked for RetryMemory {
        fn tick(&mut self) {
            if let Some(req) = self.pending.take() {
                req.complete_load_from_slice(&[0x80]);
            }
        }
    }

    impl AbstractMemoryInterface for RetryMemory {
        fn try_register_req(&mut self, req: &MemoryReqType) -> Result<(), ()> {
            if self.first_register_fails {
                self.first_register_fails = false;
                return Err(());
            }

            self.pending = Some(req.clone());
            Ok(())
        }
    }

    #[test]
    fn load_transaction_retries_and_materializes_result() {
        let access = MemoryAccess::Load {
            addr: 0x10,
            width: MemoryAccessWidth::Byte,
            extension: LoadExtension::Sign,
        };
        let mut memory = RetryMemory {
            first_register_fails: true,
            pending: None,
        };
        let mut transaction = MemoryTransaction::default();

        assert_eq!(
            transaction.progress(&mut memory, access),
            MemoryTransactionStatus::Pending
        );
        assert_eq!(
            transaction.progress(&mut memory, access),
            MemoryTransactionStatus::Pending
        );

        memory.tick();

        assert_eq!(
            transaction.progress(&mut memory, access),
            MemoryTransactionStatus::Complete {
                load_value: Some(0xffff_ff80)
            }
        );
    }
}
