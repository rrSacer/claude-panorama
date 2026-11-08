use crate::model::parse_and_validate_json;
use crate::schema::SCHEMA_SPEC;
use crate::store::{
    apply_project_json, init_project, load_all_projects, resolve_projects_dir, InitOptions,
};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Gui,
    Schema,
    List,
    Init {
        slug: String,
        title: String,
        path: String,
        progresso: f64,
        confianca: f64,
        phase: String,
    },
    Apply {
        slug: String,
        source: Option<String>,
    },
    Validate {
        file: String,
    },
    Help,
    Version,
}

pub fn usage_text() -> &'static str {
    "\
Claude Panorama (`claude-panorama` / `panorama`) — Painel Nativo GNOME + CLI

USO:
  claude-panorama                Abre o aplicativo nativo GNOME (GTK4 / Libadwaita)
  panorama gui                   Abre o aplicativo nativo GNOME (GTK4 / Libadwaita)
  panorama schema                Imprime a especificação JSON completa e exemplos de `UiNode`
  panorama list                  Lista projetos registrados com progresso, confiança e timestamp
  panorama init <slug> --title \"...\" [--path \"...\"] [--progresso 0] [--confianca 50] [--phase \"...\"]
  panorama apply <slug> [arquivo.json | -]
                                 Valida JSON (arquivo ou stdin), anexa ponto em `history` e grava atomicamente
  panorama validate <arquivo.json>
                                 Valida sintaxe, tipos, faixas e referências de um arquivo JSON
  panorama --help | -h           Exibe esta ajuda

VARIÁVEIS DE AMBIENTE:
  CLAUDE_PANORAMA_DIR            Sobrescreve `/home/sacer/.local/share/claude-panorama/projects`
"
}

pub fn parse_cli_args(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Gui);
    }
    match args[0].as_str() {
        "gui" => {
            if args.len() > 1 {
                return Err(format!(
                    "`panorama gui` não aceita argumentos extras (recebido `{}`).",
                    args[1]
                ));
            }
            Ok(Command::Gui)
        }
        "schema" => Ok(Command::Schema),
        "list" | "ls" => Ok(Command::List),
        "help" | "--help" | "-h" => Ok(Command::Help),
        "version" | "--version" | "-V" => Ok(Command::Version),
        "init" => parse_init_args(&args[1..]),
        "apply" => parse_apply_args(&args[1..]),
        "validate" => {
            if args.len() != 2 {
                return Err("Uso: `panorama validate <arquivo.json>`".to_string());
            }
            Ok(Command::Validate {
                file: args[1].clone(),
            })
        }
        other => Err(format!(
            "Subcomando desconhecido `{other}`.\n\n{}",
            usage_text()
        )),
    }
}

fn parse_init_args(rest: &[String]) -> Result<Command, String> {
    if rest.is_empty() {
        return Err(
            "Uso: `panorama init <slug> --title \"...\" [--path \"...\"] [--progresso 0] [--confianca 50] [--phase \"...\"]`"
                .to_string(),
        );
    }
    let slug = rest[0].clone();
    if slug.starts_with('-') {
        return Err("Informe `<slug>` como primeiro argumento de `panorama init`.".to_string());
    }

    let mut title: Option<String> = None;
    let mut path = String::new();
    let mut progresso = 0.0_f64;
    let mut confianca = 50.0_f64;
    let mut phase = "Inicialização".to_string();

    let mut i = 1;
    while i < rest.len() {
        let flag = rest[i].as_str();
        let next_val = |idx: usize, name: &str| -> Result<String, String> {
            rest.get(idx + 1)
                .cloned()
                .ok_or_else(|| format!("A opção `{name}` exige um valor."))
        };
        match flag {
            "--title" | "-t" => {
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
        Command::List => {
            let loaded = load_all_projects(dir);
            if loaded.projects.is_empty() && loaded.errors.is_empty() {
                return Ok(format!(
                    "Nenhum projeto registrado em `{}`.\nUse `panorama init <slug> --title \"...\"` ou `panorama apply <slug> arquivo.json`.",
                    dir.display()
                ));
            }
            let mut out = String::new();
            out.push_str(&format!(
                "{:<22} {:<11} {:>10} {:>10}  {:<25}  TÍTULO\n",
                "SLUG", "STATUS", "PROGRESSO", "CONFIANÇA", "ATUALIZADO EM"
            ));
            out.push_str(&"-".repeat(104));
            out.push('\n');
            for p in &loaded.projects {
                out.push_str(&format!(
                    "{:<22} {:<11} {:>9.1}% {:>9.1}%  {:<25}  {}\n",
                    p.slug,
                    p.status.label(),
                    p.progresso,
                    p.confianca,
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
        } => {
            let written = init_project(
                dir,
                InitOptions {
                    slug: slug.clone(),
                    title,
                    path,
                    progresso,
                    confianca,
                    phase,
                },
            )?;
            Ok(format!(
                "Projeto `{slug}` inicializado com sucesso em `{}`.",
                written.display()
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
    fn cli_parses_init_apply_validate_and_gui() {
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
            }
        );
    }

    #[test]
    fn cli_end_to_end_init_list_validate() {
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
            },
            &tmp,
        )
        .expect("init ok");
        assert!(out_init.contains("alpha"));

        let out_list = execute_non_gui_in_dir(Command::List, &tmp).expect("list ok");
        assert!(out_list.contains("Alpha Core"));
        assert!(out_list.contains("20.0%"));

        let json_path = tmp.join("alpha.json");
        let out_val = execute_non_gui_in_dir(
            Command::Validate {
                file: json_path.to_string_lossy().to_string(),
            },
            &tmp,
        )
        .expect("validate ok");
        assert!(out_val.contains("JSON válido"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
