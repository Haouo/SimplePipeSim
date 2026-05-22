use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::super::clock::Clocked;

/// define the abstract type of MemRequest
#[derive(Clone)]
pub struct MemoryLoadReq {
    addr: u32,
    len: usize,
    done: Rc<Cell<bool>>,
    buffer: Rc<RefCell<Box<[u8]>>>,
}

#[derive(Clone)]
pub struct MemoryStoreReq {
    addr: u32,
    len: usize,
    store_data: Box<[u8]>,
    done: Rc<Cell<bool>>,
}

#[derive(Clone)]
pub enum MemoryReqType {
    Load(MemoryLoadReq),
    Store(MemoryStoreReq),
}

impl MemoryLoadReq {
    fn new(addr: u32, len: usize) -> Self {
        Self {
            addr,
            len,
            done: Rc::new(Cell::new(false)),
            buffer: Rc::new(RefCell::new(vec![0u8; len].into_boxed_slice())),
        }
    }

    pub fn complete_from_slice(&self, data: &[u8]) {
        assert_eq!(
            data.len(),
            self.len,
            "Memory load completion has {} bytes for a {}-byte request",
            data.len(),
            self.len
        );
        self.buffer.borrow_mut().clone_from_slice(data);
        self.done.set(true);
    }

    pub fn data(&self) -> Box<[u8]> {
        self.buffer.borrow().clone()
    }

    fn is_done(&self) -> bool {
        self.done.get()
    }
}

impl MemoryStoreReq {
    fn new<D>(addr: u32, store_data: D) -> Self
    where
        D: Into<Box<[u8]>>,
    {
        let store_data = store_data.into();
        Self {
            addr,
            len: store_data.len(),
            store_data,
            done: Rc::new(Cell::new(false)),
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.store_data
    }

    pub fn complete(&self) {
        self.done.set(true);
    }

    fn is_done(&self) -> bool {
        self.done.get()
    }
}

impl MemoryReqType {
    pub fn load(addr: u32, len: usize) -> Self {
        Self::Load(MemoryLoadReq::new(addr, len))
    }

    pub fn store<D>(addr: u32, store_data: D) -> Self
    where
        D: Into<Box<[u8]>>,
    {
        Self::Store(MemoryStoreReq::new(addr, store_data))
    }

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

    fn load_req_ref(&self) -> &MemoryLoadReq {
        if let MemoryReqType::Load(ref load_req) = self {
            load_req
        } else {
            panic!("Try to unwrap a MemoryStoreReq with get_load_req(), which makes no sense.");
        }
    }

    fn store_req_ref(&self) -> &MemoryStoreReq {
        if let MemoryReqType::Store(ref store_req) = self {
            &store_req
        } else {
            panic!("Try to unwrap a MemoryLoadReq with get_store_req(), which makes no sense.");
        }
    }

    pub fn is_done(&self) -> bool {
        match self {
            MemoryReqType::Load(load_req) => load_req.is_done(),
            MemoryReqType::Store(store_req) => store_req.is_done(),
        }
    }

    pub fn load_data(&self) -> Box<[u8]> {
        self.load_req_ref().data()
    }

    pub fn store_data(&self) -> &[u8] {
        self.store_req_ref().data()
    }

    pub fn complete_load_from_slice(&self, data: &[u8]) {
        self.load_req_ref().complete_from_slice(data);
    }

    pub fn complete_store(&self) {
        self.store_req_ref().complete();
    }
}

/// ### AbstractMemInterface
///
/// the trait abstracts the common operations among different types of memory devices,
/// like L1-Cache, L2-Cache and Main Memory (DRAM)
///
/// the generic parameter `T` represents the granularity for manipulation the memory device
pub trait AbstractMemoryInterface: Clocked {
    fn try_register_req(&mut self, req: &MemoryReqType) -> Result<(), ()>;
}
