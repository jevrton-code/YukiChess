use crate::chess::{AppState, GameStateDto, PieceType};
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use tauri::State;

const EVAL_MOVETIME_MS: u32 = 400;
const HINT_MOVETIME_MS: u32 = 800;
const ENGINE_MOVE_MOVETIME_MS: u32 = 700;

struct Engine {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Engine {
    fn spawn() -> Result<Self, String> {
        let mut child = Command::new("stockfish")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                format!(
                    "Não foi possível iniciar o Stockfish ({e}). Verifique se ele está instalado e disponível no PATH."
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or("Falha ao abrir a entrada do Stockfish.")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Falha ao abrir a saída do Stockfish.")?;

        let mut engine = Engine {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };

        engine.send("uci")?;
        engine.wait_for("uciok")?;
        engine.send("setoption name Threads value 1")?;
        engine.send("isready")?;
        engine.wait_for("readyok")?;

        Ok(engine)
    }

    fn send(&mut self, cmd: &str) -> Result<(), String> {
        writeln!(self.stdin, "{cmd}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())
    }

    fn wait_for(&mut self, token: &str) -> Result<(), String> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self
                .stdout
                .read_line(&mut line)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("O Stockfish encerrou inesperadamente.".into());
            }
            if line.trim_start().starts_with(token) {
                return Ok(());
            }
        }
    }

    fn set_strength(&mut self, elo: Option<u32>) -> Result<(), String> {
        match elo {
            Some(value) => {
                self.send("setoption name UCI_LimitStrength value true")?;
                self.send(&format!(
                    "setoption name UCI_Elo value {}",
                    value.clamp(1320, 3190)
                ))?;
            }
            None => {
                self.send("setoption name UCI_LimitStrength value false")?;
            }
        }
        Ok(())
    }

    /// Manda a posicao para analise e le as linhas "info" ate o "bestmove",
    /// guardando apenas o ultimo score reportado (sem multi-pv).
    fn search(&mut self, fen: &str, movetime_ms: u32) -> Result<(String, Option<i32>, Option<i32>), String> {
        self.send(&format!("position fen {fen}"))?;
        self.send("isready")?;
        self.wait_for("readyok")?;
        self.send(&format!("go movetime {movetime_ms}"))?;

        let mut score_cp: Option<i32> = None;
        let mut mate_in: Option<i32> = None;
        let mut line = String::new();

        let best_move = loop {
            line.clear();
            let n = self
                .stdout
                .read_line(&mut line)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("O Stockfish encerrou inesperadamente.".into());
            }
            let trimmed = line.trim();

            if let Some(rest) = trimmed.strip_prefix("bestmove") {
                break rest.split_whitespace().next().map(str::to_string);
            }

            if trimmed.starts_with("info") {
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                for i in 0..tokens.len() {
                    if tokens[i] == "score" && i + 2 < tokens.len() {
                        match tokens[i + 1] {
                            "cp" => {
                                score_cp = tokens[i + 2].parse().ok();
                                mate_in = None;
                            }
                            "mate" => {
                                mate_in = tokens[i + 2].parse().ok();
                                score_cp = None;
                            }
                            _ => {}
                        }
                    }
                }
            }
        };

        let best_move = best_move
            .filter(|m| m != "(none)")
            .ok_or("Não há lances legais nessa posição.")?;
        Ok((best_move, score_cp, mate_in))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.send("quit");
        let _ = self.child.wait();
    }
}

pub struct EngineState(Mutex<Option<Engine>>);

impl EngineState {
    pub fn new() -> Self {
        EngineState(Mutex::new(None))
    }
}

fn with_engine<T>(
    state: &State<EngineState>,
    f: impl FnOnce(&mut Engine) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = state.0.lock().unwrap();
    if guard.is_none() {
        *guard = Some(Engine::spawn()?);
    }
    let result = f(guard.as_mut().unwrap());
    // Se algo deu errado na comunicacao, descarta o processo para
    // tentar de novo do zero na proxima chamada.
    if result.is_err() {
        *guard = None;
    }
    result
}

fn to_white_perspective(
    score_cp: Option<i32>,
    mate_in: Option<i32>,
    white_to_move: bool,
) -> (Option<i32>, Option<i32>) {
    if white_to_move {
        (score_cp, mate_in)
    } else {
        (score_cp.map(|v| -v), mate_in.map(|v| -v))
    }
}

fn parse_uci_move(mv: &str) -> Result<(String, String, Option<PieceType>), String> {
    if mv.len() < 4 {
        return Err("Lance do motor inválido.".into());
    }
    let from = mv[0..2].to_string();
    let to = mv[2..4].to_string();
    let promotion = match mv.as_bytes().get(4) {
        Some(b'q') => Some(PieceType::Queen),
        Some(b'r') => Some(PieceType::Rook),
        Some(b'b') => Some(PieceType::Bishop),
        Some(b'n') => Some(PieceType::Knight),
        Some(_) => return Err("Promoção do motor inválida.".into()),
        None => None,
    };
    Ok((from, to, promotion))
}

#[derive(Serialize)]
pub struct EvalDto {
    score_cp: Option<i32>,
    mate_in: Option<i32>,
}

#[derive(Serialize)]
pub struct HintDto {
    from: String,
    to: String,
    promotion: Option<PieceType>,
    score_cp: Option<i32>,
    mate_in: Option<i32>,
}

#[tauri::command]
pub fn engine_status(engine_state: State<EngineState>) -> bool {
    with_engine(&engine_state, |_| Ok(())).is_ok()
}

#[tauri::command]
pub fn evaluate_position(
    app_state: State<AppState>,
    engine_state: State<EngineState>,
) -> Result<EvalDto, String> {
    let (fen, white_to_move) = {
        let game = app_state.0.lock().unwrap();
        let fen = game.to_fen();
        let white_to_move = fen.split_whitespace().nth(1) == Some("w");
        (fen, white_to_move)
    };

    let (score_cp, mate_in) = with_engine(&engine_state, |engine| {
        engine.set_strength(None)?;
        let (_, score_cp, mate_in) = engine.search(&fen, EVAL_MOVETIME_MS)?;
        Ok((score_cp, mate_in))
    })?;

    let (score_cp, mate_in) = to_white_perspective(score_cp, mate_in, white_to_move);
    Ok(EvalDto { score_cp, mate_in })
}

#[tauri::command]
pub fn get_hint(
    app_state: State<AppState>,
    engine_state: State<EngineState>,
) -> Result<HintDto, String> {
    let fen = {
        let game = app_state.0.lock().unwrap();
        if game.is_over() {
            return Err("O jogo já terminou.".into());
        }
        game.to_fen()
    };

    let (best_move, score_cp, mate_in) = with_engine(&engine_state, |engine| {
        engine.set_strength(None)?;
        engine.search(&fen, HINT_MOVETIME_MS)
    })?;

    let (from, to, promotion) = parse_uci_move(&best_move)?;
    Ok(HintDto {
        from,
        to,
        promotion,
        score_cp,
        mate_in,
    })
}

#[tauri::command]
pub fn engine_move(
    elo: u32,
    app_state: State<AppState>,
    engine_state: State<EngineState>,
) -> Result<GameStateDto, String> {
    let fen = {
        let game = app_state.0.lock().unwrap();
        if game.is_over() {
            return Err("O jogo já terminou.".into());
        }
        game.to_fen()
    };

    let (best_move, ..) = with_engine(&engine_state, |engine| {
        engine.set_strength(Some(elo))?;
        engine.search(&fen, ENGINE_MOVE_MOVETIME_MS)
    })?;

    let (from, to, promotion) = parse_uci_move(&best_move)?;

    let mut game = app_state.0.lock().unwrap();
    game.make_move(&from, &to, promotion)?;
    Ok(game.to_dto())
}
