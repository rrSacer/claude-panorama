pub const SCHEMA_SPEC: &str = r#"# Claude Panorama — Especificação Completa de Schema JSON (`panorama schema`)

Arquivo alvo por projeto:
  `/home/sacer/.local/share/claude-panorama/projects/<slug>.json`

Comando recomendado para validar + anexar histórico + gravar atomicamente (`0666`):
  `panorama apply <slug> arquivo.json`
  `panorama apply <slug> - <<'EOF' ... EOF`

Regras obrigatórias de conteúdo (leia `AGENT_INSTRUCTIONS.md` do projeto claude-panorama para a versão completa):
0. PROJETO NOVO → PERGUNTE MODELO E EFFORT ANTES: antes de criar o subagente dedicado de um projeto novo, execute `panorama models` (ou `traycer_list_harness_models`) e PERGUNTE ao usuário qual harness/modelo e qual effort (`reasoningEffort`) usar; nunca escolha um padrão silenciosamente.
1. O LEITOR NÃO SABE NADA DO PROJETO: o painel deve explicar o que é, como funciona, quanto está pronto, o que está acontecendo agora e o que preocupa.
2. LINGUAGEM SIMPLES: sem nomes de classe, struct, função, arquivo, commit, flag ou caminho no texto visível. Diga o que a coisa faz. Termo técnico inevitável → explique na primeira vez e coloque no `glossary`.
3. ZERO ANALOGIAS OU METÁFORAS: explique o mecanismo real em palavras simples.
4. CAMADAS, SEM DENSIDADE: topo de cada aba = visão rápida (no máximo 4 blocos visíveis, de preferência visuais: `cards`, `donut`, `steps`, `bar_chart`, `progress_list`); explicações em `expander` fechados; nomes técnicos só num `expander` final "Detalhes técnicos". Nenhum detalhe relevante é removido, só guardado na camada certa.
5. `progresso` (0.0–100.0) mede apenas escopo feito e verificado; `confianca` (0.0–100.0) mede a chance real atual de o projeto ficar pronto e funcionar.

---

## 1. Campos de Topo (`ProjectDocument`)

```jsonc
{
  "slug": "meu-projeto",                    // opcional no input (preenchido pelo <slug> no `panorama apply`)
  "title": "Nome Real do Projeto",          // OBRIGATÓRIO
  "subtitle": "O que o projeto é, numa frase simples", // opcional
  "path": "/caminho/absoluto/do/repo",      // opcional
  "icon": "applications-engineering-symbolic", // opcional (ícone simbólico GNOME ou caminho de imagem PNG/JPG/WebP/SVG)
  "status": "ativo",                        // "ativo" | "validando" | "pausado" | "concluido" | "bloqueado"
  "phase": "Onda 0 — Integridade de Save",  // fase atual exibida no cabeçalho fixo
  "updated_at": "2026-11-07T21:45:00-03:00",// atualizado automaticamente no `panorama apply`

  // === ELEMENTO FIXO 1: PROGRESSO (0.0 a 100.0) ===
  "progresso": 34.5,
  "progresso_delta": 4.5,                   // opcional (calculado automaticamente do histórico se omitido)
  "progresso_subtitulo": "12/35 módulos validados em release",
  "progresso_calculo": "Soma ponderada: motor base 100% (peso 20%) + Onda 0 80% (peso 15%) + ...",

  // === ELEMENTO FIXO 2: CONFIANÇA (0.0 a 100.0) ===
  // Semântica automática do anel Cairo: >=75% verde (#2ec27e), 45–74.9% âmbar (#e5a50a), <45% vermelho (#e01b24)
  "confianca": 78.0,
  "confianca_delta": 2.0,                   // opcional (calculado automaticamente do histórico se omitido)
  "confianca_resumo": "Suíte unitária e ensaios de crash aprovados; resta fechar reconciliação de unload.",
  "confianca_fatores": [
    "398 testes automatizados NTM passando sem warnings no Clippy",
    "66 cortes de energia/save testados contra oráculo vanilla"
  ],
  "confianca_riscos": [
    "Callback native_chunks_changed executa após remoção de world.loaded",
    "2980 commits à frente no upstream com 14 arquivos em conflito mapeado"
  ],

  // === SÉRIE HISTÓRICA (preservada e anexada automaticamente no `panorama apply`) ===
  "history": [
    {
      "timestamp": "2026-11-07T18:00:00-03:00",
      "progresso": 30.0,
      "confianca": 76.0,
      "note": "Marco 1 revisado"
    }
  ],

  // === ARQUIVAMENTO / AGENTE / HARDWARE SELETIVO (preservados automaticamente no `panorama apply`) ===
  "archived": false,
  "hardware": {
    "heavy_cpu": true,                      // marque true SOMENTE se o projeto usa muita CPU
    "heavy_ram": true,                      // marque true SOMENTE se o projeto usa muita RAM (se não usa, false!)
    "heavy_ssd": false,                     // marque true SOMENTE se o projeto usa muito disco/SSD
    "reason": "Compilação ou simulação pesada",
    "cpu_threads": [0, 1, 6, 7],            // threads/núcleos específicos selecionados pelo usuário (preservado no apply)
    "ssd_percent": 60,                      // % máxima de banda do SSD (preservado no apply)
    "max_ram_gb": 12.0                      // limite máximo de RAM em GB (preservado no apply)
  },

  // === ÁREA DINÂMICA LIVRE (`ui`: array recursivo de `UiNode`) ===
  "ui": []
}
```

---

## 2. Catálogo Completo de Blocos `UiNode` (`"type": "..."`)

### 2.1 `tabs` — Abas de navegação (`AdwViewStack` + barra de abas nativa)
```json
{
  "type": "tabs",
  "id": "main_tabs",
  "tabs": [
    {
      "id": "visao-geral",
      "title": "Visão Geral",
      "icon": "view-grid-symbolic",
      "children": [ /* Vec<UiNode> */ ]
    }
  ]
}
```

### 2.2 `section` — Grupo Adwaita (`AdwPreferencesGroup`)
```json
{
  "type": "section",
  "title": "Subsistemas do Servidor",
  "description": "Estado por crate e contratos verificados",
  "children": [ /* Vec<UiNode> */ ]
}
```

### 2.3 `columns` — Grade responsiva lado a lado
```json
{
  "type": "columns",
  "columns": 2,
  "children": [ /* Vec<UiNode> */ ]
}
```

### 2.4 `prose` — Cartão técnico com parágrafos e bullets
`tone`: `"neutral"` | `"info"` | `"success"` | `"warning"` | `"danger"`
```json
{
  "type": "prose",
  "title": "Decisão Arquitetural",
  "tone": "info",
  "body": "Texto técnico direto.\n\nSegundo parágrafo com `identificadores`.",
  "bullets": [
    "Gravação em arquivo temporário `.tmp` seguida de `fsync` e `rename`",
    "Permissão `0666` compartilhada entre uid 0 e uid 1000"
  ]
}
```

### 2.5 `alert` — Banner de destaque semântico
`level`: `"info"` | `"success"` | `"warning"` | `"danger"`
```json
{
  "type": "alert",
  "level": "warning",
  "title": "Gargalo de Persistência Aberto",
  "message": "Sincronizar obrigações de `CHANGES` antes da retirada da coluna em `streaming.update`."
}
```

### 2.6 `kpi_grid` — Cartões numéricos de indicadores
`status`: `"ok"` | `"warn"` | `"err"` | `"neutral"`
```json
{
  "type": "kpi_grid",
  "items": [
    {
      "label": "Testes Unitários",
      "value": "372 / 372",
      "sublabel": "crates/ntm-core (0 falhas)",
      "status": "ok"
    }
  ]
}
```

### 2.7 `progress_list` — Barras de progresso por módulo/onda/subsistema
`status`: `"ok"` | `"warn"` | `"err"` | `"neutral"`
```json
{
  "type": "progress_list",
  "title": "Progresso por Onda",
  "items": [
    {
      "label": "Onda 0 — Integridade Transacional",
      "percent": 82.0,
      "detail": "Marcos 1–4 entregues; Marco 5 em revisão de borda R7",
      "status": "ok"
    }
  ]
}
```

### 2.8 `expander` — Linha expansível (`AdwExpanderRow`) com filhos arbitrários — principal ferramenta contra a densidade
`status`: `"ok"` | `"warn"` | `"err"` | `"neutral"`
```json
{
  "type": "expander",
  "title": "Inspeção Profunda: Conflitos Upstream (14 arquivos)",
  "subtitle": "Comparação entre base fc0210381 e upstream a61e02d36",
  "icon": "dialog-warning-symbolic",       // opcional: ícone simbólico GNOME à esquerda
  "badge": "14 arquivos",
  "status": "warn",
  "expanded": false,
  "children": [ /* tabelas, matrizes, textos, flow_graph etc. */ ]
}
```

### 2.9 `matrix` — Grade densa Cairo com inspeção interativa ao clicar/passar mouse
`state`: `"pass"` | `"running"` | `"fail"` | `"pending"` | `"blocked"` | `"reserved"`
```json
{
  "type": "matrix",
  "title": "Alocação de Threads — Ryzen 5 5500 (12 threads)",
  "subtitle": "Clique em qualquer célula para inspecionar afinidade e função",
  "columns": 6,
  "cells": [
    {
      "label": "CPU 0",
      "sublabel": "Core 0 T0",
      "state": "running",
      "detail": "Worker de compilação cargo / rustc permitido via taskset 0-4,6-10"
    },
    {
      "label": "CPU 5",
      "sublabel": "Core 5 T0",
      "state": "reserved",
      "detail": "Exclusivo do usuário sacer e compositor GNOME Wayland — proibido para agentes"
    }
  ]
}
```

### 2.10 `flow_graph` — Diagrama vetorial nativo Cairo de nós e arestas direcionadas
`state`: `"pass"` | `"running"` | `"fail"` | `"pending"` | `"blocked"` | `"reserved"`
```json
{
  "type": "flow_graph",
  "title": "Pipeline de Persistência de Mundo e KV",
  "subtitle": "Clique em um nó para abrir os detalhes de contrato",
  "nodes": [
    {
      "id": "tick",
      "label": "ServerTickEnd",
      "sublabel": "ntm-core",
      "state": "pass",
      "col": 0,
      "detail": "Consolida mutações de máquinas e drena fila de efeitos em memória."
    },
    {
      "id": "kv",
      "label": "store_commit",
      "sublabel": "KV + Region",
      "state": "running",
      "col": 1,
      "detail": "Persiste recibos e ledger coordenado com os arquivos de região."
    }
  ],
  "edges": [
    { "from": "tick", "to": "kv", "label": "recibos + ledger" }
  ]
}
```

### 2.11 `table` — Tabela técnica com quebra de linha completa (`WordChar`)
```json
{
  "type": "table",
  "title": "Arquivos Críticos",
  "columns": ["Arquivo", "Severidade", "Ação Técnica"],
  "rows": [
    [
      "crates/horizon-client/src/lan/menu.rs",
      "Alta (6 hunks)",
      "Integrar consentimento oficial com política condicional por slot e vacated slot"
    ]
  ]
}
```

### 2.12 `timeline` — Linha do tempo vertical cronológica
`state`: `"pass"` | `"running"` | `"fail"` | `"pending"` | `"blocked"` | `"reserved"`
```json
{
  "type": "timeline",
  "title": "Eventos Recentes de Engenharia",
  "items": [
    {
      "timestamp": "2026-11-07 12:40",
      "title": "Revisão R7 do Save Atômico",
      "detail": "Mapeados achados R7-1 a R7-4 com reprodução determinística.",
      "state": "running"
    }
  ]
}
```

### 2.13 `cards` — Cartões com ícone grande, título e texto curto
`tone`: `"info"` | `"success"` | `"warning"` | `"danger"` | `"neutral"` (cor do selo do ícone)
```json
{
  "type": "cards",
  "columns": 3,
  "items": [
    {
      "icon": "help-about-symbolic",
      "title": "O que é",
      "text": "Uma versão nova, mais rápida, de um mod antigo de tecnologia nuclear.",
      "tone": "info"
    }
  ]
}
```

### 2.14 `donut` — Rosca mostrando como um total se divide em partes
`color` (opcional): `blue` | `teal` | `green` | `yellow` | `orange` | `red` | `pink` | `purple` | `brown` | `gray`. Sem `color`, usa a cor do `state`.
```json
{
  "type": "donut",
  "title": "Quanto do conteúdo já funciona",
  "subtitle": "Contagem de blocos, itens e máquinas do mod original",
  "center_value": "3.290",
  "center_label": "itens no total",
  "slices": [
    { "label": "Funcionando", "value": 800, "state": "pass", "detail": "Já testado dentro do jogo" },
    { "label": "Aparece mas não funciona", "value": 128, "state": "blocked" },
    { "label": "Falta fazer", "value": 2362, "color": "gray" }
  ]
}
```

### 2.15 `bar_chart` — Barras horizontais com valores absolutos
`max` opcional (padrão: maior valor). `value_label` opcional substitui o número exibido.
```json
{
  "type": "bar_chart",
  "title": "Verificações automáticas dentro do jogo",
  "unit": "testes",
  "bars": [
    { "label": "Catálogo", "value": 251, "value_label": "251 de 251", "state": "pass", "detail": "Todos os blocos e itens registrados corretamente" }
  ]
}
```

### 2.16 `steps` — Jornada em etapas numeradas (clicar abre a descrição)
```json
{
  "type": "steps",
  "title": "Caminho até o fim",
  "subtitle": "Clique numa etapa para ver o que ela inclui",
  "first_number": 0,                       // opcional: número do primeiro círculo (padrão 1)
  "steps": [
    { "title": "Base", "description": "O que esta etapa entrega, em palavras simples.", "state": "pass" },
    { "title": "Reatores", "description": "...", "state": "running" },
    { "title": "Mísseis", "state": "pending" }
  ]
}
```

### 2.17 `glossary` — Termos explicados em linguagem simples
```json
{
  "type": "glossary",
  "title": "Glossário",
  "items": [
    { "term": "Save", "meaning": "A gravação do mundo no disco, para que nada se perca ao fechar o jogo." }
  ]
}
```
"#;
