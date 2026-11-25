use crate::client;
use crate::hardware::{parse_cpu_list_spec, SystemHardwareSpecs};
use crate::model::{parse_and_validate_json, PanoramaAgentConfig};
use crate::schema::SCHEMA_SPEC;
use crate::store::{
    apply_project_json, copy_project_icon_image, delete_project, init_project, load_all_projects,
    move_project_in_order, project_file_path, read_project_file, resolve_projects_dir,
    set_project_archived, update_project_agent, update_project_hardware, update_project_identity,
    InitOptions,
};
use crate::traycer_bridge::pick_default_model;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum Command {
    Gui,
    Help,
    Version,
    Schema,
    List {
        all: bool,
        archived_only: bool,
    },
    Models,
    Init {
        slug: String,
        title: String,
        path: String,
        progresso: f64,
        confianca: f64,
        phase: String,
        harness: Option<String>,
        model: Option<String>,
        reasoning_effort: Option<String>,
        no_agent: bool,
    },
    Apply {
        slug: String,
        source: Option<String>,
    },
    Validate {
        file: String,
    },
    Archive {
        slug: String,
    },
    Unarchive {
        slug: String,
    },
    Delete {
        slug: String,
        yes: bool,
    },
    Move {
        slug: String,
        up: bool,
    },
    Update {
        slug: String,
    },
    SetAgent {
        slug: String,
        agent_id: String,
        epic_id: String,
        harness: Option<String>,
        model: Option<String>,
        reasoning_effort: Option<String>,
    },
    Edit {
        slug: String,
        title: Option<String>,
        icon: Option<String>,
        reset_icon: bool,
    },
    Limits {
        slug: String,
        cpu_cores: Option<u32>,
        cpu_threads: Option<Vec<u32>>,
        ssd_percent: Option<u32>,
        max_ram_gb: Option<f64>,
        clear: bool,
    },
}

pub fn usage_text() -> &'static str {
    concat!(
        "Claude Panorama — Painel nativo GNOME + CLI de acompanhamento de projetos\n\n",
        "USO:\n",
        "  claude-panorama                            Abre a interface gráfica GNOME\n",
        "  panorama gui                               Abre a interface gráfica GNOME\n",
        "  panorama schema                            Imprime a especificação JSON completa\n",
        "  panorama list [--all | --archived]         Lista projetos (ativos por padrão)\n",
        "  panorama models                            Lista harnesses e modelos do Traycer\n",
        "  panorama init <slug> --title \"...\" [opções] Cria o registro JSON do projeto\n",
        "  panorama set-agent <slug> --agent-id ID --epic-id ID [--harness H] [--model M] [--reasoning-effort E]\n",
        "                                             Vincula o subagente criado pelo maestro\n",
        "  panorama apply <slug> [arquivo.json | -]   Valida e grava JSON atomicamente\n",
        "  panorama validate <arquivo.json | ->       Valida um JSON sem salvar\n",
        "  panorama update <slug>                     Pede ao subagente dedicado para atualizar\n",
        "  panorama archive <slug>                    Arquiva o projeto\n",
        "  panorama unarchive <slug>                  Restaura um projeto arquivado\n",
        "  panorama delete <slug> --yes               Deleta permanentemente o projeto\n",
        "  panorama move <slug> --up | --down         Altera a ordem do projeto na lista\n",
        "  panorama edit <slug> [--title \"...\"] [--icon <imagem|nome>] [--reset-icon]\n",
        "                                             Altera nome ou ícone/imagem do projeto\n",
        "  panorama limits <slug> [--cpus 0,1,6,7] [--cpu N] [--ssd %] [--ram GB] [--clear]\n",
        "                                             Configura limites reais de hardware\n",
        "  panorama --help | --version\n\n",
        "OPÇÕES DE `init`:\n",
        "  --title \"Nome do Projeto\"                  (obrigatório)\n",
        "  --path \"/caminho/do/repo\"                  Caminho do repositório\n",
        "  --progresso <0..100>                       Valor inicial (padrão: 0)\n",
        "  --confianca <0..100>                       Valor inicial (padrão: 50)\n",
        "  --phase \"Fase atual\"                       Etapa inicial\n",
        "  --harness <id>                             Harness Traycer do agente dedicado\n",
        "  --model <id>                               Modelo Traycer do agente dedicado\n",
        "  --reasoning-effort <nivel>                 Esforço de raciocínio (se suportado)\n",
        "  --no-agent                                 Mantido por compatibilidade (init não cria tarefa avulsa)\n",
    )
}

pub fn parse_cli_args(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Gui);
    }

    match args[0].as_str() {
        "gui" => Ok(Command::Gui),
        "-h" | "--help" | "help" => Ok(Command::Help),
        "-V" | "--version" | "version" => Ok(Command::Version),
        "schema" => Ok(Command::Schema),
        "models" => Ok(Command::Models),
        "list" | "ls" => parse_list_args(&args[1..]),
        "init" => parse_init_args(&args[1..]),
        "apply" => parse_apply_args(&args[1..]),
        "validate" => {
            if args.len() != 2 {
                return Err("Uso: `panorama validate <arquivo.json | ->`".to_string());
            }
            Ok(Command::Validate {
                file: args[1].clone(),
            })
        }
        "archive" => {
            if args.len() != 2 {
                return Err("Uso: `panorama archive <slug>`".to_string());
            }
            Ok(Command::Archive {
                slug: args[1].clone(),
            })
        }
        "unarchive" => {
            if args.len() != 2 {
                return Err("Uso: `panorama unarchive <slug>`".to_string());
            }
            Ok(Command::Unarchive {
                slug: args[1].clone(),
            })
        }
        "delete" => {
            if args.len() < 2 || args.len() > 3 {
                return Err("Uso: `panorama delete <slug> --yes`".to_string());
            }
            let slug = args[1].clone();
            let yes = args.get(2).map(|s| s == "--yes" || s == "-y").unwrap_or(false);
            Ok(Command::Delete { slug, yes })
        }
        "move" => {
            if args.len() != 3 {
                return Err("Uso: `panorama move <slug> --up | --down`".to_string());
            }
            let slug = args[1].clone();
            let up = match args[2].as_str() {
                "--up" | "up" => true,
                "--down" | "down" => false,
                other => return Err(format!("Direção inválida em `move`: `{other}` (use `--up` ou `--down`)")),
            };
            Ok(Command::Move { slug, up })
        }
        "update" | "refresh" => {
            if args.len() != 2 {
                return Err("Uso: `panorama update <slug>`".to_string());
            }
            Ok(Command::Update {
                slug: args[1].clone(),
            })
        }
        "set-agent" => parse_set_agent_args(&args[1..]),
        "edit" | "rename" => parse_edit_args(&args[1..]),
        "limits" => parse_limits_args(&args[1..]),
        other => Err(format!(
            "Subcomando desconhecido `{other}`. Execute `panorama --help`."
        )),
    }
}

fn parse_list_args(rest: &[String]) -> Result<Command, String> {
    let mut all = false;
    let mut archived_only = false;
    for arg in rest {
        match arg.as_str() {
            "--all" | "-a" => all = true,
            "--archived" => archived_only = true,
            other => return Err(format!("Opção desconhecida em `list`: `{other}`")),
        }
    }
    Ok(Command::List { all, archived_only })
}

fn parse_init_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() {
        return Err("Uso: `panorama init <slug> --title \"...\" [--path \"...\"]`".to_string());
    }
    let slug = rest[0].clone();
    let mut title: Option<String> = None;
    let mut path = String::new();
    let mut progresso = 0.0_f64;
    let mut confianca = 50.0_f64;
    let mut phase = String::new();
    let mut harness: Option<String> = None;
    let mut model: Option<String> = None;
    let mut reasoning_effort: Option<String> = None;
    let mut no_agent = false;

    let next_val = |idx: usize, flag: &str| -> Result<String, String> {
        rest.get(idx + 1)
            .cloned()
            .ok_or_else(|| format!("Falta valor após `{flag}`"))
    };

    let mut i = 1;
    while i < rest.len() {
        let flag = rest[i].as_str();
        match flag {
            "--title" => {
                title = Some(next_val(i, flag)?);
                i += 2;
            }
            "--path" => {
                path = next_val(i, flag)?;
                i += 2;
            }
            "--progresso" | "--progress" => {
                let raw = next_val(i, flag)?;
                progresso = raw
                    .parse::<f64>()
                    .map_err(|_| format!("Valor numérico inválido em `{flag}`: `{raw}`"))?;
                i += 2;
            }
            "--confianca" | "--confidence" => {
                let raw = next_val(i, flag)?;
                confianca = raw
                    .parse::<f64>()
                    .map_err(|_| format!("Valor numérico inválido em `{flag}`: `{raw}`"))?;
                i += 2;
            }
            "--phase" => {
                phase = next_val(i, flag)?;
                i += 2;
            }
            "--harness" => {
                harness = Some(next_val(i, flag)?);
                i += 2;
            }
            "--model" => {
                model = Some(next_val(i, flag)?);
                i += 2;
            }
            "--reasoning-effort" | "--effort" => {
                reasoning_effort = Some(next_val(i, flag)?);
                i += 2;
            }
            "--no-agent" => {
                no_agent = true;
                i += 1;
            }
            unknown => {
                return Err(format!("Opção desconhecida em `panorama init`: `{unknown}`"));
            }
        }
    }

    let Some(title) = title else {
        return Err("`panorama init` requer `--title \"...\"`.".to_string());
    };

    Ok(Command::Init {
        slug,
        title,
        path,
        progresso,
        confianca,
        phase,
        harness,
        model,
        reasoning_effort,
        no_agent,
    })
}

fn parse_set_agent_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() {
        return Err("Uso: `panorama set-agent <slug> --agent-id <id> --epic-id <id> [--harness H] [--model M] [--reasoning-effort E]`".to_string());
    }
    let slug = rest[0].clone();
    let mut agent_id: Option<String> = None;
    let mut epic_id: Option<String> = None;
    let mut harness: Option<String> = None;
    let mut model: Option<String> = None;
    let mut reasoning_effort: Option<String> = None;

    let next_val = |idx: usize, flag: &str| -> Result<String, String> {
        rest.get(idx + 1)
            .cloned()
            .ok_or_else(|| format!("Falta valor após `{flag}`"))
    };

    let mut i = 1;
    while i < rest.len() {
        let flag = rest[i].as_str();
        match flag {
            "--agent-id" => {
                agent_id = Some(next_val(i, flag)?);
                i += 2;
            }
            "--epic-id" | "--task-id" => {
                epic_id = Some(next_val(i, flag)?);
                i += 2;
            }
            "--harness" => {
                harness = Some(next_val(i, flag)?);
                i += 2;
            }
            "--model" => {
                model = Some(next_val(i, flag)?);
                i += 2;
            }
            "--reasoning-effort" | "--effort" => {
                reasoning_effort = Some(next_val(i, flag)?);
                i += 2;
            }
            unknown => return Err(format!("Opção desconhecida em `set-agent`: `{unknown}`")),
        }
    }

    let Some(agent_id) = agent_id.filter(|v| !v.trim().is_empty()) else {
        return Err("`panorama set-agent` requer `--agent-id <id>`.".to_string());
    };
    let Some(epic_id) = epic_id.filter(|v| !v.trim().is_empty()) else {
        return Err("`panorama set-agent` requer `--epic-id <id>`.".to_string());
    };

    Ok(Command::SetAgent {
        slug,
        agent_id,
        epic_id,
        harness,
        model,
        reasoning_effort,
    })
}

fn parse_apply_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() || rest.len() > 2 {
        return Err("Uso: `panorama apply <slug> [arquivo.json | -]`".to_string());
    }
    let slug = rest[0].clone();
    let source = rest.get(1).cloned();
    Ok(Command::Apply { slug, source })
}

fn parse_edit_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() {
        return Err(r#"Uso: `panorama edit <slug> [--title "..."] [--icon <imagem|nome>] [--reset-icon]`"#.to_string());
    }
    let slug = rest[0].clone();
    let mut title = None;
    let mut icon = None;
    let mut reset_icon = false;

    let next_val = |idx: usize, flag: &str| -> Result<String, String> {
        rest.get(idx + 1)
            .cloned()
            .ok_or_else(|| format!("Falta valor após `{flag}`"))
    };

    let mut i = 1;
    while i < rest.len() {
        let flag = rest[i].as_str();
        match flag {
            "--title" | "--name" => {
                title = Some(next_val(i, flag)?);
                i += 2;
            }
            "--icon" | "--image" => {
                icon = Some(next_val(i, flag)?);
                i += 2;
            }
            "--reset-icon" => {
                reset_icon = true;
                i += 1;
            }
            unknown => return Err(format!("Opção desconhecida em `edit`: `{unknown}`")),
        }
    }

    if title.is_none() && icon.is_none() && !reset_icon {
        return Err("Informe `--title`, `--icon` ou `--reset-icon` em `panorama edit`.".to_string());
    }

    Ok(Command::Edit {
        slug,
        title,
        icon,
        reset_icon,
    })
}

fn parse_limits_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() {
        return Err("Uso: `panorama limits <slug> [--cpus 0,1,6,7] [--cpu N] [--ssd %] [--ram GB] [--clear]`".to_string());
    }
    let slug = rest[0].clone();
    let mut cpu_cores = None;
    let mut cpu_threads = None;
    let mut ssd_percent = None;
    let mut max_ram_gb = None;
    let mut clear = false;

    let next_val = |idx: usize, flag: &str| -> Result<String, String> {
        rest.get(idx + 1)
            .cloned()
            .ok_or_else(|| format!("Falta valor após `{flag}`"))
    };

    let mut i = 1;
    while i < rest.len() {
        let flag = rest[i].as_str();
        match flag {
            "--cpus" | "--threads" | "--cores" => {
                let raw = next_val(i, flag)?;
                let specs = SystemHardwareSpecs::detect();
                cpu_threads = Some(parse_cpu_list_spec(&raw, specs.total_cpus)?);
                i += 2;
            }
            "--cpu" => {
                let raw = next_val(i, flag)?;
                cpu_cores = Some(
                    raw.parse::<u32>()
                        .map_err(|_| format!("Valor inválido em `--cpu`: `{raw}`"))?,
                );
                i += 2;
            }
            "--ssd" => {
                let raw = next_val(i, flag)?;
                ssd_percent = Some(
                    raw.parse::<u32>()
                        .map_err(|_| format!("Valor inválido em `--ssd`: `{raw}`"))?,
                );
                i += 2;
            }
            "--ram" => {
                let raw = next_val(i, flag)?;
                max_ram_gb = Some(
                    raw.parse::<f64>()
                        .map_err(|_| format!("Valor inválido em `--ram`: `{raw}`"))?,
                );
                i += 2;
            }
            "--clear" => {
                clear = true;
                i += 1;
            }
            unknown => return Err(format!("Opção desconhecida em `limits`: `{unknown}`")),
        }
    }

    Ok(Command::Limits {
        slug,
        cpu_cores,
        cpu_threads,
        ssd_percent,
        max_ram_gb,
        clear,
    })
}

pub fn execute_non_gui(cmd: Command) -> Result<String, String> {
    let dir = resolve_projects_dir();
    execute_non_gui_in_dir(cmd, &dir)
}

pub fn execute_non_gui_in_dir(cmd: Command, dir: &Path) -> Result<String, String> {
    match cmd {
        Command::Gui => unreachable!("GUI é tratada em gui::run_gui"),
        Command::Help => Ok(usage_text().to_string()),
        Command::Version => Ok(format!("claude-panorama {}", env!("CARGO_PKG_VERSION"))),
        Command::Schema => Ok(SCHEMA_SPEC.to_string()),
        Command::Models => {
            let catalog = client::list_traycer_models(true)?;
            let mut out = String::new();
            for h in &catalog.harnesses {
                out.push_str(&format!("[{}] {}\n", h.id, h.label));
                for m in &h.models {
                    let eff = if m.reasoning_efforts.is_empty() {
                        String::new()
                    } else {
                        format!(" (esforços: {})", m.reasoning_efforts.join(", "))
                    };
                    out.push_str(&format!("  - {}{}\n", m.id, eff));
                }
            }
            if out.is_empty() {
                out.push_str("Nenhum modelo retornado pelo Traycer.\n");
            }
            Ok(out)
        }
        Command::List { all, archived_only } => {
            let loaded = load_all_projects(dir);
            let filtered: Vec<_> = loaded
                .projects
                .iter()
                .filter(|p| {
                    if archived_only {
                        p.archived
                    } else if all {
                        true
                    } else {
                        !p.archived
                    }
                })
                .collect();

            if filtered.is_empty() && loaded.errors.is_empty() {
                return Ok(format!(
                    "Nenhum projeto encontrado em `{}`.",
                    dir.display()
                ));
            }
            let mut out = String::new();
            out.push_str(&format!(
                "{:<22} {:<11} {:>10} {:>10}  {:<8}  {:<22}  TÍTULO\n",
                "SLUG", "STATUS", "PROGRESSO", "CONFIANÇA", "ESTADO", "ATUALIZADO EM"
            ));
            out.push_str(&"-".repeat(114));
            out.push('\n');
            for p in filtered {
                let st_label = if p.archived { "arquiv." } else { "ativo" };
                out.push_str(&format!(
                    "{:<22} {:<11} {:>9.1}% {:>9.1}%  {:<8}  {:<22}  {}\n",
                    p.slug,
                    p.status.label(),
                    p.progresso,
                    p.confianca,
                    st_label,
                    if p.updated_at.is_empty() {
                        "—"
                    } else {
                        &p.updated_at
                    },
                    p.title
                ));
            }
            for (fname, err) in &loaded.errors {
                out.push_str(&format!("[ERRO] {fname}: {err}\n"));
            }
            Ok(out)
        }
        Command::Init {
            slug,
            title,
            path,
            progresso,
            confianca,
            phase,
            harness,
            model,
            reasoning_effort,
            no_agent: _,
        } => {
            let initial_agent = match (&harness, &model) {
                (Some(h), Some(m)) => Some(PanoramaAgentConfig {
                    agent_id: None,
                    epic_id: None,
                    harness_id: h.clone(),
                    model: m.clone(),
                    reasoning_effort: reasoning_effort.clone(),
                    last_requested_at: None,
                    last_status: Some("Aguardando vínculo do maestro no Traycer".to_string()),
                }),
                _ => None,
            };
            let written = init_project(
                dir,
                InitOptions {
                    slug: slug.clone(),
                    title: title.clone(),
                    path,
                    icon: None,
                    custom_icon: false,
                    progresso,
                    confianca,
                    phase,
                    agent: initial_agent,
                    hardware: None,
                },
            )?;

            Ok(format!(
                "Projeto `{slug}` inicializado com sucesso em `{}`. Vincule o subagente da conversa com `panorama set-agent {slug} --agent-id <ID> --epic-id <EPIC>`.",
                written.display()
            ))
        }
        Command::SetAgent {
            slug,
            agent_id,
            epic_id,
            harness,
            model,
            reasoning_effort,
        } => {
            let path = project_file_path(dir, &slug);
            let existing = read_project_file(&path).map_err(|e| e.to_string())?;
            let prev_ag = existing.agent.as_ref();

            let fallback_pair = if harness.is_none() || model.is_none() {
                client::list_traycer_models(false)
                    .ok()
                    .and_then(|cat| pick_default_model(&cat))
            } else {
                None
            };

            let eff_harness = harness
                .or_else(|| prev_ag.map(|a| a.harness_id.clone()).filter(|s| !s.is_empty()))
                .or_else(|| fallback_pair.as_ref().map(|p| p.0.clone()))
                .unwrap_or_else(|| "antigravity".to_string());
            let eff_model = model
                .or_else(|| prev_ag.map(|a| a.model.clone()).filter(|s| !s.is_empty()))
                .or_else(|| fallback_pair.as_ref().map(|p| p.1.clone()))
                .unwrap_or_else(|| "gemini-4-hydrogen".to_string());
            let eff_reasoning = reasoning_effort
                .or_else(|| prev_ag.and_then(|a| a.reasoning_effort.clone()))
                .or_else(|| fallback_pair.and_then(|p| p.2));

            let cfg = PanoramaAgentConfig {
                agent_id: Some(agent_id.clone()),
                epic_id: Some(epic_id.clone()),
                harness_id: eff_harness.clone(),
                model: eff_model.clone(),
                reasoning_effort: eff_reasoning.clone(),
                last_requested_at: prev_ag.and_then(|a| a.last_requested_at.clone()),
                last_status: Some(format!(
                    "Subagente vinculado ({eff_harness} · {eff_model})"
                )),
            };
            update_project_agent(dir, &slug, cfg)?;
            Ok(format!(
                "Subagente `{agent_id}` (epic `{epic_id}`, {eff_harness} · {eff_model}) vinculado ao panorama `{slug}`."
            ))
        }
        Command::Apply { slug, source } => {
            let raw = read_input_source(source.as_deref())?;
            let (written, doc) = apply_project_json(dir, &slug, &raw)?;
            Ok(format!(
                "Panorama `{slug}` aplicado atomicamente em `{}` (progresso: {:.1}%, confiança: {:.1}%, histórico: {} pontos, blocos raiz ui: {}).",
                written.display(),
                doc.progresso,
                doc.confianca,
                doc.history.len(),
                doc.ui.len()
            ))
        }
        Command::Validate { file } => {
            let raw = if file == "-" {
                read_stdin_string()?
            } else {
                let p = PathBuf::from(&file);
                fs::read_to_string(&p)
                    .map_err(|e| format!("Falha ao ler `{}`: {e}", p.display()))?
            };
            let doc = parse_and_validate_json(&raw).map_err(|e| e.to_string())?;
            Ok(format!(
                "JSON válido: `{}` — progresso {:.1}%, confiança {:.1}%, {} bloco(s) de topo em `ui`.",
                doc.title,
                doc.progresso,
                doc.confianca,
                doc.ui.len()
            ))
        }
        Command::Archive { slug } => {
            let doc = set_project_archived(dir, &slug, true)?;
            let _ = client::remove_hardware_limits(&slug);
            if let Some(ag) = &doc.agent {
                if let Some(aid) = ag.agent_id.as_deref() {
                    let _ = client::archive_traycer_agent(aid, ag.epic_id.as_deref(), true);
                }
            }
            Ok(format!("Projeto `{slug}` arquivado."))
        }
        Command::Unarchive { slug } => {
            let doc = set_project_archived(dir, &slug, false)?;
            if let Some(ag) = &doc.agent {
                if let Some(aid) = ag.agent_id.as_deref() {
                    let _ = client::archive_traycer_agent(aid, ag.epic_id.as_deref(), false);
                }
            }
            Ok(format!("Projeto `{slug}` restaurado para a lista ativa."))
        }
        Command::Delete { slug, yes } => {
            if !yes {
                return Err(format!(
                    "Confirmação obrigatória: execute `panorama delete {slug} --yes` para deletar permanentemente."
                ));
            }
            let _ = client::remove_hardware_limits(&slug);
            delete_project(dir, &slug)?;
            Ok(format!("Projeto `{slug}` deletado permanentemente."))
        }
        Command::Move { slug, up } => {
            move_project_in_order(dir, &slug, up, None)?;
            Ok(format!(
                "Projeto `{slug}` movido para {}.",
                if up { "cima" } else { "baixo" }
            ))
        }
        Command::Update { slug } => {
            let path = project_file_path(dir, &slug);
            let doc = read_project_file(&path).map_err(|e| e.to_string())?;
            let ag = doc.agent.as_ref();
            let res = client::request_panorama_update(
                dir,
                &doc.slug,
                &doc.title,
                &doc.path,
                ag.and_then(|a| a.agent_id.as_deref()),
                ag.and_then(|a| a.epic_id.as_deref()),
                ag.map(|a| a.harness_id.as_str()),
                ag.map(|a| a.model.as_str()),
                ag.and_then(|a| a.reasoning_effort.as_deref()),
            )?;
            Ok(res.message)
        }
        Command::Edit {
            slug,
            title,
            icon,
            reset_icon,
        } => {
            let mut final_icon = None;
            let mut set_custom_icon = None;
            if reset_icon {
                final_icon = Some(crate::model::default_icon());
                set_custom_icon = Some(false);
            } else if let Some(ic) = icon {
                let candidate = Path::new(ic.trim());
                if candidate.is_file() {
                    let copied = copy_project_icon_image(dir, &slug, candidate)?;
                    final_icon = Some(copied);
                } else {
                    final_icon = Some(ic);
                }
                set_custom_icon = Some(true);
            }
            let custom_title = title.as_ref().map(|_| true);
            let updated = update_project_identity(
                dir,
                &slug,
                title,
                final_icon,
                custom_title,
                set_custom_icon,
            )?;
            Ok(format!(
                "Projeto `{slug}` atualizado: título `{}`, ícone `{}`.",
                updated.title, updated.icon
            ))
        }
        Command::Limits {
            slug,
            cpu_cores,
            cpu_threads,
            ssd_percent,
            max_ram_gb,
            clear,
        } => {
            let path = project_file_path(dir, &slug);
            let doc = read_project_file(&path).map_err(|e| e.to_string())?;
            let mut hw = doc.hardware.clone().unwrap_or_default();
            if clear {
                hw.cpu_cores = None;
                hw.cpu_threads = None;
                hw.ssd_percent = None;
                hw.max_ram_gb = None;
            } else {
                if let Some(threads) = cpu_threads {
                    hw.cpu_threads = Some(threads);
                    hw.cpu_cores = None;
                } else if let Some(c) = cpu_cores {
                    hw.cpu_cores = Some(c);
                    hw.cpu_threads = None;
                }
                if let Some(s) = ssd_percent {
                    hw.ssd_percent = Some(s);
                }
                if let Some(r) = max_ram_gb {
                    hw.max_ram_gb = Some(r);
                }
            }
            let updated = update_project_hardware(dir, &slug, hw.clone())?;
            let h = updated.hardware.unwrap_or_default();
            if h.cpu_cores.is_some()
                || h.cpu_threads.as_ref().is_some_and(|v| !v.is_empty())
                || h.ssd_percent.is_some()
                || h.max_ram_gb.is_some()
            {
                let _ = client::apply_hardware_limits(
                    &slug,
                    &doc.path,
                    h.cpu_cores,
                    h.cpu_threads.clone(),
                    h.ssd_percent,
                    h.max_ram_gb,
                );
            } else {
                let _ = client::remove_hardware_limits(&slug);
            }
            Ok(format!("Limites de hardware de `{slug}` atualizados."))
        }
    }
}

fn read_input_source(source: Option<&str>) -> Result<String, String> {
    match source {
        None | Some("-") => read_stdin_string(),
        Some(path_str) => {
            fs::read_to_string(path_str).map_err(|e| format!("Falha ao ler `{path_str}`: {e}"))
        }
    }
}

fn read_stdin_string() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("Falha ao ler stdin: {e}"))?;
    if buf.trim().is_empty() {
        return Err("Entrada stdin vazia: envie um documento JSON válido.".to_string());
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_init_apply_validate_and_new_commands() {
        assert_eq!(parse_cli_args(&[]).unwrap(), Command::Gui);
        assert_eq!(
            parse_cli_args(&["gui".to_string()]).unwrap(),
            Command::Gui
        );
        assert_eq!(
            parse_cli_args(&["schema".to_string()]).unwrap(),
            Command::Schema
        );
        let init_cmd = parse_cli_args(&[
            "init".into(),
            "teste".into(),
            "--title".into(),
            "Projeto Teste".into(),
            "--progresso".into(),
            "15.5".into(),
            "--confianca".into(),
            "82".into(),
            "--phase".into(),
            "Fase 1".into(),
            "--no-agent".into(),
        ])
        .unwrap();
        assert_eq!(
            init_cmd,
            Command::Init {
                slug: "teste".into(),
                title: "Projeto Teste".into(),
                path: String::new(),
                progresso: 15.5,
                confianca: 82.0,
                phase: "Fase 1".into(),
                harness: None,
                model: None,
                reasoning_effort: None,
                no_agent: true,
            }
        );
        assert_eq!(
            parse_cli_args(&["archive".into(), "foo".into()]).unwrap(),
            Command::Archive { slug: "foo".into() }
        );
        assert_eq!(
            parse_cli_args(&["move".into(), "foo".into(), "--up".into()]).unwrap(),
            Command::Move {
                slug: "foo".into(),
                up: true
            }
        );
        assert_eq!(
            parse_cli_args(&["delete".into(), "foo".into(), "--yes".into()]).unwrap(),
            Command::Delete {
                slug: "foo".into(),
                yes: true
            }
        );
        assert_eq!(
            parse_cli_args(&[
                "edit".into(),
                "foo".into(),
                "--title".into(),
                "Novo Nome".into(),
                "--icon".into(),
                "/tmp/a.png".into(),
            ])
            .unwrap(),
            Command::Edit {
                slug: "foo".into(),
                title: Some("Novo Nome".into()),
                icon: Some("/tmp/a.png".into()),
                reset_icon: false,
            }
        );
        let lim_cpus = parse_cli_args(&[
            "limits".into(),
            "foo".into(),
            "--cpus".into(),
            "0,1".into(),
        ])
        .unwrap();
        assert_eq!(
            lim_cpus,
            Command::Limits {
                slug: "foo".into(),
                cpu_cores: None,
                cpu_threads: Some(vec![0, 1]),
                ssd_percent: None,
                max_ram_gb: None,
                clear: false,
            }
        );
    }

    #[test]
    fn cli_end_to_end_init_list_archive_move_delete() {
        let tmp = std::env::temp_dir().join(format!(
            "panorama-cli-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&tmp);

        let out_init = execute_non_gui_in_dir(
            Command::Init {
                slug: "alpha".into(),
                title: "Alpha Core".into(),
                path: "/tmp/alpha".into(),
                progresso: 20.0,
                confianca: 75.0,
                phase: "Onda 1".into(),
                harness: None,
                model: None,
                reasoning_effort: None,
                no_agent: true,
            },
            &tmp,
        )
        .expect("init ok");
        assert!(out_init.contains("alpha"));

        let out_list = execute_non_gui_in_dir(
            Command::List {
                all: false,
                archived_only: false,
            },
            &tmp,
        )
        .expect("list ok");
        assert!(out_list.contains("Alpha Core"));
        assert!(out_list.contains("20.0%"));

        execute_non_gui_in_dir(
            Command::Archive {
                slug: "alpha".into(),
            },
            &tmp,
        )
        .expect("archive ok");

        let out_active = execute_non_gui_in_dir(
            Command::List {
                all: false,
                archived_only: false,
            },
            &tmp,
        )
        .expect("list active ok");
        assert!(!out_active.contains("Alpha Core"));

        let json_path = tmp.join("alpha.json");
        let out_val = execute_non_gui_in_dir(
            Command::Validate {
                file: json_path.to_string_lossy().to_string(),
            },
            &tmp,
        )
        .expect("validate ok");
        assert!(out_val.contains("JSON válido"));

        let sa_cmd = parse_cli_args(&[
            "set-agent".into(),
            "alpha".into(),
            "--agent-id".into(),
            "agent-123".into(),
            "--epic-id".into(),
            "epic-456".into(),
            "--harness".into(),
            "antigravity".into(),
            "--model".into(),
            "gemini-4-hydrogen".into(),
            "--reasoning-effort".into(),
            "high".into(),
        ])
        .expect("parse set-agent ok");
        let out_sa = execute_non_gui_in_dir(sa_cmd, &tmp).expect("set-agent ok");
        assert!(out_sa.contains("agent-123"));
        let doc_sa = read_project_file(&json_path).expect("read after set-agent");
        assert_eq!(
            doc_sa.agent.as_ref().and_then(|a| a.agent_id.as_deref()),
            Some("agent-123")
        );
        assert_eq!(
            doc_sa.agent.as_ref().and_then(|a| a.epic_id.as_deref()),
            Some("epic-456")
        );

        execute_non_gui_in_dir(
            Command::Delete {
                slug: "alpha".into(),
                yes: true,
            },
            &tmp,
        )
        .expect("delete ok");
        assert!(!json_path.exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
