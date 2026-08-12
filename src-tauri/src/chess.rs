use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum Color {
    White,
    Black,
}

impl Color {
    fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub struct Piece {
    pub color: Color,
    pub piece_type: PieceType,
}

pub struct Game {
    // board[rank][file], rank 0 = rank "1", file 0 = file "a"
    board: [[Option<Piece>; 8]; 8],
    turn: Color,
    game_over: bool,
    winner: Option<Color>,
    captured_white: Vec<PieceType>,
    captured_black: Vec<PieceType>,
}

#[derive(Serialize)]
pub struct GameStateDto {
    board: Vec<Vec<Option<Piece>>>,
    turn: Color,
    game_over: bool,
    winner: Option<Color>,
    captured_white: Vec<PieceType>,
    captured_black: Vec<PieceType>,
}

pub struct AppState(pub Mutex<Game>);

impl Game {
    pub fn new() -> Self {
        let mut board: [[Option<Piece>; 8]; 8] = [[None; 8]; 8];

        let back_rank = [
            PieceType::Rook,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Queen,
            PieceType::King,
            PieceType::Bishop,
            PieceType::Knight,
            PieceType::Rook,
        ];

        for (file, piece_type) in back_rank.iter().enumerate() {
            board[0][file] = Some(Piece {
                color: Color::White,
                piece_type: *piece_type,
            });
            board[7][file] = Some(Piece {
                color: Color::Black,
                piece_type: *piece_type,
            });
        }
        for file in 0..8 {
            board[1][file] = Some(Piece {
                color: Color::White,
                piece_type: PieceType::Pawn,
            });
            board[6][file] = Some(Piece {
                color: Color::Black,
                piece_type: PieceType::Pawn,
            });
        }

        Game {
            board,
            turn: Color::White,
            game_over: false,
            winner: None,
            captured_white: Vec::new(),
            captured_black: Vec::new(),
        }
    }

    pub fn to_dto(&self) -> GameStateDto {
        GameStateDto {
            board: self.board.iter().map(|row| row.to_vec()).collect(),
            turn: self.turn,
            game_over: self.game_over,
            winner: self.winner,
            captured_white: self.captured_white.clone(),
            captured_black: self.captured_black.clone(),
        }
    }

    pub fn legal_moves(&self, from: &str) -> Result<Vec<String>, String> {
        let (fr, ff) = parse_square(from)?;
        let piece = match self.board[fr][ff] {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let mut moves = Vec::new();
        for tr in 0..8 {
            for tf in 0..8 {
                if (tr, tf) == (fr, ff) {
                    continue;
                }
                if let Some(target) = self.board[tr][tf] {
                    if target.color == piece.color {
                        continue;
                    }
                }
                if self.validate_piece_move(piece, fr, ff, tr, tf).is_ok() {
                    moves.push(square_name(tr, tf));
                }
            }
        }
        Ok(moves)
    }

    pub fn make_move(&mut self, from: &str, to: &str) -> Result<(), String> {
        if self.game_over {
            return Err("O jogo já terminou.".into());
        }

        let (fr, ff) = parse_square(from)?;
        let (tr, tf) = parse_square(to)?;
        if (fr, ff) == (tr, tf) {
            return Err("A casa de origem e destino são iguais.".into());
        }

        let piece = self.board[fr][ff].ok_or("Não há peça na casa de origem.")?;
        if piece.color != self.turn {
            return Err("Não é a vez dessa cor.".into());
        }

        if let Some(target) = self.board[tr][tf] {
            if target.color == piece.color {
                return Err("Não é possível capturar sua própria peça.".into());
            }
        }

        self.validate_piece_move(piece, fr, ff, tr, tf)?;

        let captured = self.board[tr][tf];
        self.board[tr][tf] = Some(piece);
        self.board[fr][ff] = None;

        // Promoção simples: peão que chega na última fileira vira dama.
        if piece.piece_type == PieceType::Pawn && (tr == 0 || tr == 7) {
            self.board[tr][tf] = Some(Piece {
                color: piece.color,
                piece_type: PieceType::Queen,
            });
        }

        if let Some(cap) = captured {
            match cap.color {
                Color::White => self.captured_white.push(cap.piece_type),
                Color::Black => self.captured_black.push(cap.piece_type),
            }
            if cap.piece_type == PieceType::King {
                self.game_over = true;
                self.winner = Some(piece.color);
            }
        }

        if !self.game_over {
            self.turn = self.turn.opposite();
        }

        Ok(())
    }

    fn validate_piece_move(
        &self,
        piece: Piece,
        fr: usize,
        ff: usize,
        tr: usize,
        tf: usize,
    ) -> Result<(), String> {
        let dr = tr as i32 - fr as i32;
        let df = tf as i32 - ff as i32;

        match piece.piece_type {
            PieceType::Pawn => self.validate_pawn(piece.color, fr, ff, tr, tf, dr, df),
            PieceType::Knight => {
                if (dr.abs(), df.abs()) == (1, 2) || (dr.abs(), df.abs()) == (2, 1) {
                    Ok(())
                } else {
                    Err("Movimento inválido para o cavalo.".into())
                }
            }
            PieceType::Bishop => {
                if dr.abs() == df.abs() && dr != 0 {
                    self.check_path_clear(fr, ff, tr, tf)
                } else {
                    Err("Movimento inválido para o bispo.".into())
                }
            }
            PieceType::Rook => {
                if (dr == 0) != (df == 0) {
                    self.check_path_clear(fr, ff, tr, tf)
                } else {
                    Err("Movimento inválido para a torre.".into())
                }
            }
            PieceType::Queen => {
                if (dr == 0 || df == 0 || dr.abs() == df.abs()) && (dr != 0 || df != 0) {
                    self.check_path_clear(fr, ff, tr, tf)
                } else {
                    Err("Movimento inválido para a dama.".into())
                }
            }
            PieceType::King => {
                if dr.abs() <= 1 && df.abs() <= 1 && (dr != 0 || df != 0) {
                    Ok(())
                } else {
                    Err("Movimento inválido para o rei.".into())
                }
            }
        }
    }

    fn validate_pawn(
        &self,
        color: Color,
        fr: usize,
        ff: usize,
        tr: usize,
        tf: usize,
        dr: i32,
        df: i32,
    ) -> Result<(), String> {
        let dir: i32 = if color == Color::White { 1 } else { -1 };
        let start_rank: usize = if color == Color::White { 1 } else { 6 };

        if df == 0 {
            if self.board[tr][tf].is_some() {
                return Err("O peão não pode capturar para frente.".into());
            }
            if dr == dir {
                return Ok(());
            }
            if dr == 2 * dir && fr == start_rank {
                let mid = (fr as i32 + dir) as usize;
                if self.board[mid][ff].is_none() {
                    return Ok(());
                }
                return Err("Caminho bloqueado.".into());
            }
            return Err("Movimento inválido para o peão.".into());
        }

        if df.abs() == 1 && dr == dir {
            if self.board[tr][tf].is_some() {
                return Ok(());
            }
            return Err("O peão só captura na diagonal quando há peça inimiga.".into());
        }

        Err("Movimento inválido para o peão.".into())
    }

    fn check_path_clear(&self, fr: usize, ff: usize, tr: usize, tf: usize) -> Result<(), String> {
        let dr = (tr as i32 - fr as i32).signum();
        let df = (tf as i32 - ff as i32).signum();
        let mut r = fr as i32 + dr;
        let mut f = ff as i32 + df;
        while (r, f) != (tr as i32, tf as i32) {
            if self.board[r as usize][f as usize].is_some() {
                return Err("Caminho bloqueado.".into());
            }
            r += dr;
            f += df;
        }
        Ok(())
    }
}

fn parse_square(s: &str) -> Result<(usize, usize), String> {
    let s = s.trim().to_lowercase();
    let bytes = s.as_bytes();
    if bytes.len() != 2 {
        return Err(format!("Casa inválida: {}", s));
    }
    let file = bytes[0];
    let rank = bytes[1];
    if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
        return Err(format!("Casa inválida: {}", s));
    }
    Ok(((rank - b'1') as usize, (file - b'a') as usize))
}

fn square_name(rank: usize, file: usize) -> String {
    format!("{}{}", (b'a' + file as u8) as char, rank + 1)
}

#[tauri::command]
pub fn get_state(state: State<AppState>) -> GameStateDto {
    let game = state.0.lock().unwrap();
    game.to_dto()
}

#[tauri::command]
pub fn new_game(state: State<AppState>) -> GameStateDto {
    let mut game = state.0.lock().unwrap();
    *game = Game::new();
    game.to_dto()
}

#[tauri::command]
pub fn make_move(from: String, to: String, state: State<AppState>) -> Result<GameStateDto, String> {
    let mut game = state.0.lock().unwrap();
    game.make_move(&from, &to)?;
    Ok(game.to_dto())
}

#[tauri::command]
pub fn legal_moves(from: String, state: State<AppState>) -> Result<Vec<String>, String> {
    let game = state.0.lock().unwrap();
    game.legal_moves(&from)
}
