/// public trait AbstractDevice
///
/// defindes the common interfaces for the devices on the bus
pub trait AbstractDevice {
    /// abstract load method to get data from the device
    fn load(&self, addr: u32, len: usize) -> Result<Vec<u8>, &'static str>;
    /// abstract store method to store data into the device
    fn store(&mut self, addr: u32, len: usize, data: &[u8]) -> Result<(), &'static str>;
    /// simulate the clock in real system to trigger the operation of the device
    fn tick(&mut self, rtc_ticks: usize);
}
