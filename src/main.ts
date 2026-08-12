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
  history: string[];
}

interface EvalDto {
  score_cp: number | null;
  mate_in: number | null;
}

interface HintDto {
  from: string;
  to: string;
  promotion: PieceType | null;
  score_cp: number | null;
  mate_in: number | null;
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
let historyEl: HTMLDivElement;
let vsEngineToggleEl: HTMLInputElement;
let humanColorSelectEl: HTMLSelectElement;
let eloSliderEl: HTMLInputElement;
let eloValueEl: HTMLSpanElement;
let hintBtnEl: HTMLButtonElement;
let engineStatusEl: HTMLParagraphElement;
let evalLabelEl: HTMLSpanElement;
let evalBarFillEl: HTMLDivElement;
let moveQualityEl: HTMLParagraphElement;

let currentState: GameState | null = null;
let selectedSquare: string | null = null;
let legalTargets: Set<string> = new Set();
let enPassantCaptureSquare: string | null = null;

let engineAvailable = false;
let vsEngine = false;
let humanColor: ChessColor = "White";
let engineElo = 1500;
let engineThinking = false;
let hintMove: { from: string; to: string } | null = null;
let currentEvalCp: number | null = null;
let currentMateIn: number | null = null;

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
      if (hintMove && (name === hintMove.from || name === hintMove.to)) {
        square.classList.add("hint");
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

function renderHistory(state: GameState) {
  historyEl.innerHTML = "";
  for (let i = 0; i < state.history.length; i += 2) {
    const row = document.createElement("div");
    row.className = "history-row";

    const number = document.createElement("span");
    number.className = "history-number";
    number.textContent = `${i / 2 + 1}.`;

    const white = document.createElement("span");
    white.textContent = state.history[i] ?? "";

    const black = document.createElement("span");
    black.textContent = state.history[i + 1] ?? "";

    row.append(number, white, black);
    historyEl.appendChild(row);
  }
  historyEl.scrollTop = historyEl.scrollHeight;
}

function renderStatus(state: GameState) {
  if (state.game_over) {
    const winnerLabel = state.winner === "White" ? "Brancas" : "Pretas";
    statusEl.textContent = `Fim de jogo! ${winnerLabel} venceram capturando o rei.`;
  } else if (engineThinking) {
    statusEl.textContent = "O Stockfish está pensando...";
  } else {
    statusEl.textContent = state.turn === "White" ? "Vez das brancas" : "Vez das pretas";
  }
}

function render() {
  if (!currentState) return;
  renderBoard(currentState);
  renderStatus(currentState);
  renderCaptured(currentState);
  renderHistory(currentState);
}

function evalToWhitePercent(cp: number | null, mateIn: number | null): number {
  if (mateIn !== null) {
    return mateIn > 0 ? 100 : 0;
  }
  if (cp === null) return 50;
  const clamped = Math.max(-800, Math.min(800, cp));
  return 50 + (clamped / 800) * 50;
}

function formatEval(cp: number | null, mateIn: number | null): string {
  if (mateIn !== null) {
    return mateIn > 0 ? `M${mateIn}` : `-M${Math.abs(mateIn)}`;
  }
  if (cp === null) return "—";
  const pawns = (cp / 100).toFixed(1);
  return cp > 0 ? `+${pawns}` : pawns;
}

function renderEvalBar() {
  const percent = evalToWhitePercent(currentEvalCp, currentMateIn);
  evalBarFillEl.style.height = `${percent}%`;
  evalLabelEl.textContent = formatEval(currentEvalCp, currentMateIn);
}

async function refreshEvaluation() {
  if (!engineAvailable) return;
  try {
    const evalResult = await invoke<EvalDto>("evaluate_position");
    currentEvalCp = evalResult.score_cp;
    currentMateIn = evalResult.mate_in;
    renderEvalBar();
    engineStatusEl.textContent = "";
  } catch (err) {
    engineStatusEl.textContent = String(err);
  }
}

// Converte um score (na perspectiva de quem joga naquela posição, padrão UCI,
// ja normalizado para a perspectiva das brancas pelo backend) para a
// perspectiva de quem jogou o lance sendo avaliado.
function toMoverPerspective(color: ChessColor, cp: number | null, mateIn: number | null): number {
  const sign = color === "White" ? 1 : -1;
  if (mateIn !== null) {
    return sign * (mateIn > 0 ? 100000 - mateIn : -100000 - mateIn);
  }
  return sign * (cp ?? 0);
}

function classifyMove(
  moverColor: ChessColor,
  cpBefore: number | null,
  mateBefore: number | null,
  cpAfter: number | null,
  mateAfter: number | null,
) {
  if (cpBefore === null && mateBefore === null) {
    moveQualityEl.textContent = "";
    return;
  }

  const before = toMoverPerspective(moverColor, cpBefore, mateBefore);
  const after = toMoverPerspective(moverColor, cpAfter, mateAfter);
  const loss = before - after;

  let label: string;
  let tier: "good" | "ok" | "bad";
  if (loss <= 20) {
    label = "Ótimo";
    tier = "good";
  } else if (loss <= 50) {
    label = "Bom";
    tier = "good";
  } else if (loss <= 120) {
    label = "Impreciso";
    tier = "ok";
  } else if (loss <= 300) {
    label = "Erro";
    tier = "bad";
  } else {
    label = "Erro grave";
    tier = "bad";
  }

  const deltaPawns = (Math.abs(loss) / 100).toFixed(2);
  moveQualityEl.textContent = `${label} (${loss > 0 ? "-" : "+"}${deltaPawns})`;
  moveQualityEl.className = `move-quality ${tier}`;
}

function renderEngineControlsAvailability() {
  vsEngineToggleEl.disabled = !engineAvailable;
  hintBtnEl.disabled = !engineAvailable;
  humanColorSelectEl.disabled = !engineAvailable || !vsEngine;
  eloSliderEl.disabled = !engineAvailable || !vsEngine;
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

async function maybeTriggerEngineMove() {
  if (!engineAvailable || !vsEngine || !currentState || currentState.game_over) return;
  if (currentState.turn === humanColor) return;

  engineThinking = true;
  renderStatus(currentState);
  try {
    const newState = await invoke<GameState>("engine_move", { elo: engineElo });
    currentState = newState;
    hintMove = null;
    render();
    await refreshEvaluation();
  } catch (err) {
    engineStatusEl.textContent = String(err);
  } finally {
    engineThinking = false;
    render();
  }
}

async function attemptMove(from: string, to: string, promotion: PieceType | null) {
  const moverColor = currentState!.turn;
  const cpBefore = currentEvalCp;
  const mateBefore = currentMateIn;

  try {
    errorEl.textContent = "";
    const newState = await invoke<GameState>("make_move", { from, to, promotion });
    currentState = newState;
    hintMove = null;
    render();

    await refreshEvaluation();
    classifyMove(moverColor, cpBefore, mateBefore, currentEvalCp, currentMateIn);

    await maybeTriggerEngineMove();
  } catch (err) {
    errorEl.textContent = String(err);
    render();
  }
}

async function onSquareClick(name: string) {
  if (!currentState || currentState.game_over || engineThinking) return;
  if (vsEngine && currentState.turn !== humanColor) return;

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
  moveQualityEl.textContent = "";
  engineThinking = false;
  currentState = await invoke<GameState>("new_game");
  render();
  await refreshEvaluation();
  await maybeTriggerEngineMove();
}

async function requestHint() {
  if (!currentState || currentState.game_over || engineThinking) return;
  try {
    errorEl.textContent = "";
    const hint = await invoke<HintDto>("get_hint");
    hintMove = { from: hint.from, to: hint.to };
    render();
  } catch (err) {
    errorEl.textContent = String(err);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  boardEl = document.querySelector<HTMLDivElement>("#board")!;
  statusEl = document.querySelector<HTMLParagraphElement>("#status-msg")!;
  errorEl = document.querySelector<HTMLParagraphElement>("#error-msg")!;
  newGameBtn = document.querySelector<HTMLButtonElement>("#new-game-btn")!;
  capturedBlackEl = document.querySelector<HTMLDivElement>("#captured-black")!;
  capturedWhiteEl = document.querySelector<HTMLDivElement>("#captured-white")!;
  historyEl = document.querySelector<HTMLDivElement>("#history")!;
  vsEngineToggleEl = document.querySelector<HTMLInputElement>("#vs-engine-toggle")!;
  humanColorSelectEl = document.querySelector<HTMLSelectElement>("#human-color-select")!;
  eloSliderEl = document.querySelector<HTMLInputElement>("#elo-slider")!;
  eloValueEl = document.querySelector<HTMLSpanElement>("#elo-value")!;
  hintBtnEl = document.querySelector<HTMLButtonElement>("#hint-btn")!;
  engineStatusEl = document.querySelector<HTMLParagraphElement>("#engine-status-msg")!;
  evalLabelEl = document.querySelector<HTMLSpanElement>("#eval-label")!;
  evalBarFillEl = document.querySelector<HTMLDivElement>("#eval-bar-fill")!;
  moveQualityEl = document.querySelector<HTMLParagraphElement>("#move-quality-msg")!;

  newGameBtn.addEventListener("click", () => {
    startNewGame();
  });

  vsEngineToggleEl.addEventListener("change", () => {
    vsEngine = vsEngineToggleEl.checked;
    renderEngineControlsAvailability();
    maybeTriggerEngineMove();
  });

  humanColorSelectEl.addEventListener("change", () => {
    humanColor = humanColorSelectEl.value as ChessColor;
    maybeTriggerEngineMove();
  });

  eloSliderEl.addEventListener("input", () => {
    engineElo = parseInt(eloSliderEl.value, 10);
    eloValueEl.textContent = String(engineElo);
  });

  hintBtnEl.addEventListener("click", () => {
    requestHint();
  });

  invoke<GameState>("get_state").then(async (state) => {
    currentState = state;
    render();

    engineAvailable = await invoke<boolean>("engine_status");
    renderEngineControlsAvailability();
    if (!engineAvailable) {
      engineStatusEl.textContent =
        "Stockfish não encontrado. Instale-o (ex.: yay -S stockfish) para habilitar avaliação, dicas e o modo contra o computador.";
    } else {
      await refreshEvaluation();
    }
  });
});
