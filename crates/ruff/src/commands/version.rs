use std::env;
use std::io::{self, BufWriter, Write};

use anyhow::Result;

use crate::args::HelpFormat;

/// Display version information
pub(crate) fn version(output_format: HelpFormat) -> Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    let version_info = crate::version::version();

    // Vauxoo fork: report the name of the binary that was actually invoked, so
    // the collision-free `ruff-odoo` wrapper introduces itself by its own name.
    let name = env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "ruff".to_string());

    match output_format {
        HelpFormat::Text => {
            writeln!(stdout, "{name} {version_info}")?;
        }
        HelpFormat::Json => {
            serde_json::to_writer_pretty(stdout, &version_info)?;
        }
    }
    Ok(())
}
