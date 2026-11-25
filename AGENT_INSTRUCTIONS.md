# Instrução Global — Painel de Projetos (`claude-panorama`)

Todo projeto conduzido pelo Claude (ou pelo Maestro e seus subagentes) DEVE criar e manter atualizado o seu painel no **Claude Panorama**. O painel é um arquivo JSON em `/home/sacer/.local/share/claude-panorama/projects/<slug>.json`, gravado com a CLI `panorama`. O app nativo do GNOME o mostra e recarrega sozinho quando o arquivo muda.

## 1. Para quem o painel é escrito

O leitor é **alguém que não sabe nada do projeto**. Ele abre o painel e precisa sair entendendo:

1. **O que é o projeto** e para que serve;
2. **Como ele funciona**, em linhas gerais;
3. **Quanto já está pronto** e **o que falta**;
4. **O que está acontecendo agora** e **o que vem depois**;
5. **O que está dando errado** ou preocupa, e por que isso importa.

Se uma frase só faz sentido para quem já leu o código, ela está escrita para o leitor errado.

## 2. Regras de escrita

1. **Linguagem simples e direta.** Escreva como se explicasse a uma pessoa inteligente que nunca viu o projeto. Frases curtas. Um assunto por frase.
2. **Sem jargão na camada visível.** Nada de nomes de classe, struct, função, arquivo, commit, flag, variável de ambiente ou caminho de pasta no texto principal. Diga **o que a coisa faz**, não como ela se chama no código.
   - Ruim: "128 blocos `Kind::Inert` em `crates/ntm-core/src/registry.rs`".
   - Bom: "128 blocos já aparecem no jogo, mas ainda não fazem nada quando você interage com eles".
3. **Termo técnico inevitável → explique na primeira vez** e coloque-o no **glossário** (`glossary`). Ex.: "o *save* (a gravação do mundo no disco)".
4. **Sem analogias nem metáforas.** O usuário não gosta delas. Explique o mecanismo real em palavras simples, sem comparar com carros, cozinha, construção etc.
5. **Números com contexto.** "683 verificações automáticas passaram" é melhor que "683 checks". Diga de quê, e se é bom ou ruim.
6. **Nada de conteúdo perdido.** Todo detalhe relevante continua no painel, só que **organizado em camadas** (seção 3).

## 3. Organização em camadas (não deixar a tela densa)

O painel nunca deve despejar tudo de uma vez. Use três camadas:

| Camada | Onde fica | O que contém |
|---|---|---|
| **1. Visão rápida** | Topo de cada aba, sempre visível | 1 alerta ou cartão curto com o momento atual + 1 ou 2 visuais (rosca, etapas, barras, cartões). Lê-se em 30 segundos. |
| **2. Explicação** | `expander`s fechados (`"expanded": false`), com título claro e subtítulo de uma linha | O porquê, o como, a lista completa, o que falta em cada parte. Texto simples. |
| **3. Detalhe técnico** | Um `expander` final em cada aba, chamado **"Detalhes técnicos"** (ícone `applications-engineering-symbolic`), fechado | Aqui (e só aqui) podem aparecer nomes de arquivo, classe, commit, linha de código, para quem for mexer no projeto. |

Regras de densidade:
- No máximo **4 blocos visíveis** por aba antes do primeiro `expander`.
- Listas longas (mais de ~6 itens), tabelas e explicações vão **dentro de `expander`**.
- Prefira **um visual a um parágrafo** quando o conteúdo for número, proporção, sequência ou estado.
- Textos curtos: `prose` com no máximo ~4 frases; o resto vai para um `expander`.

## 4. Estrutura recomendada de abas

Adapte ao projeto, mas comece por esta base:

1. **Começo aqui**: o que é o projeto (`cards`), como funciona (`flow_graph` com rótulos simples), o momento atual (`alert`) e o glossário (`glossary` dentro de `expander`).
2. **Progresso**: jornada em etapas (`steps`), quanto está pronto (`donut`), progresso por parte (`progress_list` ou `bar_chart`), detalhes de cada parte em `expander`s.
3. **Agora**: o que está sendo feito, o que travou e por quê (`alert`, `cards`, `expander`s com cada problema explicado: o que acontece, por que importa, como será resolvido).
4. **Testes e qualidade**: o que já foi verificado (`bar_chart`/`matrix`), em linguagem simples.
5. Abas específicas do domínio (máquina, servidor, hardware, migração etc.), seguindo as mesmas camadas.

Use ícones simbólicos do GNOME nas abas, nos `cards` e nos `expander`s (ex.: `help-about-symbolic`, `view-grid-symbolic`, `object-select-symbolic`, `dialog-warning-symbolic`, `media-playlist-repeat-symbolic`, `computer-symbolic`, `folder-symbolic`, `system-run-symbolic`). Antes de usar um ícone, confira se ele existe no tema instalado: `find /usr/share/icons/Adwaita -name "<nome>.svg"`. Ícone inexistente aparece como um quadrado vazio.

## 5. Os dois indicadores fixos do topo

- **`progresso` (0.0–100.0)**: quanto do escopo total já está **feito e verificado**. Código escrito sem prova não conta.
  - `progresso_subtitulo`: uma linha simples ("cerca de 1/4 do jogo original já funciona").
  - `progresso_calculo`: como o número foi obtido, em linguagem simples. Fica recolhido em "Como esse número é calculado".
- **`confianca` (0.0–100.0)**: chance honesta, hoje, de o projeto ficar pronto e funcionar. Sobe com provas; desce com problemas reais. Nunca um número decorativo.
  - `confianca_resumo`: 1 ou 2 frases simples, sempre visíveis.
  - `confianca_fatores` / `confianca_riscos`: frases simples, sem jargão. Ficam recolhidas em "O que dá segurança" e "O que ainda preocupa".
- `subtitle` e `phase` também em linguagem simples ("Fase atual: corrigindo a gravação do mundo").

## 5.1 Fluxo do Agente Dedicado (Maestro → Subagente na aba Agents)

Cada projeto tem sua própria conversa (task/epic) no Traycer. **O maestro dessa conversa** (e não um botão solto fora do chat) deve criar o subagente dedicado do panorama na aba **Agents** usando `traycer_create_agent`, instruí-lo com `traycer_send_message` e vinculá-lo ao painel executando:

```bash
panorama set-agent <slug> --agent-id <ID_DO_SUBAGENTE> --epic-id <ID_DA_CONVERSA> --harness <harness> --model <modelo> [--reasoning-effort <nivel>]
```

Depois disso, o usuário poderá trocar o modelo do subagente diretamente no painel e clicar em **Atualizar** para enviar pedidos de atualização para esse subagente.

## 6. Quando atualizar

Atualize o painel sempre que:
- iniciar ou assumir um projeto (crie o registro na hora);
- concluir uma etapa, módulo ou onda;
- iniciar, avançar ou encerrar testes ou subagentes;
- descobrir um problema ou mudança de escopo que altere `progresso` ou `confianca`;
- terminar um turno relevante de trabalho.

## 7. CLI (`panorama` / `claude-panorama`)

Binários em `/home/sacer/.local/bin/`.

- `claude-panorama` ou `panorama gui`: abre a janela.
- `panorama schema`: imprime a especificação completa com exemplos de todos os blocos.
- `panorama list [--all | --archived]`: lista os projetos (ativos por padrão).
- `panorama models`: lista harnesses e modelos disponíveis no Traycer.
- `panorama init <slug> --title "..." [--path "..."] [--progresso 0] [--confianca 50] [--phase "..."]`: cria o registro JSON do projeto.
- `panorama set-agent <slug> --agent-id <id> --epic-id <id> [--harness id] [--model id] [--reasoning-effort nivel]`: vincula o subagente criado pelo maestro da conversa no Traycer ao panorama do projeto.
- `panorama validate <arquivo.json>`: valida o JSON (campos, faixas, cores, referências).
- `panorama apply <slug> [arquivo.json | -]`: valida, anexa um ponto ao histórico, calcula as variações e grava de forma atômica (preservando `archived`, `agent` e limites de `hardware`).
- `panorama update <slug>`: pede ao agente dedicado para reinspecionar o projeto e atualizar o panorama.
- `panorama archive <slug>` / `panorama unarchive <slug>`: arquiva ou restaura o projeto.
- `panorama delete <slug> --yes`: remove permanentemente o projeto.
- `panorama move <slug> --up | --down`: reordena o projeto na lista lateral.
- `panorama edit <slug> [--title "..."] [--icon <imagem|nome>] [--reset-icon]`: altera o nome exibido e/ou a imagem de ícone do projeto (preservados em futuras atualizações).
- `panorama limits <slug> [--cpus 0,1,6,7] [--cpu N] [--ssd %] [--ram GB] [--clear]`: define limites reais de hardware (núcleos/threads específicos, SSD e RAM) via cgroups v2.

### Campo `hardware` (Limites Seletivos)
Marque `heavy_cpu`, `heavy_ram` e `heavy_ssd` no JSON **somente** para os recursos em que o projeto realmente consome muito hardware (ex.: Rust usa muita CPU, RAM e SSD; projetos leves deixam todos `false`; se não usa muita RAM, deixe `heavy_ram: false`). A interface só exibe os controles para o que está marcado como pesado.

## 8. Blocos disponíveis (`"type": "..."`)

Estrutura:
- **`tabs`**: abas. `{ "type": "tabs", "tabs": [{ "id": "inicio", "title": "Começo aqui", "icon": "help-about-symbolic", "children": [...] }] }`
- **`section`**: grupo com título e descrição. `{ "type": "section", "title": "...", "description": "...", "children": [...] }`
- **`columns`**: blocos lado a lado. `{ "type": "columns", "columns": 2, "children": [...] }`
- **`expander`**: linha que abre e fecha. É a principal ferramenta contra a densidade. `{ "type": "expander", "title": "...", "subtitle": "uma linha", "icon": "dialog-warning-symbolic", "badge": "4 itens", "status": "ok|warn|err|neutral", "expanded": false, "children": [...] }`

Texto:
- **`prose`**: cartão de texto. `{ "type": "prose", "title": "...", "tone": "neutral|info|success|warning|danger", "body": "...", "bullets": ["..."] }`
- **`alert`**: aviso em destaque. `{ "type": "alert", "level": "info|success|warning|danger", "title": "...", "message": "..." }`
- **`cards`**: cartões com ícone grande, título e texto curto. Ótimo para "o que é", "como funciona", "próximos passos". `{ "type": "cards", "columns": 3, "items": [{ "icon": "help-about-symbolic", "title": "...", "text": "...", "tone": "info|success|warning|danger|neutral" }] }`
- **`glossary`**: termos explicados. `{ "type": "glossary", "title": "Glossário", "items": [{ "term": "Save", "meaning": "A gravação do mundo no disco." }] }`

Visuais:
- **`kpi_grid`**: cartões de números. `{ "type": "kpi_grid", "items": [{ "label": "...", "value": "...", "sublabel": "...", "status": "ok|warn|err|neutral" }] }`
- **`donut`**: rosca mostrando como um total se divide. `{ "type": "donut", "title": "...", "center_value": "3.290", "center_label": "itens no total", "slices": [{ "label": "Funcionando", "value": 800, "state": "pass", "detail": "..." }, { "label": "Falta fazer", "value": 2490, "color": "gray" }] }`
- **`bar_chart`**: barras horizontais com valores absolutos. `{ "type": "bar_chart", "title": "...", "unit": "testes", "max": 300, "bars": [{ "label": "...", "value": 133, "value_label": "133 de 133", "state": "pass", "detail": "..." }] }`
- **`progress_list`**: barras de porcentagem. `{ "type": "progress_list", "title": "...", "items": [{ "label": "...", "percent": 75.0, "detail": "...", "status": "ok|warn|err|neutral" }] }`
- **`steps`**: jornada em etapas numeradas; clicar numa etapa mostra a descrição. `{ "type": "steps", "title": "...", "subtitle": "...", "first_number": 1, "steps": [{ "title": "Base", "description": "...", "state": "pass" }] }`
- **`timeline`**: linha do tempo. `{ "type": "timeline", "title": "...", "items": [{ "timestamp": "07/11 12:40", "title": "...", "detail": "...", "state": "..." }] }`
- **`matrix`**: grade de quadradinhos clicáveis. `{ "type": "matrix", "title": "...", "columns": 6, "cells": [{ "label": "...", "sublabel": "...", "state": "...", "detail": "..." }] }`
- **`flow_graph`**: diagrama de caixas e setas, clicável. `{ "type": "flow_graph", "title": "...", "nodes": [{ "id": "a", "label": "...", "sublabel": "...", "state": "...", "col": 0, "detail": "..." }], "edges": [{ "from": "a", "to": "b", "label": "..." }] }`
- **`table`**: tabela. `{ "type": "table", "title": "...", "columns": ["A", "B"], "rows": [["...", "..."]] }`. Use pouco e quase sempre dentro de `expander`.

Valores aceitos:
- `state`: `pass` (pronto), `running` (em andamento), `fail` (com falha), `pending` (a fazer), `blocked` (travado), `reserved` (reservado).
- `color`: `blue`, `teal`, `green`, `yellow`, `orange`, `red`, `pink`, `purple`, `brown`, `gray`. Sem `color`, a cor vem do `state`.
