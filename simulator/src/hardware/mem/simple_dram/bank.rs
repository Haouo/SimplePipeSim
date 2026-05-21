//! Single DRAM bank as a cycle-counting state machine.
//!
//! A bank moves through five visible states. The "countdown" variants
//! (`Activating`, `Precharging`, `Reading`, `Writing`) decrement once
//! per [`Bank::advance_one_cycle`] call; when `remaining` reaches 1 and
//! is then advanced, the bank snaps to the next stable state and emits
//! a [`BankEvent`] so the controller above can react.
//!
//! ```text
//!                ACT(row, tRCD)               READ/WRITE(row, tCL)
//!   Idle ─────────────────────▶  Activating ─────────▶  Active(row) ─────────▶  Reading/Writing(row)
//!    ▲                            (countdown)                                   │  (countdown)
//!    │                                                                          │
//!    │            PRE(tRP)                                                      │  on remaining==1
//!    └─────────  Precharging  ◀──────────────────────  Active(row) ◀────────────┘  (data ready)
//!                (countdown)
//! ```

/// State of a single DRAM bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankState {
    /// No row is open; bank is precharged and ready for an ACT.
    Idle,
    /// ACT in progress. `remaining` cycles until the row buffer is
    /// usable.
    Activating { row: u32, remaining: usize },
    /// `row` is in the row buffer; ready for CAS or PRE.
    Active(u32),
    /// PRE in progress. `remaining` cycles until the bank returns to
    /// [`BankState::Idle`].
    Precharging { remaining: usize },
    /// CAS-read in progress. Snaps back to `Active(row)` and emits
    /// [`BankEvent::ReadFinished`] when the countdown elapses.
    Reading { row: u32, remaining: usize },
    /// CAS-write in progress. Snaps back to `Active(row)` and emits
    /// [`BankEvent::WriteFinished`] when the countdown elapses.
    Writing { row: u32, remaining: usize },
}

/// Edge events emitted by [`Bank::advance_one_cycle`] whenever a
/// countdown state finishes. The controller uses `ReadFinished` /
/// `WriteFinished` to know when to copy data into the requester's
/// buffer and set `done`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankEvent {
    ActivationFinished,
    PrechargeFinished,
    ReadFinished,
    WriteFinished,
}

/// A single bank. Holds its current state and exposes command
/// issuance + one-cycle advancement as separate operations so the
/// controller can perform its "decide-then-advance" cycle ordering.
pub struct Bank {
    state: BankState,
}

impl Bank {
    pub fn new() -> Self {
        Self {
            state: BankState::Idle,
        }
    }

    pub fn state(&self) -> BankState {
        self.state
    }

    /// ACT — open `row` into the row buffer. Only legal from
    /// [`BankState::Idle`]. `t_rcd` must be >= 1.
    pub fn issue_activate(&mut self, row: u32, t_rcd: usize) {
        assert!(
            matches!(self.state, BankState::Idle),
            "ACT issued in non-Idle state {:?}",
            self.state
        );
        assert!(t_rcd >= 1, "t_rcd must be >= 1");
        self.state = BankState::Activating {
            row,
            remaining: t_rcd,
        };
    }

    /// PRE — close the currently open row. Only legal from
    /// [`BankState::Active`]. `t_rp` must be >= 1.
    pub fn issue_precharge(&mut self, t_rp: usize) {
        assert!(
            matches!(self.state, BankState::Active(_)),
            "PRE issued in non-Active state {:?}",
            self.state
        );
        assert!(t_rp >= 1, "t_rp must be >= 1");
        self.state = BankState::Precharging { remaining: t_rp };
    }

    /// READ — CAS-read against the open row. Only legal from
    /// [`BankState::Active`] and the row must match. `t_cl` must be
    /// >= 1.
    pub fn issue_read(&mut self, row: u32, t_cl: usize) {
        match self.state {
            BankState::Active(open) => assert_eq!(
                open, row,
                "READ targets row {} but bank has row {} open",
                row, open
            ),
            _ => panic!("READ issued in non-Active state {:?}", self.state),
        }
        assert!(t_cl >= 1, "t_cl must be >= 1");
        self.state = BankState::Reading {
            row,
            remaining: t_cl,
        };
    }

    /// WRITE — CAS-write against the open row. Same legality rules as
    /// [`Bank::issue_read`].
    pub fn issue_write(&mut self, row: u32, t_cl: usize) {
        match self.state {
            BankState::Active(open) => assert_eq!(
                open, row,
                "WRITE targets row {} but bank has row {} open",
                row, open
            ),
            _ => panic!("WRITE issued in non-Active state {:?}", self.state),
        }
        assert!(t_cl >= 1, "t_cl must be >= 1");
        self.state = BankState::Writing {
            row,
            remaining: t_cl,
        };
    }

    /// Advance the bank by one core clock cycle. Returns `Some(event)`
    /// on the cycle a countdown state finishes; otherwise `None`.
    pub fn advance_one_cycle(&mut self) -> Option<BankEvent> {
        let (next, event) = match self.state {
            // Stable states: no progression without a fresh command.
            BankState::Idle | BankState::Active(_) => (self.state, None),
            // Countdown states. `remaining == 1` is the *trigger* cycle:
            // the state has spent its full latency budget and snaps
            // forward.
            BankState::Activating { row, remaining: 1 } => {
                (BankState::Active(row), Some(BankEvent::ActivationFinished))
            }
            BankState::Activating { row, remaining } => (
                BankState::Activating {
                    row,
                    remaining: remaining - 1,
                },
                None,
            ),
            BankState::Precharging { remaining: 1 } => {
                (BankState::Idle, Some(BankEvent::PrechargeFinished))
            }
            BankState::Precharging { remaining } => (
                BankState::Precharging {
                    remaining: remaining - 1,
                },
                None,
            ),
            BankState::Reading { row, remaining: 1 } => {
                (BankState::Active(row), Some(BankEvent::ReadFinished))
            }
            BankState::Reading { row, remaining } => (
                BankState::Reading {
                    row,
                    remaining: remaining - 1,
                },
                None,
            ),
            BankState::Writing { row, remaining: 1 } => {
                (BankState::Active(row), Some(BankEvent::WriteFinished))
            }
            BankState::Writing { row, remaining } => (
                BankState::Writing {
                    row,
                    remaining: remaining - 1,
                },
                None,
            ),
        };
        self.state = next;
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_bank_starts_idle() {
        let b = Bank::new();
        assert_eq!(b.state(), BankState::Idle);
    }

    #[test]
    fn activate_takes_exactly_t_rcd_cycles() {
        let mut b = Bank::new();
        b.issue_activate(7, 4);
        assert!(matches!(b.advance_one_cycle(), None));
        assert!(matches!(b.advance_one_cycle(), None));
        assert!(matches!(b.advance_one_cycle(), None));
        assert!(matches!(
            b.advance_one_cycle(),
            Some(BankEvent::ActivationFinished)
        ));
        assert_eq!(b.state(), BankState::Active(7));
    }

    #[test]
    fn read_emits_event_and_returns_to_active() {
        let mut b = Bank::new();
        b.issue_activate(3, 1);
        b.advance_one_cycle();
        assert_eq!(b.state(), BankState::Active(3));
        b.issue_read(3, 2);
        assert!(matches!(b.advance_one_cycle(), None));
        assert!(matches!(b.advance_one_cycle(), Some(BankEvent::ReadFinished)));
        assert_eq!(b.state(), BankState::Active(3));
    }

    #[test]
    fn write_emits_event_and_returns_to_active() {
        let mut b = Bank::new();
        b.issue_activate(5, 1);
        b.advance_one_cycle();
        b.issue_write(5, 3);
        assert!(matches!(b.advance_one_cycle(), None));
        assert!(matches!(b.advance_one_cycle(), None));
        assert!(matches!(
            b.advance_one_cycle(),
            Some(BankEvent::WriteFinished)
        ));
        assert_eq!(b.state(), BankState::Active(5));
    }

    #[test]
    fn precharge_returns_bank_to_idle() {
        let mut b = Bank::new();
        b.issue_activate(2, 1);
        b.advance_one_cycle();
        b.issue_precharge(2);
        assert!(matches!(b.advance_one_cycle(), None));
        assert!(matches!(
            b.advance_one_cycle(),
            Some(BankEvent::PrechargeFinished)
        ));
        assert_eq!(b.state(), BankState::Idle);
    }

    #[test]
    #[should_panic(expected = "ACT issued in non-Idle state")]
    fn cannot_activate_an_already_active_bank() {
        let mut b = Bank::new();
        b.issue_activate(1, 1);
        b.advance_one_cycle();
        b.issue_activate(2, 1); // panics
    }

    #[test]
    #[should_panic(expected = "READ targets row 9 but bank has row 1 open")]
    fn read_must_match_open_row() {
        let mut b = Bank::new();
        b.issue_activate(1, 1);
        b.advance_one_cycle();
        b.issue_read(9, 1); // panics
    }
}
