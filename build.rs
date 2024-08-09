include!("src/config/cli.rs");

use clap::Command as ClapCommand;
use clap::CommandFactory;
use clap_complete::generate_to;
use clap_complete::Shell::{Bash, Fish, Zsh};
use clap_mangen::Man;
use std::error::Error;
use std::fs;
use vergen_gitcl::Emitter;
use vergen_gitcl::GitclBuilder;

static NAME: &str = "rmpc";

fn generate_man_pages(cmd: ClapCommand) -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("man");
    let mut buffer = Vec::default();

    Man::new(cmd).render(&mut buffer)?;
    fs::create_dir_all(&out)?;
    fs::write(out.join(NAME.to_owned() + ".1"), buffer)?;
    Ok(())
}

fn generate_shell_completions(mut cmd: ClapCommand) -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("completions");

    std::fs::create_dir_all(&out)?;
    for shell in [Bash, Fish, Zsh] {
        generate_to(shell, &mut cmd, NAME, &out)?;
    }
    Ok(())
}

fn emit_git_info() -> Result<(), Box<dyn Error>> {
    Emitter::default()
        .add_instructions(
            &GitclBuilder::default()
                .commit_date(true)
                .describe(false, false, None)
                .build()?,
        )?
        .emit()?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut cmd = Args::command();
    cmd.set_bin_name(NAME);

    generate_man_pages(cmd.clone())?;
    generate_shell_completions(cmd)?;

    emit_git_info()?;

    Ok(())
}
