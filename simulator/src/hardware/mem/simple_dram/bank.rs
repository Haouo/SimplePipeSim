/// State of a single DRAM bank
pub enum BankState {
    /// No row is open; bank is precharged (idle)
    Idle,
    /// A row is loaded in the row buffer
    Active(u32),
}

pub struct Bank {
    pub state: BankState,
}

impl Bank {
    pub fn new() -> Self {
        Bank {
            state: BankState::Idle,
        }
    }

    pub fn active_row(&self) -> Option<u32> {
        if let BankState::Active(row) = self.state {
            Some(row)
        } else {
            None
        }
    }

    /// ACT: load a row into the row buffer
    pub fn activate(&mut self, row: u32) {
        self.state = BankState::Active(row);
    }

    /// PRE: close the row buffer and restore the row to the array
    pub fn precharge(&mut self) {
        self.state = BankState::Idle;
    }
}
