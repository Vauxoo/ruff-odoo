//! Rules that only apply to paid apps (manifests with a "price" key), published on the
//! [Odoo Apps store](https://apps.odoo.com/apps).
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

    #[test_case(
        Rule::CategoryAllowedApp,
        Path::new("category_allowed_app/__manifest__.py")
    )]
    #[test_case(
        Rule::MissingOdooFileApp,
        Path::new("missing_odoo_file_app/missing/__manifest__.py")
    )]
    #[test_case(
        Rule::MissingOdooFileApp,
        Path::new("missing_odoo_file_app/present/__manifest__.py")
    )]
    #[test_case(
        Rule::MissingOdooFileApp,
        Path::new("missing_odoo_file_app/no_price/__manifest__.py")
    )]
    #[test_case(
        Rule::ManifestRequiredKeyApp,
        Path::new("manifest_required_key_app/__manifest__.py")
    )]
    fn rules(rule_code: Rule, path: &Path) -> Result<()> {
        let snapshot = path.to_string_lossy().to_string();
        let diagnostics = test_path(
            Path::new("odoo_app").join(path).as_path(),
            &LinterSettings::for_rule(rule_code),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }
}
