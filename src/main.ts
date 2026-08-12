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
  captured_white: PieceType[];
  captured_black: PieceType[];
  en_passant_target: string | null;
}

const PROMOTION_CHOICES: PieceType[] = ["Queen", "Rook", "Bishop", "Knight"];

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
let capturedBlackEl: HTMLDivElement;
let capturedWhiteEl: HTMLDivElement;

let currentState: GameState | null = null;
let selectedSquare: string | null = null;
let legalTargets: Set<string> = new Set();
let enPassantCaptureSquare: string | null = null;

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

      if (legalTargets.has(name)) {
        square.classList.add(piece ? "capturable" : "legal-move");
      }
      if (name === enPassantCaptureSquare) {
        square.classList.add("capturable");
      }

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

function renderCaptured(state: GameState) {
  capturedBlackEl.innerHTML = "";
  for (const pieceType of state.captured_black) {
    const span = document.createElement("span");
    span.className = "captured-piece black";
    span.textContent = PIECE_SYMBOLS.Black[pieceType];
    capturedBlackEl.appendChild(span);
  }

  capturedWhiteEl.innerHTML = "";
  for (const pieceType of state.captured_white) {
    const span = document.createElement("span");
    span.className = "captured-piece white";
    span.textContent = PIECE_SYMBOLS.White[pieceType];
    capturedWhiteEl.appendChild(span);
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
  renderCaptured(currentState);
}

async function selectSquare(name: string) {
  selectedSquare = name;
  errorEl.textContent = "";
  legalTargets = new Set(await invoke<string[]>("legal_moves", { from: name }));

  enPassantCaptureSquare = null;
  const epTarget = currentState?.en_passant_target ?? null;
  if (epTarget && legalTargets.has(epTarget)) {
    const targetFile = epTarget[0];
    const sourceRank = name[1];
    enPassantCaptureSquare = `${targetFile}${sourceRank}`;
  }

  render();
}

function clearSelection() {
  selectedSquare = null;
  legalTargets = new Set();
  enPassantCaptureSquare = null;
}

function pickPromotion(color: ChessColor): Promise<PieceType> {
  return new Promise((resolve) => {
    const picker = document.createElement("div");
    picker.className = "promotion-picker";
    for (const pieceType of PROMOTION_CHOICES) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = `promotion-option ${color === "White" ? "white" : "black"}`;
      btn.textContent = PIECE_SYMBOLS[color][pieceType];
      btn.addEventListener("click", () => {
        picker.remove();
        resolve(pieceType);
      });
      picker.appendChild(btn);
    }
    boardEl.appendChild(picker);
  });
}

async function attemptMove(from: string, to: string, promotion: PieceType | null) {
  try {
    errorEl.textContent = "";
    const newState = await invoke<GameState>("make_move", { from, to, promotion });
    currentState = newState;
    render();
  } catch (err) {
    errorEl.textContent = String(err);
    render();
  }
}

async function onSquareClick(name: string) {
  if (!currentState || currentState.game_over) return;

  const [rank, file] = [parseInt(name[1], 10) - 1, FILES.indexOf(name[0])];
  const piece = currentState.board[rank][file];

  if (!selectedSquare) {
    if (piece && piece.color === currentState.turn) {
      await selectSquare(name);
    }
    return;
  }

  if (selectedSquare === name) {
    clearSelection();
    render();
    return;
  }

  // Clicar em outra peça própria troca a seleção em vez de tentar um movimento inválido.
  if (piece && piece.color === currentState.turn) {
    await selectSquare(name);
    return;
  }

  const from = selectedSquare;
  const movingPiece = currentState.board[parseInt(from[1], 10) - 1][FILES.indexOf(from[0])];
  const targetRank = rank;
  clearSelection();
  render();

  if (movingPiece?.piece_type === "Pawn" && (targetRank === 0 || targetRank === 7)) {
    const promotion = await pickPromotion(movingPiece.color);
    await attemptMove(from, name, promotion);
    return;
  }

  await attemptMove(from, name, null);
}

async function startNewGame() {
  clearSelection();
  errorEl.textContent = "";
  currentState = await invoke<GameState>("new_game");
  render();
}

window.addEventListener("DOMContentLoaded", () => {
  boardEl = document.querySelector<HTMLDivElement>("#board")!;
  statusEl = document.querySelector<HTMLParagraphElement>("#status-msg")!;
  errorEl = document.querySelector<HTMLParagraphElement>("#error-msg")!;
  newGameBtn = document.querySelector<HTMLButtonElement>("#new-game-btn")!;
  capturedBlackEl = document.querySelector<HTMLDivElement>("#captured-black")!;
  capturedWhiteEl = document.querySelector<HTMLDivElement>("#captured-white")!;

  newGameBtn.addEventListener("click", () => {
    startNewGame();
  });

  invoke<GameState>("get_state").then((state) => {
    currentState = state;
    render();
  });
});
