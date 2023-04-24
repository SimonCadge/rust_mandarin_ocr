use std::fmt;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum SupportedLanguages {
    ChiTra,
    ChiSim,
}

impl fmt::Display for SupportedLanguages {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ChiTra => write!(f, "chi_tra"),
            Self::ChiSim => write!(f, "chi_sim"),
        }
    }
}