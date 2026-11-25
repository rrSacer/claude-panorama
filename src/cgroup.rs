use crate::hardware::{LiveHardwareStats, SystemHardwareSpecs};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const PANORAMA_SLICE: &str = "/sys/fs/cgroup/panorama.slice";

const PROTECTED_PROCS: &[&str] = &[
    "claude-panorama",
    "claude-panorama-daemon",
    "traycer-host",
    "traycer-native",
    "traycer",
    "agy",
    "antigravity",
    "codex",
    "claude",
    "opencode",
    "gnome-shell",
    "gnome-session",
    "xwayland",
    "systemd",
    "dbus-daemon",
    "dbus-broker",
    "pipewire",
    "wireplumber",
    "bwrap",
    "sshd",
    "bash",
    "zsh",
    "fish",
    "sh",
    "dash",
    "kitty",
    "gnome-terminal",
    "alacritty",
    "code",
];

const WORKER_PROCS: &[&str] = &[
    "cargo",
    "rustc",
    "rustdoc",
    "clippy-driver",
    "rust-analyzer",
    "cc1",
    "cc1plus",
    "gcc",
    "g++",
    "clang",
    "clang++",
    "ld",
    "ld.lld",
    "lld",
    "mold",
    "collect2",
    "ar",
    "cmake",
    "ctest",
    "ninja",
    "make",
    "zig",
    "java",
    "javac",
    "gradle",
    "mvn",
    "node",
    "npm",
    "pnpm",
    "yarn",
    "bun",
    "vite",
    "webpack",
    "tsc",
    "esbuild",
    "rollup",
    "python",
    "python3",
    "pytest",
    "uvicorn",
    "ffmpeg",
    "blender",
    "qemu-system-x86_64",
];

#[derive(Debug, Clone, Default)]
pub struct TrackedLimits {
    pub path: String,
    pub cpu_cores: Option<u32>,
    pub ssd_percent: Option<u32>,
    pub max_ram_gb: Option<f64>,
}

impl TrackedLimits {
    pub fn has_any_limit(&self) -> bool {
        self.cpu_cores.is_some() || self.ssd_percent.is_some() || self.max_ram_gb.is_some()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct PidSample {
    cpu_ticks: u64,
    io_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct CgroupSample {
    cpu_usec: u64,
    io_bytes: u64,
}

pub struct HardwareMonitor {
    pub specs: SystemHardwareSpecs,
    last_tick: Instant,
    prev_pids: HashMap<u32, PidSample>,
    prev_cgroups: HashMap<String, CgroupSample>,
    pub tracked: HashMap<String, TrackedLimits>,
    pub latest_stats: HashMap<String, LiveHardwareStats>,
}

impl HardwareMonitor {
    pub fn new() -> Self {
        Self {
            specs: SystemHardwareSpecs::detect(),
            last_tick: Instant::now(),
            prev_pids: HashMap::new(),
            prev_cgroups: HashMap::new(),
            tracked: HashMap::new(),
            latest_stats: HashMap::new(),
        }
    }

    pub fn set_limits(
        &mut self,
        slug: &str,
        path: &str,
        cpu_cores: Option<u32>,
        ssd_percent: Option<u32>,
        max_ram_gb: Option<f64>,
    ) -> Result<(), String> {
        let limits = TrackedLimits {
            path: path.to_string(),
            cpu_cores,
            ssd_percent,
            max_ram_gb,
        };
        if limits.has_any_limit() {
            apply_cgroup_limits(slug, path, &limits, &self.specs)?;
            self.tracked.insert(slug.to_string(), limits);
        } else {
            remove_cgroup_limits(slug, &self.specs);
            self.tracked.remove(slug);
        }
        Ok(())
    }

    pub fn remove_limits(&mut self, slug: &str) {
        remove_cgroup_limits(slug, &self.specs);
        self.tracked.remove(slug);
        self.prev_cgroups.remove(slug);
    }

    pub fn sample_projects(&mut self, projects: &[(String, String)]) -> HashMap<String, LiveHardwareStats> {
        let now = Instant::now();
        let elapsed_secs = now.duration_since(self.last_tick).as_secs_f64().max(0.2);
        self.last_tick = now;

        let proc_table = scan_proc_table();
        let mut new_pid_samples: HashMap<u32, PidSample> = HashMap::new();
        let mut out: HashMap<String, LiveHardwareStats> = HashMap::new();
        let clk_tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;

        for (slug, path) in projects {
            let mut matched = match_project_pids(&proc_table, path);
            let slice_pids = read_slice_pids(slug);
            for pid in &slice_pids {
                if !matched.contains(pid) && proc_table.contains_key(pid) {
                    matched.push(*pid);
                }
            }

            let limits = self.tracked.get(slug);
            let enforced = limits.map(|l| l.has_any_limit()).unwrap_or(false);

            // Se há limites ativos, garante que novos PIDs do projeto entrem na cgroup slice
            if enforced {
                let slice_dir = slice_path(slug);
                if slice_dir.is_dir() {
                    let procs_file = slice_dir.join("cgroup.procs");
                    for &pid in &matched {
                        if !slice_pids.contains(&pid) {
                            let _ = fs::write(&procs_file, pid.to_string());
                        }
                    }
                }
            }

            let mut ram_kb: u64 = 0;
            let mut pid_cpu_ticks_delta: u64 = 0;
            let mut pid_io_bytes_delta: u64 = 0;

            for &pid in &matched {
                if let Some(info) = proc_table.get(&pid) {
                    ram_kb = ram_kb.saturating_add(info.rss_kb);
                    let cur = PidSample {
                        cpu_ticks: info.cpu_ticks,
                        io_bytes: info.io_bytes,
                    };
                    if let Some(prev) = self.prev_pids.get(&pid) {
                        pid_cpu_ticks_delta = pid_cpu_ticks_delta
                            .saturating_add(cur.cpu_ticks.saturating_sub(prev.cpu_ticks));
                        pid_io_bytes_delta = pid_io_bytes_delta
                            .saturating_add(cur.io_bytes.saturating_sub(prev.io_bytes));
                    }
                    new_pid_samples.insert(pid, cur);
                }
            }

            let mut cpu_secs = (pid_cpu_ticks_delta as f64) / clk_tck;
            let mut io_bytes_delta = pid_io_bytes_delta;
            let mut ram_used_mb = (ram_kb as f64) / 1024.0;

            if enforced {
                if let Some(cg_sample) = read_cgroup_usage(slug) {
                    if let Some(prev_cg) = self.prev_cgroups.get(slug) {
                        let d_usec = cg_sample.cpu_usec.saturating_sub(prev_cg.cpu_usec);
                        let d_io = cg_sample.io_bytes.saturating_sub(prev_cg.io_bytes);
                        let cg_cpu_secs = (d_usec as f64) / 1_000_000.0;
                        if cg_cpu_secs > cpu_secs {
                            cpu_secs = cg_cpu_secs;
                        }
                        if d_io > io_bytes_delta {
                            io_bytes_delta = d_io;
                        }
                    }
                    self.prev_cgroups.insert(slug.clone(), cg_sample);
                }
                if let Some(cg_ram_mb) = read_cgroup_memory_mb(slug) {
                    if cg_ram_mb > ram_used_mb {
                        ram_used_mb = cg_ram_mb;
                    }
                }
            }

            let cpu_percent = ((cpu_secs / elapsed_secs) / (self.specs.total_cpus as f64) * 100.0)
                .clamp(0.0, 100.0);
            let ssd_mb_s = (io_bytes_delta as f64) / (1024.0 * 1024.0) / elapsed_secs;
            let ssd_max = self.specs.ssd_max_mb_s.max(50.0);
            let ssd_percent = ((ssd_mb_s / ssd_max) * 100.0).clamp(0.0, 100.0);

            let stats = LiveHardwareStats {
                cpu_percent: (cpu_percent * 10.0).round() / 10.0,
                ram_used_mb: (ram_used_mb * 10.0).round() / 10.0,
                ssd_mb_s: (ssd_mb_s * 10.0).round() / 10.0,
                ssd_percent: (ssd_percent * 10.0).round() / 10.0,
                ssd_max_mb_s: self.specs.ssd_max_mb_s,
                active_pids: matched.len(),
                enforced,
            };
            out.insert(slug.clone(), stats);
        }

        self.prev_pids = new_pid_samples;
        self.latest_stats = out.clone();
        out
    }
}

fn slice_path(slug: &str) -> PathBuf {
    let safe: String = slug
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    PathBuf::from(PANORAMA_SLICE).join(format!("panorama-{safe}.slice"))
}

fn ensure_panorama_slice_controllers() -> Result<(), String> {
    if !Path::new(CGROUP_ROOT).join("cgroup.controllers").exists() {
        return Err("cgroup v2 não montado em /sys/fs/cgroup".to_string());
    }
    let _ = fs::write(
        Path::new(CGROUP_ROOT).join("cgroup.subtree_control"),
        "+cpuset +cpu +io +memory",
    );
    if !Path::new(PANORAMA_SLICE).exists() {
        fs::create_dir_all(PANORAMA_SLICE)
            .map_err(|e| format!("Falha ao criar {PANORAMA_SLICE}: {e}"))?;
    }
    let _ = fs::write(
        Path::new(PANORAMA_SLICE).join("cgroup.subtree_control"),
        "+cpuset +cpu +io +memory",
    );
    Ok(())
}

pub fn apply_cgroup_limits(
    slug: &str,
    project_path: &str,
    limits: &TrackedLimits,
    specs: &SystemHardwareSpecs,
) -> Result<(), String> {
    ensure_panorama_slice_controllers()?;
    let dir = slice_path(slug);
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Falha ao criar cgroup `{}`: {e}", dir.display()))?;
    }

    // 1. CPU cores via cpuset.cpus
    let total = specs.total_cpus.max(1);
    let cores = limits.cpu_cores.unwrap_or(total).clamp(1, total);
    let cpuset_val = if cores <= 1 {
        "0".to_string()
    } else {
        format!("0-{}", cores - 1)
    };
    if let Err(e) = fs::write(dir.join("cpuset.cpus"), &cpuset_val) {
        return Err(format!("Falha ao gravar cpuset.cpus ({cpuset_val}): {e}"));
    }

    // 2. Memória RAM via memory.high (90%) + memory.max (100%)
    if let Some(gb) = limits.max_ram_gb {
        let bytes = ((gb * 1024.0 * 1024.0 * 1024.0).round() as u64).max(64 * 1024 * 1024);
        let high = (bytes * 90 / 100).max(48 * 1024 * 1024);
        let _ = fs::write(dir.join("memory.high"), high.to_string());
        fs::write(dir.join("memory.max"), bytes.to_string())
            .map_err(|e| format!("Falha ao gravar memory.max: {e}"))?;
    } else {
        let _ = fs::write(dir.join("memory.high"), "max");
        let _ = fs::write(dir.join("memory.max"), "max");
    }

    // 3. SSD I/O via io.max (<maj>:<min> rbps=.. wbps=..) + io.weight
    if let Some((maj_min, ssd_max)) = resolve_block_device_for_path(project_path, specs.ssd_max_mb_s) {
        if let Some(pct) = limits.ssd_percent.filter(|&p| p < 100) {
            let pct_clamped = pct.clamp(1, 100);
            let bps = ((ssd_max * 1024.0 * 1024.0 * (pct_clamped as f64) / 100.0).round() as u64)
                .max(1024 * 1024);
            let line = format!("{maj_min} rbps={bps} wbps={bps}");
            fs::write(dir.join("io.max"), &line)
                .map_err(|e| format!("Falha ao gravar io.max ({line}): {e}"))?;
            let weight = pct_clamped.clamp(10, 100);
            let _ = fs::write(dir.join("io.weight"), format!("default {weight}"));
        } else {
            let _ = fs::write(dir.join("io.max"), format!("{maj_min} rbps=max wbps=max"));
            let _ = fs::write(dir.join("io.weight"), "default 100");
        }
    }

    // 4. Anexar imediatamente PIDs em execução do projeto
    let proc_table = scan_proc_table();
    let matched = match_project_pids(&proc_table, project_path);
    let procs_file = dir.join("cgroup.procs");
    for pid in matched {
        let _ = fs::write(&procs_file, pid.to_string());
    }

    Ok(())
}

pub fn remove_cgroup_limits(slug: &str, specs: &SystemHardwareSpecs) {
    let dir = slice_path(slug);
    if !dir.is_dir() {
        return;
    }
    let total = specs.total_cpus.max(1);
    let all_cpus = if total <= 1 {
        "0".to_string()
    } else {
        format!("0-{}", total - 1)
    };
    let _ = fs::write(dir.join("cpuset.cpus"), all_cpus);
    let _ = fs::write(dir.join("memory.high"), "max");
    let _ = fs::write(dir.join("memory.max"), "max");
    let _ = fs::write(dir.join("io.weight"), "default 100");
    if let Ok(cur_io) = fs::read_to_string(dir.join("io.max")) {
        for line in cur_io.lines() {
            if let Some(maj_min) = line.split_whitespace().next() {
                let _ = fs::write(dir.join("io.max"), format!("{maj_min} rbps=max wbps=max"));
            }
        }
    }

    // Tenta mover processos de volta para o root cgroup e remover a slice vazia
    let pids = read_slice_pids(slug);
    let root_procs = Path::new(CGROUP_ROOT).join("cgroup.procs");
    for pid in pids {
        let _ = fs::write(&root_procs, pid.to_string());
    }
    let _ = fs::remove_dir(&dir);
}

fn read_slice_pids(slug: &str) -> Vec<u32> {
    let p = slice_path(slug).join("cgroup.procs");
    let Ok(raw) = fs::read_to_string(p) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .collect()
}

fn read_cgroup_usage(slug: &str) -> Option<CgroupSample> {
    let dir = slice_path(slug);
    let cpu_raw = fs::read_to_string(dir.join("cpu.stat")).ok()?;
    let mut cpu_usec: u64 = 0;
    for line in cpu_raw.lines() {
        if let Some(v) = line.strip_prefix("usage_usec ") {
            cpu_usec = v.trim().parse().unwrap_or(0);
            break;
        }
    }

    let mut io_bytes: u64 = 0;
    if let Ok(io_raw) = fs::read_to_string(dir.join("io.stat")) {
        for line in io_raw.lines() {
            for tok in line.split_whitespace().skip(1) {
                if let Some(v) = tok.strip_prefix("rbytes=").or_else(|| tok.strip_prefix("wbytes=")) {
                    io_bytes = io_bytes.saturating_add(v.parse::<u64>().unwrap_or(0));
                }
            }
        }
    }

    Some(CgroupSample { cpu_usec, io_bytes })
}

fn read_cgroup_memory_mb(slug: &str) -> Option<f64> {
    let raw = fs::read_to_string(slice_path(slug).join("memory.current")).ok()?;
    let bytes: f64 = raw.trim().parse().ok()?;
    Some(bytes / (1024.0 * 1024.0))
}

fn resolve_block_device_for_path(path: &str, fallback_speed: f64) -> Option<(String, f64)> {
    let target = if !path.trim().is_empty() && Path::new(path.trim()).exists() {
        path.trim()
    } else {
        "/home/sacer"
    };

    if let Ok(out) = Command::new("findmnt")
        .args(["-no", "SOURCE", "-T", target])
        .output()
    {
        if out.status.success() {
            let src = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let dev_path = src.split('[').next().unwrap_or(&src).trim();
            if let Ok(canon) = fs::canonicalize(dev_path) {
                if let Some(name) = canon.file_name().and_then(|s| s.to_str()) {
                    if let Some(res) = resolve_whole_disk_from_block_name(name, fallback_speed) {
                        return Some(res);
                    }
                }
            }
        }
    }

    // Fallback: primeiro disco físico não-virtual em /sys/block
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("nvme") || name.starts_with("sd") || name.starts_with("vd") {
                if let Ok(maj_min) = fs::read_to_string(e.path().join("dev")) {
                    let speed = disk_speed_from_sysfs(&e.path(), &name, fallback_speed);
                    return Some((maj_min.trim().to_string(), speed));
                }
            }
        }
    }
    None
}

fn resolve_whole_disk_from_block_name(name: &str, fallback_speed: f64) -> Option<(String, f64)> {
    let sys_blk = PathBuf::from("/sys/class/block").join(name);
    let real_blk = fs::canonicalize(&sys_blk).ok()?;
    let disk_dir = if real_blk.join("partition").exists() {
        real_blk.parent()?.to_path_buf()
    } else if let Ok(slaves) = fs::read_dir(real_blk.join("slaves")) {
        if let Some(first_slave) = slaves.flatten().next() {
            let sname = first_slave.file_name().to_string_lossy().to_string();
            return resolve_whole_disk_from_block_name(&sname, fallback_speed);
        }
        real_blk
    } else {
        real_blk
    };

    let maj_min = fs::read_to_string(disk_dir.join("dev")).ok()?;
    let disk_name = disk_dir.file_name().and_then(|s| s.to_str()).unwrap_or(name);
    let speed = disk_speed_from_sysfs(&disk_dir, disk_name, fallback_speed);
    Some((maj_min.trim().to_string(), speed))
}

fn disk_speed_from_sysfs(disk_dir: &Path, name: &str, fallback: f64) -> f64 {
    if name.starts_with("nvme") {
        2000.0
    } else if fs::read_to_string(disk_dir.join("queue/rotational"))
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
    {
        500.0
    } else if name.starts_with("sd") {
        150.0
    } else {
        fallback
    }
}

#[derive(Debug, Clone)]
struct ProcInfo {
    #[allow(dead_code)]
    pid: u32,
    ppid: u32,
    comm: String,
    cwd: String,
    cmdline: String,
    exe: String,
    cpu_ticks: u64,
    rss_kb: u64,
    io_bytes: u64,
}

fn scan_proc_table() -> HashMap<u32, ProcInfo> {
    let mut map = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return map;
    };
    let self_pid = std::process::id();

    for entry in entries.flatten() {
        let fname = entry.file_name();
        let Some(pid_str) = fname.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        if pid <= 2 || pid == self_pid {
            continue;
        }

        let pdir = entry.path();
        let Ok(stat_raw) = fs::read_to_string(pdir.join("stat")) else {
            continue;
        };
        let Some((comm, ppid, cpu_ticks)) = parse_proc_stat(&stat_raw) else {
            continue;
        };

        let cwd = fs::read_link(pdir.join("cwd"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let exe = fs::read_link(pdir.join("exe"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let cmdline = fs::read(pdir.join("cmdline"))
            .map(|bytes| {
                bytes
                    .split(|&b| b == 0)
                    .filter(|s| !s.is_empty())
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        let rss_kb = fs::read_to_string(pdir.join("status"))
            .ok()
            .and_then(|st| {
                st.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse::<u64>().ok())
            })
            .unwrap_or(0);

        let io_bytes = fs::read_to_string(pdir.join("io"))
            .ok()
            .map(|io| {
                let mut total = 0u64;
                for l in io.lines() {
                    if let Some(v) = l.strip_prefix("read_bytes:").or_else(|| l.strip_prefix("write_bytes:")) {
                        total = total.saturating_add(v.trim().parse::<u64>().unwrap_or(0));
                    }
                }
                total
            })
            .unwrap_or(0);

        map.insert(
            pid,
            ProcInfo {
                pid,
                ppid,
                comm,
                cwd,
                cmdline,
                exe,
                cpu_ticks,
                rss_kb,
                io_bytes,
            },
        );
    }

    map
}

fn parse_proc_stat(raw: &str) -> Option<(String, u32, u64)> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    if close <= open {
        return None;
    }
    let comm = raw[open + 1..close].to_string();
    let after = raw[close + 2..].split_whitespace().collect::<Vec<_>>();
    // after[0]=state, after[1]=ppid, after[11]=utime, after[12]=stime
    if after.len() < 13 {
        return None;
    }
    let ppid = after[1].parse::<u32>().ok()?;
    let utime = after[11].parse::<u64>().unwrap_or(0);
    let stime = after[12].parse::<u64>().unwrap_or(0);
    Some((comm, ppid, utime.saturating_add(stime)))
}

fn is_unsafe_project_path(p: &str) -> bool {
    let trimmed = p.trim().trim_end_matches('/');
    trimmed.is_empty()
        || matches!(
            trimmed,
            "/" | "/home" | "/home/sacer" | "/root" | "/tmp" | "/usr" | "/var" | "/etc"
        )
}

fn path_matches_prefix(candidate: &str, project_root: &str) -> bool {
    if candidate == project_root {
        return true;
    }
    if let Some(rest) = candidate.strip_prefix(project_root) {
        return rest.starts_with('/');
    }
    false
}

fn cmdline_mentions_project(cmdline: &str, project_root: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = cmdline[start..].find(project_root) {
        let abs = start + pos;
        let before_ok = abs == 0
            || matches!(
                cmdline.as_bytes()[abs - 1],
                b' ' | b'=' | b':' | b'"' | b'\''
            );
        let after_idx = abs + project_root.len();
        let after_ok = after_idx >= cmdline.len()
            || matches!(
                cmdline.as_bytes()[after_idx],
                b'/' | b' ' | b':' | b'"' | b'\''
            );
        if before_ok && after_ok {
            return true;
        }
        start = abs + project_root.len();
    }
    false
}

fn is_protected_proc(info: &ProcInfo) -> bool {
    let c = info.comm.to_lowercase();
    if PROTECTED_PROCS.iter().any(|p| c == *p) {
        return true;
    }
    let exe_base = Path::new(&info.exe)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    PROTECTED_PROCS.iter().any(|p| exe_base == *p)
}

fn is_worker_or_project_binary(info: &ProcInfo, project_root: &str) -> bool {
    if path_matches_prefix(&info.exe, project_root) {
        return true;
    }
    let c = info.comm.to_lowercase();
    if WORKER_PROCS.iter().any(|w| c == *w) {
        return true;
    }
    let exe_base = Path::new(&info.exe)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    WORKER_PROCS.iter().any(|w| exe_base == *w)
}

fn match_project_pids(table: &HashMap<u32, ProcInfo>, raw_path: &str) -> Vec<u32> {
    if is_unsafe_project_path(raw_path) {
        return Vec::new();
    }
    let canon = fs::canonicalize(raw_path.trim())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| raw_path.trim().trim_end_matches('/').to_string());
    if is_unsafe_project_path(&canon) {
        return Vec::new();
    }

    let mut matched: HashSet<u32> = HashSet::new();
    for (&pid, info) in table {
        if is_protected_proc(info) {
            continue;
        }
        if !is_worker_or_project_binary(info, &canon) {
            continue;
        }
        if path_matches_prefix(&info.cwd, &canon)
            || path_matches_prefix(&info.exe, &canon)
            || cmdline_mentions_project(&info.cmdline, &canon)
        {
            matched.insert(pid);
        }
    }

    // Propaga para filhos de processos já casados (ex: cargo -> rustc -> cc1 -> ld)
    loop {
        let mut added = false;
        for (&pid, info) in table {
            if matched.contains(&pid) || is_protected_proc(info) {
                continue;
            }
            if matched.contains(&info.ppid) {
                matched.insert(pid);
                added = true;
            }
        }
        if !added {
            break;
        }
    }

    let mut out: Vec<u32> = matched.into_iter().collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proc_stat_handles_spaces_and_parens() {
        let raw = "1234 (cargo build (x)) S 100 100 100 0 -1 4194304 10 0 0 0 250 50 0 0 20 0 4 0 1000";
        let (comm, ppid, ticks) = parse_proc_stat(raw).unwrap();
        assert_eq!(comm, "cargo build (x)");
        assert_eq!(ppid, 100);
        assert_eq!(ticks, 300);
    }

    #[test]
    fn unsafe_paths_never_match_and_prefix_respects_boundaries() {
        assert!(is_unsafe_project_path("/"));
        assert!(is_unsafe_project_path("/home/sacer"));
        assert!(path_matches_prefix("/home/sacer/Projects/foo/src", "/home/sacer/Projects/foo"));
        assert!(!path_matches_prefix("/home/sacer/Projects/foo-bar", "/home/sacer/Projects/foo"));
        assert!(cmdline_mentions_project(
            "cargo build --manifest-path /home/sacer/Projects/foo/Cargo.toml",
            "/home/sacer/Projects/foo"
        ));
        assert!(!cmdline_mentions_project(
            "cargo build --manifest-path /home/sacer/Projects/foo-2/Cargo.toml",
            "/home/sacer/Projects/foo"
        ));
    }
}
