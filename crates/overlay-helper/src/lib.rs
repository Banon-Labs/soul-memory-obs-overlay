use interprocess::local_socket::{
    prelude::LocalSocketStream, traits::Listener, GenericNamespaced, ListenerOptions, ToNsName,
};
use overlay_proto::{OverlayConfig, OverlayMessage};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait MemoryReader {
    fn read_value(&mut self, cfg: &OverlayConfig) -> Result<i32, String>;
}

pub fn debug_probe(cfg: &OverlayConfig) -> Vec<String> {
    #[cfg(windows)]
    {
        return windows_reader::debug_probe_windows(cfg, &candidate_chains(cfg));
    }

    #[cfg(not(windows))]
    {
        let _ = cfg;
        vec![
            "windows memory probe is unavailable on this binary; run windows target helper"
                .to_string(),
        ]
    }
}

pub struct MockMemoryReader {
    value: i32,
}

impl MockMemoryReader {
    pub fn new(value: i32) -> Self {
        Self { value }
    }
}

impl MemoryReader for MockMemoryReader {
    fn read_value(&mut self, _cfg: &OverlayConfig) -> Result<i32, String> {
        Ok(self.value)
    }
}

pub struct ProcessMemoryReader;

impl MemoryReader for ProcessMemoryReader {
    fn read_value(&mut self, cfg: &OverlayConfig) -> Result<i32, String> {
        read_soul_memory(cfg)
    }
}

pub trait PointerChainMemory {
    fn read_u64(&self, address: u64) -> Result<u64, String>;
    fn read_u32(&self, address: u64) -> Result<u32, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerChainError {
    EmptyChain,
    CharacterUnavailable,
    AddressOverflow,
    ReadFailed(String),
    ValueOutOfRange,
}

pub fn resolve_pointer_chain_i32(
    memory: &dyn PointerChainMemory,
    base: u64,
    chain: &[u64],
    initial_deref: bool,
) -> Result<i32, PointerChainError> {
    if chain.is_empty() {
        return Err(PointerChainError::EmptyChain);
    }

    let mut ptr = if initial_deref {
        let p = memory
            .read_u64(base)
            .map_err(PointerChainError::ReadFailed)?;
        if p == 0 {
            return Err(PointerChainError::CharacterUnavailable);
        }
        p
    } else {
        base
    };

    if chain.len() > 1 {
        for offset in &chain[..chain.len() - 1] {
            let addr = ptr
                .checked_add(*offset)
                .ok_or(PointerChainError::AddressOverflow)?;
            let next = memory
                .read_u64(addr)
                .map_err(PointerChainError::ReadFailed)?;
            if next == 0 {
                return Err(PointerChainError::CharacterUnavailable);
            }
            ptr = next;
        }
    }

    let value_addr = ptr
        .checked_add(*chain.last().expect("chain is non-empty"))
        .ok_or(PointerChainError::AddressOverflow)?;
    let value_u32 = memory
        .read_u32(value_addr)
        .map_err(PointerChainError::ReadFailed)?;

    i32::try_from(value_u32).map_err(|_| PointerChainError::ValueOutOfRange)
}

#[cfg(windows)]
fn candidate_chains(cfg: &OverlayConfig) -> Vec<Vec<u64>> {
    if let Some(chains) = &cfg.memory.soul_memory_chains {
        let filtered: Vec<Vec<u64>> = chains.iter().filter(|c| !c.is_empty()).cloned().collect();
        if !filtered.is_empty() {
            return filtered;
        }
    }

    if !(cfg.memory.pointer_offsets.len() == 1 && cfg.memory.pointer_offsets[0] == 0)
        && !cfg.memory.pointer_offsets.is_empty()
    {
        return vec![cfg.memory.pointer_offsets.clone()];
    }

    vec![vec![0xD0, 0x490, 0xF4], vec![0xD0, 0x490, 0xFC]]
}

fn read_soul_memory(cfg: &OverlayConfig) -> Result<i32, String> {
    #[cfg(windows)]
    {
        return windows_reader::read_soul_memory_windows(cfg, &candidate_chains(cfg));
    }

    #[cfg(not(windows))]
    {
        let _ = cfg;
        Err(
            "windows memory reading is unavailable on this binary; build/run overlay-helper for windows target"
                .to_string(),
        )
    }
}

#[cfg(windows)]
mod windows_reader {
    use super::*;
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
        MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Memory::{
        VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };

    #[derive(Clone, Debug)]
    struct CandidateHit {
        pid: u32,
        base: u64,
        chain: Vec<u64>,
        initial_deref: bool,
        value: i32,
    }

    pub fn read_soul_memory_windows(
        cfg: &OverlayConfig,
        chains: &[Vec<u64>],
    ) -> Result<i32, String> {
        let pids = find_process_ids(&cfg.process.exe);
        if pids.is_empty() {
            return Err(format!("target process '{}' not found", cfg.process.exe));
        }

        let mut successes: Vec<CandidateHit> = Vec::new();

        let mut had_character_unavailable = false;
        let mut other_errors = Vec::new();

        for pid in pids {
            let process = match ProcessHandle::open(pid) {
                Ok(p) => p,
                Err(err) => {
                    other_errors.push(format!("pid {pid}: {err}"));
                    continue;
                }
            };

            let bases = match resolve_game_manager_bases(&process, cfg) {
                Ok(bases) => bases,
                Err(err) => {
                    other_errors.push(format!("pid {pid}: {err}"));
                    continue;
                }
            };

            for base in bases {
                for chain in chains {
                    for initial_deref in [true, false] {
                        match read_chain_value(&process, base, chain, initial_deref) {
                            Ok(value) => successes.push(CandidateHit {
                                pid,
                                base,
                                chain: chain.clone(),
                                initial_deref,
                                value,
                            }),
                            Err(PointerChainError::CharacterUnavailable) => {
                                had_character_unavailable = true
                            }
                            Err(err) => other_errors.push(format!("pid {pid}: {err:?}")),
                        }
                    }
                }
            }
        }

        let fresh_candidates: Vec<CandidateHit> = successes
            .iter()
            .filter(|c| c.initial_deref)
            .cloned()
            .collect();

        if let Some(best) = choose_best(&fresh_candidates) {
            return Ok(best.value);
        }

        if had_character_unavailable {
            return Err(
                "character data unavailable; load into a character/world before reading soul memory"
                    .to_string(),
            );
        }

        if other_errors.is_empty() {
            Err("unable to resolve soul memory from configured candidate chains".to_string())
        } else {
            Err(format!(
                "unable to resolve soul memory: {}",
                other_errors.join(" | ")
            ))
        }
    }

    pub fn debug_probe_windows(cfg: &OverlayConfig, chains: &[Vec<u64>]) -> Vec<String> {
        let pids = find_process_ids(&cfg.process.exe);
        if pids.is_empty() {
            return vec![format!("target process '{}' not found", cfg.process.exe)];
        }

        let mut out = Vec::new();

        for pid in pids {
            let process = match ProcessHandle::open(pid) {
                Ok(p) => p,
                Err(e) => {
                    out.push(format!("pid={pid} open_error={e}"));
                    continue;
                }
            };

            let bases = match resolve_game_manager_bases(&process, cfg) {
                Ok(v) => v,
                Err(e) => {
                    out.push(format!("pid={pid} base_error={e}"));
                    continue;
                }
            };

            out.push(format!("pid={pid} bases_found={}", bases.len()));

            for base in bases {
                for chain in chains {
                    let chain_hex = chain
                        .iter()
                        .map(|o| format!("0x{o:X}"))
                        .collect::<Vec<_>>()
                        .join(",");

                    for initial_deref in [true, false] {
                        match read_chain_value(&process, base, chain, initial_deref) {
                            Ok(v) => out.push(format!(
                                "ok pid={pid} base=0x{base:X} chain=[{chain_hex}] deref={} value={v}",
                                initial_deref
                            )),
                            Err(PointerChainError::CharacterUnavailable) => out.push(format!(
                                "unavailable pid={pid} base=0x{base:X} chain=[{chain_hex}] deref={}",
                                initial_deref
                            )),
                            Err(e) => out.push(format!(
                                "err pid={pid} base=0x{base:X} chain=[{chain_hex}] deref={} msg={}",
                                initial_deref, format!("{e:?}")
                            )),
                        }
                    }
                }
            }
        }

        out
    }

    fn choose_best(candidates: &[CandidateHit]) -> Option<CandidateHit> {
        if candidates.is_empty() {
            return None;
        }

        let mut ranked: Vec<(i32, &CandidateHit)> = candidates
            .iter()
            .map(|c| {
                let mut score = 0;

                if c.initial_deref {
                    score += 3;
                }

                if c.chain == [0xD0, 0x490, 0xF4] || c.chain == [0xD0, 0x490, 0xFC] {
                    score += 4;
                }

                if (1..=1_000_000_000).contains(&c.value) {
                    score += 6;
                }

                if c.value >= 1 {
                    score += 1;
                }

                (score, c)
            })
            .collect();

        ranked.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.value.cmp(&b.1.value))
                .then_with(|| a.1.pid.cmp(&b.1.pid))
                .then_with(|| a.1.base.cmp(&b.1.base))
        });

        ranked.first().map(|(_, c)| (*c).clone())
    }

    fn resolve_game_manager_bases(
        process: &ProcessHandle,
        cfg: &OverlayConfig,
    ) -> Result<Vec<u64>, String> {
        let mut out: Vec<u64> = Vec::new();
        let mut primary_error: Option<String> = None;

        if let Some(addr) = cfg.memory.game_manager_imp {
            if addr > 0 {
                out.push(addr);
            }
        }

        if cfg.memory.base_offset > 0 {
            let module_base = find_module_base(process.pid, &cfg.memory.module)
                .ok_or_else(|| format!("module '{}' not found in process", cfg.memory.module));

            match module_base {
                Ok(module_base) => {
                    let base =
                        module_base
                            .checked_add(cfg.memory.base_offset)
                            .ok_or_else(|| {
                                "base address overflow while applying base_offset".to_string()
                            })?;
                    if !out.contains(&base) {
                        out.push(base);
                    }
                }
                Err(err) => {
                    primary_error = Some(err);
                }
            }
        }

        if cfg.signature.enabled {
            match resolve_game_manager_by_aob(
                process,
                &cfg.memory.module,
                &cfg.signature.pattern,
                cfg.signature.relative_offset,
            ) {
                Ok(sig_bases) => {
                    for base in sig_bases {
                        if !out.contains(&base) {
                            out.push(base);
                        }
                    }
                }
                Err(err) => {
                    if out.is_empty() {
                        return match primary_error {
                            Some(primary) => {
                                Err(format!("{primary}; signature fallback failed: {err}"))
                            }
                            None => Err(err),
                        };
                    }
                }
            }
        }

        if out.is_empty() {
            if let Some(err) = primary_error {
                Err(err)
            } else {
                Err("no configured game manager base and signature fallback disabled".to_string())
            }
        } else {
            Ok(out)
        }
    }

    fn resolve_game_manager_by_aob(
        process: &ProcessHandle,
        module_name: &str,
        aob_pattern: &str,
        relative_offset: i64,
    ) -> Result<Vec<u64>, String> {
        let module = find_module_info(process.pid, module_name)
            .ok_or_else(|| format!("module '{}' not found in process", module_name))?;

        let bytes = read_module_bytes(process.handle, module.base, module.size as usize)?;
        let pattern = parse_aob(aob_pattern)?;

        let hits = find_pattern_all(&bytes, &pattern);
        if hits.is_empty() {
            return Err("GameManagerImp AOB pattern not found in module".to_string());
        }

        let mut out: Vec<u64> = Vec::new();
        for hit in hits {
            if hit + 7 > bytes.len() {
                continue;
            }

            let instr_addr = module.base + hit as u64;
            let disp = i32::from_le_bytes([
                bytes[hit + 3],
                bytes[hit + 4],
                bytes[hit + 5],
                bytes[hit + 6],
            ]) as i64;

            let Some(target) = (instr_addr as i64)
                .checked_add(7)
                .and_then(|v| v.checked_add(disp))
                .and_then(|v| v.checked_add(relative_offset))
            else {
                continue;
            };
            let target = target as u64;
            if !out.contains(&target) {
                out.push(target);
            }
        }

        if out.is_empty() {
            return Err("failed to decode GameManagerImp target from AOB hits".to_string());
        }

        Ok(out)
    }

    fn read_module_bytes(
        handle: HANDLE,
        module_base: u64,
        module_size: usize,
    ) -> Result<Vec<u8>, String> {
        let module_end = module_base
            .checked_add(module_size as u64)
            .ok_or_else(|| "module range overflow while scanning memory".to_string())?;

        let mut out = vec![0u8; module_size];
        let mut cursor = module_base;

        while cursor < module_end {
            let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { zeroed() };
            let queried = unsafe {
                VirtualQueryEx(
                    handle,
                    cursor as *const c_void,
                    &mut mbi,
                    size_of::<MEMORY_BASIC_INFORMATION>(),
                )
            };

            if queried == 0 {
                return Err(format!(
                    "VirtualQueryEx failed while scanning module at 0x{cursor:016X}"
                ));
            }

            let region_base = mbi.BaseAddress as usize as u64;
            let region_size = mbi.RegionSize;
            if region_size == 0 {
                return Err("VirtualQueryEx returned zero-sized region".to_string());
            }

            let region_end = region_base.saturating_add(region_size as u64);
            let overlaps_module = region_end > module_base && region_base < module_end;
            let is_readable = mbi.State == MEM_COMMIT
                && (mbi.Protect & PAGE_NOACCESS) == 0
                && (mbi.Protect & PAGE_GUARD) == 0;

            if overlaps_module && is_readable {
                let read_start = region_base.max(module_base);
                let read_end = region_end.min(module_end);
                if read_end > read_start {
                    let len = (read_end - read_start) as usize;
                    if let Ok(chunk) = read_region(handle, read_start, len) {
                        let dst_offset = (read_start - module_base) as usize;
                        let copy_len = chunk.len().min(len);
                        out[dst_offset..dst_offset + copy_len].copy_from_slice(&chunk[..copy_len]);
                    }
                }
            }

            cursor = region_end;
        }

        Ok(out)
    }

    fn read_chain_value(
        process: &ProcessHandle,
        base: u64,
        chain: &[u64],
        initial_deref: bool,
    ) -> Result<i32, PointerChainError> {
        struct ProcessMemoryView {
            handle: HANDLE,
        }

        impl PointerChainMemory for ProcessMemoryView {
            fn read_u64(&self, address: u64) -> Result<u64, String> {
                read_u64(self.handle, address)
            }

            fn read_u32(&self, address: u64) -> Result<u32, String> {
                read_u32(self.handle, address)
            }
        }

        let view = ProcessMemoryView {
            handle: process.handle,
        };
        resolve_pointer_chain_i32(&view, base, chain, initial_deref)
    }

    fn find_process_ids(exe_name: &str) -> Vec<u32> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return Vec::new();
            }

            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;

            let target = exe_name.to_ascii_lowercase();
            let mut found: Vec<u32> = Vec::new();

            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let current = utf16z_to_string(&entry.szExeFile);
                    if current.eq_ignore_ascii_case(&target) {
                        found.push(entry.th32ProcessID);
                    }

                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
            found
        }
    }

    #[derive(Clone, Copy)]
    struct ModuleInfo {
        base: u64,
        size: u32,
    }

    fn find_module_base(pid: u32, module_name: &str) -> Option<u64> {
        find_module_info(pid, module_name).map(|m| m.base)
    }

    fn find_module_info(pid: u32, module_name: &str) -> Option<ModuleInfo> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
            if snapshot == INVALID_HANDLE_VALUE {
                return None;
            }

            let mut entry: MODULEENTRY32W = zeroed();
            entry.dwSize = size_of::<MODULEENTRY32W>() as u32;

            let target = module_name.to_ascii_lowercase();
            let mut found = None;

            if Module32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let current = utf16z_to_string(&entry.szModule);
                    if current.eq_ignore_ascii_case(&target) {
                        found = Some(ModuleInfo {
                            base: entry.modBaseAddr as usize as u64,
                            size: entry.modBaseSize,
                        });
                        break;
                    }

                    if Module32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
            found
        }
    }

    fn read_region(handle: HANDLE, base: u64, size: usize) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; size];
        unsafe {
            let mut bytes_read: usize = 0;
            let ok = ReadProcessMemory(
                handle,
                base as *const c_void,
                buf.as_mut_ptr() as *mut c_void,
                size,
                &mut bytes_read as *mut usize,
            );

            if ok == 0 {
                return Err(format!("ReadProcessMemory(region) failed at 0x{base:016X}"));
            }

            buf.truncate(bytes_read);
        }
        Ok(buf)
    }

    fn parse_aob(aob: &str) -> Result<Vec<Option<u8>>, String> {
        let mut out = Vec::new();
        for token in aob.split_whitespace() {
            if token == "??" {
                out.push(None);
            } else {
                let byte = u8::from_str_radix(token, 16)
                    .map_err(|_| format!("invalid AOB token '{token}'"))?;
                out.push(Some(byte));
            }
        }

        if out.is_empty() {
            return Err("empty AOB pattern".to_string());
        }

        Ok(out)
    }

    fn find_pattern_all(haystack: &[u8], pattern: &[Option<u8>]) -> Vec<usize> {
        let mut out = Vec::new();
        if pattern.len() > haystack.len() {
            return out;
        }

        'outer: for i in 0..=(haystack.len() - pattern.len()) {
            for (j, p) in pattern.iter().enumerate() {
                if let Some(expected) = p {
                    if haystack[i + j] != *expected {
                        continue 'outer;
                    }
                }
            }
            out.push(i);
        }

        out
    }

    fn read_u64(handle: HANDLE, address: u64) -> Result<u64, String> {
        unsafe {
            let mut value: u64 = 0;
            let mut bytes_read: usize = 0;
            let ok = ReadProcessMemory(
                handle,
                address as *const c_void,
                &mut value as *mut u64 as *mut c_void,
                size_of::<u64>(),
                &mut bytes_read as *mut usize,
            );

            if ok == 0 || bytes_read != size_of::<u64>() {
                return Err(format!("ReadProcessMemory(u64) failed at 0x{address:016X}"));
            }

            Ok(value)
        }
    }

    fn read_u32(handle: HANDLE, address: u64) -> Result<u32, String> {
        unsafe {
            let mut value: u32 = 0;
            let mut bytes_read: usize = 0;
            let ok = ReadProcessMemory(
                handle,
                address as *const c_void,
                &mut value as *mut u32 as *mut c_void,
                size_of::<u32>(),
                &mut bytes_read as *mut usize,
            );

            if ok == 0 || bytes_read != size_of::<u32>() {
                return Err(format!("ReadProcessMemory(u32) failed at 0x{address:016X}"));
            }

            Ok(value)
        }
    }

    fn utf16z_to_string(buf: &[u16]) -> String {
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    struct ProcessHandle {
        handle: HANDLE,
        pid: u32,
    }

    impl ProcessHandle {
        fn open(pid: u32) -> Result<Self, String> {
            unsafe {
                let handle =
                    OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if handle.is_null() {
                    return Err(format!("OpenProcess failed for pid {pid}"));
                }
                Ok(Self { handle, pid })
            }
        }
    }

    impl Drop for ProcessHandle {
        fn drop(&mut self) {
            unsafe {
                if !self.handle.is_null() {
                    CloseHandle(self.handle);
                }
            }
        }
    }
}

pub fn load_config(path: &Path) -> Result<OverlayConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read config failed: {e}"))?;
    OverlayConfig::from_toml(&text).map_err(|e| format!("parse config failed: {e}"))
}

pub fn build_message(
    reader: &mut dyn MemoryReader,
    cfg: &OverlayConfig,
    now_ms: u64,
) -> OverlayMessage {
    match reader.read_value(cfg) {
        Ok(value) => OverlayMessage::ok(value, now_ms),
        Err(err) => OverlayMessage::error(err, now_ms),
    }
}

pub fn now_ms() -> u64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_millis(0));
    dur.as_millis() as u64
}

pub fn run_loop(
    reader: &mut dyn MemoryReader,
    cfg: &OverlayConfig,
    out: &mut dyn Write,
    once: bool,
) -> io::Result<()> {
    loop {
        let msg = build_message(reader, cfg, now_ms());
        log_overlay_message(&msg);
        let line = msg
            .to_line()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        out.write_all(line.as_bytes())?;
        out.flush()?;

        if once {
            return Ok(());
        }

        thread::sleep(Duration::from_millis(cfg.poll.interval_ms));
    }
}

pub fn run_pipe_server(
    reader: &mut dyn MemoryReader,
    cfg: &OverlayConfig,
    once: bool,
) -> io::Result<()> {
    let name = cfg
        .ipc
        .pipe_name
        .as_str()
        .to_ns_name::<GenericNamespaced>()
        .map_err(io::Error::other)?;
    let listener = ListenerOptions::new().name(name).create_sync()?;
    log_stderr(&format!(
        "named pipe server listening on {}",
        cfg.ipc.pipe_name
    ));

    loop {
        let mut stream = match listener.accept() {
            Ok(stream) => {
                log_stderr("pipe client connected");
                stream
            }
            Err(err) => {
                log_stderr(&format!("pipe accept failed: {err}"));
                thread::sleep(Duration::from_millis(250));
                continue;
            }
        };

        if let Err(err) = emit_to_client(reader, cfg, &mut stream, once) {
            log_stderr(&format!("pipe client disconnected: {err}"));
        }

        if once {
            return Ok(());
        }
    }
}

fn emit_to_client(
    reader: &mut dyn MemoryReader,
    cfg: &OverlayConfig,
    stream: &mut LocalSocketStream,
    once: bool,
) -> io::Result<()> {
    loop {
        let msg = build_message(reader, cfg, now_ms());
        log_overlay_message(&msg);
        let line = msg
            .to_line()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        stream.write_all(line.as_bytes())?;
        stream.flush()?;

        if once {
            return Ok(());
        }

        thread::sleep(Duration::from_millis(cfg.poll.interval_ms));
    }
}

fn log_overlay_message(msg: &OverlayMessage) {
    if let Some(err) = &msg.error {
        log_stderr(&format!("memory read error: {err}"));
    }
}

fn log_stderr(message: &str) {
    let ms = now_ms();
    let secs = ms / 1000;
    let millis = ms % 1000;
    eprintln!("[{secs}.{millis:03}] {message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use interprocess::local_socket::{
        prelude::LocalSocketStream, traits::Stream, GenericNamespaced, ToNsName,
    };
    use std::collections::HashMap;
    use std::{
        io::{BufRead, BufReader},
        thread,
        time::Duration,
    };

    fn cfg() -> OverlayConfig {
        OverlayConfig::from_toml(
            r#"
[process]
exe = "missing.exe"

[memory]
module = "missing.exe"
base_offset = 0
pointer_offsets = [0]

[signature]
enabled = true
pattern = "AA BB ?? CC"
relative_offset = 0

[ipc]
pipe_name = "SoulMemoryOverlay"

[poll]
interval_ms = 1
"#,
        )
        .expect("valid config")
    }

    fn cfg_with_pipe(pipe_name: &str) -> OverlayConfig {
        OverlayConfig::from_toml(&format!(
            r#"
[process]
exe = "missing.exe"

[memory]
module = "missing.exe"
base_offset = 0
pointer_offsets = [0]

[signature]
enabled = true
pattern = "AA BB ?? CC"
relative_offset = 0

[ipc]
pipe_name = "{pipe_name}"

[poll]
interval_ms = 1
"#
        ))
        .expect("valid config")
    }

    #[derive(Default)]
    struct MockPointerMemory {
        u64_values: HashMap<u64, u64>,
        u32_values: HashMap<u64, u32>,
    }

    impl PointerChainMemory for MockPointerMemory {
        fn read_u64(&self, address: u64) -> Result<u64, String> {
            self.u64_values
                .get(&address)
                .copied()
                .ok_or_else(|| format!("missing u64 at 0x{address:X}"))
        }

        fn read_u32(&self, address: u64) -> Result<u32, String> {
            self.u32_values
                .get(&address)
                .copied()
                .ok_or_else(|| format!("missing u32 at 0x{address:X}"))
        }
    }

    #[test]
    fn pointer_chain_resolves_with_initial_deref() {
        let mut mem = MockPointerMemory::default();
        mem.u64_values.insert(0x1000, 0x2000);
        mem.u64_values.insert(0x2010, 0x3000);
        mem.u32_values.insert(0x3020, 12345);

        let value = resolve_pointer_chain_i32(&mem, 0x1000, &[0x10, 0x20], true)
            .expect("pointer chain resolves");
        assert_eq!(value, 12_345);
    }

    #[test]
    fn pointer_chain_resolves_without_initial_deref() {
        let mut mem = MockPointerMemory::default();
        mem.u64_values.insert(0x1010, 0x3000);
        mem.u32_values.insert(0x3020, 777);

        let value =
            resolve_pointer_chain_i32(&mem, 0x1000, &[0x10, 0x20], false).expect("chain resolves");
        assert_eq!(value, 777);
    }

    #[test]
    fn pointer_chain_reports_character_unavailable_for_null_pointer() {
        let mut mem = MockPointerMemory::default();
        mem.u64_values.insert(0x1000, 0);

        let err = resolve_pointer_chain_i32(&mem, 0x1000, &[0x10, 0x20], true)
            .expect_err("null pointer should fail");
        assert_eq!(err, PointerChainError::CharacterUnavailable);
    }

    #[test]
    fn pointer_chain_reports_empty_chain() {
        let mem = MockPointerMemory::default();
        let err = resolve_pointer_chain_i32(&mem, 0x1000, &[], true)
            .expect_err("empty chain should fail");
        assert_eq!(err, PointerChainError::EmptyChain);
    }

    #[test]
    fn mock_reader_returns_ok_message() {
        let mut reader = MockMemoryReader::new(77);
        let msg = build_message(&mut reader, &cfg(), 100);
        assert_eq!(msg.value, Some(77));
        assert_eq!(msg.error, None);
    }

    #[test]
    fn process_reader_returns_error_message() {
        let mut reader = ProcessMemoryReader;
        let msg = build_message(&mut reader, &cfg(), 100);
        assert_eq!(msg.value, None);
        assert!(msg.error.is_some());
    }

    #[test]
    fn ipc_integration_emits_json_line() {
        let mut reader = MockMemoryReader::new(11);
        let mut out: Vec<u8> = Vec::new();
        run_loop(&mut reader, &cfg(), &mut out, true).expect("loop runs");
        let line = String::from_utf8(out).expect("utf8");
        let parsed = overlay_proto::OverlayMessage::from_line(&line).expect("json line");
        assert_eq!(parsed.value, Some(11));
    }

    #[test]
    fn named_pipe_ipc_integration_emits_json_line() {
        let pipe_name = format!("SoulMemoryOverlayTest{}", now_ms());
        let cfg = cfg_with_pipe(&pipe_name);

        let client = thread::spawn(move || {
            for _ in 0..100 {
                let namespaced = pipe_name
                    .as_str()
                    .to_ns_name::<GenericNamespaced>()
                    .expect("valid namespaced pipe name");
                if let Ok(stream) = LocalSocketStream::connect(namespaced) {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    let bytes = reader.read_line(&mut line).expect("read from named pipe");
                    assert!(bytes > 0, "named pipe returned a payload line");
                    let parsed =
                        overlay_proto::OverlayMessage::from_line(&line).expect("json line");
                    assert_eq!(parsed.value, Some(44));
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }

            panic!("client connected to helper named pipe");
        });

        let mut reader = MockMemoryReader::new(44);
        let server_result = run_pipe_server(&mut reader, &cfg, true);
        assert!(server_result.is_ok(), "server completed cleanly");

        client.join().expect("client thread join");
    }

    #[test]
    fn ipc_client_reconnects_after_server_restart() {
        let pipe_name = format!("SoulMemoryOverlayReconnect{}", now_ms());

        for expected in [51, 52] {
            let cfg = cfg_with_pipe(&pipe_name);
            let server_cfg = cfg.clone();
            let server = thread::spawn(move || {
                let mut reader = MockMemoryReader::new(expected);
                run_pipe_server(&mut reader, &server_cfg, true)
            });

            let mut stream = None;
            for _ in 0..100 {
                let namespaced = pipe_name
                    .as_str()
                    .to_ns_name::<GenericNamespaced>()
                    .expect("valid namespaced pipe name");
                if let Ok(candidate) = LocalSocketStream::connect(namespaced) {
                    stream = Some(candidate);
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }

            let stream = stream.expect("client connected to helper named pipe");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read from named pipe");
            assert!(bytes > 0, "named pipe returned a payload line");

            let parsed = overlay_proto::OverlayMessage::from_line(&line).expect("json line");
            assert_eq!(parsed.value, Some(expected));

            let server_result = server.join().expect("server thread join");
            assert!(server_result.is_ok(), "server completed cleanly");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn log_timestamp_monotonic_now_ms() {
        let first = now_ms();
        let second = now_ms();
        assert!(second >= first);
    }
}
