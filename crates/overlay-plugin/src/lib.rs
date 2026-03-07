#[cfg(windows)]
mod windows_plugin {
    use interprocess::local_socket::{
        prelude::LocalSocketStream, traits::Stream, GenericNamespaced, ToNsName,
    };
    use obs_wrapper::{
        data::DataObj,
        module::{LoadContext, Module, ModuleContext},
        obs_register_module, obs_string,
        properties::{NumberProp, Properties, TextProp, TextType},
        source::{
            traits::{
                GetDefaultsSource, GetHeightSource, GetNameSource, GetPropertiesSource,
                GetWidthSource, Sourceable, UpdateSource, VideoRenderSource, VideoTickSource,
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
        panic::{catch_unwind, AssertUnwindSafe},
        ptr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
        time::Duration,
    };

    const KEY_PREFIX: &str = "prefix";
    const KEY_PIPE_NAME: &str = "pipe_name";
    const KEY_RECONNECT_MS: &str = "reconnect_ms";

    const DEFAULT_PREFIX: &str = "Soul Memory: ";
    const DEFAULT_PIPE_NAME: &str = "SoulMemoryOverlay";
    const DEFAULT_RECONNECT_MS: u64 = 1_000;
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

        fn new(prefix: String, pipe_name: String, reconnect_ms: u64) -> Self {
            let mut state = Self {
                prefix,
                pipe_name,
                reconnect_ms,
                payload: PayloadState::Disconnected,
                display_text: String::new(),
                dirty: true,
            };
            state.refresh_display_text();
            state
        }

        fn refresh_display_text(&mut self) {
            self.display_text = match &self.payload {
                PayloadState::Value(value) => format!("{}{}", self.prefix, value),
                PayloadState::Error(message) => {
                    format!("Error: {}", Self::summarize_error(message))
                }
                PayloadState::Disconnected => "Disconnected".to_string(),
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

        fn set_config(&mut self, prefix: String, pipe_name: String, reconnect_ms: u64) {
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
            if changed {
                self.refresh_display_text();
                self.dirty = true;
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
            source.update_text(initial_text);
            source
        }

        fn update_text(&mut self, text: &str) {
            if self.raw.is_null() {
                return;
            }

            let text = text.replace('\0', " ");
            let Ok(c_text) = CString::new(text) else {
                return;
            };

            unsafe {
                let data = obs_wrapper::obs_sys::obs_data_create();
                if data.is_null() {
                    return;
                }

                obs_wrapper::obs_sys::obs_data_set_string(
                    data,
                    obs_string!("text").as_ptr(),
                    c_text.as_ptr(),
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

                        let namespaced = match pipe_name.as_str().to_ns_name::<GenericNamespaced>()
                        {
                            Ok(name) => name,
                            Err(err) => {
                                let mut guard = Self::lock_state(&state);
                                guard.set_payload(PayloadState::Error(format!(
                                    "invalid pipe name '{pipe_name}': {err}"
                                )));
                                Self::sleep_interruptible(&stop, reconnect_ms);
                                return;
                            }
                        };

                        let stream = match LocalSocketStream::connect(namespaced) {
                            Ok(stream) => stream,
                            Err(_) => {
                                let mut guard = Self::lock_state(&state);
                                guard.set_payload(PayloadState::Disconnected);
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
                                    guard.set_payload(PayloadState::Disconnected);
                                    break;
                                }
                                Ok(_) => match OverlayMessage::from_line(&line) {
                                    Ok(msg) => {
                                        let mut guard = Self::lock_state(&state);
                                        match msg.status {
                                            Status::Ok => {
                                                if let Some(value) = msg.value {
                                                    guard.set_payload(PayloadState::Value(value));
                                                } else {
                                                    guard.set_payload(PayloadState::Error(
                                                        "helper returned ok without value"
                                                            .to_string(),
                                                    ));
                                                }
                                            }
                                            Status::Error => {
                                                let err = msg
                                                    .error
                                                    .unwrap_or_else(|| "helper error".to_string());
                                                guard.set_payload(PayloadState::Error(err));
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        let mut guard = Self::lock_state(&state);
                                        guard.set_payload(PayloadState::Error(format!(
                                            "invalid IPC payload: {err}"
                                        )));
                                    }
                                },
                                Err(_) => {
                                    let mut guard = Self::lock_state(&state);
                                    guard.set_payload(PayloadState::Disconnected);
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
            let _ = self.worker.take();
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

            let state = Arc::new(Mutex::new(OverlayState::new(
                prefix,
                pipe_name,
                reconnect_ms,
            )));
            let stop_worker = Arc::new(AtomicBool::new(false));
            let worker = Some(Self::spawn_worker(state.clone(), stop_worker.clone()));

            let initial_text = {
                let guard = Self::lock_state(&state);
                guard.display_text.clone()
            };

            Self {
                state,
                stop_worker,
                worker,
                text_source: {
                    let source = TextRenderSource::new(&initial_text);
                    source
                },
                cached_width: 0,
                cached_height: 0,
            }
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
            props
        }
    }

    impl UpdateSource for SoulMemorySource {
        fn update(&mut self, settings: &mut DataObj, _context: &mut GlobalContext) {
            let prefix = Self::read_string_setting(settings, KEY_PREFIX, DEFAULT_PREFIX);
            let pipe_name = Self::read_string_setting(settings, KEY_PIPE_NAME, DEFAULT_PIPE_NAME);
            let reconnect_ms = Self::read_reconnect_setting(settings);

            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.set_config(prefix, pipe_name, reconnect_ms);
        }
    }

    impl VideoTickSource for SoulMemorySource {
        fn video_tick(&mut self, _seconds: f32) {
            let (maybe_text, should_render) = {
                let mut guard = self
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let should_render = !guard.display_text.is_empty();
                if guard.dirty {
                    guard.dirty = false;
                    (Some(guard.display_text.clone()), should_render)
                } else {
                    (None, should_render)
                }
            };

            if let Some(text) = maybe_text {
                self.text_source.update_text(&text);
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
