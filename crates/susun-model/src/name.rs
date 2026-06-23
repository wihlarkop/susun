//! Newtype wrappers for project and service identifiers.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

macro_rules! string_newtype {
    ($(#[$meta:meta])* $vis:vis struct $name:ident;) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        #[cfg_attr(feature = "serde", serde(transparent))]
        $vis struct $name(String);

        impl $name {
            /// Creates a new instance.
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Returns the inner string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self::new(s)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
    };
}

string_newtype! {
    /// Name of the Compose project (derived from `name:` field or directory).
    pub struct ProjectName;
}

string_newtype! {
    /// Key identifying a service within a project (e.g. `web`, `db`).
    pub struct ServiceName;
}

string_newtype! {
    /// Opaque image reference (e.g. `nginx:1.25`, `myapp:latest`).
    pub struct ImageRef;
}
