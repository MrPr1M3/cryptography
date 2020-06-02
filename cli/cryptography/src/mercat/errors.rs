use failure::{Error, Fail};

/// Represents an error in asset issuance transaction.
/// TODO: as we implement the methods, we will find more explicit types for
/// the error.
#[derive(Fail, Clone, Debug, Eq, PartialEq)]
pub enum AssetTXError {
    #[fail(display = "CHANGEME as needed")]
    ChangeMe,
}

/// Represents an error in confidential transaction.
/// TODO: as we implement the methods, we will find more explicit types for
/// the error.
#[derive(Fail, Clone, Debug, Eq, PartialEq)]
pub enum ConfidentialTXError {
    #[fail(display = "CHANGEME as needed")]
    ChangeMe,
}

pub type Result<T, E = Error> = sp_std::result::Result<T, E>;
