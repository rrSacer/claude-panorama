# Instrução Global — Dashboard Nativo GNOME (`claude-panorama`)

Todo projeto conduzido pelo Claude (ou pelo Maestro e seus subagentes) DEVE registrar e manter atualizado seu painel em tempo real no **Claude Panorama** (`panorama` CLI / JSON em `/home/sacer/.local/share/claude-panorama/projects/<slug>.json`).

## 1. Propósito do Dashboard
Dar ao usuário visibilidade completa, fiel e atualizada sobre o estado real do projeto no aplicativo nativo GNOME:
- **2 indicadores fixos obrigatórios** no topo de todo projeto:
  1. **`confianca` (`0.0` a `100.0`)**: estimativa honesta e dinâmica da chance percentual de o projeto ficar pronto e funcionar com sucesso. Sobe quando provas empíricas, compilações e testes passam; desce imediatamente quando surgem conflitos graves, regressões, limites técnicos ou gargalos reais. Nunca congele um número decorativo: explique objetivamente o que sustenta (`confianca_fatores`) e o que reduz (`confianca_riscos`) essa porcentagem. No aplicativo, o anel circular Cairo assume automaticamente verde (`>= 75%`), âmbar (`45–74.9%`) ou vermelho (`< 45%`).
  2. **`progresso` (`0.0` a `100.0`)**: porcentagem real de quanto do escopo total do projeto já está implementado e verificado (sem chutes otimistas; diferencie código escrito de código validado em `progresso_calculo`).
- **Área dinâmica livre (`ui`)**: começa vazia e **você (o agente construtor do projeto) projeta, monta e reconstrói dinamicamente toda a interface abaixo dos 2 indicadores fixos**, adaptando os componentes visuais ao domínio específico do projeto e à etapa em que ele se encontra.

## 2. Regras Inegociáveis de Conteúdo e Estilo
1. **ZERO ANALOGIAS OU METÁFORAS**: O usuário não gosta de analogias. Nunca compare software a construção civil, carros, cozinha, orquestras, encanamento etc. Explique sempre o mecanismo real de forma direta, clara, objetiva e acessível (ex.: diga exatamente qual arquivo, processo, protocolo, memória, fila ou teste está atuando, o que ele faz e qual o efeito prático).
2. **MÁXIMO DE DETALHES POSSÍVEL (estruturado em camadas)**:
   - O panorama NÃO é um resumo raso de 3 linhas. Ele deve conter **todos os detalhes relevantes do projeto**: arquitetura real, módulos/subsistemas, arquivos críticos, ondas/etapas, testes executados vs. pendentes (com contagens reais), métricas de desempenho/hardware, subagentes ativos, decisões técnicas tomadas, gargalos atuais, falhas abertas e próximos passos exatos.
   - Use **abas (`tabs`)**, **grupos (`section`)**, **matrizes visuais (`matrix`)**, **diagramas de fluxo/arquitetura (`flow_graph`)**, **tabelas (`table`)** e **linhas expansíveis (`expander`)** para entregar densidade máxima de detalhes sem poluir a leitura inicial: o topo explica claramente o momento atual, e cada seção/expander entrega a radiografia profunda.
3. **CRIATIVIDADE ADAPTADA AO PROJETO E À FASE**:
   - A estrutura da UI não é um formulário fixo: adapte-a à natureza do projeto:
     - *Projeto de motor/mod/compilador*: mostre ondas de engenharia, matriz de baterias de testes/crashes, cobertura de famílias de máquinas/blocos, alocação de núcleos/threads/RAM, conflitos de merge.
     - *Projeto de rede/streaming/daemon*: mostre topologia do pipeline de captura → codificação → transporte, latência medida, estados dos serviços systemd/sockets, protocolos.
     - *Projeto de app/web/ferramenta*: mostre telas/rotas prontas, contratos de dados, integrações, validações de build.
   - Conforme o projeto avança (ex.: de *Planejamento* → *Implementação* → *Validação Massiva* → *Revisão Final*), reorganize os widgets dinamicamente para destacar o que está acontecendo **agora**.

## 3. Quando Atualizar
Atualize o JSON do projeto sempre que:
- Iniciar ou assumir um projeto (crie o registro inicial imediatamente);
- Concluir uma etapa, módulo, commit ou onda;
- Disparar, avançar ou encerrar baterias de testes ou subagentes;
- Descobrir um bug, gargalo ou mudança de escopo que altere `progresso` ou `confianca`;
- Finalizar um turno relevante de engenharia.

## 4. Referência Exata da CLI (`panorama` / `claude-panorama`)
Os binários estão instalados em `/home/sacer/.local/bin/panorama` e `/home/sacer/.local/bin/claude-panorama`:

- `claude-panorama` ou `panorama gui`
  Abre a janela nativa GNOME (`AdwApplicationWindow` + `AdwNavigationSplitView`) com recarregamento em tempo real (`GIO FileMonitor` + fallback de 2s) preservando projeto selecionado, aba ativa, `expander`s abertos e posição de rolagem.
- `panorama schema`
  Imprime no `stdout` a especificação JSON completa e comentada de todos os campos de topo e exemplos prontos de todos os 12 blocos `UiNode`.
- `panorama list`
  Lista os projetos registrados em `/home/sacer/.local/share/claude-panorama/projects` com status, `progresso`, `confianca` e timestamp.
- `panorama init <slug> --title "..." [--path "..."] [--progresso 0] [--confianca 50] [--phase "..."]`
  Inicializa atomicamente um novo projeto com permissões compartilhadas (`0666` no arquivo, `0777` no diretório, compatível entre `root` e `sacer`).
- `panorama validate <arquivo.json>`
  Valida sintaxe JSON, nomes de campos (rejeita chaves com erro de digitação informando linha/coluna), faixas `0.0..=100.0`, consistência de colunas de tabelas e referências de nós em `flow_graph`.
- `panorama apply <slug> [arquivo.json | -]`
  Valida o JSON (de arquivo ou `stdin`), preserva/anexa automaticamente um novo ponto em `history`, calcula `progresso_delta` e `confianca_delta` se omitidos, e grava de forma atômica (`.tmp` + `fsync` + `rename` com modo `0666`).

Exemplo rápido via heredoc:
```bash
panorama apply meu-projeto - <<'EOF'
{
  "title": "Meu Projeto",
  "status": "ativo",
  "phase": "Implementação do Core",
  "progresso": 40.0,
  "progresso_calculo": "2 de 5 subsistemas implementados com suíte de testes passando.",
  "confianca": 82.0,
  "confianca_resumo": "Contratos principais provados em testes de integração.",
  "confianca_fatores": ["Build limpo e 48 testes unitários verdes"],
  "confianca_riscos": ["Integração IPC ainda não exercitada sob carga"],
  "ui": []
}
EOF
```

## 5. Resumo Conciso da Raiz JSON e dos 12 Blocos `UiNode`

### Campos de topo (`ProjectDocument`)
- Obrigatório: `title` (`string`).
- Principais opcionais (`#[serde(default)]`): `slug`, `subtitle`, `path`, `icon` (ex.: `"applications-engineering-symbolic"`, `"utilities-terminal-symbolic"`, `"cpu-symbolic"`), `status` (`"ativo"` | `"validando"` | `"pausado"` | `"concluido"` | `"bloqueado"`), `phase`, `updated_at`.
- Progresso fixo: `progresso` (`0.0..=100.0`), `progresso_delta` (`number` opcional), `progresso_subtitulo`, `progresso_calculo`.
- Confiança fixa: `confianca` (`0.0..=100.0`), `confianca_delta` (`number` opcional), `confianca_resumo`, `confianca_fatores` (`string[]`), `confianca_riscos` (`string[]`).
- Histórico e UI: `history` (`[{ "timestamp", "progresso", "confianca", "note" }]`), `ui` (`UiNode[]`).

### Blocos `UiNode` (sempre identificados por `"type": "<nome>"`)
1. **`tabs`**: `{ "type": "tabs", "id": "main", "tabs": [{ "id": "visao", "title": "Visão Geral", "icon": "view-grid-symbolic", "children": [...] }] }`
2. **`section`**: `{ "type": "section", "title": "...", "description": "...", "children": [...] }` (`AdwPreferencesGroup`).
3. **`columns`**: `{ "type": "columns", "columns": 2, "children": [...] }` (grade responsiva lado a lado).
4. **`prose`**: `{ "type": "prose", "title": "...", "tone": "neutral|info|success|warning|danger", "body": "Parágrafos com `código` e **negrito**\n- Bullet", "bullets": ["..."] }`.
5. **`alert`**: `{ "type": "alert", "level": "info|success|warning|danger", "title": "...", "message": "..." }`.
6. **`kpi_grid`**: `{ "type": "kpi_grid", "items": [{ "label": "...", "value": "...", "sublabel": "...", "status": "ok|warn|err|neutral" }] }`.
7. **`progress_list`**: `{ "type": "progress_list", "title": "...", "items": [{ "label": "...", "percent": 75.0, "detail": "...", "status": "ok|warn|err|neutral" }] }`.
8. **`expander`**: `{ "type": "expander", "title": "...", "subtitle": "...", "badge": "63/63 OK", "status": "ok|warn|err|neutral", "expanded": false, "children": [...] }` (`AdwExpanderRow` aninhando qualquer `UiNode`).
9. **`matrix`**: `{ "type": "matrix", "title": "...", "subtitle": "...", "columns": 6, "cells": [{ "label": "...", "sublabel": "...", "state": "pass|running|fail|pending|blocked|reserved", "detail": "..." }] }` (grade densa Cairo com tooltip e popover de inspeção ao clicar).
10. **`flow_graph`**: `{ "type": "flow_graph", "title": "...", "subtitle": "...", "nodes": [{ "id": "n1", "label": "...", "sublabel": "...", "state": "pass|running|fail|pending|blocked|reserved", "col": 0, "detail": "..." }], "edges": [{ "from": "n1", "to": "n2", "label": "..." }] }` (diagrama vetorial Cairo clicável).
11. **`table`**: `{ "type": "table", "title": "...", "columns": ["Col A", "Col B"], "rows": [["...", "..."]] }` (com quebra `WordChar` sem truncar texto).
12. **`timeline`**: `{ "type": "timeline", "title": "...", "items": [{ "timestamp": "...", "title": "...", "detail": "...", "state": "pass|running|fail|pending|blocked|reserved" }] }`.
