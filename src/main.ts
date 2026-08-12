import { invoke } from "@tauri-apps/api/core";

type ChessColor = "White" | "Black";
type PieceType = "Pawn" | "Knight" | "Bishop" | "Rook" | "Queen" | "King";

interface Piece {
  color: ChessColor;
  piece_type: PieceType;
}

interface GameState {
  board: (Piece | null)[][]; // board[rank][file], rank 0 = "1", file 0 = "a"
  turn: ChessColor;
  game_over: boolean;
  winner: ChessColor | null;
}

const PIECE_SYMBOLS: Record<ChessColor, Record<PieceType, string>> = {
  White: {
    King: "♔",
    Queen: "♕",
    Rook: "♖",
    Bishop: "♗",
    Knight: "♘",
    Pawn: "♙",
  },
  Black: {
    King: "♚",
    Queen: "♛",
    Rook: "♜",
    Bishop: "♝",
    Knight: "♞",
    Pawn: "♟",
  },
};

const FILES = ["a", "b", "c", "d", "e", "f", "g", "h"];

let boardEl: HTMLDivElement;
let statusEl: HTMLParagraphElement;
let errorEl: HTMLParagraphElement;
let newGameBtn: HTMLButtonElement;

let currentState: GameState | null = null;
let selectedSquare: string | null = null;

function squareName(rank: number, file: number): string {
  return `${FILES[file]}${rank + 1}`;
}

function renderBoard(state: GameState) {
  boardEl.innerHTML = "";

  // Renderiza da fileira 8 (topo) até a fileira 1 (base), como um tabuleiro real.
  for (let rank = 7; rank >= 0; rank--) {
    for (let file = 0; file < 8; file++) {
      const square = document.createElement("div");
      const name = squareName(rank, file);
      const isLight = (rank + file) % 2 === 1;
      square.className = `square ${isLight ? "light" : "dark"}`;
      square.dataset.square = name;

      if (name === selectedSquare) {
        square.classList.add("selected");
      }

      const piece = state.board[rank][file];
      if (piece) {
        const pieceEl = document.createElement("span");
        pieceEl.className = `piece ${piece.color === "White" ? "white" : "black"}`;
        pieceEl.textContent = PIECE_SYMBOLS[piece.color][piece.piece_type];
        square.appendChild(pieceEl);
      }

      square.addEventListener("click", () => onSquareClick(name));
      boardEl.appendChild(square);
    }
  }
}

function renderStatus(state: GameState) {
  if (state.game_over) {
    const winnerLabel = state.winner === "White" ? "Brancas" : "Pretas";
    statusEl.textContent = `Fim de jogo! ${winnerLabel} venceram capturando o rei.`;
  } else {
    statusEl.textContent = state.turn === "White" ? "Vez das brancas" : "Vez das pretas";
  }
}

function render() {
  if (!currentState) return;
  renderBoard(currentState);
  renderStatus(currentState);
}

async function onSquareClick(name: string) {
  if (!currentState || currentState.game_over) return;

  const [rank, file] = [parseInt(name[1], 10) - 1, FILES.indexOf(name[0])];
  const piece = currentState.board[rank][file];

  if (!selectedSquare) {
    if (piece && piece.color === currentState.turn) {
      selectedSquare = name;
      errorEl.textContent = "";
      render();
    }
    return;
  }

  if (selectedSquare === name) {
    selectedSquare = null;
    render();
    return;
  }

  const from = selectedSquare;
  selectedSquare = null;

  try {
    errorEl.textContent = "";
    const newState = await invoke<GameState>("make_move", { from, to: name });
    currentState = newState;
    render();
  } catch (err) {
    errorEl.textContent = String(err);
    render();
  }
}

async function startNewGame() {
  selectedSquare = null;
  errorEl.textContent = "";
  currentState = await invoke<GameState>("new_game");
  render();
}

window.addEventListener("DOMContentLoaded", () => {
  boardEl = document.querySelector<HTMLDivElement>("#board")!;
  statusEl = document.querySelector<HTMLParagraphElement>("#status-msg")!;
  errorEl = document.querySelector<HTMLParagraphElement>("#error-msg")!;
  newGameBtn = document.querySelector<HTMLButtonElement>("#new-game-btn")!;

  newGameBtn.addEventListener("click", () => {
    startNewGame();
  });

  invoke<GameState>("get_state").then((state) => {
    currentState = state;
    render();
  });
});
