use crate::doc::templates::generator::validate_external_templates;
use console::style;

pub fn handle_template_validate() {
    let results = validate_external_templates();
    if results.is_empty() {
        println!("{} No external templates found.", style("ℹ").cyan().bold());
        return;
    }
    let mut ok_count = 0usize;
    let mut err_count = 0usize;
    for (key, res) in results {
        match res {
            Ok(()) => {
                ok_count += 1;
                println!(
                    "{} {}",
                    style("✔").green().bold(),
                    style(format!("{} OK", key)).green()
                );
            }
            Err(e) => {
                err_count += 1;
                println!("{} {} — {}", style("✖").red().bold(), key, e);
            }
        }
    }
    println!(
        "\n{} {} OK, {} failed.",
        style("Summary:").bold(),
        style(ok_count).green(),
        style(err_count).red()
    );
}
