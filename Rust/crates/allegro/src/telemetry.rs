//! 系统底层性能与休眠引擎：全屏游戏检测自动休眠 + GPU 占用与实时上下行网速采集（全懒加载零额外链接依赖）。
use std::{
    collections::VecDeque,
    sync::mpsc,
    time::{Duration, Instant},
};
use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::{FreeLibrary, HMODULE},
    System::LibraryLoader::{GetProcAddress, LoadLibraryW},
};

/// 记录动态库所有权，线程退出时先关闭采样句柄，再卸载动态库。
struct Module(HMODULE);

impl Module {
    fn load(name: PCWSTR) -> Option<Self> { unsafe { LoadLibraryW(name).ok().map(Self) } }
}

impl Drop for Module {
    fn drop(&mut self) { unsafe { let _ = FreeLibrary(self.0); } }
}

/// 全屏游戏与视频独占场景守卫：自动降频或跳过渲染以做到零存在感。
pub struct FullscreenGuard {
    last_check: Instant,
    sleeping: bool,
    _module: Option<Module>,
    query_state_fn: Option<unsafe extern "system" fn(*mut i32) -> i32>,
}

impl Default for FullscreenGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl FullscreenGuard {
    pub fn new() -> Self {
        let module = Module::load(windows::core::w!("shell32.dll"));
        let query_state_fn = module.as_ref().and_then(|m| unsafe {
            GetProcAddress(m.0, windows::core::s!("SHQueryUserNotificationState"))
                .map(|p| std::mem::transmute(p))
        });
        Self {
            last_check: Instant::now() - Duration::from_secs(2),
            sleeping: false,
            _module: module,
            query_state_fn,
        }
    }

    /// 每 500ms 刷新一次当前是否处于独占全屏状态（游戏/D3D全屏/演示模式）。
    pub fn should_sleep(&mut self) -> bool {
        if self.last_check.elapsed() < Duration::from_millis(500) {
            return self.sleeping;
        }
        self.last_check = Instant::now();
        let Some(query_fn) = self.query_state_fn else {
            self.sleeping = false;
            return false;
        };
        let mut state: i32 = 0;
        let hr = unsafe { query_fn(&mut state) };
        // 仅在真正的独占全屏游戏(3)或演示模式(4)下休眠；QUNS_BUSY(2)在存在桌宠/浮层时会误触发
        self.sleeping = hr == 0 && matches!(state, 3 | 4);
        self.sleeping
    }
}

/// 30 点环形历史缓冲区，用于系统监控 Pro 波形曲线。
#[derive(Clone, Debug)]
pub struct HistoryRing {
    pub samples: VecDeque<f64>,
    capacity: usize,
}

impl HistoryRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity.max(2)),
            capacity: capacity.max(2),
        }
    }

    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value.max(0.0));
    }

    pub fn peak(&self, floor: f64) -> f64 {
        self.samples.iter().copied().fold(floor, f64::max)
    }
}

/// 系统监控 Pro：汇总 CPU / 内存 / GPU / 实时上下行网速历史。
pub struct ProTelemetry {
    pub cpu_history: HistoryRing,
    pub gpu_history: HistoryRing,
    pub net_rx_history: HistoryRing,
    pub net_tx_history: HistoryRing,
    pub gpu_percent: Option<f64>,
    pub net_rx_kbps: Option<f64>,
    pub net_tx_kbps: Option<f64>,
    last_net: Option<(u64, u64, Instant)>,
    last_poll: Instant,
    worker: Option<Sampler>,
    wake: fn(),
    sleeping: bool,
}

impl Default for ProTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProTelemetry {
    pub fn new() -> Self {
        Self::with_wake(|| {})
    }

    pub fn with_wake(wake: fn()) -> Self {
        Self {
            cpu_history: HistoryRing::new(30),
            gpu_history: HistoryRing::new(30),
            net_rx_history: HistoryRing::new(30),
            net_tx_history: HistoryRing::new(30),
            gpu_percent: None,
            net_rx_kbps: None,
            net_tx_kbps: None,
            last_net: None,
            last_poll: Instant::now() - Duration::from_secs(2),
            worker: None,
            wake,
            sleeping: false,
        }
    }

    /// 保持旧调用入口；系统计数器由后台线程采集。
    pub fn poll(&mut self, cpu_percent: Option<f64>) {
        self.poll_updates(cpu_percent);
    }

    /// 每秒最多提交一次，采样慢时不堆积任务。只在 UI 线程合并历史数据。
    pub fn poll_updates(&mut self, cpu_percent: Option<f64>) -> bool {
        if self.last_poll.elapsed() >= Duration::from_secs(1) {
            self.last_poll = Instant::now();
            if let Some(cpu) = cpu_percent {
                self.cpu_history.push(cpu.clamp(0.0, 100.0));
            }
            if self.worker.is_none() {
                self.worker = Some(Sampler::new(self.wake));
            }
            let _ = self.worker.as_ref().and_then(|sampler| sampler.request.as_ref()).unwrap().try_send(());
        }
        let mut updated = false;
        while let Some(sample) = self.worker.as_ref().and_then(|worker| worker.result.try_recv().ok()) {
            self.apply_sample(sample);
            updated = true;
        }
        updated
    }

    pub fn sleeping(&self) -> bool { self.sleeping }

    fn apply_sample(&mut self, sample: Sample) {
        self.sleeping = sample.sleeping;
        if let Some((rx_total, tx_total)) = sample.net {
            let now = sample.at;
            if let Some((prev_rx, prev_tx, prev_at)) = self.last_net {
                let secs = now.duration_since(prev_at).as_secs_f64().max(0.2);
                let rx_kbps = rx_total.saturating_sub(prev_rx) as f64 / 1024.0 / secs;
                let tx_kbps = tx_total.saturating_sub(prev_tx) as f64 / 1024.0 / secs;
                self.net_rx_kbps = Some(rx_kbps);
                self.net_tx_kbps = Some(tx_kbps);
                self.net_rx_history.push(rx_kbps);
                self.net_tx_history.push(tx_kbps);
            }
            self.last_net = Some((rx_total, tx_total, now));
        }

        if let Some(gpu) = sample.gpu {
            let clamped = gpu.clamp(0.0, 100.0);
            self.gpu_percent = Some(clamped);
            self.gpu_history.push(clamped);
        }
    }
}

struct Sample {
    at: Instant,
    net: Option<(u64, u64)>,
    gpu: Option<f64>,
    sleeping: bool,
}

struct Sampler {
    request: Option<mpsc::SyncSender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
    result: mpsc::Receiver<Sample>,
}

impl Sampler {
    fn new(wake: fn()) -> Self {
        let (results, result) = mpsc::channel();
        // 首次请求也必须能在后台线程尚未开始等待时进入队列。
        let (request, pending) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new().name("allegro-telemetry".into()).spawn(move || {
            let net = NetReader::new();
            let mut gpu = GpuReader::new();
            let mut fullscreen = FullscreenGuard::new();
            while pending.recv().is_ok() {
                let sample = Sample {
                    net: net.total_bytes(),
                    at: Instant::now(),
                    gpu: gpu.sample(),
                    sleeping: fullscreen.should_sleep(),
                };
                if results.send(sample).is_err() { break; }
                wake();
            }
        }).map_err(|err| eprintln!("Allegro telemetry thread: {err}")).ok();
        Self { request: Some(request), worker, result }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.request.take();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

/// 懒加载 `iphlpapi.dll` (`GetIfTable`) 读取物理/活跃网络接口总收发字节。
struct NetReader {
    _module: Option<Module>,
    get_if_table: Option<unsafe extern "system" fn(*mut u8, *mut u32, i32) -> u32>,
}

#[repr(C)]
struct MibIfRowRaw {
    wsz_name: [u16; 256],
    dw_index: u32,
    dw_type: u32,
    dw_mtu: u32,
    dw_speed: u32,
    dw_phys_addr_len: u32,
    b_phys_addr: [u8; 8],
    dw_admin_status: u32,
    dw_oper_status: u32,
    dw_last_change: u32,
    dw_in_octets: u32,
    dw_in_ucast_pkts: u32,
    dw_in_nucast_pkts: u32,
    dw_in_discards: u32,
    dw_in_errors: u32,
    dw_in_unknown_protos: u32,
    dw_out_octets: u32,
    dw_out_ucast_pkts: u32,
    dw_out_nucast_pkts: u32,
    dw_out_discards: u32,
    dw_out_errors: u32,
    dw_out_qlen: u32,
    dw_descr_len: u32,
    b_descr: [u8; 256],
}

impl NetReader {
    fn new() -> Self {
        let module = Module::load(windows::core::w!("iphlpapi.dll"));
        let get_if_table = module.as_ref().and_then(|m| unsafe {
            GetProcAddress(m.0, windows::core::s!("GetIfTable")).map(|p| std::mem::transmute(p))
        });
        Self { _module: module, get_if_table }
    }

    fn total_bytes(&self) -> Option<(u64, u64)> {
        let func = self.get_if_table?;
        let mut size: u32 = 0;
        let _ = unsafe { func(std::ptr::null_mut(), &mut size, 0) };
        if size < 8 || size > 4 * 1024 * 1024 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let ret = unsafe { func(buf.as_mut_ptr(), &mut size, 0) };
        if ret != 0 {
            return None;
        }
        let count = u32::from_le_bytes(buf[0..4].try_into().ok()?) as usize;
        let row_size = std::mem::size_of::<MibIfRowRaw>();
        let mut rx_sum = 0u64;
        let mut tx_sum = 0u64;
        for i in 0..count {
            let offset = 4 + i * row_size;
            if offset + row_size > buf.len() {
                break;
            }
            let row = unsafe { (buf.as_ptr().add(offset) as *const MibIfRowRaw).read_unaligned() };
            // 排除环回接口 (dwType == 24 = MIB_IF_TYPE_LOOPBACK)
            if row.dw_type == 24 {
                continue;
            }
            rx_sum += row.dw_in_octets as u64;
            tx_sum += row.dw_out_octets as u64;
        }
        Some((rx_sum, tx_sum))
    }
}

/// 懒加载 `pdh.dll` 通过 Windows Performance Counters 采集通用 GPU 利用率。
struct GpuReader {
    _module: Option<Module>,
    close_fn: Option<unsafe extern "system" fn(isize) -> u32>,
    query: isize,
    counter: isize,
    collect_fn: Option<unsafe extern "system" fn(isize) -> u32>,
    formatted_fn: Option<unsafe extern "system" fn(isize, u32, *mut u32, *mut PdhFmtCounterValue) -> u32>,
    primed: bool,
}

#[repr(C)]
#[derive(Default)]
struct PdhFmtCounterValue {
    c_status: u32,
    double_value: f64,
}

impl GpuReader {
    fn new() -> Self {
        unsafe {
            let Some(module) = Module::load(windows::core::w!("pdh.dll")) else {
                return Self::empty();
            };
            let m = module.0;
            let Some(close_fn) = GetProcAddress(m, windows::core::s!("PdhCloseQuery")) else {
                return Self::empty();
            };
            let close_fn: unsafe extern "system" fn(isize) -> u32 = std::mem::transmute(close_fn);
            let open_q: Option<unsafe extern "system" fn(*const u16, usize, *mut isize) -> u32> =
                GetProcAddress(m, windows::core::s!("PdhOpenQueryW")).map(|p| std::mem::transmute(p));
            let add_c: Option<unsafe extern "system" fn(isize, *const u16, usize, *mut isize) -> u32> =
                GetProcAddress(m, windows::core::s!("PdhAddEnglishCounterW"))
                    .map(|p| std::mem::transmute(p));
            let collect_fn: Option<unsafe extern "system" fn(isize) -> u32> =
                GetProcAddress(m, windows::core::s!("PdhCollectQueryData"))
                    .map(|p| std::mem::transmute(p));
            let formatted_fn: Option<
                unsafe extern "system" fn(isize, u32, *mut u32, *mut PdhFmtCounterValue) -> u32,
            > = GetProcAddress(m, windows::core::s!("PdhGetFormattedCounterValue"))
                .map(|p| std::mem::transmute(p));

            let (Some(open_q), Some(add_c), Some(collect), Some(fmt)) =
                (open_q, add_c, collect_fn, formatted_fn)
            else {
                return Self::empty();
            };

            let mut query = 0isize;
            if open_q(std::ptr::null(), 0, &mut query) != 0 || query == 0 {
                return Self::empty();
            }
            let path: Vec<u16> = "\\GPU Engine(*)\\Utilization Percentage\0"
                .encode_utf16()
                .collect();
            let mut counter = 0isize;
            if add_c(query, path.as_ptr(), 0, &mut counter) != 0 || counter == 0 {
                let _ = close_fn(query);
                return Self::empty();
            }
            let _ = collect(query);
            Self {
                _module: Some(module),
                close_fn: Some(close_fn),
                query,
                counter,
                collect_fn: Some(collect),
                formatted_fn: Some(fmt),
                primed: true,
            }
        }
    }

    fn empty() -> Self {
        Self {
            _module: None,
            close_fn: None,
            query: 0,
            counter: 0,
            collect_fn: None,
            formatted_fn: None,
            primed: false,
        }
    }

    fn sample(&mut self) -> Option<f64> {
        if !self.primed || self.query == 0 || self.counter == 0 {
            return None;
        }
        let collect = self.collect_fn?;
        let fmt = self.formatted_fn?;
        unsafe {
            if collect(self.query) != 0 {
                return None;
            }
            let mut val = PdhFmtCounterValue::default();
            // PDH_FMT_DOUBLE = 0x00000200
            if fmt(self.counter, 0x0000_0200, std::ptr::null_mut(), &mut val) == 0 && val.c_status == 0 {
                Some(val.double_value)
            } else {
                None
            }
        }
    }
}

impl Drop for GpuReader {
    fn drop(&mut self) {
        if let Some(close) = self.close_fn {
            unsafe { let _ = close(self.query); }
        }
    }
}

/// 格式化速率字符串（自动在 KB/s 与 MB/s 之间切换）。
pub fn format_speed(kbps: f64) -> String {
    if kbps >= 1024.0 {
        format!("{:.1} MB/s", kbps / 1024.0)
    } else if kbps >= 10.0 {
        format!("{:.0} KB/s", kbps)
    } else {
        format!("{:.1} KB/s", kbps)
    }
}
