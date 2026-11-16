use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// Bits of `PerfEventAttr::flags`, in header order starting at bit 0.
/// Create the counter stopped; it counts only after an enable ioctl.
pub const DISABLED: u64 = 1 << 0;
/// Don't count events that happen in kernel mode.
pub const EXCLUDE_KERNEL: u64 = 1 << 5;
/// Don't count events that happen in the hypervisor.
pub const EXCLUDE_HV: u64 = 1 << 6;

/// `PerfEventAttr::type` for generic hardware events (cycles, instructions, ...).
pub const PERF_TYPE_HARDWARE: u32 = 0;
/// On hybrid CPUs, `config` carries the PMU type id in its upper 32 bits.
pub const PERF_PMU_TYPE_SHIFT: u32 = 32;
/// `perf_event_open` flag: close the fd on `exec`.
pub const PERF_FLAG_FD_CLOEXEC: u64 = 1 << 3;
/// Generic hardware event id: CPU cycles.
pub const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;

/// ioctl "buttons" for a perf counter: _IO('$', n) = (0x24 << 8) | n
/// Counter ioctl commands; `_IO('$', n)` expands to `(0x24 << 8) | n`.
/// Start counting.
pub const PERF_EVENT_IOC_ENABLE: u64 = 0x2400;
/// Stop counting.
pub const PERF_EVENT_IOC_DISABLE: u64 = 0x2401;
/// Set the count to zero.
pub const PERF_EVENT_IOC_RESET: u64 = 0x2403;

/// Set r#type for cache event (Lid/LLC misses)
pub const PERF_TYPE_HW_CACHE: u32 = 3;
/// eventID: count instructions
pub const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
/// eventID: count mispredicted branches
pub const PERF_COUNT_HW_BRANCH_MISSES: u64 = 5;

pub const PERF_COUNT_HW_CACHE_L1D: u64 = 0;
pub const PERF_COUNT_HW_CACHE_LL: u64 = 2;
pub const PERF_COUNT_HW_CACHE_OP_READ: u64 = 0;
pub const PERF_COUNT_HW_CACHE_RESULT_MISS: u64 = 1;
pub const PERF_FORMAT_GROUP: u64 = 1 << 3;
pub const PERF_IOC_FLAG_GROUP: usize = 1 << 0;

#[repr(C)]
pub union Sample {
    pub sample_period: u64,
    pub sample_freq: u64,
}

#[repr(C)]
pub union Wakeup {
    pub wakeup_events: u32,
    pub wakeup_watermark: u32,
}

#[repr(C)]
pub union Config1 {
    pub bp_addr: u64,
    pub kprobe_func: u64,
    pub uprobe_path: u64,
    pub config1: u64,
}

#[repr(C)]
pub union Config2 {
    pub bp_len: u64,
    pub kprobe_addr: u64,
    pub probe_offset: u64,
    pub config2: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AuxFlags {
    pub flags: u32,
}

#[repr(C)]
pub union Aux {
    pub aux_action: u32,
    pub aux_flags: AuxFlags,
}

#[repr(C)]
pub struct PerfEventAttr {
    pub r#type: u32,
    pub size: u32,
    pub config: u64,

    pub sample: Sample,

    pub sample_type: u64,
    pub read_format: u64,

    pub flags: u64,

    pub wakeup: Wakeup,

    pub bp_type: u32,

    pub config1: Config1,
    pub config2: Config2,
    pub branch_sample_type: u64,

    pub sample_regs_user: u64,

    pub sample_stack_user: u32,

    pub clockid: i32,

    pub sample_regs_intr: u64,

    pub aux_watermark: u32,
    pub sample_max_stack: u16,
    pub __reserved_2: u16,
    pub aux_sample_size: u32,

    pub aux: Aux,

    pub sig_data: u64,

    pub config3: u64,
}

impl Default for PerfEventAttr {
    fn default() -> Self {
        // SAFETY: every field is a plain integer (or a union of integers), so all-zero bytes
        // are a valid value; the kernel treats 0 as "off" or "unused" for each of them.
        unsafe { std::mem::zeroed() }
    }
}

/// A counter the tool knows how to open, independent of which PMU it runs on.
pub struct Event {
    /// Short label shown in output.
    pub name: &'static str,
    /// `PerfEventAttr::type` (`PERF_TYPE_HARDWARE` or `PERF_TYPE_HW_CACHE`).
    pub type_: u32,
    /// Builds `PerfEventAttr::config` for a given PMU type id.
    pub config: fn(u32) -> u64,
}

/// Default events. The first one becomes the group leader and must open; the kernel maps
/// each generic event to the PMU's own event code, and events a PMU lacks are skipped.
pub const EVENTS: &[Event] = &[
    Event {
        name: "cycles",
        type_: PERF_TYPE_HARDWARE,
        config: |pmu| hw(pmu, PERF_COUNT_HW_CPU_CYCLES),
    },
    Event {
        name: "instructions",
        type_: PERF_TYPE_HARDWARE,
        config: |pmu| hw(pmu, PERF_COUNT_HW_INSTRUCTIONS),
    },
    Event {
        name: "l1d-misses",
        type_: PERF_TYPE_HW_CACHE,
        config: |pmu| {
            hw_cache(
                pmu,
                PERF_COUNT_HW_CACHE_L1D,
                PERF_COUNT_HW_CACHE_OP_READ,
                PERF_COUNT_HW_CACHE_RESULT_MISS,
            )
        },
    },
    Event {
        name: "llc-misses",
        type_: PERF_TYPE_HW_CACHE,
        config: |pmu| {
            hw_cache(
                pmu,
                PERF_COUNT_HW_CACHE_LL,
                PERF_COUNT_HW_CACHE_OP_READ,
                PERF_COUNT_HW_CACHE_RESULT_MISS,
            )
        },
    },
    Event {
        name: "branch-misses",
        type_: PERF_TYPE_HARDWARE,
        config: |pmu| hw(pmu, PERF_COUNT_HW_BRANCH_MISSES),
    },
];

/// Counters that are reset, enabled, disabled and read together through one leader fd.
pub struct Group {
    /// First counter opened; every group operation goes through it.
    pub leader: OwnedFd,
    /// Never read directly; kept so the member fds stay open and remain in the group.
    pub members: Vec<OwnedFd>,
    /// Names of the counters actually opened, in the order `read_counters` returns them.
    pub names: Vec<String>,
}

impl Group {
    /// Opens `events` on `pmu`: the first as leader (required), the rest best-effort.
    pub fn new(pmu: u32, events: &[Event]) -> Result<Self, String> {
        let (first, rest) = events.split_first().ok_or("no events to open")?;
        let leader = open(first.type_, (first.config)(pmu), None).map_err(|e| {
            format!(
                "cannot open leader event `{}` on pmu {pmu}: {e}",
                first.name
            )
        })?;

        let mut members = Vec::with_capacity(rest.len());
        let mut names = vec![first.name.to_string()];
        for event in rest {
            match open(event.type_, (event.config)(pmu), Some(&leader)) {
                Ok(fd) => {
                    members.push(fd);
                    names.push(event.name.to_string());
                }
                Err(e) => eprintln!("warning: skipping `{}` on pmu {pmu}: {e}", event.name),
            }
        }

        Ok(Self {
            members,
            names,
            leader,
        })
    }

    /// Zeroes every counter in the group.
    pub fn reset(&self) -> Result<(), String> {
        ioctl(&self.leader, PERF_EVENT_IOC_RESET, PERF_IOC_FLAG_GROUP)
    }

    /// Starts every counter in the group at once.
    pub fn enable(&self) -> Result<(), String> {
        ioctl(&self.leader, PERF_EVENT_IOC_ENABLE, PERF_IOC_FLAG_GROUP)
    }

    /// Stops every counter in the group at once.
    pub fn disable(&self) -> Result<(), String> {
        ioctl(&self.leader, PERF_EVENT_IOC_DISABLE, PERF_IOC_FLAG_GROUP)
    }

    /// Returns one value per counter, in `names` order. The kernel's layout is
    /// `[nr, value0, value1, ...]`; `nr` is checked against `names`.
    pub fn read_counters(&self) -> Result<Vec<u64>, String> {
        let mut buf = vec![0u64; 1 + self.names.len()];
        read_counters(&self.leader, &mut buf)?;
        match buf[0] {
            n if n == self.names.len() as u64 => Ok(buf[1..].to_vec()),
            _ => Err(format!(
                "Unexpected Data: expected: {} data | got: {} data",
                self.names.len(),
                buf[0]
            )),
        }
    }
}

/// Opens a stopped counter for hardware `event` on PMU `pmu_type`, counting the calling
/// thread in user mode on any CPU. The returned fd closes itself when dropped.
pub fn open(type_: u32, config: u64, leader: Option<&OwnedFd>) -> Result<OwnedFd, String> {
    let group_id: libc::c_int = match leader {
        Some(fd) => fd.as_raw_fd(),
        None => -1,
    };

    let mut flags = EXCLUDE_KERNEL | EXCLUDE_HV;
    let mut read_format = 0;
    if leader.is_none() {
        flags |= DISABLED;
        read_format = PERF_FORMAT_GROUP;
    }

    let pea = PerfEventAttr {
        r#type: type_,
        size: size_of::<PerfEventAttr>() as u32,
        config,
        flags,
        read_format,
        ..Default::default()
    };

    // SAFETY: `pea` is a valid, fully initialised attr whose `size` matches its layout; the
    // kernel only reads it during the call. The remaining arguments are plain integers.
    let out = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &pea,
            0 as libc::pid_t,
            -1 as libc::c_int,
            group_id,
            PERF_FLAG_FD_CLOEXEC,
        )
    };

    if out == -1 {
        return Err(format!(
            "Syscall error: {}",
            std::io::Error::last_os_error()
        ));
    }

    // SAFETY: `out` is a new fd the kernel just returned to us, so it is open and has no
    // other owner.
    Ok(unsafe { OwnedFd::from_raw_fd(out as i32) })
}

/// Sends perf ioctl command `req` (one of `PERF_EVENT_IOC_*`) with argument `arg` to the counter.
pub fn ioctl(fd: &OwnedFd, req: u64, arg: usize) -> Result<(), String> {
    // SAFETY: `fd` is an open perf fd for as long as the borrow lives, and the perf ioctls
    // used here take an integer argument, not a pointer.
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), req, arg) };
    if rc == -1 {
        return Err(format!(
            "ioctl syscall error: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Fills `buf` completely from `fd`, or fails.
pub fn read_counters(fd: &OwnedFd, buf: &mut [u64]) -> Result<(), String> {
    // SAFETY: `buf` is a live, writable slice and we ask for at most its size in bytes.
    let n = unsafe {
        libc::read(
            fd.as_raw_fd(),
            buf.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of_val(buf),
        )
    };

    match n {
        k if k as usize == std::mem::size_of_val(buf) => Ok(()),
        -1 => Err(format!("Read error: {}", std::io::Error::last_os_error())),
        _ => Err(format!(
            "short read: got {n} of {} bytes",
            std::mem::size_of_val(buf)
        )),
    }
}

/// `config` for a generic hardware event on a hybrid CPU: PMU id in the upper 32 bits.
pub fn hw(pmu: u32, event: u64) -> u64 {
    (pmu as u64) << PERF_PMU_TYPE_SHIFT | event
}

/// `config` for a cache event, laid out as `0xPPPPPPPP00RRooCC` (result, op, cache id).
pub fn hw_cache(pmu: u32, cache: u64, op: u64, result: u64) -> u64 {
    (pmu as u64) << PERF_PMU_TYPE_SHIFT | result << 16 | op << 8 | cache
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_size() {
        assert_eq!(std::mem::size_of::<PerfEventAttr>(), 136)
    }

    #[test]
    fn field_offsets_match_header() {
        use std::mem::offset_of;
        // type(4) + size(4) + config(8) + sample(8) + sample_type(8) + read_format(8) = 40
        assert_eq!(offset_of!(PerfEventAttr, flags), 40);
        // flags(8) + wakeup(4) + bp_type(4) after offset 40 = 56
        assert_eq!(offset_of!(PerfEventAttr, config1), 56);
        assert_eq!(offset_of!(PerfEventAttr, config3), 128);
    }
}
