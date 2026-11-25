use crate::client;
use crate::model::{parse_and_validate_json, PanoramaAgentConfig};
use crate::schema::SCHEMA_SPEC;
use crate::store::{
    apply_project_json, delete_project, init_project, load_all_projects, move_project_in_order,
    project_file_path, read_project_file, resolve_projects_dir, set_project_archived,
    update_project_hardware, InitOptions,
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
    Limits {
        slug: String,
        cpu_cores: Option<u32>,
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
        "  panorama init <slug> --title \"...\" [opções] Cria projeto + inicia agente dedicado\n",
        "  panorama apply <slug> [arquivo.json | -]   Valida e grava JSON atomicamente\n",
        "  panorama validate <arquivo.json | ->       Valida um JSON sem salvar\n",
        "  panorama update <slug>                     Pede ao agente dedicado para atualizar\n",
        "  panorama archive <slug>                    Arquiva o projeto\n",
        "  panorama unarchive <slug>                  Restaura um projeto arquivado\n",
        "  panorama delete <slug> --yes               Deleta permanentemente o projeto\n",
        "  panorama move <slug> --up | --down         Altera a ordem do projeto na lista\n",
        "  panorama limits <slug> [--cpu N] [--ssd %] [--ram GB] [--clear]\n",
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
        "  --no-agent                                 Apenas cria o JSON sem iniciar agente\n",
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

fn parse_apply_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() || rest.len() > 2 {
        return Err("Uso: `panorama apply <slug> [arquivo.json | -]`".to_string());
    }
    let slug = rest[0].clone();
    let source = rest.get(1).cloned();
    Ok(Command::Apply { slug, source })
}

fn parse_limits_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() {
        return Err("Uso: `panorama limits <slug> [--cpu N] [--ssd %] [--ram GB] [--clear]`".to_string());
    }
    let slug = rest[0].clone();
    let mut cpu_cores = None;
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
            no_agent,
        } => {
            let initial_agent = match (&harness, &model) {
                (Some(h), Some(m)) => Some(PanoramaAgentConfig {
                    agent_id: None,
                    epic_id: None,
                    harness_id: h.clone(),
                    model: m.clone(),
                    reasoning_effort: reasoning_effort.clone(),
                    last_requested_at: None,
                    last_status: None,
                }),
                _ => None,
            };
            let written = init_project(
                dir,
                InitOptions {
                    slug: slug.clone(),
                    title: title.clone(),
                    path: path.clone(),
                    progresso,
                    confianca,
                    phase,
                    agent: initial_agent,
                    hardware: None,
                },
            )?;

            let mut msg = format!(
                "Projeto `{slug}` inicializado com sucesso em `{}`.",
                written.display()
            );

            if !no_agent {
                let chosen = match (harness, model) {
                    (Some(h), Some(m)) => Ok((h, m, reasoning_effort)),
                    (h_opt, m_opt) => match client::list_traycer_models(false) {
                        Ok(cat) => {
                            if let Some(h) = h_opt {
                                if let Some(hi) = cat.harnesses.iter().find(|x| x.id == h) {
                                    let chosen_m = m_opt
                                        .or_else(|| hi.models.first().map(|mm| mm.id.clone()))
                                        .ok_or_else(|| format!("Harness `{h}` sem modelos."))?;
                                    Ok((h, chosen_m, reasoning_effort))
                                } else {
                                    Err(format!("Harness `{h}` não encontrado no Traycer."))
                                }
                            } else {
                                pick_default_model(&cat)
                                    .ok_or_else(|| "Nenhum modelo disponível no Traycer.".to_string())
                            }
                        }
                        Err(e) => Err(e),
                    },
                };

                match chosen {
                    Ok((h, m, eff)) => {
                        match client::start_panorama_agent(
                            dir,
                            &slug,
                            &title,
                            &path,
                            &h,
                            &m,
                            eff.as_deref(),
                        ) {
                            Ok(res) => {
                                msg.push_str(&format!("\n{}", res.message));
                            }
                            Err(e) => {
                                msg.push_str(&format!(
                                    "\nAviso: JSON criado, mas não foi possível iniciar o agente dedicado ({e})."
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        msg.push_str(&format!(
                            "\nAviso: JSON criado sem agente ativo ({e})."
                        ));
                    }
                }
            }

            Ok(msg)
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
        Command::Limits {
            slug,
            cpu_cores,
            ssd_percent,
            max_ram_gb,
            clear,
        } => {
            let path = project_file_path(dir, &slug);
            let doc = read_project_file(&path).map_err(|e| e.to_string())?;
            let mut hw = doc.hardware.clone().unwrap_or_default();
            if clear {
                hw.cpu_cores = None;
                hw.ssd_percent = None;
                hw.max_ram_gb = None;
            } else {
                if let Some(c) = cpu_cores {
                    hw.cpu_cores = Some(c);
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
            if h.cpu_cores.is_some() || h.ssd_percent.is_some() || h.max_ram_gb.is_some() {
                let _ = client::apply_hardware_limits(
                    &slug,
                    &doc.path,
                    h.cpu_cores,
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
