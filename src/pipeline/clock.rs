/// ### Public `Clocked` trait
///
/// It defines the common methods for the clock-triggered components.
pub trait Clocked {
    fn tick(&mut self);
}
