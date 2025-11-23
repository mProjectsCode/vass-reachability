use serde::{Deserialize, Serialize};

pub mod lsg_reach;
mod utils;
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

    pub fn unwrap_success(self) -> T {
        match self {
            SolverStatus::True(t) => t,
            _ => panic!("Called unwrap_success on a non-successful SolverStatus"),
        }
    }

    pub fn unwrap_failure(self) -> F {
        match self {
            SolverStatus::False(f) => f,
            _ => panic!("Called unwrap_failure on a non-failure SolverStatus"),
        }
    }

    pub fn unwrap_unknown(self) -> U {
        match self {
            SolverStatus::Unknown(u) => u,
            _ => panic!("Called unwrap_unknown on a non-unknown SolverStatus"),
        }
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

    pub fn unwrap_success(self) -> T {
        self.status.unwrap_success()
    }

    pub fn unwrap_failure(self) -> F {
        self.status.unwrap_failure()
    }

    pub fn unwrap_unknown(self) -> U {
        self.status.unwrap_unknown()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerializableSolverStatus {
    True,
    False,
    Unknown,
}

impl From<bool> for SerializableSolverStatus {
    fn from(b: bool) -> Self {
        if b {
            SerializableSolverStatus::True
        } else {
            SerializableSolverStatus::False
        }
    }
}

impl<T, F, U> From<SolverStatus<T, F, U>> for SerializableSolverStatus {
    fn from(status: SolverStatus<T, F, U>) -> Self {
        match status {
            SolverStatus::True(_) => SerializableSolverStatus::True,
            SolverStatus::False(_) => SerializableSolverStatus::False,
            SolverStatus::Unknown(_) => SerializableSolverStatus::Unknown,
        }
    }
}

impl SerializableSolverStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, SerializableSolverStatus::True)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, SerializableSolverStatus::False)
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, SerializableSolverStatus::Unknown)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializableSolverResult<Statistics = ()> {
    pub status: SerializableSolverStatus,
    pub statistics: Statistics,
}

impl<Statistics> SerializableSolverResult<Statistics> {
    pub fn new(status: SerializableSolverStatus, statistics: Statistics) -> Self {
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

    pub fn to_empty_status(self) -> SerializableSolverResult<()> {
        SerializableSolverResult {
            status: self.status,
            statistics: (),
        }
    }
}

impl<T, F, U, Statistics> From<SolverResult<T, F, U, Statistics>>
    for SerializableSolverResult<Statistics>
{
    fn from(result: SolverResult<T, F, U, Statistics>) -> Self {
        SerializableSolverResult {
            status: result.status.into(),
            statistics: result.statistics,
        }
    }
}
