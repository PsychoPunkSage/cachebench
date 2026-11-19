use std::hint::black_box;

use crate::{bench::measure, perf::Group};

pub fn run(group: &Group, size_mib: usize, warmup: usize, reps: usize) -> Result<(), String> {
    let n = size_mib * 1024 * 1024 / std::mem::size_of::<u32>();

    let data = (0..n as u32).collect::<Vec<u32>>();

    println!(
        "stride: {:.1} MiB ({n} x u32), warmup {warmup}, reps {reps}; counters are per access",
        std::mem::size_of_val(data.as_slice()) as f64 / (1024.0 * 1024.0),
    );
    print!(
        "  {:>5} {:>7} {:>10} {:>10}",
        "k", "bytes", "accesses", "ns/access"
    );
    group.names.iter().for_each(|name| print!(" {name:>13}"));
    println!();

    (0..=10).try_for_each(|pow| {
        let k = 1 << pow;

        let kernel = || {
            let d = black_box(&data);
            d.iter().step_by(k).map(|e| *e as u64).sum::<u64>()
        };

        let stats = measure(warmup, reps, group, kernel)?;

        // `step_by(k)` visits indices 0, k, 2k, ..., so ceil(n / k) elements.
        let accesses = n.div_ceil(k);
        print!(
            "  {k:>5} {:>7} {accesses:>10} {:>10.3}",
            k * std::mem::size_of::<u32>(),
            stats.median.as_secs_f64() * 1e9 / accesses as f64,
        );
        stats
            .counters
            .iter()
            .for_each(|&v| print!(" {:>13.4}", v as f64 / accesses as f64));
        println!();

        Ok(())
    })
}
