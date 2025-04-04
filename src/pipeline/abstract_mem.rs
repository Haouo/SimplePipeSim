/// ### AbstractMemInterface
///
/// the trait abstracts the common operations among different types of memory devices,
/// like L1-Cache, L2-Cache and Main Memory (DRAM)
///
/// the generic parameter `T` represents the granularity for manipulation the memory device
pub trait AbstraceMemInterface<T> {
    /// ### load function of trait AbstractMemInterface
    ///
    /// Generic load function which takes (addr, len) as arguments
    /// and returns uncertain length of byte vector.
    fn load(&self, addr: u32, len: u32) -> Result<Vec<T>, &'static str>;
    /// ### store function of trait AbstractMemInterface
    ///
    /// Generic store function which takes (addr, len, data) as arguments
    /// and returns the result whether the action is success.
    fn store(&mut self, addr: u32, len: u32, data: &[T]) -> Result<(), &'static str>;
    /// ### tick function of trait AbstractMemInterface
    ///
    /// imitate the clock trigger of real memory circuit
    fn tick() {
        unimplemented!();
    }
}
