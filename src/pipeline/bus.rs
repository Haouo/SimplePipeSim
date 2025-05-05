use crate::pipeline::clock::Clocked;

pub trait BusInterface {
    fn req_send();
    fn req_back();
}

pub struct ZeroDelayedBus {
    devices: Box<dyn Clocked>,
}

impl ZeroDelayedBus {
    //
}
