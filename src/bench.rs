use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use crate::perf::Group;

pub struct Stats {
    pub min: Duration,
    pub median: Duration,
    pub max: Duration,
    pub counters: Vec<u64>, // Contains median
}

pub fn measure<R>(
    warmup: usize,
    reps: usize,
    group: &Group,
    mut f: impl FnMut() -> R,
) -> Result<Stats, String> {
    (0..warmup).for_each(|_| {
        black_box(f());
    });
    let (mut times, counts): (Vec<Duration>, Vec<Vec<u64>>) = (0..reps)
        .map(|_| {
            group.reset()?;
            group.enable()?;

            let now = Instant::now();
            black_box(f());
            // Stop the clock before the ioctl, so its cost stays out of the timing.
            let elapsed = now.elapsed();
            group.disable()?;

            Ok((elapsed, group.read_counters()?))
        })
        .collect::<Result<Vec<(Duration, Vec<u64>)>, String>>()?
        .into_iter()
        .unzip();

    times.sort_unstable();

    let counters = (0..group.names.len())
        .map(|i| {
            let mut col: Vec<u64> = counts.iter().map(|row| row[i]).collect();
            col.sort_unstable();
            col[col.len() / 2]
        })
        .collect::<Vec<u64>>();

    Ok(Stats {
        min: *times.first().unwrap(),
        median: times[times.len() / 2],
        max: *times.last().unwrap(),
        counters, //median
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure() {
        let pmu = crate::cpu::pmu_type(0).unwrap();
        let group = Group::new(pmu, &crate::perf::EVENTS[..1]).unwrap();
        let stats = measure(0, 3, &group, || 1 + 1).unwrap();
        assert_eq!(stats.counters.len(), 1);
        assert!(stats.min <= stats.median);
        assert!(stats.median <= stats.max);
    }
}
