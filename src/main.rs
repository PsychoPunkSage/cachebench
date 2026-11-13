mod bench;
mod cli;
mod cpu;
mod seq;

use clap::Parser;

use cli::{Cli, Cmd};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cpu = cpu::resolve(&cli.core)?;
    cpu::pin(cpu)?;
    // SAFETY: `sched_getcpu` takes no arguments and has no preconditions.
    eprintln!("pinned to cpu {} (running on {})", cpu, unsafe {
        libc::sched_getcpu()
    });

    match cli.cmd {
        Cmd::Seq { size_mib } => seq::run(size_mib, cli.warmup, cli.reps),
    }

    Ok(())
}
