pub mod vass_reach;
pub mod vass_z_reach;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolverStatus<T = (), F = (), U = ()> {
    True(T),
    False(F),
    Unknown(U),
}

impl<U> From<bool> for SolverStatus<(), (), U> {
    fn from(b: bool) -> Self {
        if b {
            SolverStatus::True(())
        } else {
            SolverStatus::False(())
        }
    }
}

impl<T, F, U> SolverStatus<T, F, U> {
    pub fn is_success(&self) -> bool {
        matches!(self, SolverStatus::True(_))
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, SolverStatus::False(_))
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, SolverStatus::Unknown(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverResult<T = (), F = (), U = (), Statistics = ()> {
    pub status: SolverStatus<T, F, U>,
    pub statistics: Statistics,
}

impl<T, F, U, Statistics> SolverResult<T, F, U, Statistics> {
    pub fn new(status: SolverStatus<T, F, U>, statistics: Statistics) -> Self {
        Self { status, statistics }
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    pub fn is_failure(&self) -> bool {
        self.status.is_failure()
    }

    pub fn is_unknown(&self) -> bool {
        self.status.is_unknown()
    }
}
