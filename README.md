# Claude Panorama 🔭

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![GNOME / GTK4](https://img.shields.io/badge/GTK-4.14-blue?logo=gnome)](https://www.gtk.org/)
[![Libadwaita](https://img.shields.io/badge/Libadwaita-1.6-purple)](https://gnome.pages.gitlab.gnome.org/libadwaita/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

**Claude Panorama** é um painel de controle nativo GNOME (GTK4 + Libadwaita), CLI e daemon de telemetria/cgroups v2 para monitoramento de projetos em tempo real, acompanhamento visual de progresso e orquestração de **agentes dedicados no Traycer**.

---

## ✨ Recursos Principais

### 🎨 Interface Nativa GNOME (Libadwaita)
- **Design moderno e adaptativo**: barra lateral com busca instantânea, status com pills coloridas, reordenação de projetos com setas ↑/↓ e modo compacto responsivo (breakpoint para telas pequenas).
- **Identidade visual do projeto**: suporte a títulos customizados e ícones personalizados (carregue imagens em PNG, JPG, WebP, SVG ou ícones temáticos do sistema).
- **Organização em abas e camadas**: cartões de resumo, gráficos de rosca (*donut*), gráficos de barras, grafos de fluxo (*flow graph*), etapas (*steps*), métricas de qualidade, alertas contextuais, glossários e seções recolhíveis (*expanders*).
- **Indicadores essenciais**:
  - **Progresso (%)**: percentual verificado do escopo com cálculo explicado e histórico evolutivo.
  - **Confiança (%)**: métrica honesta de viabilidade técnica, fatores de segurança e riscos monitorados.

### 🤖 Agentes Dedicados e Integração Traycer
- **Subagentes vinculados à conversa do projeto**: o maestro cria o subagente dedicado na aba *Agents* do Traycer e vincula ao panorama (`panorama set-agent`).
- **Configuração em tempo de execução**: escolha qualquer harness disponível no Traycer (**Antigravity**, **Codex**, **Hermes Agent**), modelo e nível de raciocínio (*effort*: `low`, `medium`, `high`, `xhigh`, `max`, `ultra`).
- **Botão "Pedir Atualização"**: envia instruções ao agente dedicado via WebSocket/IPC local para reinspecionar o repositório (`git status`, testes, logs) e reemitir o estado atualizado.
- **Ciclo de vida sincronizado**: arquivamento e desarquivamento de projetos no panorama sincronizam automaticamente com o agente no Traycer.

### ⚡ Limites de Hardware em Tempo Real (Cgroup v2)
- **Detecção contextual**: exibe controles de limitação apenas quando o projeto realmente demanda (ex.: projetos Rust com compilações pesadas de CPU, RAM e SSD).
- **Afinidade por núcleo e thread de CPU**: seletor interativo por núcleo físico e hiperthreads individuais (ex.: 6 núcleos / 12 threads), além de limite percentual global.
- **Controle de I/O de SSD e Teto de RAM**: limites em MB/s e GB com aplicação atômica via cgroup v2 pelo daemon em segundo plano (`claude-panorama-daemon`).

---

## 🏛️ Arquitetura

```text
┌────────────────────────────────────────┐
│      Claude Panorama GUI (GTK4)        │  <-- Interface de usuário nativa
│     cli: `claude-panorama` / `panorama`│
└──────────────────┬─────────────────────┘
                   │ UNIX Socket (/run/claude-panorama/daemon.sock)
                   ▼
┌────────────────────────────────────────┐
│        claude-panorama-daemon          │  <-- Serviço systemd local
│  • Cgroups v2 & Hardware Limiter       │
│  • Traycer Bridge & Catalog Cache      │
└──────────────────┬─────────────────────┘
                   │ WebSocket & CLI
                   ▼
┌────────────────────────────────────────┐
│             Traycer Host               │  <-- Antigravity / Codex / Hermes
│     Agentes Dedicados nos Projetos     │
└────────────────────────────────────────┘
```

Os dados dos panoramas são armazenados em JSON limpo e legível em:
`~/.local/share/claude-panorama/projects/<slug>.json`

---

## 🚀 Instalação e Execução

### Pré-requisitos
- Linux com **systemd** e **cgroup v2** habilitado
- **Rust 1.80+** (`cargo`)
- Bibliotecas GTK4 e Libadwaita (`libgtk-4-dev`, `libadwaita-1-dev`)

### Compilando e Instalando

```bash
# Clone o repositório
git clone https://github.com/rrSacer/claude-panorama.git
cd claude-panorama

# Testes de unidade e integridade
cargo test

# Build otimizado em release
cargo build --release

# Instalar binários do usuário
install -m 755 target/release/claude-panorama ~/.local/bin/claude-panorama
ln -sf ~/.local/bin/claude-panorama ~/.local/bin/panorama

# Instalar o daemon do sistema (requer sudo)
sudo install -m 755 target/release/claude-panorama-daemon /usr/local/bin/claude-panorama-daemon
sudo systemctl restart claude-panorama-daemon.service
```

---

## 🛠️ Uso da Linha de Comando (`panorama`)

A CLI oferece controle completo e scriptável de todos os aspectos dos projetos:

```bash
# Abrir a interface gráfica
panorama gui
# ou simplesmente: claude-panorama

# Listar modelos e harnesses disponíveis no Traycer
panorama models

# Criar um novo panorama para um projeto
panorama init meu-projeto \
  --title "Meu Projeto Incrível" \
  --path "/home/usuario/Projects/meu-projeto" \
  --progresso 25 \
  --confianca 80 \
  --phase "Fase 1: Protótipo"

# Vincular o subagente dedicado criado no Traycer
panorama set-agent meu-projeto \
  --agent-id "7161daf3-1d7b-4f1d-a684-51c8a944cd4c" \
  --epic-id "f4800439-32f0-4c7c-bf3f-316838746f25" \
  --harness "antigravity" \
  --model "gemini-4-hydrogen" \
  --reasoning-effort "high"

# Solicitar atualização ao subagente dedicado
panorama update meu-projeto

# Definir limites de hardware em tempo real (ex.: núcleos 0 a 5, 4 GB de RAM, 50% de SSD)
panorama limits meu-projeto --cpus 0,1,2,3,4,5 --ram 4.0 --ssd 50

# Ajustar identidade (nome ou ícone com imagem personalizada)
panorama edit meu-projeto --title "Novo Nome" --icon "/caminho/para/icone.png"

# Reordenar projetos na lista lateral
panorama move meu-projeto --up
panorama move meu-projeto --down

# Arquivar ou desarquivar
panorama archive meu-projeto
panorama unarchive meu-projeto

# Validar e aplicar schemas JSON diretamente
panorama validate meu-projeto.json
panorama apply meu-projeto meu-projeto.json
```

---

## 📄 Regras de Redação para Agentes

Consulte o documento completo em [AGENT_INSTRUCTIONS.md](AGENT_INSTRUCTIONS.md) para detalhes de como os agentes devem preencher os panoramas:
- **Linguagem simples e direta**: frases curtas sem jargão hermético no texto principal.
- **Divisão em 3 camadas**: visão rápida em até 30 segundos, explicações em expanders e detalhes técnicos isolados no rodapé.
- **Sem metáforas nem dados inventados**: medições reais baseadas no código e testes verificados.

---

## 📜 Licença

Distribuído sob a licença **MIT**. Veja [LICENSE](LICENSE) para mais detalhes.
