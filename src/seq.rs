//! Sequential read of a large array: the baseline every other access pattern is compared against.

use std::{hint::black_box, time::Duration};

use crate::{bench::measure, perf::Group};

/// Sums a `size_mib` array of `u32` front to back and prints time per element and bandwidth.
pub fn run(group: &Group, size_mib: usize, warmup: usize, reps: usize) -> Result<(), String> {
    let n = size_mib * 1024 * 1024 / std::mem::size_of::<u32>();
    // Non-zero values, so every page is really written and backed by its own memory.
    let data = (0..n as u32).collect::<Vec<u32>>();
    let bytes = std::mem::size_of_val(data.as_slice());

    let kernel = || {
        // `black_box` hides the contents, so the sum can't be precomputed outside the timed region.
        let d = black_box(&data);
        d.iter().map(|x| *x as u64).sum::<u64>()
    };

    let s = measure(warmup, reps, group, kernel)?;

    let row = |label: &str, t: Duration| {
        let secs = t.as_secs_f64();
        println!(
            "  {label:<6} {:>9.3} ms  {:>7.3} ns/elem  {:>7.2} GB/s",
            secs * 1e3,
            secs * 1e9 / n as f64,
            bytes as f64 / secs / 1e9,
        );
    };

    println!(
        "seq: {:.1} MiB ({n} x u32), warmup {warmup}, reps {reps}",
        bytes as f64 / (1024.0 * 1024.0),
    );
    row("median", s.median);
    row("min", s.min);
    row("max", s.max);
    println!(
        "  spread {:.1}% of median",
        (s.max - s.min).as_secs_f64() / s.median.as_secs_f64() * 100.0,
    );

    // Counter medians per rep, plus the same value divided by the element count.
    println!("  {:<14} {:>14} {:>10}", "counter", "total", "per elem");
    for (name, &v) in group.names.iter().zip(&s.counters) {
        println!("  {name:<14} {v:>14} {:>10.4}", v as f64 / n as f64);
    }

    let get = |name: &str| {
        group
            .names
            .iter()
            .position(|x| x == name)
            .map(|i| s.counters[i])
    };
    if let (Some(ins), Some(cyc)) = (get("instructions"), get("cycles"))
        && cyc > 0
    {
        println!("  IPC {:.2}", ins as f64 / cyc as f64);
    }

    Ok(())
}
