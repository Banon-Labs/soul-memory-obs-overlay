use overlay_proto::{OverlayConfig, OverlayMessage};
use std::fs;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
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
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };

    const GAME_MANAGER_AOB: &str = "48 8B 05 ?? ?? ?? ?? 48 8B 58 38 48 85 DB 74 ?? F6";

    #[derive(Debug)]
    enum ChainError {
        CharacterUnavailable,
        Generic(String),
    }

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
                            Err(ChainError::CharacterUnavailable) => {
                                had_character_unavailable = true
                            }
                            Err(ChainError::Generic(err)) => {
                                other_errors.push(format!("pid {pid}: {err}"))
                            }
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
                            Err(ChainError::CharacterUnavailable) => out.push(format!(
                                "unavailable pid={pid} base=0x{base:X} chain=[{chain_hex}] deref={}",
                                initial_deref
                            )),
                            Err(ChainError::Generic(e)) => out.push(format!(
                                "err pid={pid} base=0x{base:X} chain=[{chain_hex}] deref={} msg={}",
                                initial_deref, e
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
        if let Some(addr) = cfg.memory.game_manager_imp {
            if addr > 0 {
                return Ok(vec![addr]);
            }
        }

        if cfg.memory.base_offset > 0 {
            let module_base = find_module_base(process.pid, &cfg.memory.module)
                .ok_or_else(|| format!("module '{}' not found in process", cfg.memory.module))?;
            let base = module_base
                .checked_add(cfg.memory.base_offset)
                .ok_or_else(|| "base address overflow while applying base_offset".to_string());
            return base.map(|b| vec![b]);
        }

        resolve_game_manager_by_aob(process, &cfg.memory.module)
    }

    fn resolve_game_manager_by_aob(
        process: &ProcessHandle,
        module_name: &str,
    ) -> Result<Vec<u64>, String> {
        let module = find_module_info(process.pid, module_name)
            .ok_or_else(|| format!("module '{}' not found in process", module_name))?;

        let bytes = read_region(process.handle, module.base, module.size as usize)?;
        let pattern = parse_aob(GAME_MANAGER_AOB)?;

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

    fn read_chain_value(
        process: &ProcessHandle,
        base: u64,
        chain: &[u64],
        initial_deref: bool,
    ) -> Result<i32, ChainError> {
        if chain.is_empty() {
            return Err(ChainError::Generic("empty candidate chain".to_string()));
        }

        let mut ptr = if initial_deref {
            let p = read_u64(process.handle, base).map_err(ChainError::Generic)?;
            if p == 0 {
                return Err(ChainError::CharacterUnavailable);
            }
            p
        } else {
            base
        };

        if chain.len() > 1 {
            for offset in &chain[..chain.len() - 1] {
                let addr = ptr.checked_add(*offset).ok_or_else(|| {
                    ChainError::Generic("address overflow while resolving chain".to_string())
                })?;
                let next = read_u64(process.handle, addr).map_err(ChainError::Generic)?;
                if next == 0 {
                    return Err(ChainError::CharacterUnavailable);
                }
                ptr = next;
            }
        }

        let value_addr = ptr.checked_add(*chain.last().unwrap()).ok_or_else(|| {
            ChainError::Generic("address overflow while resolving value address".to_string())
        })?;

        let value_u32 = read_u32(process.handle, value_addr).map_err(ChainError::Generic)?;
        i32::try_from(value_u32)
            .map_err(|_| ChainError::Generic("soul memory value exceeds i32 range".to_string()))
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

pub fn pipe_name_to_port(pipe_name: &str) -> u16 {
    let mut hash: u32 = 2166136261;
    for b in pipe_name.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    40000 + (hash % 20000) as u16
}

pub fn run_tcp_server(
    reader: &mut dyn MemoryReader,
    cfg: &OverlayConfig,
    once: bool,
) -> io::Result<()> {
    let port = pipe_name_to_port(&cfg.ipc.pipe_name);
    let bind_addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&bind_addr)?;

    loop {
        let (mut stream, _) = listener.accept()?;
        emit_to_client(reader, cfg, &mut stream, once)?;
        if once {
            return Ok(());
        }
    }
}

fn emit_to_client(
    reader: &mut dyn MemoryReader,
    cfg: &OverlayConfig,
    stream: &mut TcpStream,
    once: bool,
) -> io::Result<()> {
    loop {
        let msg = build_message(reader, cfg, now_ms());
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn pipe_name_maps_to_stable_port() {
        assert_eq!(
            pipe_name_to_port("SoulMemoryOverlay"),
            pipe_name_to_port("SoulMemoryOverlay")
        );
    }
}
