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

    #[test_case(Rule::ManifestRequiredKey, Path::new("ODOO001/__manifest__.py"))]
    #[test_case(Rule::ManifestDeprecatedKey, Path::new("ODOO002/__manifest__.py"))]
    #[test_case(Rule::VimComment, Path::new("ODOO003.py"))]
    #[test_case(Rule::ExceptPass, Path::new("ODOO004.py"))]
    #[test_case(Rule::MethodRequiredSuper, Path::new("ODOO005.py"))]
    #[test_case(Rule::UnusedLogger, Path::new("ODOO006_0.py"))]
    #[test_case(Rule::UnusedLogger, Path::new("ODOO006_1.py"))]
    #[test_case(Rule::FieldStringRedundant, Path::new("ODOO007.py"))]
    #[test_case(Rule::ManifestRequiredAuthor, Path::new("ODOO008/__manifest__.py"))]
    #[test_case(Rule::ManifestAuthorString, Path::new("ODOO009/__manifest__.py"))]
    #[test_case(Rule::LicenseAllowed, Path::new("ODOO010/__manifest__.py"))]
    #[test_case(Rule::ManifestMaintainersList, Path::new("ODOO011/__manifest__.py"))]
    #[test_case(Rule::ManifestSummaryMultiline, Path::new("ODOO012/__manifest__.py"))]
    #[test_case(Rule::DevelopmentStatusAllowed, Path::new("ODOO013/__manifest__.py"))]
    #[test_case(
        Rule::WebsiteManifestKeyNotValidUri,
        Path::new("ODOO014/__manifest__.py")
    )]
    #[test_case(Rule::InvalidEmail, Path::new("ODOO015/__manifest__.py"))]
    #[test_case(Rule::MissingReadme, Path::new("ODOO016/missing/__manifest__.py"))]
    #[test_case(Rule::MissingReadme, Path::new("ODOO016/present/__manifest__.py"))]
    #[test_case(Rule::InvalidCommit, Path::new("ODOO017.py"))]
    #[test_case(Rule::ContextOverridden, Path::new("ODOO018.py"))]
    #[test_case(Rule::BadBuiltinGroupby, Path::new("ODOO019.py"))]
    #[test_case(Rule::OdooExceptionWarning, Path::new("ODOO020.py"))]
    #[test_case(Rule::AttributeDeprecated, Path::new("ODOO021.py"))]
    #[test_case(Rule::MissingReturn, Path::new("ODOO022.py"))]
    #[test_case(
        Rule::OdooAddonsRelativeImport,
        Path::new("ODOO023/my_module/models/foo.py")
    )]
    #[test_case(Rule::DirectTranslationCall, Path::new("ODOO024.py"))]
    #[test_case(Rule::ManifestSuperfluousKey, Path::new("ODOO025/__manifest__.py"))]
    #[test_case(Rule::HeaderComments, Path::new("ODOO026.py"))]
    #[test_case(Rule::MethodCompute, Path::new("ODOO027.py"))]
    #[test_case(Rule::MethodSearch, Path::new("ODOO028.py"))]
    #[test_case(Rule::MethodInverse, Path::new("ODOO029.py"))]
    #[test_case(Rule::RenamedFieldParameter, Path::new("ODOO030.py"))]
    #[test_case(Rule::TranslationField, Path::new("ODOO031.py"))]
    #[test_case(Rule::InheritableMethodString, Path::new("ODOO032.py"))]
    #[test_case(Rule::InheritableMethodLambda, Path::new("ODOO033.py"))]
    #[test_case(Rule::DeprecatedNameGet, Path::new("ODOO034.py"))]
    #[test_case(Rule::DeprecatedSelfCr, Path::new("ODOO035.py"))]
    #[test_case(Rule::SuperMethodMismatch, Path::new("ODOO036.py"))]
    #[test_case(Rule::DeprecatedOdooModelMethod, Path::new("ODOO037.py"))]
    #[test_case(Rule::NoSearchAll, Path::new("ODOO038.py"))]
    #[test_case(Rule::NoRaiseUnlink, Path::new("ODOO039.py"))]
    #[test_case(Rule::NoWriteInCompute, Path::new("ODOO040.py"))]
    #[test_case(Rule::TranslationContainsVariable, Path::new("ODOO041.py"))]
    #[test_case(Rule::TranslationPositional, Path::new("ODOO042.py"))]
    #[test_case(Rule::TranslationInjection, Path::new("ODOO043.py"))]
    #[test_case(Rule::DeprecatedInselectOperator, Path::new("ODOO044.py"))]
    #[test_case(Rule::TestFolderImported, Path::new("ODOO045/__init__.py"))]
    #[test_case(Rule::ManifestExternalAssets, Path::new("ODOO046/__manifest__.py"))]
    #[test_case(Rule::PylintDisableComment, Path::new("ODOO047.py"))]
    #[test_case(Rule::ManifestDataDuplicated, Path::new("ODOO048/__manifest__.py"))]
    #[test_case(Rule::TranslationRequired, Path::new("ODOO049.py"))]
    #[test_case(
        Rule::TranslationRequired,
        Path::new("ODOO049/tests/test_translation.py")
    )]
    #[test_case(Rule::NoWizardInModels, Path::new("ODOO050/models/sale_import.py"))]
    #[test_case(Rule::NoWizardInModels, Path::new("ODOO050/wizards/sale_import.py"))]
    #[test_case(Rule::ExternalRequestTimeout, Path::new("ODOO051.py"))]
    #[test_case(Rule::SqlInjection, Path::new("ODOO052.py"))]
    #[test_case(Rule::ResourceNotExist, Path::new("ODOO053/__manifest__.py"))]
    #[test_case(
        Rule::ManifestBehindMigrations,
        Path::new("ODOO054/behind/__manifest__.py")
    )]
    #[test_case(
        Rule::ManifestBehindMigrations,
        Path::new("ODOO054/ok/__manifest__.py")
    )]
    #[test_case(Rule::CategoryAllowedApp, Path::new("ODOOAPP001/__manifest__.py"))]
    #[test_case(
        Rule::MissingOdooFileApp,
        Path::new("ODOOAPP002/missing/__manifest__.py")
    )]
    #[test_case(
        Rule::MissingOdooFileApp,
        Path::new("ODOOAPP002/present/__manifest__.py")
    )]
    #[test_case(
        Rule::MissingOdooFileApp,
        Path::new("ODOOAPP002/no_price/__manifest__.py")
    )]
    #[test_case(Rule::ManifestRequiredKeyApp, Path::new("ODOOAPP003/__manifest__.py"))]
    fn rules(rule_code: Rule, path: &Path) -> Result<()> {
        let snapshot = format!("{}_{}", rule_code.noqa_code(), path.to_string_lossy());
        let diagnostics = test_path(
            Path::new("odoo").join(path).as_path(),
            &LinterSettings::for_rule(rule_code),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }

    #[test]
    fn prohibited_method_override() -> Result<()> {
        let snapshot = "prohibited_method_override".to_string();
        let diagnostics = test_path(
            Path::new("odoo/ODOO055.py"),
            &LinterSettings {
                odoo: super::settings::Settings {
                    prohibited_override_methods: vec![
                        "action_post".to_string(),
                        "unlink".to_string(),
                    ],
                },
                ..LinterSettings::for_rule(Rule::ProhibitedMethodOverride)
            },
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }
}
