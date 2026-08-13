# ♟️ YukiChess

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-2021-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Stockfish](https://img.shields.io/badge/Powered%20by-Stockfish-769656?logo=chess&logoColor=white)](https://stockfishchess.org/)

Um jogo de xadrez de mesa construído com **Tauri + TypeScript** no front-end e um **motor de regras em Rust** escrito do zero — sem bibliotecas de xadrez de terceiros. Roque, en passant, promoção de peão e todo o resto das regras são validados manualmente no back-end.

Como bônus, o **Stockfish** entra como parceiro de treino: barra de avaliação em tempo real, dicas de lance, feedback de qualidade de jogada e um oponente com força ajustável por Elo.

## ✨ Funcionalidades

- **Tabuleiro interativo** com destaque de lances legais, casas de captura e seleção de peça
- **Regras completas** implementadas em Rust: roque (com verificação de casas atacadas), en passant, promoção escolhível e detecção de fim de jogo
- **Histórico de lances** em notação simplificada, peças capturadas em bandeja e status da partida em tempo real
- **Modo vs. motor**: jogue contra o Stockfish escolhendo sua cor e a força (Elo) do oponente
- **Barra de avaliação** ao vivo mostrando quem está melhor na posição
- **Dicas de lance** e **classificação de jogadas** (Ótimo / Bom / Impreciso / Erro / Erro grave) comparando a avaliação antes e depois de cada lance — ótimo para quem está aprendendo
- Interface 100% em português

## 🧱 Stack técnica

| Camada       | Tecnologia                          |
|--------------|--------------------------------------|
| Desktop shell | [Tauri 2](https://tauri.app/)       |
| Front-end     | TypeScript + Vite (vanilla, sem framework) |
| Back-end      | Rust (regras do jogo em `src-tauri/src/chess.rs`) |
| Motor de xadrez | [Stockfish](https://stockfishchess.org/) via UCI (processo externo) |

## 🚀 Como rodar

### Pré-requisitos

- [Node.js](https://nodejs.org/) (com npm)
- [Rust](https://www.rust-lang.org/tools/install) + toolchain do [Tauri](https://tauri.app/start/prerequisites/)
- [Stockfish](https://stockfishchess.org/download/) instalado e disponível no `PATH` (necessário apenas para os recursos de análise/oponente — o jogo entre dois jogadores humanos funciona sem ele)

### Instalação e execução

```bash
npm install
npm run tauri dev
```

### Build de produção

```bash
npm run tauri build
```

## 🎮 Como jogar

1. Clique em uma peça para ver seus lances legais destacados no tabuleiro
2. Clique na casa de destino para mover
3. Ative "vs. motor" para jogar contra o Stockfish, escolhendo sua cor e o Elo do oponente
4. Use o botão de dica para receber uma sugestão de lance do motor
5. Acompanhe a barra de avaliação e a classificação da sua última jogada para entender melhor a posição

## 📁 Estrutura do projeto

```
src/                  # Front-end (TypeScript, DOM puro)
src-tauri/
  src/chess.rs         # Motor de regras do xadrez (roque, en passant, promoção...)
  src/engine.rs         # Integração com o Stockfish via protocolo UCI
  src/lib.rs            # Registro dos comandos Tauri
```

## 📄 Licença

Distribuído sob a licença [MIT](LICENSE).
