use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// define the abstract type of MemRequest
#[derive(Clone)]
pub struct MemoryLoadReq {
    pub addr: u32,
    pub len: usize,
    pub done: Rc<Cell<bool>>,
    pub buffer: Rc<RefCell<Box<[u8]>>>,
}

#[derive(Clone)]
pub struct MemoryStoreReq {
    pub addr: u32,
    pub len: usize,
    pub store_data: Box<[u8]>,
    pub done: Rc<Cell<bool>>,
}

#[derive(Clone)]
pub enum MemoryReqType {
    Load(MemoryLoadReq),
    Store(MemoryStoreReq),
}

impl MemoryReqType {
    pub fn get_addr(&self) -> u32 {
        match self {
            MemoryReqType::Load(x) => x.addr,
            MemoryReqType::Store(x) => x.addr,
        }
    }

    pub fn get_len(&self) -> usize {
        match self {
            MemoryReqType::Load(x) => x.len,
            MemoryReqType::Store(x) => x.len,
        }
    }

    pub fn get_load_req_ref(&self) -> &MemoryLoadReq {
        if let MemoryReqType::Load(ref load_req) = self {
            load_req
        } else {
            panic!("Try to unwrap a MemoryStoreReq with get_load_req(), which makes no sense.");
        }
    }

    pub fn get_store_req_ref(&self) -> &MemoryStoreReq {
        if let MemoryReqType::Store(ref store_req) = self {
            &store_req
        } else {
            panic!("Try to unwrap a MemoryLoadReq with get_store_req(), which makes no sense.");
        }
    }

    pub fn get_done(&self) -> bool {
        match self {
            MemoryReqType::Load(load_req) => load_req.done.get(),
            MemoryReqType::Store(store_req) => store_req.done.get(),
        }
    }
}

/// ### AbstractMemInterface
///
/// the trait abstracts the common operations among different types of memory devices,
/// like L1-Cache, L2-Cache and Main Memory (DRAM)
///
/// the generic parameter `T` represents the granularity for manipulation the memory device
pub trait AbstraceMemInterface {
    fn try_register_req(&mut self, req: &MemoryReqType) -> Result<(), ()>;
}
