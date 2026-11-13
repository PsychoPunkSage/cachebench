use std::{fs::read_to_string, num::ParseIntError};

use crate::cli::CoreSel;

/// Parses a kernel CPU list such as `0-3,6` into `[0, 1, 2, 3, 6]`.
pub fn parse_cpu_list(s: &str) -> Result<Vec<usize>, ParseIntError> {
    if s.trim().is_empty() {
        return Ok(vec![]);
    }

    s.trim()
        .split(',')
        .map(|e| {
            if let Some((a, b)) = e.split_once('-') {
                Ok((a.parse::<usize>()?..=b.parse()?).collect::<Vec<_>>())
            } else {
                Ok(vec![e.parse()?])
            }
        })
        .collect::<Result<Vec<Vec<usize>>, ParseIntError>>()
        .map(|v| v.into_iter().flatten().collect())
}

/// Maps a core selection to a CPU number; `P`/`E` pick the last CPU of that type.
pub fn resolve(sel: &CoreSel) -> Result<usize, String> {
    let last_cpu = |file_path: &str| -> Result<usize, String> {
        let s = read_to_string(file_path).map_err(|e| format!("Error reading CPU: {e}"))?;
        Ok(*parse_cpu_list(&s)
            .map_err(|e| format!("Parsing error: {e}"))?
            .last()
            .ok_or("No CPU listed")?)
    };
    match sel {
        CoreSel::P => last_cpu("/sys/devices/cpu_core/cpus"),
        CoreSel::E => last_cpu("/sys/devices/cpu_atom/cpus"),
        CoreSel::Id(i) => Ok(*i as usize),
    }
}

/// Restricts the calling thread to run only on `cpu`.
pub fn pin(cpu: usize) -> Result<(), String> {
    if cpu >= libc::CPU_SETSIZE as usize {
        return Err("cpu index is greater than CPU_SETSIZE".to_string());
    }

    // SAFETY: `cpu_set_t` is a plain bitmask; all zeros is a valid empty set.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    // SAFETY: `cpu < CPU_SETSIZE` was checked above, so the bit index is in bounds.
    unsafe { libc::CPU_SET(cpu, &mut set) };

    // SAFETY: the kernel only reads `set` during the call, and the size passed matches its type.
    let rc = unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) };

    if rc != 0 {
        return Err(format!(
            "Error setting affinity: {}",
            std::io::Error::last_os_error()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_list() {
        let s = "\n 0-5,7,10-15\t\n";
        let out = parse_cpu_list(s);
        assert_eq!(
            out.unwrap(),
            vec![0, 1, 2, 3, 4, 5, 7, 10, 11, 12, 13, 14, 15]
        )
    }

    #[test]
    fn test_parse_cpu_list_empty_input() {
        let s = "\n\t\n";
        let out = parse_cpu_list(s);
        assert_eq!(out.unwrap(), vec![])
    }

    #[test]
    fn test_resolve_valid_input() {
        // P, E assetions are not possible as it depends on machine
        assert_eq!(resolve(&CoreSel::Id(5)).unwrap(), 5);
        assert_eq!(resolve(&CoreSel::Id(500)).unwrap(), 500);
    }
}
