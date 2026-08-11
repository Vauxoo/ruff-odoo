//! Rules from [odoo](https://pypi.org/project/pylint-odoo/).
pub(crate) mod helpers;
pub(crate) mod rules;

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
    fn rules(rule_code: Rule, path: &Path) -> Result<()> {
        let snapshot = format!("{}_{}", rule_code.noqa_code(), path.to_string_lossy());
        let diagnostics = test_path(
            Path::new("odoo").join(path).as_path(),
            &LinterSettings::for_rule(rule_code),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }
}
