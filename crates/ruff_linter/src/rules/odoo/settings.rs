//! Settings for the `odoo` plugin.

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use crate::display_settings;
use ruff_macros::CacheKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, CacheKey)]
pub struct Settings {
    pub prohibited_override_methods: Vec<String>,
    pub odoo_version: Option<OdooVersion>,
    pub category_allowed: Vec<String>,
    pub odoo_required_files: Vec<String>,
}

impl Display for Settings {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        display_settings! {
            formatter = f,
            namespace = "linter.odoo",
            fields = [
                self.prohibited_override_methods | array,
                self.odoo_version | optional,
                self.category_allowed | array,
                self.odoo_required_files | array,
            ]
        }
        Ok(())
    }
}

/// The Odoo version being targeted (e.g. `17.0`).
///
/// Used to gate rules that only apply to a specific range of Odoo versions, mirroring
/// pylint-odoo's `checks_maxmin_odoo_version` (e.g. `self._cr` was only deprecated in 19.0,
/// so `deprecated-self-cr` shouldn't fire against a module targeting an older version).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, CacheKey)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub struct OdooVersion {
    pub major: u16,
    pub minor: u16,
}

impl OdooVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

impl Display for OdooVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Error type returned when parsing an [`OdooVersion`] from a string fails.
#[derive(Debug, Clone)]
pub struct OdooVersionParseError(String);

impl Display for OdooVersionParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a valid Odoo version; expected a version like `17.0`",
            self.0
        )
    }
}

impl std::error::Error for OdooVersionParseError {}

impl FromStr for OdooVersion {
    type Err = OdooVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let major = parts
            .next()
            .filter(|part| !part.is_empty())
            .and_then(|part| part.parse().ok())
            .ok_or_else(|| OdooVersionParseError(s.to_string()))?;
        let minor = match parts.next() {
            Some(part) => part
                .parse()
                .map_err(|_| OdooVersionParseError(s.to_string()))?,
            None => 0,
        };
        if parts.next().is_some() {
            return Err(OdooVersionParseError(s.to_string()));
        }
        Ok(Self { major, minor })
    }
}

impl TryFrom<String> for OdooVersion {
    type Error = OdooVersionParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<OdooVersion> for String {
    fn from(value: OdooVersion) -> Self {
        value.to_string()
    }
}
