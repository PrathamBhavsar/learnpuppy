//! `learnpuppy config show | set-profile | set-style | apply-defaults`.

use std::io::Write;
use std::path::Path;

use clap::Subcommand;

use crate::state::{LearningStyle, Profile, Settings};

#[derive(Subcommand)]
pub enum Cmd {
    /// Print each role's profile and the learning style.
    Show,
    /// Pick the CLI, model and reasoning effort for one role.
    SetProfile {
        role: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        effort: Option<String>,
    },
    /// `visual` explains with diagrams and tables first; `default` leaves prompts alone.
    SetStyle { style: String },
    /// Write the starter profile table and the visual style.
    ApplyDefaults,
}

pub fn run(root: &Path, s: &mut Settings, cmd: Cmd, out: &mut dyn Write) -> anyhow::Result<()> {
    match cmd {
        Cmd::Show => {
            for (role, p) in &s.profiles {
                writeln!(
                    out,
                    "{role}\t{}\t{}\t{}",
                    p.provider,
                    p.model,
                    p.effort.as_deref().unwrap_or("-")
                )?;
            }
            writeln!(
                out,
                "style\t{}",
                serde_json::to_value(s.learning_style)?
                    .as_str()
                    .unwrap_or("")
            )?;
            return Ok(());
        }
        Cmd::SetProfile {
            role,
            provider,
            model,
            effort,
        } => {
            s.set_profile(&role, Profile::new(&provider, &model, effort.as_deref())?)?;
            writeln!(out, "set {role}")?;
        }
        Cmd::SetStyle { style } => {
            s.learning_style = LearningStyle::parse(&style)?;
            writeln!(out, "style {style}")?;
        }
        Cmd::ApplyDefaults => {
            s.apply_defaults();
            writeln!(out, "applied defaults")?;
        }
    }
    s.save(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go(root: &Path, cmd: Cmd) -> anyhow::Result<String> {
        let mut s = Settings::load(root)?;
        let mut out = Vec::new();
        run(root, &mut s, cmd, &mut out)?;
        Ok(String::from_utf8(out)?)
    }

    fn set(role: &str, provider: &str, effort: Option<&str>) -> Cmd {
        Cmd::SetProfile {
            role: role.into(),
            provider: provider.into(),
            model: "m1".into(),
            effort: effort.map(Into::into),
        }
    }

    #[test]
    fn set_profile_validates_then_persists_and_show_lists_it() {
        let d = tempfile::tempdir().unwrap();
        for bad in [
            set("nobody", "codex", None),
            set("prober", "gpt", None),
            set("prober", "codex", Some("turbo")),
        ] {
            assert!(go(d.path(), bad).is_err());
        }
        assert!(
            !d.path().join("settings.json").exists(),
            "a rejected command must not write"
        );
        go(d.path(), set("prober", "codex", Some("low"))).unwrap();
        assert!(go(
            d.path(),
            Cmd::SetStyle {
                style: "loud".into()
            }
        )
        .is_err());
        go(
            d.path(),
            Cmd::SetStyle {
                style: "visual".into(),
            },
        )
        .unwrap();
        let shown = go(d.path(), Cmd::Show).unwrap();
        assert_eq!(shown, "prober\tcodex\tm1\tlow\nstyle\tvisual\n");
    }

    #[test]
    fn apply_defaults_writes_the_starter_table() {
        let d = tempfile::tempdir().unwrap();
        go(d.path(), Cmd::ApplyDefaults).unwrap();
        let shown = go(d.path(), Cmd::Show).unwrap();
        assert!(
            shown.contains("auditor\tagy\tclaude-opus-5-5-high\t-\n"),
            "{shown}"
        );
        assert!(
            shown.contains("pdf\tagy\tgemini-3.8-flash-medium\t-\n"),
            "{shown}"
        );
        assert!(shown.ends_with("style\tvisual\n"));
    }
}
