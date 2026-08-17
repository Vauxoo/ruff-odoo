//! Rules from [odoo](https://pypi.org/project/pylint-odoo/).
pub(crate) mod helpers;
pub(crate) mod rules;
pub mod settings;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Result;
    use test_case::test_case;

    use crate::assert_diagnostics;
    use crate::registry::Rule;
    use crate::settings::LinterSettings;
    use crate::test::test_path;

    #[test_case(
        Rule::ManifestRequiredKey,
        Path::new("manifest_required_key/__manifest__.py")
    )]
    #[test_case(
        Rule::ManifestDeprecatedKey,
        Path::new("manifest_deprecated_key/__manifest__.py")
    )]
    #[test_case(Rule::UseVimComment, Path::new("use_vim_comment.py"))]
    #[test_case(Rule::ExceptPass, Path::new("except_pass.py"))]
    #[test_case(Rule::MethodRequiredSuper, Path::new("method_required_super.py"))]
    #[test_case(Rule::UnusedLogger, Path::new("unused_logger_0.py"))]
    #[test_case(Rule::UnusedLogger, Path::new("unused_logger_1.py"))]
    #[test_case(
        Rule::AttributeStringRedundant,
        Path::new("attribute_string_redundant.py")
    )]
    #[test_case(
        Rule::ManifestRequiredAuthor,
        Path::new("manifest_required_author/__manifest__.py")
    )]
    #[test_case(
        Rule::ManifestAuthorString,
        Path::new("manifest_author_string/__manifest__.py")
    )]
    #[test_case(Rule::LicenseAllowed, Path::new("license_allowed/__manifest__.py"))]
    #[test_case(
        Rule::ManifestMaintainersList,
        Path::new("manifest_maintainers_list/__manifest__.py")
    )]
    #[test_case(
        Rule::ManifestSummaryMultiline,
        Path::new("manifest_summary_multiline/__manifest__.py")
    )]
    #[test_case(
        Rule::DevelopmentStatusAllowed,
        Path::new("development_status_allowed/__manifest__.py")
    )]
    #[test_case(
        Rule::WebsiteManifestKeyNotValidUri,
        Path::new("website_manifest_key_not_valid_uri/__manifest__.py")
    )]
    #[test_case(Rule::InvalidEmail, Path::new("invalid_email/__manifest__.py"))]
    #[test_case(
        Rule::MissingReadme,
        Path::new("missing_readme/missing/__manifest__.py")
    )]
    #[test_case(
        Rule::MissingReadme,
        Path::new("missing_readme/present/__manifest__.py")
    )]
    #[test_case(Rule::InvalidCommit, Path::new("invalid_commit.py"))]
    #[test_case(Rule::ContextOverridden, Path::new("context_overridden.py"))]
    #[test_case(Rule::BadBuiltinGroupby, Path::new("bad_builtin_groupby.py"))]
    #[test_case(Rule::OdooExceptionWarning, Path::new("odoo_exception_warning.py"))]
    #[test_case(Rule::AttributeDeprecated, Path::new("attribute_deprecated.py"))]
    #[test_case(Rule::MissingReturn, Path::new("missing_return.py"))]
    #[test_case(
        Rule::OdooAddonsRelativeImport,
        Path::new("odoo_addons_relative_import/my_module/models/foo.py")
    )]
    #[test_case(
        Rule::OdooAddonsRelativeImport,
        Path::new("odoo_addons_relative_import/my_module/tests/test_foo.py")
    )]
    #[test_case(
        Rule::OdooAddonsRelativeImport,
        Path::new("odoo_addons_relative_import/my_module/migrations/16.0.1.0/pre-migrate.py")
    )]
    #[test_case(Rule::PreferEnvTranslation, Path::new("prefer_env_translation.py"))]
    #[test_case(
        Rule::ManifestSuperfluousKey,
        Path::new("manifest_superfluous_key/__manifest__.py")
    )]
    #[test_case(Rule::HeaderComments, Path::new("header_comments.py"))]
    #[test_case(Rule::HeaderComments, Path::new("header_comments_comments_only.py"))]
    #[test_case(Rule::MethodCompute, Path::new("method_compute.py"))]
    #[test_case(Rule::MethodSearch, Path::new("method_search.py"))]
    #[test_case(Rule::MethodInverse, Path::new("method_inverse.py"))]
    #[test_case(Rule::RenamedFieldParameter, Path::new("renamed_field_parameter.py"))]
    #[test_case(Rule::TranslationField, Path::new("translation_field.py"))]
    #[test_case(
        Rule::InheritableMethodString,
        Path::new("inheritable_method_string.py")
    )]
    #[test_case(
        Rule::InheritableMethodLambda,
        Path::new("inheritable_method_lambda.py")
    )]
    #[test_case(Rule::DeprecatedNameGet, Path::new("deprecated_name_get.py"))]
    #[test_case(Rule::SuperMethodMismatch, Path::new("super_method_mismatch.py"))]
    #[test_case(
        Rule::DeprecatedOdooModelMethod,
        Path::new("deprecated_odoo_model_method.py")
    )]
    #[test_case(Rule::NoSearchAll, Path::new("no_search_all.py"))]
    #[test_case(Rule::NoRaiseUnlink, Path::new("no_raise_unlink.py"))]
    #[test_case(Rule::NoWriteInCompute, Path::new("no_write_in_compute.py"))]
    #[test_case(
        Rule::TranslationContainsVariable,
        Path::new("translation_contains_variable.py")
    )]
    #[test_case(
        Rule::TranslationPositionalUsed,
        Path::new("translation_positional_used.py")
    )]
    #[test_case(Rule::TranslationInjection, Path::new("translation_injection.py"))]
    #[test_case(
        Rule::DeprecatedInselectOperator,
        Path::new("deprecated_inselect_operator.py")
    )]
    #[test_case(
        Rule::TestFolderImported,
        Path::new("test_folder_imported/__init__.py")
    )]
    #[test_case(
        Rule::ManifestExternalAssets,
        Path::new("manifest_external_assets/__manifest__.py")
    )]
    #[test_case(Rule::PylintDisableComment, Path::new("pylint_disable_comment.py"))]
    #[test_case(
        Rule::ManifestDataDuplicated,
        Path::new("manifest_data_duplicated/__manifest__.py")
    )]
    #[test_case(Rule::TranslationRequired, Path::new("translation_required.py"))]
    #[test_case(
        Rule::TranslationRequired,
        Path::new("translation_required/tests/test_translation.py")
    )]
    #[test_case(
        Rule::NoWizardInModels,
        Path::new("no_wizard_in_models/models/sale_import.py")
    )]
    #[test_case(
        Rule::NoWizardInModels,
        Path::new("no_wizard_in_models/wizards/sale_import.py")
    )]
    #[test_case(Rule::ExternalRequestTimeout, Path::new("external_request_timeout.py"))]
    #[test_case(Rule::SqlInjection, Path::new("sql_injection.py"))]
    #[test_case(
        Rule::ResourceNotExist,
        Path::new("resource_not_exist/__manifest__.py")
    )]
    #[test_case(
        Rule::ManifestBehindMigrations,
        Path::new("manifest_behind_migrations/behind/__manifest__.py")
    )]
    #[test_case(
        Rule::ManifestBehindMigrations,
        Path::new("manifest_behind_migrations/ok/__manifest__.py")
    )]
    #[test_case(
        Rule::TranslationFormatInterpolation,
        Path::new("translation_format_interpolation.py")
    )]
    #[test_case(
        Rule::TranslationFormatTruncated,
        Path::new("translation_format_truncated.py")
    )]
    #[test_case(
        Rule::TranslationFstringInterpolation,
        Path::new("translation_fstring_interpolation.py")
    )]
    #[test_case(Rule::TranslationNotLazy, Path::new("translation_not_lazy.py"))]
    #[test_case(Rule::TranslationTooFewArgs, Path::new("translation_too_few_args.py"))]
    #[test_case(
        Rule::TranslationTooManyArgs,
        Path::new("translation_too_many_args.py")
    )]
    #[test_case(
        Rule::TranslationUnsupportedFormat,
        Path::new("translation_unsupported_format.py")
    )]
    #[test_case(
        Rule::ManifestVersionFormat,
        Path::new("manifest_version_format/__manifest__.py")
    )]
    #[test_case(Rule::PreferEnvAttribute, Path::new("prefer_env_attribute.py"))]
    #[test_case(
        Rule::DeprecatedOdooMethodCall,
        Path::new("deprecated_odoo_method_call.py")
    )]
    fn rules(rule_code: Rule, path: &Path) -> Result<()> {
        let snapshot = path.to_string_lossy().to_string();
        let diagnostics = test_path(
            Path::new("odoo").join(path).as_path(),
            &LinterSettings::for_rule(rule_code),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    #[test]
    fn prefer_env_attribute_suppressed_before_odoo_19() -> Result<()> {
        let snapshot = "prefer_env_attribute_suppressed_before_odoo_19".to_string();
        let diagnostics = test_path(
            Path::new("odoo/prefer_env_attribute.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(18, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::PreferEnvAttribute)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    #[test]
    fn prefer_env_attribute_enabled_at_odoo_19() -> Result<()> {
        let snapshot = "prefer_env_attribute_enabled_at_odoo_19".to_string();
        let diagnostics = test_path(
            Path::new("odoo/prefer_env_attribute.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(19, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::PreferEnvAttribute)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// The deprecated-method table is keyed by the version that deprecated each method, so a
    /// project on 18.0 sees the 18.0 entries and none of the 19.0 ones (`read_group`,
    /// `check_field_access_rights`, `toggle_active`).
    #[test]
    fn deprecated_odoo_method_call_at_odoo_18() -> Result<()> {
        let snapshot = "deprecated_odoo_method_call_at_odoo_18".to_string();
        let diagnostics = test_path(
            Path::new("odoo/deprecated_odoo_method_call.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(18, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::DeprecatedOdooMethodCall)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Before Odoo 18.0 there is no `self.env._`, so the bare `_()` is correct and
    /// `prefer-env-translation` must stay quiet.
    #[test]
    fn prefer_env_translation_suppressed_before_odoo_18() -> Result<()> {
        let snapshot = "prefer_env_translation_suppressed_before_odoo_18".to_string();
        let diagnostics = test_path(
            Path::new("odoo/prefer_env_translation.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(17, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::PreferEnvTranslation)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// `translation-required` has to suggest the same translation call that
    /// `prefer-env-translation` would accept for the configured version — the bare `_()`
    /// before 18.0, `self.env._()` from 18.0 on — or the two rules would fight each other.
    #[test]
    fn translation_required_suggests_bare_underscore_before_odoo_18() -> Result<()> {
        let snapshot = "translation_required_suggests_bare_underscore_before_odoo_18".to_string();
        let diagnostics = test_path(
            Path::new("odoo/translation_required.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(17, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::TranslationRequired)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Every `translation-*` check and the fixture exercising it. pylint-odoo derives them all
    /// from pylint's `logging-*` checks in `custom_logging.py`, whose constructor sets
    /// `odoo_minversion = "14.0"` on the whole family.
    const TRANSLATION_FAMILY: &[(Rule, &str)] = &[
        (
            Rule::TranslationFormatInterpolation,
            "translation_format_interpolation.py",
        ),
        (
            Rule::TranslationFormatTruncated,
            "translation_format_truncated.py",
        ),
        (
            Rule::TranslationFstringInterpolation,
            "translation_fstring_interpolation.py",
        ),
        (Rule::TranslationNotLazy, "translation_not_lazy.py"),
        (Rule::TranslationTooFewArgs, "translation_too_few_args.py"),
        (Rule::TranslationTooManyArgs, "translation_too_many_args.py"),
        (
            Rule::TranslationUnsupportedFormat,
            "translation_unsupported_format.py",
        ),
    ];

    fn translation_family_diagnostics(rule: Rule, fixture: &str, series: u16) -> Result<usize> {
        Ok(test_path(
            Path::new("odoo").join(fixture).as_path(),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(series, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(rule)
            },
        )?
        .len())
    }

    /// Before 14.0 the translation terms were interpolated eagerly, which is what
    /// `translation-contains-variable` reports for those series instead, so none of the
    /// family may fire there.
    #[test]
    fn translation_family_suppressed_before_odoo_14() -> Result<()> {
        for (rule, fixture) in TRANSLATION_FAMILY {
            let count = translation_family_diagnostics(*rule, fixture, 13)?;
            assert_eq!(
                count,
                0,
                "`{}` reported {count} diagnostics on Odoo 13.0, but pylint-odoo scopes the \
                 translation family to 14.0 and up",
                rule.name()
            );
        }
        Ok(())
    }

    /// The counterpart of the test above: the gate must not silence the family outright.
    #[test]
    fn translation_family_enabled_at_odoo_14() -> Result<()> {
        for (rule, fixture) in TRANSLATION_FAMILY {
            let count = translation_family_diagnostics(*rule, fixture, 14)?;
            assert!(
                count > 0,
                "`{}` reported nothing on Odoo 14.0, where it applies",
                rule.name()
            );
        }
        Ok(())
    }

    /// With a configured series, a version for a different series is wrong even though its
    /// shape is right.
    #[test]
    fn manifest_version_format_checks_the_configured_series() -> Result<()> {
        let snapshot = "manifest_version_format_checks_the_configured_series".to_string();
        let diagnostics = test_path(
            Path::new("odoo/manifest_version_format/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(17, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ManifestVersionFormat)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// `qweb` only became deprecated in 16.0, when the `web.assets_qweb` bundle was removed,
    /// so a module targeting an older series keeps it.
    #[test]
    fn manifest_deprecated_key_before_odoo_16() -> Result<()> {
        let snapshot = "manifest_deprecated_key_before_odoo_16".to_string();
        let diagnostics = test_path(
            Path::new("odoo/manifest_deprecated_key/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(15, 0)),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ManifestDeprecatedKey)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// A configured `manifest-deprecated-keys` replaces the built-in list outright: `qweb` is
    /// reported even below 16.0, and `description`, deprecated by default, is not.
    #[test]
    fn manifest_deprecated_key_configured() -> Result<()> {
        let snapshot = "manifest_deprecated_key_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/manifest_deprecated_key/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_version: Some(super::settings::OdooVersion::new(15, 0)),
                    manifest_deprecated_keys: super::settings::ManifestDeprecatedKeys::UserProvided(
                        vec!["qweb".to_string()],
                    ),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ManifestDeprecatedKey)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// `category-allowed` stays inert until the project lists its categories.
    #[test]
    fn category_allowed_is_inert_without_configuration() -> Result<()> {
        let snapshot = "category_allowed_is_inert_without_configuration".to_string();
        let diagnostics = test_path(
            Path::new("odoo/category_allowed/__manifest__.py"),
            &LinterSettings::for_rule(Rule::CategoryAllowed),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    #[test]
    fn category_allowed() -> Result<()> {
        let snapshot = "category_allowed".to_string();
        let diagnostics = test_path(
            Path::new("odoo/category_allowed/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    category_allowed: vec!["Accounting".to_string(), "Sales".to_string()],
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::CategoryAllowed)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// A project that accepts other authors names them, and the built-in list stops applying:
    /// the module authored by the OCA alone is now the one reported.
    #[test]
    fn manifest_required_authors_configured() -> Result<()> {
        let snapshot = "manifest_required_authors_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/manifest_required_author/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    manifest_required_authors: super::settings::ConfiguredList::UserProvided(vec![
                        "Someone Else".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ManifestRequiredAuthor)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Likewise for licenses: naming `GPL` accepts it and drops everything else, so the
    /// `LGPL-3` module the built-in list allows is reported instead.
    #[test]
    fn license_allowed_configured() -> Result<()> {
        let snapshot = "license_allowed_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/license_allowed/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    license_allowed: super::settings::ConfiguredList::UserProvided(vec![
                        "GPL".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::LicenseAllowed)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Requiring a second key reports it alongside the missing `license`.
    #[test]
    fn manifest_required_keys_configured() -> Result<()> {
        let snapshot = "manifest_required_keys_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/manifest_required_key/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    manifest_required_keys: super::settings::ConfiguredList::UserProvided(vec![
                        "license".to_string(),
                        "author".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ManifestRequiredKey)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Narrowing the statuses reports one the built-in list accepts.
    #[test]
    fn development_status_allowed_configured() -> Result<()> {
        let snapshot = "development_status_allowed_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/development_status_allowed/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    development_status_allowed: super::settings::ConfiguredList::UserProvided(
                        vec!["Production/Stable".to_string()],
                    ),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::DevelopmentStatusAllowed)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Dropping `length` from the list stops it being reported.
    #[test]
    fn attribute_deprecated_configured() -> Result<()> {
        let snapshot = "attribute_deprecated_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/attribute_deprecated.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    attribute_deprecated: super::settings::ConfiguredList::UserProvided(vec![
                        "_columns".to_string(),
                        "_defaults".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::AttributeDeprecated)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// A project that only cares about the ORM writes names them, and `read` goes quiet.
    #[test]
    fn method_required_super_configured() -> Result<()> {
        let snapshot = "method_required_super_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/method_required_super.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    method_required_super: super::settings::ConfiguredList::UserProvided(vec![
                        "create".to_string(),
                        "write".to_string(),
                        "unlink".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::MethodRequiredSuper)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Exempting a method of the project's own stops `missing-return` reporting it.
    #[test]
    fn no_missing_return_configured() -> Result<()> {
        let snapshot = "no_missing_return_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/missing_return.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    no_missing_return: super::settings::ConfiguredList::UserProvided(vec![
                        "__init__".to_string(),
                        "setUp".to_string(),
                        "_setup_company".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::MissingReturn)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Only `self.env.cr` counts as a cursor here, so the other spellings go quiet.
    #[test]
    fn cursor_expr_configured() -> Result<()> {
        let snapshot = "cursor_expr_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/invalid_commit.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    cursor_expr: super::settings::ConfiguredList::UserProvided(vec![
                        "self.env.cr".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::InvalidCommit)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Narrowing the exceptions leaves the ones dropped from the list untranslated in peace.
    #[test]
    fn odoo_exceptions_configured() -> Result<()> {
        let snapshot = "odoo_exceptions_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/translation_required.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_exceptions: super::settings::ConfiguredList::UserProvided(vec![
                        "UserError".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::TranslationRequired)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// `installable` no longer counts as defaulting to `True`, so stating it stops being superfluous.
    #[test]
    fn manifest_keys_values_true_configured() -> Result<()> {
        let snapshot = "manifest_keys_values_true_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/manifest_superfluous_key/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    manifest_keys_values_true: super::settings::ConfiguredList::UserProvided(vec![
                        "active".to_string(),
                    ]),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ManifestSuperfluousKey)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Only `requests.get` must carry a timeout, so the other calls stop being reported.
    #[test]
    fn external_request_timeout_methods_configured() -> Result<()> {
        let snapshot = "external_request_timeout_methods_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/external_request_timeout.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    external_request_timeout_methods: super::settings::ConfiguredList::UserProvided(
                        vec!["requests.get".to_string()],
                    ),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ExternalRequestTimeout)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Renaming a parameter of the project's own reports it; `select` is no longer listed.
    #[test]
    fn deprecated_field_parameters_configured() -> Result<()> {
        let snapshot = "deprecated_field_parameters_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/renamed_field_parameter.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    deprecated_field_parameters: super::settings::ConfiguredList::UserProvided(
                        vec![
                            "digits_compute:digits".to_string(),
                            "oldname:string".to_string(),
                        ],
                    ),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::RenamedFieldParameter)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// Naming the methods drops the built-in list, so `fields_view_get` stops being reported.
    #[test]
    fn deprecated_odoo_model_methods_configured() -> Result<()> {
        let snapshot = "deprecated_odoo_model_methods_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/deprecated_odoo_model_method.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    deprecated_odoo_model_methods: super::settings::ConfiguredList::UserProvided(
                        vec!["get_formview_id".to_string()],
                    ),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::DeprecatedOdooModelMethod)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// The configured template is named in the diagnostic, as pylint-odoo's message does.
    #[test]
    fn readme_template_url_configured() -> Result<()> {
        let snapshot = "readme_template_url_configured".to_string();
        let diagnostics = test_path(
            Path::new("odoo/missing_readme/missing/__manifest__.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    readme_template_url: Some(
                        "https://github.com/Vauxoo/templates/blob/main/README.md".to_string(),
                    ),
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::MissingReadme)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// `missing-odoo-file` stays inert until the project lists its required files.
    #[test]
    fn missing_odoo_file_is_inert_without_configuration() -> Result<()> {
        let snapshot = "missing_odoo_file_is_inert_without_configuration".to_string();
        let diagnostics = test_path(
            Path::new("odoo/missing_odoo_file/missing/__manifest__.py"),
            &LinterSettings::for_rule(Rule::MissingOdooFile),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    #[test_case("missing")]
    #[test_case("present")]
    fn missing_odoo_file(module: &str) -> Result<()> {
        let snapshot = format!("missing_odoo_file_{module}");
        let diagnostics = test_path(
            Path::new("odoo/missing_odoo_file")
                .join(module)
                .join("__manifest__.py")
                .as_path(),
            &LinterSettings {
                odoo: super::settings::Settings {
                    odoo_required_files: vec!["static/description/index.html".to_string()],
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::MissingOdooFile)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    /// A standalone `# pylint: disable` is only rewritten when the rules it names are part of
    /// the run, so this fixture is linted with those rules enabled alongside `ODC8502`.
    #[test]
    fn pylint_disable_comment_standalone() -> Result<()> {
        let snapshot = "pylint_disable_comment_standalone".to_string();
        let diagnostics = test_path(
            Path::new("odoo/pylint_disable_comment_standalone.py"),
            &LinterSettings::for_rules([
                Rule::PylintDisableComment,
                Rule::MethodRequiredSuper,
                Rule::ContextOverridden,
                Rule::InvalidCommit,
            ]),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    #[test]
    fn prohibited_method_override() -> Result<()> {
        let snapshot = "prohibited_method_override".to_string();
        let diagnostics = test_path(
            Path::new("odoo/prohibited_method_override.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    prohibited_override_methods: vec![
                        "action_post".to_string(),
                        "unlink".to_string(),
                    ],
                    ..super::settings::Settings::default()
                },
                ..LinterSettings::for_rule(Rule::ProhibitedMethodOverride)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }
}
