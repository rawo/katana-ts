//! The y/N question before anything is overwritten.

use std::io::{BufRead, IsTerminal, Write};

use anyhow::bail;

/// `yes` answers without asking. Without a terminal to ask on, refuses rather
/// than guessing. Only `y` or `yes` (any case) count as yes.
pub fn confirm(
    question: &str,
    yes: bool,
    interactive: bool,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }
    if !interactive {
        bail!("{question} — no terminal to ask on; add --yes to answer yes without a question");
    }
    write!(out, "{question} [y/N] ")?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?;
    Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

pub fn confirm_stdin(question: &str, yes: bool) -> anyhow::Result<bool> {
    let stdin = std::io::stdin();
    let interactive = stdin.is_terminal();
    confirm(question, yes, interactive, &mut stdin.lock(), &mut std::io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(yes: bool, interactive: bool, input: &str) -> (anyhow::Result<bool>, String) {
        let mut out = Vec::new();
        let r = confirm("Overwrite x.tsl?", yes, interactive, &mut input.as_bytes(), &mut out);
        (r, String::from_utf8(out).unwrap())
    }

    #[test]
    fn yes_skips_question() {
        let (r, out) = ask(true, false, "");
        assert!(r.unwrap());
        assert_eq!(out, "");
    }

    #[test]
    fn default_is_no() {
        assert!(!ask(false, true, "\n").0.unwrap());
        assert!(!ask(false, true, "").0.unwrap());
        assert!(!ask(false, true, "n\n").0.unwrap());
    }

    #[test]
    fn y_and_yes_accept() {
        assert!(ask(false, true, "Y\n").0.unwrap());
        assert!(ask(false, true, " yes \n").0.unwrap());
    }

    #[test]
    fn not_interactive_refuses() {
        assert!(ask(false, false, "y\n").0.unwrap_err().to_string().contains("--yes"));
    }

    #[test]
    fn question_text_printed() {
        assert_eq!(ask(false, true, "\n").1, "Overwrite x.tsl? [y/N] ");
    }
}
