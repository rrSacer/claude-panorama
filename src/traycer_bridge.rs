use crate::ipc::{AgentActionResult, TraycerCatalog, TraycerHarnessInfo, TraycerModelInfo};
use crate::model::PanoramaAgentConfig;
use crate::store::{now_iso, update_project_agent};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;

pub fn resolve_traycer_home() -> PathBuf {
    if let Ok(v) = std::env::var("TRAYCER_HOME") {
        if !v.trim().is_empty() {
            return PathBuf::from(v);
        }
    }
    let root_dir = PathBuf::from("/root/.traycer");
    if root_dir.join("host/pid.json").exists() {
        return root_dir;
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".traycer");
    }
    root_dir
}

pub fn resolve_traycer_cli() -> PathBuf {
    let home_cli = resolve_traycer_home().join("cli/bin/traycer");
    if home_cli.is_file() {
        return home_cli;
    }
    PathBuf::from("traycer")
}

fn new_uuid_v4() -> String {
    use std::io::Read;
    let mut b = [0u8; 16];
    let ok = fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok();
    if !ok {
        b = [0u8; 16];
    }
    if b == [0u8; 16] {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(1);
        for (i, byte) in b.iter_mut().enumerate() {
            *byte = ((t >> ((i % 8) * 8)) as u8) ^ (i as u8).wrapping_mul(73);
        }
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3],
        b[4], b[5],
        b[6], b[7],
        b[8], b[9],
        b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

fn run_traycer_cli_json(args: &[&str], agent_ctx: Option<(&str, &str)>) -> Result<Value, String> {
    let cli = resolve_traycer_cli();
    let traycer_home = resolve_traycer_home();
    let mut cmd = Command::new(&cli);
    cmd.args(args).env("TRAYCER_HOME", &traycer_home);
    if let Some((epic_id, agent_id)) = agent_ctx {
        cmd.env("TRAYCER_EPIC_ID", epic_id)
            .env("TRAYCER_AGENT_ID", agent_id);
    } else {
        cmd.env_remove("TRAYCER_AGENT_ID")
            .env_remove("TRAYCER_EPIC_ID");
    }
    if traycer_home == Path::new("/root/.traycer") {
        cmd.env("HOME", "/root");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Falha ao executar `{}`: {e}", cli.display()))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            if val["status"] == "ok" {
                return Ok(val["data"].clone());
            }
            if val["status"] == "error" || val["error"].is_object() {
                let msg = val["error"]["message"]
                    .as_str()
                    .or_else(|| val["message"].as_str())
                    .unwrap_or(trimmed);
                return Err(msg.to_string());
            }
            return Ok(val);
        }
    }

    if !out.status.success() {
        let msg = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        return Err(format!("Traycer CLI retornou erro: {msg}"));
    }

    Err(format!(
        "Resposta inesperada do Traycer CLI: {}",
        stdout.trim()
    ))
}

fn parse_models_payload(mdata: Value) -> Vec<TraycerModelInfo> {
    mdata["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let mid = m["id"].as_str()?.to_string();
                    let efforts = m["reasoningEfforts"]
                        .as_array()
                        .map(|ea| {
                            ea.iter()
                                .filter_map(|e| e.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let fast = m["fastModeAvailable"].as_bool().unwrap_or(false);
                    Some(TraycerModelInfo {
                        id: mid,
                        reasoning_efforts: efforts,
                        fast_mode_available: fast,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_traycer_catalog() -> Result<TraycerCatalog, String> {
    let data = run_traycer_cli_json(&["agent", "list-harnesses", "--json"], None)?;
    let raw_harnesses = data["harnesses"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut harnesses = Vec::new();
    for h in raw_harnesses {
        let available = h["available"].as_bool().unwrap_or(false);
        if !available {
            continue;
        }
        let Some(id) = h["id"].as_str().map(|s| s.to_string()) else {
            continue;
        };
        let label = h["label"].as_str().unwrap_or(&id).to_string();

        let mut models = match run_traycer_cli_json(
            &["agent", "list-harness-models", &id, "--json"],
            None,
        ) {
            Ok(mdata) => parse_models_payload(mdata),
            Err(_) => Vec::new(),
        };

        // Em cold-start do host Traycer, tenta uma vez mais após breve espera
        if models.is_empty() {
            std::thread::sleep(Duration::from_millis(350));
            if let Ok(mdata) =
                run_traycer_cli_json(&["agent", "list-harness-models", &id, "--json"], None)
            {
                models = parse_models_payload(mdata);
            }
        }

        if !models.is_empty() {
            harnesses.push(TraycerHarnessInfo { id, label, models });
        }
    }

    Ok(TraycerCatalog { harnesses })
}

pub fn pick_default_model(catalog: &TraycerCatalog) -> Option<(String, String, Option<String>)> {
    let preferred_harness = ["antigravity", "codex", "claude", "hermes"];
    let harness = preferred_harness
        .iter()
        .find_map(|id| catalog.harnesses.iter().find(|h| h.id == *id))
        .or_else(|| catalog.harnesses.first())?;

    let preferred_models = ["gemini-4-hydrogen", "gpt-6.1-terra", "sonnet", "gemini-4-neon"];
    let model = preferred_models
        .iter()
        .find_map(|mid| harness.models.iter().find(|m| m.id == *mid))
        .or_else(|| harness.models.first())?;

    let effort = if model.reasoning_efforts.iter().any(|e| e == "high") {
        Some("high".to_string())
    } else if model.reasoning_efforts.iter().any(|e| e == "medium") {
        Some("medium".to_string())
    } else {
        model.reasoning_efforts.first().cloned()
    };

    Some((harness.id.clone(), model.id.clone(), effort))
}

pub fn build_initial_agent_prompt(slug: &str, title: &str, path: &str) -> String {
    let path_note = if path.trim().is_empty() {
        "nenhum diretório local informado ainda (verifique se existe em `/home/sacer/Projects/` ou inspecione pelo título)".to_string()
    } else {
        format!("`{path}`")
    };
    format!(
        "Você é o agente dedicado ao **Claude Panorama** do projeto **{title}** (`slug`: `{slug}`, diretório: {path_note}).\n\n\
        Seu papel exclusivo é:\n\
        1. Montar este panorama do zero após inspecionar o projeto real.\n\
        2. Monitorar o projeto e manter o panorama atualizado sempre que solicitado.\n\n\
        Instruções obrigatórias:\n\
        - Execute `panorama schema` e leia `/home/sacer/Projects/claude-panorama/AGENT_INSTRUCTIONS.md` antes de escrever o JSON.\n\
        - Inspecione o diretório do projeto ({path_note}): código-fonte, git status/log, testes, processos em execução e uso de hardware.\n\
        - Avalie com rigor:\n\
          - `progresso` (0.0–100.0): apenas o que já está implementado e verificado;\n\
          - `confianca` (0.0–100.0): chance honesta atual do projeto ficar pronto e funcionar;\n\
          - `status`, `phase`, `subtitle`, `progresso_subtitulo`, `progresso_calculo`, `confianca_resumo`, `confianca_fatores`, `confianca_riscos`;\n\
          - `hardware`: preencha `{{\"heavy_cpu\": bool, \"heavy_ram\": bool, \"heavy_ssd\": bool, \"reason\": \"...\"}}` marcando `true` **somente** nas peças que este projeto realmente usa muito (ex.: Rust usa muita CPU, RAM e SSD; se o projeto não usa muita RAM, deixe `heavy_ram: false`).\n\
        - Construa abas em linguagem simples para quem não conhece código (`Começo aqui`, `Progresso`, `Agora`, `Testes e qualidade`), com visão rápida no topo e detalhes técnicos recolhidos em `expander`.\n\
        - Grave o painel atomicamente executando:\n\
          `panorama apply {slug} - <<'EOF'\n...JSON...\nEOF`\n\
        - Confirme no final o resumo do que foi publicado no panorama."
    )
}

pub fn build_update_agent_prompt(slug: &str, title: &str, path: &str) -> String {
    let path_note = if path.trim().is_empty() {
        "sem caminho definido".to_string()
    } else {
        format!("`{path}`")
    };
    format!(
        "O usuário clicou em **Atualizar** no Claude Panorama para o projeto **{title}** (`slug`: `{slug}`, diretório: {path_note}).\n\n\
        Por favor, atualize o panorama agora:\n\
        1. Reinspecione o estado atual do projeto em {path_note} (arquivos modificados, `git status` / `git log`, testes, bugs/bloqueios, uso de hardware).\n\
        2. Recalcule `progresso`, `confianca`, `phase`, `status`, os blocos visuais das abas e o campo `hardware` (`heavy_cpu`, `heavy_ram`, `heavy_ssd`, `reason`) caso algo tenha mudado.\n\
        3. Grave a atualização atomicamente com `panorama apply {slug} -` seguindo `panorama schema` e `/home/sacer/Projects/claude-panorama/AGENT_INSTRUCTIONS.md`."
    )
}

pub fn start_panorama_agent(
    projects_dir: &Path,
    slug: &str,
    title: &str,
    path: &str,
    harness_id: &str,
    model: &str,
    reasoning_effort: Option<&str>,
) -> Result<AgentActionResult, String> {
    let prompt = build_initial_agent_prompt(slug, title, path);
    let task_title = format!("Panorama: {title}");
    let mut args: Vec<String> = vec![
        "task".into(),
        "create".into(),
        "--title".into(),
        task_title,
        "--harness".into(),
        harness_id.to_string(),
        "--model".into(),
        model.to_string(),
        "--message".into(),
        prompt,
        "--json".into(),
    ];
    let p_trimmed = path.trim();
    if !p_trimmed.is_empty() && Path::new(p_trimmed).is_dir() {
        args.push("--workspace".into());
        args.push(p_trimmed.to_string());
    }
    if let Some(eff) = reasoning_effort.filter(|s| !s.trim().is_empty()) {
        args.push("--reasoning-effort".into());
        args.push(eff.to_string());
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let data = run_traycer_cli_json(&arg_refs, None)?;
    let epic_id = data["taskId"].as_str().map(|s| s.to_string());
    let agent_id = data["agentId"].as_str().map(|s| s.to_string());
    if agent_id.is_none() {
        return Err(format!("Traycer não retornou agentId: {data}"));
    }

    let cfg = PanoramaAgentConfig {
        agent_id,
        epic_id,
        harness_id: harness_id.to_string(),
        model: model.to_string(),
        reasoning_effort: reasoning_effort.map(|s| s.to_string()),
        last_requested_at: Some(now_iso()),
        last_status: Some("Agente iniciado — montando panorama".to_string()),
    };
    let _ = update_project_agent(projects_dir, slug, cfg.clone());

    Ok(AgentActionResult {
        agent: cfg,
        message: format!("Agente dedicado criado no Traycer ({harness_id} · {model})."),
        created_new: true,
    })
}

pub fn configure_panorama_agent(
    projects_dir: &Path,
    slug: &str,
    agent_id: &str,
    epic_id: Option<&str>,
    harness_id: &str,
    model: &str,
    reasoning_effort: Option<&str>,
) -> Result<AgentActionResult, String> {
    let epic = epic_id
        .filter(|s| !s.trim().is_empty())
        .ok_or("Agente sem epic_id; inicie o agente primeiro.")?;

    let mut args: Vec<String> = vec![
        "agent".into(),
        "configure".into(),
        "--agent-id".into(),
        agent_id.to_string(),
        "--harness".into(),
        harness_id.to_string(),
        "--model".into(),
        model.to_string(),
        "--profile".into(),
        "ambient".into(),
        "--permission-mode".into(),
        "full_access".into(),
        "--json".into(),
    ];
    if let Some(eff) = reasoning_effort.filter(|s| !s.trim().is_empty()) {
        args.push("--reasoning-effort".into());
        args.push(eff.to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_traycer_cli_json(&arg_refs, Some((epic, agent_id)))?;

    let cfg = PanoramaAgentConfig {
        agent_id: Some(agent_id.to_string()),
        epic_id: Some(epic.to_string()),
        harness_id: harness_id.to_string(),
        model: model.to_string(),
        reasoning_effort: reasoning_effort.map(|s| s.to_string()),
        last_requested_at: None,
        last_status: Some(format!("Modelo atualizado para {harness_id} · {model}")),
    };
    let _ = update_project_agent(projects_dir, slug, cfg.clone());

    Ok(AgentActionResult {
        agent: cfg,
        message: format!("Modelo do agente alterado para {model}."),
        created_new: false,
    })
}

pub fn archive_traycer_agent(
    agent_id: &str,
    epic_id: Option<&str>,
    archived: bool,
) -> Result<(), String> {
    let Some(eid) = epic_id.filter(|s| !s.trim().is_empty()) else {
        return Ok(());
    };
    let mut args = vec!["agent", "archive", "--agent-id", agent_id];
    if !archived {
        args.push("--unarchive");
    }
    args.push("--json");

    match run_traycer_cli_json(&args, Some((eid, agent_id))) {
        Ok(_) => Ok(()),
        Err(err) if archived && (err.contains("BUSY") || err.contains("in progress") || err.contains("refused to archive")) => {
            let _ = run_traycer_cli_json(
                &["agent", "stop", "--agent-id", agent_id, "--json"],
                Some((eid, agent_id)),
            );
            std::thread::sleep(Duration::from_millis(300));
            run_traycer_cli_json(&args, Some((eid, agent_id))).map(|_| ())
        }
        Err(err) => Err(err),
    }
}

struct HostCreds {
    websocket_url: String,
    host_id: String,
    token: String,
    user_id: String,
}

fn read_host_creds() -> Result<HostCreds, String> {
    let base = resolve_traycer_home();
    let pid_raw = fs::read_to_string(base.join("host/pid.json"))
        .map_err(|e| format!("Traycer host não está acessível ({}): {e}", base.display()))?;
    let pid_val: Value =
        serde_json::from_str(&pid_raw).map_err(|e| format!("pid.json inválido: {e}"))?;
    let websocket_url = pid_val["websocketUrl"]
        .as_str()
        .ok_or("websocketUrl ausente em pid.json")?
        .to_string();
    let host_id = pid_val["hostId"].as_str().unwrap_or_default().to_string();

    let cred_path = base.join("cli/credentials");
    let mut cred_raw = fs::read_to_string(&cred_path)
        .map_err(|e| format!("Credenciais do Traycer não encontradas: {e}"))?;
    let mut cred_val: Value =
        serde_json::from_str(&cred_raw).map_err(|e| format!("credentials inválido: {e}"))?;

    if cred_val["token"].as_str().unwrap_or("").is_empty() {
        let _ = run_traycer_cli_json(&["whoami", "--json"], None);
        cred_raw = fs::read_to_string(&cred_path)
            .map_err(|e| format!("Credenciais do Traycer não encontradas: {e}"))?;
        cred_val = serde_json::from_str(&cred_raw).unwrap_or(cred_val);
    }

    let token = cred_val["token"]
        .as_str()
        .ok_or("Token ausente em cli/credentials")?
        .to_string();
    let user_id = cred_val["user"]["id"]
        .as_str()
        .unwrap_or("30e017a0-43f0-4a1a-926e-c6b27f785b6b")
        .to_string();

    Ok(HostCreds {
        websocket_url,
        host_id,
        token,
        user_id,
    })
}

fn set_ws_timeouts(
    ws: &tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    timeout: Duration,
) {
    if let MaybeTlsStream::Plain(tcp) = ws.get_ref() {
        let _ = tcp.set_read_timeout(Some(timeout));
        let _ = tcp.set_write_timeout(Some(timeout));
    }
}

fn read_next_ws_text(
    ws: &mut tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>,
) -> Result<String, String> {
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => return Ok(t.to_string()),
            Ok(Message::Ping(p)) => {
                let _ = ws.send(Message::Pong(p));
            }
            Ok(Message::Binary(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => continue,
            Ok(Message::Close(_)) => return Err("Conexão WS fechada pelo Traycer host".to_string()),
            Err(e) => return Err(format!("Erro lendo WS do Traycer: {e}")),
        }
    }
}

fn send_user_message_to_chat(
    epic_id: &str,
    chat_id: &str,
    harness_id: Option<&str>,
    model: Option<&str>,
    reasoning_effort: Option<&str>,
    body: &str,
) -> Result<(), String> {
    let creds = read_host_creds()?;
    let stream_url = creds
        .websocket_url
        .strip_suffix("/rpc")
        .map(|b| format!("{b}/stream"))
        .unwrap_or_else(|| creds.websocket_url.replace("/rpc", "/stream"));

    let (mut ws, _) = tungstenite::connect(&stream_url)
        .map_err(|e| format!("Falha ao abrir stream do Traycer ({stream_url}): {e}"))?;
    set_ws_timeouts(&ws, Duration::from_secs(5));

    let open = json!({
        "kind": "open",
        "token": creds.token,
        "manifest": {
            "chat.subscribe": { "major": 1, "minor": 41 }
        },
        "clientIdentity": {
            "kind": "desktop",
            "compatibilityEpoch": 2,
            "appVersion": "1.5.4"
        },
        "cloudAuthorized": true
    });
    ws.send(Message::Text(open.to_string().into()))
        .map_err(|e| format!("Erro open stream: {e}"))?;

    let ack_txt = read_next_ws_text(&mut ws)?;
    let ack: Value =
        serde_json::from_str(&ack_txt).map_err(|e| format!("Stream ack inválido: {e}"))?;
    if ack["kind"] == "fatalError" {
        return Err(format!(
            "Handshake stream Traycer falhou: {}",
            ack["details"]["reason"].as_str().unwrap_or(&ack_txt)
        ));
    }

    let version = ack["manifest"]["chat.subscribe"]
        .as_object()
        .map(|o| json!({"major": o["major"], "minor": o["minor"]}))
        .unwrap_or_else(|| json!({"major": 1, "minor": 41}));

    let subscribe = json!({
        "kind": "subscribe",
        "method": "chat.subscribe",
        "schemaVersion": version,
        "params": {
            "epicId": epic_id,
            "chatId": chat_id,
            "resume": null
        }
    });
    ws.send(Message::Text(subscribe.to_string().into()))
        .map_err(|e| format!("Erro subscribe chat: {e}"))?;

    let mut current_settings: Option<Value> = None;
    for _ in 0..12 {
        let txt = read_next_ws_text(&mut ws)?;
        let frame: Value = match serde_json::from_str(&txt) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match frame["kind"].as_str() {
            Some("ping") => {
                let _ = ws.send(Message::Text(
                    json!({"kind": "pong", "hasBinaryPayload": false})
                        .to_string()
                        .into(),
                ));
            }
            Some("error") | Some("fatalError") => {
                let msg = frame["details"]["reason"]
                    .as_str()
                    .or_else(|| frame["message"].as_str())
                    .unwrap_or(&txt);
                return Err(format!("Erro no stream do chat: {msg}"));
            }
            Some("snapshot") => {
                current_settings = frame["snapshot"]["chat"]["settings"].as_object().map(|_| {
                    frame["snapshot"]["chat"]["settings"].clone()
                });
                break;
            }
            _ => {}
        }
    }

    let effective_harness = harness_id
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            current_settings
                .as_ref()
                .and_then(|s| s["harnessId"].as_str())
        })
        .unwrap_or("antigravity");
    let effective_model = model
        .filter(|s| !s.trim().is_empty())
        .or_else(|| current_settings.as_ref().and_then(|s| s["model"].as_str()))
        .unwrap_or("gemini-4-hydrogen");
    let effective_effort = reasoning_effort
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            current_settings
                .as_ref()
                .and_then(|s| s["reasoningEffort"].as_str())
        });

    let paragraphs: Vec<Value> = body
        .lines()
        .map(|line| {
            if line.is_empty() {
                json!({"type": "paragraph"})
            } else {
                json!({"type": "paragraph", "content": [{"type": "text", "text": line}]})
            }
        })
        .collect();

    let action_id = new_uuid_v4();
    let send_frame = json!({
        "kind": "send",
        "hasBinaryPayload": false,
        "epicId": epic_id,
        "chatId": chat_id,
        "clientActionId": action_id,
        "messageId": new_uuid_v4(),
        "content": { "type": "doc", "content": paragraphs },
        "sender": { "type": "user", "userId": creds.user_id },
        "settings": {
            "harnessId": effective_harness,
            "model": effective_model,
            "permissionMode": "full_access",
            "reasoningEffort": effective_effort,
            "serviceTier": null,
            "agentMode": current_settings.as_ref().and_then(|s| s["agentMode"].as_str()).or(Some("regular")),
            "profileId": current_settings.as_ref().and_then(|s| s["profileId"].as_str())
        },
        "accountContext": { "type": "PERSONAL" },
        "sentFromHostId": if creds.host_id.is_empty() { Value::Null } else { json!(creds.host_id) },
        "deliveryPolicy": "auto",
        "worktreeIntent": null,
        "browserAnnotations": []
    });

    ws.send(Message::Text(send_frame.to_string().into()))
        .map_err(|e| format!("Falha ao enviar mensagem para o agente: {e}"))?;

    for _ in 0..16 {
        match read_next_ws_text(&mut ws) {
            Ok(txt) => {
                let Ok(v) = serde_json::from_str::<Value>(&txt) else {
                    continue;
                };
                match v["kind"].as_str() {
                    Some("ping") => {
                        let _ = ws.send(Message::Text(
                            json!({"kind": "pong", "hasBinaryPayload": false})
                                .to_string()
                                .into(),
                        ));
                    }
                    Some("actionAck") if v["clientActionId"].as_str() == Some(action_id.as_str()) => {
                        let status = v["status"].as_str().unwrap_or("accepted");
                        if v["accepted"] == false || (status != "accepted" && status != "ok" && status != "queued") {
                            let msg = v["reason"]
                                .as_str()
                                .or_else(|| v["message"].as_str())
                                .unwrap_or(&txt);
                            let _ = ws.close(None);
                            return Err(format!("Traycer recusou envio: {msg}"));
                        }
                        break;
                    }
                    Some("error") | Some("refusal") | Some("fatalError") => {
                        let msg = v["message"]
                            .as_str()
                            .or_else(|| v["error"]["message"].as_str())
                            .or_else(|| v["details"]["reason"].as_str())
                            .unwrap_or(&txt);
                        let _ = ws.close(None);
                        return Err(format!("Traycer recusou envio: {msg}"));
                    }
                    Some("turnStateChanged") if v["turnInProgress"] == true => {
                        break;
                    }
                    _ => {}
                }
            }
            Err(_) => break,
        }
    }
    let _ = ws.close(None);
    Ok(())
}

pub fn request_panorama_update(
    projects_dir: &Path,
    slug: &str,
    title: &str,
    path: &str,
    agent_id: Option<&str>,
    epic_id: Option<&str>,
    harness_id: Option<&str>,
    model: Option<&str>,
    reasoning_effort: Option<&str>,
) -> Result<AgentActionResult, String> {
    if let (Some(aid), Some(eid)) = (
        agent_id.filter(|s| !s.trim().is_empty()),
        epic_id.filter(|s| !s.trim().is_empty()),
    ) {
        let prompt = build_update_agent_prompt(slug, title, path);
        match send_user_message_to_chat(eid, aid, harness_id, model, reasoning_effort, &prompt) {
            Ok(()) => {
                let cfg = PanoramaAgentConfig {
                    agent_id: Some(aid.to_string()),
                    epic_id: Some(eid.to_string()),
                    harness_id: harness_id.unwrap_or("antigravity").to_string(),
                    model: model.unwrap_or("gemini-4-hydrogen").to_string(),
                    reasoning_effort: reasoning_effort.map(|s| s.to_string()),
                    last_requested_at: Some(now_iso()),
                    last_status: Some("Atualização solicitada ao agente".to_string()),
                };
                let _ = update_project_agent(projects_dir, slug, cfg.clone());
                return Ok(AgentActionResult {
                    agent: cfg,
                    message: "Pedido de atualização enviado para o agente dedicado.".to_string(),
                    created_new: false,
                });
            }
            Err(err) => {
                let lower = err.to_lowercase();
                if !(lower.contains("not found") || lower.contains("não encontrad") || lower.contains("closed")) {
                    return Err(err);
                }
            }
        }
    }

    let (h, m, eff) = match (
        harness_id.filter(|s| !s.trim().is_empty()),
        model.filter(|s| !s.trim().is_empty()),
    ) {
        (Some(hh), Some(mm)) => (
            hh.to_string(),
            mm.to_string(),
            reasoning_effort.map(|s| s.to_string()),
        ),
        _ => {
            let cat = list_traycer_catalog()?;
            pick_default_model(&cat)
                .ok_or("Nenhum modelo disponível no Traycer para criar o agente dedicado.")?
        }
    };
    start_panorama_agent(projects_dir, slug, title, path, &h, &m, eff.as_deref())
}
