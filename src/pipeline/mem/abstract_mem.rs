use std::cell::{Cell, RefCell};
use std::rc::Weak;

/// define the abstract type of MemRequest
#[derive(Clone)]
pub struct MemoryLoadReq<T> {
    pub addr: u32,
    pub len: u32,
    pub buffer: Weak<RefCell<Box<[T]>>>,
}

#[derive(Clone)]
pub struct MemoryStoreReq<T> {
    pub addr: u32,
    pub len: u32,
    pub store_data: Box<[T]>,
    pub done: Weak<Cell<bool>>,
}

#[derive(Clone)]
pub enum MemoryReqType<T> {
    Load(MemoryLoadReq<T>),
    Store(MemoryStoreReq<T>),
}

impl<T> MemoryReqType<T> {
    pub fn get_addr(&self) -> u32 {
        match self {
            MemoryReqType::Load(x) => x.addr,
            MemoryReqType::Store(x) => x.addr,
        }
    }

    pub fn get_len(&self) -> u32 {
            match self {
                MemoryReqType::Load(x) => x.len,
                MemoryReqType::Store(x) => x.len,
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
    type Granularity;
    fn try_register_req(&mut self, req: MemoryReqType<Self::Granularity>) -> Result<(), ()>;
}
