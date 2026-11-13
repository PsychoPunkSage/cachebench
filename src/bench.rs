use std::{
    hint::black_box,
    time::{Duration, Instant},
};

pub struct Stats {
    pub min: Duration,
    pub median: Duration,
    pub max: Duration,
}

pub fn measure<R>(warmup: usize, reps: usize, mut f: impl FnMut() -> R) -> Stats {
    (0..warmup).for_each(|_| {
        black_box(f());
    });
    let mut data = (0..reps)
        .map(|_| {
            let now = Instant::now();
            black_box(f());
            now.elapsed()
        })
        .collect::<Vec<Duration>>();

    data.sort_unstable();

    Stats {
        min: *data.first().unwrap(),
        median: data[data.len() / 2],
        max: *data.last().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure() {
        let stats = measure(0, 3, || 1 + 1);
        assert!(stats.min <= stats.median);
        assert!(stats.median <= stats.max);
    }
}
