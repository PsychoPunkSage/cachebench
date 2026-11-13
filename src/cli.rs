pub use clap::{Parser, Subcommand};
use std::str::FromStr;

/// Which CPU to pin the benchmark thread to.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreSel {
    /// A performance core.
    P,
    /// An efficiency core.
    E,
    /// An explicit CPU number.
    Id(u32),
}

impl FromStr for CoreSel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "p" => Ok(Self::P),
            "e" => Ok(Self::E),
            _ => s
                .parse::<u32>()
                .map(Self::Id)
                .map_err(|_| format!("Invalid core: {s}. Use p, e, or a CPU ID")),
        }
    }
}

/// Microbenchmarks for CPU cache and branch-prediction effects.
#[derive(Parser)]
pub struct Cli {
    /// Core to pin to: `p`, `e`, or a CPU number.
    #[arg(long, global = true, default_value = "p")]
    pub core: CoreSel,
    /// Untimed runs before measuring.
    #[arg(long, global = true, default_value_t = 5)]
    pub warmup: usize,
    /// Timed runs; must be odd so the median is a real sample.
    #[arg(long, global = true, default_value_t = 21, value_parser = parse_odd)]
    pub reps: usize,
    #[command(subcommand)]
    pub cmd: Cmd,
}

/// Parses a `usize` and rejects even values.
fn parse_odd(s: &str) -> Result<usize, String> {
    let n = s.parse::<usize>().map_err(|e| e.to_string())?;

    if n % 2 == 0 {
        return Err("Value must be odd".into());
    }

    Ok(n)
}

/// Experiment to run.
#[derive(Subcommand)]
pub enum Cmd {
    /// Sequential read of a large array (baseline bandwidth).
    Seq {
        /// Array size in MiB.
        #[arg(long, default_value_t = 64)]
        size_mib: usize,
    },
}
