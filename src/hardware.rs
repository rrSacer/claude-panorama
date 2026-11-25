use crate::model::ProjectDocument;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemHardwareSpecs {
    pub total_cpus: u32,
    pub total_ram_gb: f64,
    pub ssd_max_mb_s: f64,
}

impl Default for SystemHardwareSpecs {
    fn default() -> Self {
        Self::detect()
    }
}

impl SystemHardwareSpecs {
    pub fn detect() -> Self {
        let total_cpus = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4)
            .max(1);
        let total_ram_gb = read_total_ram_gb().unwrap_or(16.0);
        let ssd_max_mb_s = detect_default_ssd_mb_s();
        Self {
            total_cpus,
            total_ram_gb,
            ssd_max_mb_s,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LiveHardwareStats {
    pub cpu_percent: f64,
    pub ram_used_mb: f64,
    pub ssd_mb_s: f64,
    pub ssd_percent: f64,
    pub ssd_max_mb_s: f64,
    pub active_pids: usize,
    pub enforced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveHardwareNeeds {
    pub show_cpu: bool,
    pub show_ram: bool,
    pub show_ssd: bool,
    pub reason: String,
}

impl EffectiveHardwareNeeds {
    pub fn any(&self) -> bool {
        self.show_cpu || self.show_ram || self.show_ssd
    }
}

pub fn detect_project_hardware(
    doc: &ProjectDocument,
    live: Option<&LiveHardwareStats>,
) -> EffectiveHardwareNeeds {
    let explicit = doc.hardware.as_ref();
    let (auto_cpu, auto_ram, auto_ssd, auto_reason) = detect_static_profile(doc);

    let mut show_cpu = explicit.and_then(|h| h.heavy_cpu).unwrap_or(auto_cpu);
    let mut show_ram = explicit.and_then(|h| h.heavy_ram).unwrap_or(auto_ram);
    let mut show_ssd = explicit.and_then(|h| h.heavy_ssd).unwrap_or(auto_ssd);

    // Se o usuário já configurou um limite explícito, mantenha aquele controle visível
    if explicit.and_then(|h| h.cpu_cores).is_some() {
        show_cpu = true;
    }
    if explicit.and_then(|h| h.max_ram_gb).is_some() {
        show_ram = true;
    }
    if explicit.and_then(|h| h.ssd_percent).is_some() {
        show_ssd = true;
    }

    let mut live_notes: Vec<&str> = Vec::new();
    if let Some(st) = live {
        if st.active_pids > 0 {
            if st.cpu_percent >= 25.0 && explicit.and_then(|h| h.heavy_cpu).is_none() {
                show_cpu = true;
                live_notes.push("uso alto de CPU detectado agora");
            }
            if st.ram_used_mb >= 1024.0 && explicit.and_then(|h| h.heavy_ram).is_none() {
                show_ram = true;
                live_notes.push("uso alto de RAM detectado agora");
            }
            if st.ssd_mb_s >= 15.0 && explicit.and_then(|h| h.heavy_ssd).is_none() {
                show_ssd = true;
                live_notes.push("uso intenso de SSD detectado agora");
            }
        }
    }

    let reason = if let Some(r) = explicit.map(|h| h.reason.trim()).filter(|s| !s.is_empty()) {
        r.to_string()
    } else if !live_notes.is_empty() && auto_reason.is_empty() {
        live_notes.join(" · ")
    } else {
        auto_reason
    };

    EffectiveHardwareNeeds {
        show_cpu,
        show_ram,
        show_ssd,
        reason,
    }
}

fn detect_static_profile(doc: &ProjectDocument) -> (bool, bool, bool, String) {
    let p_str = doc.path.trim();
    if !p_str.is_empty() {
        let root = Path::new(p_str);
        if root.is_dir() {
            // 1. Rust (Cargo.toml) -> CPU + RAM + SSD pesados
            if root.join("Cargo.toml").is_file() {
                return (
                    true,
                    true,
                    true,
                    "Projeto Rust: compilação paralela usa bastante CPU, RAM e SSD".to_string(),
                );
            }
            // 2. C/C++/Zig/CMake/Meson -> CPU + RAM + SSD pesados
            if root.join("CMakeLists.txt").is_file()
                || root.join("meson.build").is_file()
                || root.join("build.zig").is_file()
            {
                return (
                    true,
                    true,
                    true,
                    "Compilação nativa (C/C++/Zig): uso intenso de CPU, RAM e disco".to_string(),
                );
            }
            // 3. Java / Gradle / Maven / Modpack -> CPU + RAM pesados
            if root.join("build.gradle").is_file()
                || root.join("build.gradle.kts").is_file()
                || root.join("pom.xml").is_file()
                || root.join("gradlew").is_file()
            {
                return (
                    true,
                    true,
                    false,
                    "Projeto JVM/Gradle: uso pesado de CPU e RAM".to_string(),
                );
            }
            // 4. Extensões GNOME / navegador / scripts leves
            if root.join("metadata.json").is_file() && !root.join("package.json").is_file() {
                return (false, false, false, String::new());
            }
            if root.join("manifest.json").is_file() && !root.join("Cargo.toml").is_file() {
                return (false, false, false, String::new());
            }
            // 5. Node/JS com bundler ou Electron
            let pkg_path = root.join("package.json");
            if pkg_path.is_file() {
                if let Ok(pkg) = fs::read_to_string(&pkg_path) {
                    let lower = pkg.to_lowercase();
                    if lower.contains("electron")
                        || lower.contains("webpack")
                        || lower.contains("vite")
                        || lower.contains("next")
                        || lower.contains("typescript")
                    {
                        return (
                            true,
                            true,
                            false,
                            "Projeto JS/TS com bundler/runtime: usa bastante CPU e RAM".to_string(),
                        );
                    }
                }
                return (false, false, false, String::new());
            }
            // 6. Python com ML/visão computacional/vídeo pesado
            for req in ["requirements.txt", "pyproject.toml"] {
                let rp = root.join(req);
                if rp.is_file() {
                    if let Ok(txt) = fs::read_to_string(&rp) {
                        let lower = txt.to_lowercase();
                        let has_ml = lower.contains("torch")
                            || lower.contains("tensorflow")
                            || lower.contains("onnx")
                            || lower.contains("cuda");
                        let has_cv = lower.contains("opencv") || lower.contains("mediapipe");
                        if has_ml {
                            return (
                                true,
                                true,
                                true,
                                "Projeto de IA/ML em Python: uso alto de CPU, RAM e disco".to_string(),
                            );
                        }
                        if has_cv {
                            return (
                                true,
                                false,
                                false,
                                "Visão computacional em tempo real: uso contínuo de CPU".to_string(),
                            );
                        }
                    }
                }
            }
        }
    }

    // Fallback por pistas no texto (ex: projeto cujo código está remoto/pausado)
    let hay = format!(
        "{} {} {} {}",
        doc.slug, doc.title, doc.subtitle, doc.phase
    )
    .to_lowercase();
    if hay.contains("rust") || hay.contains("cargo") {
        return (
            true,
            true,
            true,
            "Projeto Rust: compilação paralela usa bastante CPU, RAM e SSD".to_string(),
        );
    }
    if hay.contains("modpack") || hay.contains("minecraft") || hay.contains("gradle") {
        return (
            true,
            true,
            false,
            "Projeto Java/Minecraft: uso intenso de CPU e RAM".to_string(),
        );
    }

    (false, false, false, String::new())
}

fn read_total_ram_gb() -> Option<f64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: f64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some((kb / 1024.0 / 1024.0 * 10.0).round() / 10.0);
        }
    }
    None
}

fn detect_default_ssd_mb_s() -> f64 {
    // Verifica se / é NVMe ou SATA SSD (padrão 500 MB/s SATA, 2000 MB/s NVMe, 150 MB/s HDD)
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("nvme") {
                return 2000.0;
            }
        }
    }
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("sd") {
                let rot_path = e.path().join("queue/rotational");
                if fs::read_to_string(rot_path).map(|s| s.trim() == "0").unwrap_or(false) {
                    return 500.0;
                }
                return 150.0;
            }
        }
    }
    500.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HardwareConfig, ProjectStatus};

    fn dummy_doc(slug: &str, path: &str, subtitle: &str) -> ProjectDocument {
        ProjectDocument {
            slug: slug.to_string(),
            title: slug.to_string(),
            subtitle: subtitle.to_string(),
            path: path.to_string(),
            icon: "utilities-terminal-symbolic".to_string(),
            status: ProjectStatus::Ativo,
            phase: String::new(),
            updated_at: String::new(),
            progresso: 0.0,
            progresso_delta: None,
            progresso_calculo: String::new(),
            progresso_subtitulo: String::new(),
            confianca: 50.0,
            confianca_delta: None,
            confianca_resumo: String::new(),
            confianca_fatores: Vec::new(),
            confianca_riscos: Vec::new(),
            history: Vec::new(),
            archived: false,
            agent: None,
            hardware: None,
            ui: Vec::new(),
        }
    }

    #[test]
    fn rust_project_needs_cpu_ram_ssd_light_extension_needs_none() {
        let tmp = std::env::temp_dir().join(format!("hw-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let rust_dir = tmp.join("rust_proj");
        let ext_dir = tmp.join("gnome_ext");
        fs::create_dir_all(&rust_dir).unwrap();
        fs::create_dir_all(&ext_dir).unwrap();
        fs::write(rust_dir.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        fs::write(ext_dir.join("metadata.json"), "{}").unwrap();

        let rust_doc = dummy_doc("r", rust_dir.to_str().unwrap(), "");
        let rust_needs = detect_project_hardware(&rust_doc, None);
        assert!(rust_needs.show_cpu && rust_needs.show_ram && rust_needs.show_ssd);

        let ext_doc = dummy_doc("e", ext_dir.to_str().unwrap(), "");
        let ext_needs = detect_project_hardware(&ext_doc, None);
        assert!(!ext_needs.any());

        // Explicit partial override: only CPU, no RAM
        let mut custom = dummy_doc("c", ext_dir.to_str().unwrap(), "");
        custom.hardware = Some(HardwareConfig {
            heavy_cpu: Some(true),
            heavy_ram: Some(false),
            heavy_ssd: Some(false),
            ..Default::default()
        });
        let custom_needs = detect_project_hardware(&custom, None);
        assert!(custom_needs.show_cpu && !custom_needs.show_ram && !custom_needs.show_ssd);

        let _ = fs::remove_dir_all(&tmp);
    }
}
