//! Versioned deterministic configuration fingerprints.

pub mod digest;
pub mod input;
pub mod schema;

pub use digest::{compute_configuration_fingerprint, parse_configuration_fingerprint};
pub use input::{
    CanonicalFingerprintInput, FingerprintInput, ResolvedImageIdentity, ResolvedResourceNames,
    RuntimeDefaults,
};
pub use schema::{FingerprintDigest, FingerprintVersion, VersionedFingerprint};
