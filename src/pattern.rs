use std::hint::black_box;

use crate::{bench::measure, perf::Group};

pub fn run(group: &Group, size_mib: usize, warmup: usize, reps: usize) -> Result<(), String> {
    let n = size_mib * 1024 * 1024 / std::mem::size_of::<u32>();

    let data = (0..n as u32).collect::<Vec<u32>>();
    let seq_idx = (0..n as u32).collect::<Vec<u32>>();
    let mut rdm_idx = seq_idx.clone();
    shuffle(&mut rdm_idx, 64);

    println!(
        "pattern: {:.1} MiB ({n} x u32), warmup {warmup}, reps {reps}; counters are per access",
        std::mem::size_of_val(data.as_slice()) as f64 / (1024.0 * 1024.0),
    );
    println!(
        "  random order starts with {:?}",
        &rdm_idx[..rdm_idx.len().min(5)]
    );
    print!("  {:<8} {:>10}", "order", "ns/access");
    group.names.iter().for_each(|name| print!(" {name:>13}"));
    println!();

    for ops in [("seq", seq_idx), ("random", rdm_idx)] {
        let kernel = || {
            let d = black_box(&data);
            let idx = black_box(&ops.1);
            idx.iter().map(|e| d[*e as usize] as u64).sum::<u64>()
        };

        let s = measure(warmup, reps, group, kernel)?;

        // Every index is visited once, so both orders make exactly n accesses.
        print!(
            "  {:<8} {:>10.3}",
            ops.0,
            s.median.as_secs_f64() * 1e9 / n as f64
        );
        s.counters
            .iter()
            .for_each(|&v| print!(" {:>13.4}", v as f64 / n as f64));
        println!();
    }

    Ok(())
}

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

fn shuffle(v: &mut [u32], mut seed: u64) {
    for i in (1..v.len()).rev() {
        xorshift(&mut seed);

        v.swap(i, seed as usize % (i + 1));
    }
}
