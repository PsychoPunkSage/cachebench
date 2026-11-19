mod bench;
mod cli;
mod cpu;
mod pattern;
mod perf;
mod seq;
mod stride;

use clap::Parser;

use cli::{Cli, Cmd};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cpu = cpu::resolve(&cli.core)?;
    cpu::pin(cpu)?;
    let pmu = cpu::pmu_type(cpu)?;
    let group = perf::Group::new(pmu, perf::EVENTS)?;
    // SAFETY: `sched_getcpu` takes no arguments and has no preconditions.
    eprintln!(
        "pinned to cpu {} (running on {}), pmu type {}",
        cpu,
        unsafe { libc::sched_getcpu() },
        pmu
    );

    match cli.cmd {
        Cmd::Seq { size_mib } => seq::run(&group, size_mib, cli.warmup, cli.reps)?,
        Cmd::Stride { size_mib } => stride::run(&group, size_mib, cli.warmup, cli.reps)?,
        Cmd::Pattern { size_mib } => pattern::run(&group, size_mib, cli.warmup, cli.reps)?,
    }

    Ok(())
}
