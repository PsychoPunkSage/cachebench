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

/// Opens a stopped counter for hardware `event` on PMU `pmu_type`, counting the calling
/// thread in user mode on any CPU. The returned fd closes itself when dropped.
pub fn open(pmu_type: u32, event: u64) -> Result<OwnedFd, String> {
    let pea = PerfEventAttr {
        r#type: PERF_TYPE_HARDWARE,
        size: size_of::<PerfEventAttr>() as u32,
        config: (pmu_type as u64) << PERF_PMU_TYPE_SHIFT | event,
        flags: DISABLED | EXCLUDE_KERNEL | EXCLUDE_HV,
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
            -1 as libc::c_int,
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

/// Reads the counter's current value.
pub fn read_count(fd: &OwnedFd) -> Result<u64, String> {
    let mut val: u64 = 0;

    // SAFETY: `val` is a live, writable 8-byte buffer and we ask for exactly 8 bytes.
    let n = unsafe { libc::read(fd.as_raw_fd(), &mut val as *mut u64 as *mut libc::c_void, 8) };

    match n {
        8 => Ok(val),
        -1 => Err(format!("Read error: {}", std::io::Error::last_os_error())),
        _ => Err(format!("short read: got {n} of 8 bytes")),
    }
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
