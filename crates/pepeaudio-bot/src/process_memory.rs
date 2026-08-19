use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Samples the resident memory of this process only.
///
/// `sysinfo` reports bytes currently mapped into physical RAM. On the
/// production Linux host this is RSS; on Windows it is the comparable process
/// working-set measurement. Separate `PostgreSQL` and `Valkey` processes are not
/// included.
pub(crate) struct ProcessMemory {
    pid: Pid,
    system: System,
}

impl ProcessMemory {
    pub(crate) fn new() -> Self {
        Self {
            pid: Pid::from_u32(std::process::id()),
            system: System::new(),
        }
    }

    pub(crate) fn resident_bytes(&mut self) -> Option<u64> {
        let pid = self.pid;
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().without_tasks().with_memory(),
        );
        self.system.process(pid).map(sysinfo::Process::memory)
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessMemory;

    #[test]
    fn current_process_has_resident_memory_on_supported_hosts() {
        if !sysinfo::IS_SUPPORTED_SYSTEM {
            return;
        }

        let bytes = ProcessMemory::new()
            .resident_bytes()
            .expect("current process is visible to sysinfo");

        assert!(bytes > 0);
    }
}
