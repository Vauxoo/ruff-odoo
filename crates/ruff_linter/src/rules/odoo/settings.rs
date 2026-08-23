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
    pub manifest_deprecated_keys: ManifestDeprecatedKeys,
    pub manifest_required_authors: ConfiguredList,
    pub license_allowed: ConfiguredList,
    pub manifest_required_keys: ConfiguredList,
    pub manifest_keys_values_true: ConfiguredList,
    pub development_status_allowed: ConfiguredList,
    pub attribute_deprecated: ConfiguredList,
    pub method_required_super: ConfiguredList,
    pub no_missing_return: ConfiguredList,
    pub cursor_expr: ConfiguredList,
    pub odoo_exceptions: ConfiguredList,
    pub external_request_timeout_methods: ConfiguredList,
    pub external_request_timeout_seconds: TimeoutSeconds,
    pub deprecated_field_parameters: ConfiguredList,
    pub deprecated_odoo_model_methods: ConfiguredList,
    pub no_search_all_models: ConfiguredList,
    pub readme_template_url: Option<String>,
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
                self.manifest_deprecated_keys,
                self.manifest_required_authors,
                self.license_allowed,
                self.manifest_required_keys,
                self.manifest_keys_values_true,
                self.development_status_allowed,
                self.attribute_deprecated,
                self.method_required_super,
                self.no_missing_return,
                self.cursor_expr,
                self.odoo_exceptions,
                self.external_request_timeout_methods,
                self.external_request_timeout_seconds,
                self.deprecated_field_parameters,
                self.deprecated_odoo_model_methods,
                self.no_search_all_models,
                self.readme_template_url | optional,
            ]
        }
        Ok(())
    }
}

/// The number of seconds the `external-request-timeout` (`ODE8106`) fix passes as
/// `timeout=`. A plain `u32` cannot carry the two-minute default through `derive(Default)`,
/// hence the newtype.
#[derive(Debug, Clone, Copy, CacheKey)]
pub struct TimeoutSeconds(pub u32);

impl Default for TimeoutSeconds {
    fn default() -> Self {
        Self(120)
    }
}

impl Display for TimeoutSeconds {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The manifest keys `manifest-deprecated-key` (`ODC8103`) reports.
#[derive(Debug, Clone, Default, CacheKey)]
pub enum ManifestDeprecatedKeys {
    /// The keys Odoo itself deprecated, each one scoped to the versions it is deprecated in.
    #[default]
    Default,
    /// The exact list configured through `manifest-deprecated-keys`, reported whatever the
    /// configured `odoo-version` is.
    UserProvided(Vec<String>),
}

impl Display for ManifestDeprecatedKeys {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ManifestDeprecatedKeys::Default => f.write_str("default"),
            ManifestDeprecatedKeys::UserProvided(keys) => {
                write!(f, "[{}]", keys.join(", "))
            }
        }
    }
}

/// A list of accepted values that a project can replace outright.
///
/// The fork ships the same list pylint-odoo defaults to, so a project that agrees with it
/// configures nothing. A project that does not — Vauxoo accepts its own authors and the
/// `OPL-1` license, for instance — names its own list, which replaces the built-in one rather
/// than adding to it, exactly as the corresponding pylint-odoo option does.
#[derive(Debug, Clone, Default, CacheKey)]
pub enum ConfiguredList {
    /// The list built into this fork.
    #[default]
    BuiltIn,
    /// The exact list configured for the project.
    UserProvided(Vec<String>),
}

impl ConfiguredList {
    /// Whether `value` is in the list, given the built-in list to fall back to.
    pub(crate) fn contains(&self, value: &str, built_in: &[&str]) -> bool {
        match self {
            ConfiguredList::BuiltIn => built_in.contains(&value),
            ConfiguredList::UserProvided(entries) => entries.iter().any(|entry| entry == value),
        }
    }

    /// Whether `value` matches any entry of the list, reading each entry as a glob.
    ///
    /// An entry with no wildcard is compared literally, so `sale.order` matches only that
    /// model; one with a wildcard is a pattern, so `account.move*` covers `account.move` and
    /// `account.move.line` alike. An entry that is not a valid pattern matches nothing rather
    /// than aborting the check.
    pub(crate) fn matches_glob(&self, value: &str, built_in: &[&str]) -> bool {
        fn matches(pattern: &str, value: &str) -> bool {
            if pattern.contains(['*', '?', '[']) {
                glob::Pattern::new(pattern).is_ok_and(|pattern| pattern.matches(value))
            } else {
                pattern == value
            }
        }
        match self {
            ConfiguredList::BuiltIn => built_in.iter().any(|pattern| matches(pattern, value)),
            ConfiguredList::UserProvided(entries) => {
                entries.iter().any(|pattern| matches(pattern, value))
            }
        }
    }

    /// Every entry of the list in effect.
    pub(crate) fn entries<'a>(
        &'a self,
        built_in: &'a [&'a str],
    ) -> Box<dyn Iterator<Item = std::borrow::Cow<'a, str>> + 'a> {
        match self {
            ConfiguredList::BuiltIn => Box::new(
                built_in
                    .iter()
                    .map(|entry| std::borrow::Cow::Borrowed(*entry)),
            ),
            ConfiguredList::UserProvided(entries) => Box::new(
                entries
                    .iter()
                    .map(|entry| std::borrow::Cow::Borrowed(entry.as_str())),
            ),
        }
    }

    /// The dotted path in the list that `segments` spells, if any.
    ///
    /// The built-in list is written as segments because that is what the semantic model hands
    /// back; a configured one is written the way a person would type it, `requests.get`.
    pub(crate) fn matching_path(&self, segments: &[&str], built_in: &[&[&str]]) -> Option<String> {
        match self {
            ConfiguredList::BuiltIn => built_in
                .iter()
                .find(|path| segments == **path)
                .map(|path| path.join(".")),
            ConfiguredList::UserProvided(entries) => {
                let dotted = segments.join(".");
                entries.iter().find(|entry| **entry == dotted).cloned()
            }
        }
    }

    /// The replacement for `name`, if the list renames it.
    ///
    /// Configured entries are written `old:new`, as pylint-odoo's
    /// `deprecated-field-parameters` takes them; one without a `:` renames nothing and is
    /// ignored rather than read as a parameter with an empty replacement.
    pub(crate) fn renamed(&self, name: &str, built_in: &[(&str, &str)]) -> Option<String> {
        match self {
            ConfiguredList::BuiltIn => built_in
                .iter()
                .find(|(old, _)| *old == name)
                .map(|(_, new)| (*new).to_string()),
            ConfiguredList::UserProvided(entries) => entries.iter().find_map(|entry| {
                entry
                    .split_once(':')
                    .filter(|(old, _)| *old == name)
                    .map(|(_, new)| new.to_string())
            }),
        }
    }

    /// The list in effect, rendered for a diagnostic message.
    pub(crate) fn joined(&self, built_in: &[&str]) -> String {
        match self {
            ConfiguredList::BuiltIn => built_in.join(", "),
            ConfiguredList::UserProvided(entries) => entries.join(", "),
        }
    }
}

impl Display for ConfiguredList {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ConfiguredList::BuiltIn => f.write_str("default"),
            ConfiguredList::UserProvided(entries) => write!(f, "[{}]", entries.join(", ")),
        }
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
