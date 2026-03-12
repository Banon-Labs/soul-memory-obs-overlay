#[cfg(windows)]
mod windows_plugin {
    use obs_wrapper::{
        data::DataObj,
        module::{LoadContext, Module, ModuleContext},
        obs_register_module, obs_string,
        properties::{BoolProp, ColorProp, NumberProp, Properties, TextProp, TextType},
        source::{
            traits::{
                ActivateSource, DeactivateSource, GetDefaultsSource, GetHeightSource,
                GetNameSource, GetPropertiesSource, GetWidthSource, Sourceable, UpdateSource,
                VideoRenderSource, VideoTickSource,
            },
            CreatableSourceContext, GlobalContext, Icon, SourceContext, SourceType,
            VideoRenderContext,
        },
        string::ObsString,
    };
    use overlay_proto::{OverlayMessage, Status};
    use std::{
        ffi::CString,
        io::{BufRead, BufReader},
        net::TcpStream,
        panic::{catch_unwind, AssertUnwindSafe},
        path::PathBuf,
        process::{Child, Command, Stdio},
        ptr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    #[cfg(windows)]
    use std::os::windows::process::CommandExt;

    const KEY_PREFIX: &str = "prefix";
    const KEY_PIPE_NAME: &str = "pipe_name";
    const KEY_RECONNECT_MS: &str = "reconnect_ms";
    const KEY_FONT_FACE: &str = "font_face";
    const KEY_FONT_SIZE: &str = "font_size";
    const KEY_TEXT_COLOR: &str = "text_color";
    const KEY_STALE_COLOR: &str = "stale_color";
    const KEY_SHOW_STALE_PREFIX: &str = "show_stale_prefix";
    const KEY_STATUS: &str = "status";
    const KEY_OPEN_CONFIG_FOLDER: &str = "open_config_folder";

    const DEFAULT_PREFIX: &str = "Soul Memory: ";
    const DEFAULT_PIPE_NAME: &str = "SoulMemoryOverlay";
    const DEFAULT_RECONNECT_MS: u64 = 1_000;
    const DEFAULT_FONT_FACE: &str = "Segoe UI";
    const DEFAULT_FONT_SIZE: u64 = 36;
    const DEFAULT_TEXT_COLOR: u64 = 0x00FF_FFFF;
    const DEFAULT_STALE_COLOR: u64 = 0x0000_FFFF;
    const DEFAULT_SHOW_STALE_PREFIX: bool = true;
    const HELPER_BINARY_NAME: &str = "overlay-helper.exe";
    const MODULE_FOLDER_NAME: &str = "soul-memory-obs-overlay";
    const CONFIG_FILE_NAME: &str = "overlay.toml";
    const HELPER_CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const TEXT_EXTENTS_CX: i64 = 600;
    const TEXT_EXTENTS_CY: i64 = 80;

    #[derive(Clone, Debug)]
    enum PayloadState {
        Value(i32),
        Error(String),
        Disconnected,
    }

    #[derive(Clone, Debug)]
    struct OverlayState {
        prefix: String,
        pipe_name: String,
        reconnect_ms: u64,
        font_face: String,
        font_size: u32,
        text_color: u32,
        stale_color: u32,
        show_stale_prefix: bool,
        status_text: String,
        update_status: String,
        value: Option<i32>,
        stale: bool,
        last_error: Option<String>,
        last_update_ms: Option<u64>,
        payload: PayloadState,
        display_text: String,
        dirty: bool,
    }

    impl OverlayState {
        fn summarize_error(message: &str) -> String {
            const MAX_LEN: usize = 96;
            if message.len() <= MAX_LEN {
                return message.to_string();
            }

            let truncated: String = message.chars().take(MAX_LEN).collect();
            format!("{truncated}...")
        }

        fn new(
            prefix: String,
            pipe_name: String,
            reconnect_ms: u64,
            font_face: String,
            font_size: u32,
            text_color: u32,
            stale_color: u32,
            show_stale_prefix: bool,
        ) -> Self {
            let mut state = Self {
                prefix,
                pipe_name,
                reconnect_ms,
                font_face,
                font_size,
                text_color,
                stale_color,
                show_stale_prefix,
                status_text: "Disconnected".to_string(),
                update_status: "checking for updates".to_string(),
                value: None,
                stale: true,
                last_error: None,
                last_update_ms: None,
                payload: PayloadState::Disconnected,
                display_text: String::new(),
                dirty: true,
            };
            state.refresh_display_text();
            state
        }

        fn refresh_display_text(&mut self) {
            self.display_text = match &self.payload {
                PayloadState::Value(value) => {
                    self.status_text = "Connected".to_string();
                    format!("{}{}", self.prefix, value)
                }
                PayloadState::Error(message) => {
                    self.status_text = "Error".to_string();
                    format!("Error: {}", Self::summarize_error(message))
                }
                PayloadState::Disconnected => {
                    self.status_text = "Disconnected".to_string();
                    if self.show_stale_prefix {
                        format!("[STALE] {}", self.prefix)
                    } else {
                        "Disconnected".to_string()
                    }
                }
            };
        }

        fn set_payload(&mut self, payload: PayloadState) {
            let old_text = self.display_text.clone();
            self.payload = payload;
            self.refresh_display_text();
            if self.display_text != old_text {
                self.dirty = true;
            }
        }

        fn set_config(
            &mut self,
            prefix: String,
            pipe_name: String,
            reconnect_ms: u64,
            font_face: String,
            font_size: u32,
            text_color: u32,
            stale_color: u32,
            show_stale_prefix: bool,
        ) {
            let mut changed = false;
            if self.prefix != prefix {
                self.prefix = prefix;
                changed = true;
            }
            if self.pipe_name != pipe_name {
                self.pipe_name = pipe_name;
                changed = true;
            }
            if self.reconnect_ms != reconnect_ms {
                self.reconnect_ms = reconnect_ms;
                changed = true;
            }
            if self.font_face != font_face {
                self.font_face = font_face;
                changed = true;
            }
            if self.font_size != font_size {
                self.font_size = font_size;
                changed = true;
            }
            if self.text_color != text_color {
                self.text_color = text_color;
                changed = true;
            }
            if self.stale_color != stale_color {
                self.stale_color = stale_color;
                changed = true;
            }
            if self.show_stale_prefix != show_stale_prefix {
                self.show_stale_prefix = show_stale_prefix;
                changed = true;
            }
            if changed {
                self.refresh_display_text();
                self.dirty = true;
            }
        }

        fn status_with_update(&self) -> String {
            format!("{} | Update: {}", self.status_text, self.update_status)
        }

        fn apply_message(&mut self, msg: OverlayMessage) {
            self.last_update_ms = Some(msg.timestamp_ms);
            match msg.status {
                Status::Ok => {
                    if let Some(value) = msg.value {
                        self.value = Some(value);
                        self.last_error = None;
                        self.stale = false;
                        self.set_payload(PayloadState::Value(value));
                    } else {
                        self.stale = true;
                        self.last_error = Some("helper returned ok without value".to_string());
                        self.set_payload(PayloadState::Error(
                            "helper returned ok without value".to_string(),
                        ));
                    }
                }
                Status::Error => {
                    let err = msg.error.unwrap_or_else(|| "helper error".to_string());
                    self.stale = true;
                    self.last_error = Some(err.clone());
                    self.set_payload(PayloadState::Error(err));
                }
            }
        }

        fn mark_disconnected(&mut self, reason: Option<String>) {
            self.stale = true;
            self.last_error = reason;
            self.set_payload(PayloadState::Disconnected);
        }
    }

    #[derive(Clone, Debug)]
    struct HelperLaunchConfig {
        helper_path: PathBuf,
        config_path: PathBuf,
        helper_source: &'static str,
        config_source: &'static str,
    }

    impl HelperLaunchConfig {
        fn diagnostics(&self) -> String {
            format!(
                "helper={} [{}], config={} [{}]",
                self.helper_path.display(),
                self.helper_source,
                self.config_path.display(),
                self.config_source
            )
        }
    }

    struct HelperManager {
        stop: Arc<AtomicBool>,
        monitor: Option<thread::JoinHandle<()>>,
    }

    impl HelperManager {
        fn resolve_bundle_paths() -> Result<HelperLaunchConfig, String> {
            let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
            let exe_dir = exe
                .parent()
                .ok_or_else(|| "failed to resolve executable directory".to_string())?;

            let program_data_root = std::env::var_os("PROGRAMDATA").map(PathBuf::from).map(|p| {
                p.join("obs-studio")
                    .join("plugins")
                    .join(MODULE_FOLDER_NAME)
            });

            let mut helper_candidates: Vec<(PathBuf, &'static str)> = vec![
                (exe_dir.join(HELPER_BINARY_NAME), "exe-dir"),
                (
                    exe_dir
                        .join("obs-plugins")
                        .join("64bit")
                        .join(HELPER_BINARY_NAME),
                    "legacy-obs-root-bin",
                ),
                (
                    exe_dir
                        .join("data")
                        .join("obs-plugins")
                        .join(MODULE_FOLDER_NAME)
                        .join(HELPER_BINARY_NAME),
                    "legacy-obs-root-data",
                ),
            ];

            if let Some(program_data) = &program_data_root {
                helper_candidates.push((
                    program_data
                        .join("bin")
                        .join("64bit")
                        .join(HELPER_BINARY_NAME),
                    "programdata-plugin-bin",
                ));
            }

            let (helper_path, helper_source) = helper_candidates
                .iter()
                .find(|(path, _)| path.is_file())
                .map(|(path, source)| (path.clone(), *source))
                .ok_or_else(|| {
                    format!(
                        "could not find bundled helper executable '{}': checked {}",
                        HELPER_BINARY_NAME,
                        helper_candidates
                            .iter()
                            .map(|(path, source)| format!("{} [{}]", path.display(), source))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;

            let helper_dir = helper_path.parent().unwrap_or(exe_dir);
            let appdata_override = std::env::var_os("APPDATA").map(PathBuf::from).map(|p| {
                p.join(MODULE_FOLDER_NAME)
                    .join("config")
                    .join(CONFIG_FILE_NAME)
            });

            let mut config_candidates: Vec<(PathBuf, &'static str)> = Vec::new();

            if let Some(user_cfg) = appdata_override {
                config_candidates.push((user_cfg, "appdata-override"));
            }

            config_candidates.push((
                helper_dir.join("config").join(CONFIG_FILE_NAME),
                "helper-dir-config",
            ));
            config_candidates.push((
                exe_dir.join("config").join(CONFIG_FILE_NAME),
                "exe-dir-config",
            ));
            config_candidates.push((
                exe_dir
                    .join("data")
                    .join("obs-plugins")
                    .join(MODULE_FOLDER_NAME)
                    .join("config")
                    .join(CONFIG_FILE_NAME),
                "legacy-obs-root-data-config",
            ));

            if let Some(program_data) = &program_data_root {
                config_candidates.push((
                    program_data
                        .join("data")
                        .join("config")
                        .join(CONFIG_FILE_NAME),
                    "programdata-plugin-data-config",
                ));
            }

            let (config_path, config_source) = config_candidates
                .iter()
                .find(|(path, _)| path.is_file())
                .map(|(path, source)| (path.clone(), *source))
                .ok_or_else(|| {
                    format!(
                        "could not find bundled config '{}': checked {}",
                        CONFIG_FILE_NAME,
                        config_candidates
                            .iter()
                            .map(|(path, source)| format!("{} [{}]", path.display(), source))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;

            Ok(HelperLaunchConfig {
                helper_path,
                config_path,
                helper_source,
                config_source,
            })
        }

        fn build_command(launch: &HelperLaunchConfig) -> Command {
            let mut command = Command::new(&launch.helper_path);
            command
                .arg("--tcp")
                .arg("--config")
                .arg(&launch.config_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            command.creation_flags(HELPER_CREATE_NO_WINDOW);
            command
        }

        fn spawn_once(launch: &HelperLaunchConfig) -> Result<Child, String> {
            let mut command = Self::build_command(launch);
            command
                .spawn()
                .map_err(|e| format!("spawn helper failed ({}): {e}", launch.diagnostics()))
        }

        fn backoff_delay_ms(attempt: u32) -> u64 {
            let shifted = 100u64.checked_shl(attempt.min(10)).unwrap_or(5_000);
            shifted.min(5_000)
        }

        fn report_helper_error(state: &Arc<Mutex<OverlayState>>, message: String) {
            let mut guard = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.set_payload(PayloadState::Error(message));
        }

        fn start(state: Arc<Mutex<OverlayState>>) -> Self {
            let stop = Arc::new(AtomicBool::new(false));
            let monitor_stop = stop.clone();

            let monitor = thread::spawn(move || {
                let mut failure_count: u32 = 0;
                loop {
                    if monitor_stop.load(Ordering::Relaxed) {
                        return;
                    }

                    let launch = match Self::resolve_bundle_paths() {
                        Ok(v) => v,
                        Err(err) => {
                            Self::report_helper_error(
                                &state,
                                format!("helper path resolution failed: {err}"),
                            );
                            let delay = Self::backoff_delay_ms(failure_count);
                            failure_count = failure_count.saturating_add(1);
                            thread::sleep(Duration::from_millis(delay));
                            continue;
                        }
                    };

                    let mut child = match Self::spawn_once(&launch) {
                        Ok(child) => child,
                        Err(err) => {
                            Self::report_helper_error(&state, err);
                            let delay = Self::backoff_delay_ms(failure_count);
                            failure_count = failure_count.saturating_add(1);
                            thread::sleep(Duration::from_millis(delay));
                            continue;
                        }
                    };

                    let started = Instant::now();
                    loop {
                        if monitor_stop.load(Ordering::Relaxed) {
                            let _ = child.kill();
                            let _ = child.wait();
                            return;
                        }

                        match child.try_wait() {
                            Ok(Some(status)) => {
                                Self::report_helper_error(
                                    &state,
                                    format!(
                                        "helper exited with status: {status} ({})",
                                        launch.diagnostics()
                                    ),
                                );
                                break;
                            }
                            Ok(None) => thread::sleep(Duration::from_millis(250)),
                            Err(err) => {
                                let _ = child.kill();
                                let _ = child.wait();
                                Self::report_helper_error(
                                    &state,
                                    format!(
                                        "helper process status check failed: {err} ({})",
                                        launch.diagnostics()
                                    ),
                                );
                                break;
                            }
                        }
                    }

                    if started.elapsed() >= Duration::from_secs(30) {
                        failure_count = 0;
                    } else {
                        failure_count = failure_count.saturating_add(1);
                    }

                    let delay = Self::backoff_delay_ms(failure_count);
                    thread::sleep(Duration::from_millis(delay));
                }
            });

            Self {
                stop,
                monitor: Some(monitor),
            }
        }
    }

    impl Drop for HelperManager {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(handle) = self.monitor.take() {
                let _ = handle.join();
            }
        }
    }

    struct TextRenderSource {
        raw: *mut obs_wrapper::obs_sys::obs_source_t,
    }

    impl TextRenderSource {
        fn new(initial_text: &str) -> Self {
            let raw = unsafe {
                let data = obs_wrapper::obs_sys::obs_data_create();
                if data.is_null() {
                    ptr::null_mut()
                } else {
                    obs_wrapper::obs_sys::obs_data_set_bool(
                        data,
                        obs_string!("extents").as_ptr(),
                        true,
                    );
                    obs_wrapper::obs_sys::obs_data_set_int(
                        data,
                        obs_string!("extents_cx").as_ptr(),
                        TEXT_EXTENTS_CX,
                    );
                    obs_wrapper::obs_sys::obs_data_set_int(
                        data,
                        obs_string!("extents_cy").as_ptr(),
                        TEXT_EXTENTS_CY,
                    );
                    let source = obs_wrapper::obs_sys::obs_source_create_private(
                        obs_string!("text_gdiplus").as_ptr(),
                        obs_string!("SoulMemoryOverlayText").as_ptr(),
                        data,
                    );
                    obs_wrapper::obs_sys::obs_data_release(data);
                    source
                }
            };

            let mut source = Self { raw };
            source.update_text(
                initial_text,
                DEFAULT_TEXT_COLOR as u32,
                DEFAULT_FONT_FACE,
                DEFAULT_FONT_SIZE as u32,
            );
            source
        }

        fn update_text(&mut self, text: &str, color: u32, font_face: &str, font_size: u32) {
            if self.raw.is_null() {
                return;
            }

            let text = text.replace('\0', " ");
            let Ok(c_text) = CString::new(text) else {
                return;
            };
            let font_face = font_face.replace('\0', " ");
            let Ok(c_face) = CString::new(font_face) else {
                return;
            };

            unsafe {
                let data = obs_wrapper::obs_sys::obs_data_create();
                if data.is_null() {
                    return;
                }

                let font = obs_wrapper::obs_sys::obs_data_create();
                if font.is_null() {
                    obs_wrapper::obs_sys::obs_data_release(data);
                    return;
                }

                obs_wrapper::obs_sys::obs_data_set_string(
                    data,
                    obs_string!("text").as_ptr(),
                    c_text.as_ptr(),
                );
                obs_wrapper::obs_sys::obs_data_set_int(
                    data,
                    obs_string!("color").as_ptr(),
                    color as i64,
                );
                obs_wrapper::obs_sys::obs_data_set_bool(
                    data,
                    obs_string!("extents").as_ptr(),
                    true,
                );
                obs_wrapper::obs_sys::obs_data_set_int(
                    data,
                    obs_string!("extents_cx").as_ptr(),
                    TEXT_EXTENTS_CX,
                );
                obs_wrapper::obs_sys::obs_data_set_int(
                    data,
                    obs_string!("extents_cy").as_ptr(),
                    TEXT_EXTENTS_CY,
                );
                obs_wrapper::obs_sys::obs_data_set_string(
                    font,
                    obs_string!("face").as_ptr(),
                    c_face.as_ptr(),
                );
                obs_wrapper::obs_sys::obs_data_set_int(
                    font,
                    obs_string!("size").as_ptr(),
                    i64::from(font_size),
                );
                obs_wrapper::obs_sys::obs_data_set_obj(data, obs_string!("font").as_ptr(), font);
                obs_wrapper::obs_sys::obs_data_release(font);
                obs_wrapper::obs_sys::obs_source_update(self.raw, data);
                obs_wrapper::obs_sys::obs_data_release(data);
            }
        }

        fn video_render(&self) {
            if self.raw.is_null() {
                return;
            }
            unsafe {
                obs_wrapper::obs_sys::obs_source_video_render(self.raw);
            }
        }
    }

    impl Drop for TextRenderSource {
        fn drop(&mut self) {
            if self.raw.is_null() {
                return;
            }
            unsafe {
                obs_wrapper::obs_sys::obs_source_release(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }

    pub struct SoulMemorySource {
        state: Arc<Mutex<OverlayState>>,
        stop_worker: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
        helper_manager: Option<HelperManager>,
        text_source: TextRenderSource,
        cached_width: u32,
        cached_height: u32,
    }

    impl SoulMemorySource {
        fn lock_state(state: &Arc<Mutex<OverlayState>>) -> std::sync::MutexGuard<'_, OverlayState> {
            state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        fn sleep_interruptible(stop: &AtomicBool, millis: u64) {
            let mut remaining = millis;
            while remaining > 0 {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let chunk = remaining.min(50);
                thread::sleep(Duration::from_millis(chunk));
                remaining -= chunk;
            }
        }

        fn read_string_setting(settings: &DataObj, key: &str, fallback: &str) -> String {
            settings
                .get::<ObsString>(key)
                .map(|v| v.as_str().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| fallback.to_string())
        }

        fn read_reconnect_setting(settings: &DataObj) -> u64 {
            let value = settings
                .get::<i64>(KEY_RECONNECT_MS)
                .unwrap_or(DEFAULT_RECONNECT_MS as i64);
            value.max(100) as u64
        }

        fn read_font_size_setting(settings: &DataObj) -> u32 {
            let value = settings
                .get::<i64>(KEY_FONT_SIZE)
                .unwrap_or(DEFAULT_FONT_SIZE as i64);
            value.clamp(8, 144) as u32
        }

        fn read_color_setting(settings: &DataObj, key: &str, fallback: u64) -> u32 {
            settings
                .get::<i64>(key)
                .map(|v| v.max(0) as u32)
                .unwrap_or(fallback as u32)
        }

        fn read_bool_setting(settings: &DataObj, key: &str, fallback: bool) -> bool {
            settings.get::<bool>(key).unwrap_or(fallback)
        }

        fn preferred_config_dir() -> Option<PathBuf> {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|p| p.join("soul-memory-obs-overlay").join("config"))
        }

        fn open_config_folder() {
            let Some(dir) = Self::preferred_config_dir() else {
                return;
            };
            let _ = std::fs::create_dir_all(&dir);
            let _ = Command::new("explorer.exe")
                .arg(dir.as_os_str())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }

        fn pipe_name_to_port(pipe_name: &str) -> u16 {
            let mut hash: u32 = 0x811C9DC5;
            for b in pipe_name.as_bytes() {
                hash ^= u32::from(*b);
                hash = hash.wrapping_mul(16777619);
            }

            let base: u16 = 20_000;
            let span: u16 = 20_000;
            base + (hash % u32::from(span)) as u16
        }

        fn spawn_update_check(state: Arc<Mutex<OverlayState>>) {
            thread::spawn(move || {
                let output = Command::new("curl.exe")
                    .args([
                        "-fsSL",
                        "-H",
                        "User-Agent: soul-memory-obs-overlay",
                        "https://api.github.com/repos/chozandrias76/soul-memory-obs-overlay/releases/latest",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output();

                let mut guard = state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());

                let Ok(output) = output else {
                    guard.update_status = "unable to check".to_string();
                    return;
                };

                let body = String::from_utf8_lossy(&output.stdout);
                let latest = Self::extract_tag_name(&body);
                let Some(latest) = latest else {
                    guard.update_status = "unable to check".to_string();
                    return;
                };

                let current = env!("CARGO_PKG_VERSION");
                guard.update_status =
                    match (Self::parse_semver(current), Self::parse_semver(&latest)) {
                        (Some(cur), Some(lat)) if lat > cur => {
                            format!("update available ({latest})")
                        }
                        (Some(_), Some(_)) => "up-to-date".to_string(),
                        _ => "unable to check".to_string(),
                    };
            });
        }

        fn extract_tag_name(body: &str) -> Option<String> {
            let key = "\"tag_name\"";
            let key_idx = body.find(key)?;
            let rest = &body[key_idx + key.len()..];
            let colon_idx = rest.find(':')?;
            let after_colon = rest[colon_idx + 1..].trim_start();
            if !after_colon.starts_with('"') {
                return None;
            }

            let without_quote = &after_colon[1..];
            let end_quote = without_quote.find('"')?;
            let tag = &without_quote[..end_quote];
            Some(tag.trim_start_matches('v').to_string())
        }

        fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
            let parts: Vec<&str> = version.split('.').collect();
            if parts.len() < 3 {
                return None;
            }

            let major = parts[0].parse::<u64>().ok()?;
            let minor = parts[1].parse::<u64>().ok()?;
            let patch_part = parts[2].split('-').next()?;
            let patch = patch_part.parse::<u64>().ok()?;
            Some((major, minor, patch))
        }

        fn spawn_worker(
            state: Arc<Mutex<OverlayState>>,
            stop: Arc<AtomicBool>,
        ) -> thread::JoinHandle<()> {
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let outcome = catch_unwind(AssertUnwindSafe(|| {
                        let (pipe_name, reconnect_ms) = {
                            let guard = Self::lock_state(&state);
                            (guard.pipe_name.clone(), guard.reconnect_ms.max(100))
                        };

                        let port = Self::pipe_name_to_port(&pipe_name);
                        let stream = match TcpStream::connect(("127.0.0.1", port)) {
                            Ok(stream) => stream,
                            Err(_) => {
                                let mut guard = Self::lock_state(&state);
                                guard.mark_disconnected(Some(
                                    "helper socket not connected".to_string(),
                                ));
                                Self::sleep_interruptible(&stop, reconnect_ms);
                                return;
                            }
                        };

                        let mut reader = BufReader::new(stream);
                        let mut line = String::new();

                        loop {
                            if stop.load(Ordering::Relaxed) {
                                return;
                            }

                            line.clear();
                            match reader.read_line(&mut line) {
                                Ok(0) => {
                                    let mut guard = Self::lock_state(&state);
                                    guard.mark_disconnected(Some(
                                        "helper socket closed".to_string(),
                                    ));
                                    break;
                                }
                                Ok(_) => match OverlayMessage::from_line(&line) {
                                    Ok(msg) => {
                                        let mut guard = Self::lock_state(&state);
                                        guard.apply_message(msg);
                                    }
                                    Err(err) => {
                                        let mut guard = Self::lock_state(&state);
                                        guard.mark_disconnected(Some(format!(
                                            "invalid IPC payload: {err}"
                                        )));
                                    }
                                },
                                Err(_) => {
                                    let mut guard = Self::lock_state(&state);
                                    guard.mark_disconnected(Some(
                                        "helper socket read failed".to_string(),
                                    ));
                                    break;
                                }
                            }
                        }

                        Self::sleep_interruptible(&stop, reconnect_ms);
                    }));

                    if outcome.is_err() {
                        let reconnect_ms = {
                            let guard = Self::lock_state(&state);
                            guard.reconnect_ms.max(100)
                        };
                        let mut guard = Self::lock_state(&state);
                        guard
                            .set_payload(PayloadState::Error("worker thread panicked".to_string()));
                        Self::sleep_interruptible(&stop, reconnect_ms);
                    }
                }
            })
        }
    }

    impl Drop for SoulMemorySource {
        fn drop(&mut self) {
            self.stop_worker.store(true, Ordering::Relaxed);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
            let _ = self.helper_manager.take();
        }
    }

    impl Sourceable for SoulMemorySource {
        fn get_id() -> ObsString {
            obs_string!("soul_memory_overlay_source")
        }

        fn get_type() -> SourceType {
            SourceType::INPUT
        }

        fn create(create: &mut CreatableSourceContext<Self>, _source: SourceContext) -> Self {
            let prefix = Self::read_string_setting(&create.settings, KEY_PREFIX, DEFAULT_PREFIX);
            let pipe_name =
                Self::read_string_setting(&create.settings, KEY_PIPE_NAME, DEFAULT_PIPE_NAME);
            let reconnect_ms = Self::read_reconnect_setting(&create.settings);
            let font_face =
                Self::read_string_setting(&create.settings, KEY_FONT_FACE, DEFAULT_FONT_FACE);
            let font_size = Self::read_font_size_setting(&create.settings);
            let text_color =
                Self::read_color_setting(&create.settings, KEY_TEXT_COLOR, DEFAULT_TEXT_COLOR);
            let stale_color =
                Self::read_color_setting(&create.settings, KEY_STALE_COLOR, DEFAULT_STALE_COLOR);
            let show_stale_prefix = Self::read_bool_setting(
                &create.settings,
                KEY_SHOW_STALE_PREFIX,
                DEFAULT_SHOW_STALE_PREFIX,
            );

            let state = Arc::new(Mutex::new(OverlayState::new(
                prefix,
                pipe_name,
                reconnect_ms,
                font_face,
                font_size,
                text_color,
                stale_color,
                show_stale_prefix,
            )));
            Self::spawn_update_check(state.clone());
            let stop_worker = Arc::new(AtomicBool::new(false));
            let worker = Some(Self::spawn_worker(state.clone(), stop_worker.clone()));
            let helper_manager = Some(HelperManager::start(state.clone()));

            let initial_text = {
                let guard = Self::lock_state(&state);
                guard.display_text.clone()
            };

            Self {
                state,
                stop_worker,
                worker,
                helper_manager,
                text_source: {
                    let source = TextRenderSource::new(&initial_text);
                    source
                },
                cached_width: 0,
                cached_height: 0,
            }
        }
    }

    impl ActivateSource for SoulMemorySource {
        fn activate(&mut self) {
            if self.helper_manager.is_none() {
                self.helper_manager = Some(HelperManager::start(self.state.clone()));
            }
        }
    }

    impl DeactivateSource for SoulMemorySource {
        fn deactivate(&mut self) {
            let _ = self.helper_manager.take();
        }
    }

    impl GetNameSource for SoulMemorySource {
        fn get_name() -> ObsString {
            obs_string!("Soul Memory Overlay")
        }
    }

    impl GetDefaultsSource for SoulMemorySource {
        fn get_defaults(settings: &mut DataObj) {
            settings.set_default::<ObsString>(KEY_PREFIX, DEFAULT_PREFIX);
            settings.set_default::<ObsString>(KEY_PIPE_NAME, DEFAULT_PIPE_NAME);
            settings.set_default::<i64>(KEY_RECONNECT_MS, DEFAULT_RECONNECT_MS as i64);
            settings.set_default::<ObsString>(KEY_FONT_FACE, DEFAULT_FONT_FACE);
            settings.set_default::<i64>(KEY_FONT_SIZE, DEFAULT_FONT_SIZE as i64);
            settings.set_default::<i64>(KEY_TEXT_COLOR, DEFAULT_TEXT_COLOR as i64);
            settings.set_default::<i64>(KEY_STALE_COLOR, DEFAULT_STALE_COLOR as i64);
            settings.set_default::<bool>(KEY_SHOW_STALE_PREFIX, DEFAULT_SHOW_STALE_PREFIX);
            settings.set_default::<ObsString>(KEY_STATUS, "Disconnected");
            settings.set_default::<bool>(KEY_OPEN_CONFIG_FOLDER, false);
        }
    }

    impl GetPropertiesSource for SoulMemorySource {
        fn get_properties(&mut self) -> Properties {
            let mut props = Properties::new();
            props.add(
                obs_string!("prefix"),
                obs_string!("Label Prefix"),
                TextProp::new(TextType::Default),
            );
            props.add(
                obs_string!("pipe_name"),
                obs_string!("Pipe Name"),
                TextProp::new(TextType::Default),
            );
            props.add(
                obs_string!("reconnect_ms"),
                obs_string!("Reconnect Interval (ms)"),
                NumberProp::new_int().with_range(100..=60_000),
            );
            props.add(
                obs_string!("font_face"),
                obs_string!("Font Face"),
                TextProp::new(TextType::Default),
            );
            props.add(
                obs_string!("font_size"),
                obs_string!("Font Size"),
                NumberProp::new_int().with_range(8..=144),
            );
            props.add(
                obs_string!("text_color"),
                obs_string!("Text Color"),
                ColorProp,
            );
            props.add(
                obs_string!("stale_color"),
                obs_string!("Stale Color"),
                ColorProp,
            );
            props.add(
                obs_string!("show_stale_prefix"),
                obs_string!("Show Stale Prefix"),
                BoolProp,
            );
            let status_label = {
                let guard = self
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.status_with_update()
            };
            props.add(
                obs_string!("status"),
                ObsString::from(status_label),
                TextProp::new(TextType::Default),
            );
            props.add(
                obs_string!("open_config_folder"),
                obs_string!("Open Config Folder (toggle)"),
                BoolProp,
            );
            props
        }
    }

    impl UpdateSource for SoulMemorySource {
        fn update(&mut self, settings: &mut DataObj, _context: &mut GlobalContext) {
            let prefix = Self::read_string_setting(settings, KEY_PREFIX, DEFAULT_PREFIX);
            let pipe_name = Self::read_string_setting(settings, KEY_PIPE_NAME, DEFAULT_PIPE_NAME);
            let reconnect_ms = Self::read_reconnect_setting(settings);
            let font_face = Self::read_string_setting(settings, KEY_FONT_FACE, DEFAULT_FONT_FACE);
            let font_size = Self::read_font_size_setting(settings);
            let text_color = Self::read_color_setting(settings, KEY_TEXT_COLOR, DEFAULT_TEXT_COLOR);
            let stale_color =
                Self::read_color_setting(settings, KEY_STALE_COLOR, DEFAULT_STALE_COLOR);
            let show_stale_prefix =
                Self::read_bool_setting(settings, KEY_SHOW_STALE_PREFIX, DEFAULT_SHOW_STALE_PREFIX);
            let open_config_folder =
                Self::read_bool_setting(settings, KEY_OPEN_CONFIG_FOLDER, false);

            if open_config_folder {
                Self::open_config_folder();
            }

            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.set_config(
                prefix,
                pipe_name,
                reconnect_ms,
                font_face,
                font_size,
                text_color,
                stale_color,
                show_stale_prefix,
            );
        }
    }

    impl VideoTickSource for SoulMemorySource {
        fn video_tick(&mut self, _seconds: f32) {
            let (maybe_text, should_render, color, font_face, font_size) = {
                let mut guard = self
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let should_render = !guard.display_text.is_empty();
                let color = if matches!(guard.payload, PayloadState::Disconnected) {
                    guard.stale_color
                } else {
                    guard.text_color
                };
                let font_face = guard.font_face.clone();
                let font_size = guard.font_size;
                if guard.dirty {
                    guard.dirty = false;
                    (
                        Some(guard.display_text.clone()),
                        should_render,
                        color,
                        font_face,
                        font_size,
                    )
                } else {
                    (None, should_render, color, font_face, font_size)
                }
            };

            if let Some(text) = maybe_text {
                self.text_source
                    .update_text(&text, color, &font_face, font_size);
            }

            if should_render {
                self.cached_width = TEXT_EXTENTS_CX as u32;
                self.cached_height = TEXT_EXTENTS_CY as u32;
            } else {
                self.cached_width = 0;
                self.cached_height = 0;
            }
        }
    }

    impl VideoRenderSource for SoulMemorySource {
        fn video_render(&mut self, _context: &mut GlobalContext, _render: &mut VideoRenderContext) {
            if self.cached_width > 0 && self.cached_height > 0 {
                self.text_source.video_render();
            }
        }
    }

    impl GetWidthSource for SoulMemorySource {
        fn get_width(&mut self) -> u32 {
            self.cached_width
        }
    }

    impl GetHeightSource for SoulMemorySource {
        fn get_height(&mut self) -> u32 {
            self.cached_height
        }
    }

    pub struct OverlayPluginModule {
        ctx: ModuleContext,
    }

    impl Module for OverlayPluginModule {
        fn new(ctx: ModuleContext) -> Self {
            Self { ctx }
        }

        fn get_ctx(&self) -> &ModuleContext {
            &self.ctx
        }

        fn load(&mut self, load_context: &mut LoadContext) -> bool {
            let source = load_context
                .create_source_builder::<SoulMemorySource>()
                .enable_get_name()
                .enable_get_defaults()
                .enable_get_properties()
                .enable_activate()
                .enable_deactivate()
                .enable_update()
                .enable_video_tick()
                .enable_video_render()
                .enable_get_width()
                .enable_get_height()
                .with_icon(Icon::Text)
                .build();

            load_context.register_source(source);
            true
        }

        fn description() -> ObsString {
            obs_string!("Displays Soul Memory data from overlay-helper IPC")
        }

        fn name() -> ObsString {
            obs_string!("Soul Memory OBS Overlay")
        }

        fn author() -> ObsString {
            obs_string!("soul-memory-obs-overlay")
        }
    }

    obs_register_module!(OverlayPluginModule);
}

#[cfg(not(windows))]
mod non_windows_stub {
    use std::os::raw::c_char;

    #[no_mangle]
    pub extern "C" fn obs_module_load() -> bool {
        true
    }

    #[no_mangle]
    pub extern "C" fn obs_module_description() -> *const c_char {
        static DESCRIPTION: &[u8] = b"Soul Memory Overlay plugin (windows-only implementation)\0";
        DESCRIPTION.as_ptr() as *const c_char
    }
}
