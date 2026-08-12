use serde::{Deserialize, Serialize};
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Copy)]
struct CastlingRights {
    white_king_side: bool,
    white_queen_side: bool,
    black_king_side: bool,
    black_queen_side: bool,
}

impl CastlingRights {
    fn new() -> Self {
        CastlingRights {
            white_king_side: true,
            white_queen_side: true,
            black_king_side: true,
            black_queen_side: true,
        }
    }
}

pub struct Game {
    // board[rank][file], rank 0 = rank "1", file 0 = file "a"
    board: [[Option<Piece>; 8]; 8],
    turn: Color,
    game_over: bool,
    winner: Option<Color>,
    captured_white: Vec<PieceType>,
    captured_black: Vec<PieceType>,
    castling: CastlingRights,
    // Casa que um peão inimigo pode capturar via en passant no próximo lance.
    en_passant_target: Option<(usize, usize)>,
    history: Vec<String>,
}

#[derive(Serialize)]
pub struct GameStateDto {
    board: Vec<Vec<Option<Piece>>>,
    turn: Color,
    game_over: bool,
    winner: Option<Color>,
    captured_white: Vec<PieceType>,
    captured_black: Vec<PieceType>,
    en_passant_target: Option<String>,
    history: Vec<String>,
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
            castling: CastlingRights::new(),
            en_passant_target: None,
            history: Vec::new(),
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
            en_passant_target: self.en_passant_target.map(|(r, f)| square_name(r, f)),
            history: self.history.clone(),
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
                let is_en_passant = piece.piece_type == PieceType::Pawn
                    && self.en_passant_target == Some((tr, tf));
                let ok = if is_en_passant {
                    self.validate_en_passant_move(piece.color, fr, ff, tr, tf)
                        .is_ok()
                } else {
                    self.validate_piece_move(piece, fr, ff, tr, tf).is_ok()
                };
                if ok {
                    moves.push(square_name(tr, tf));
                }
            }
        }

        if piece.piece_type == PieceType::King && ff == 4 {
            let home_rank = if piece.color == Color::White { 0 } else { 7 };
            if fr == home_rank {
                if self.can_castle(piece.color, true) {
                    moves.push(square_name(home_rank, 6));
                }
                if self.can_castle(piece.color, false) {
                    moves.push(square_name(home_rank, 2));
                }
            }
        }

        Ok(moves)
    }

    pub fn make_move(
        &mut self,
        from: &str,
        to: &str,
        promotion: Option<PieceType>,
    ) -> Result<(), String> {
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

        let home_rank = if piece.color == Color::White { 0 } else { 7 };
        if piece.piece_type == PieceType::King
            && fr == home_rank
            && ff == 4
            && tr == fr
            && (tf as i32 - ff as i32).abs() == 2
        {
            return self.castle(piece.color, tf);
        }

        if let Some(target) = self.board[tr][tf] {
            if target.color == piece.color {
                return Err("Não é possível capturar sua própria peça.".into());
            }
        }

        let is_en_passant =
            piece.piece_type == PieceType::Pawn && self.en_passant_target == Some((tr, tf));

        if is_en_passant {
            self.validate_en_passant_move(piece.color, fr, ff, tr, tf)?;
        } else {
            self.validate_piece_move(piece, fr, ff, tr, tf)?;
        }

        let is_promotion = piece.piece_type == PieceType::Pawn && (tr == 0 || tr == 7);
        let promoted_type = if is_promotion {
            match promotion {
                Some(
                    pt @ (PieceType::Queen | PieceType::Rook | PieceType::Bishop | PieceType::Knight),
                ) => Some(pt),
                Some(_) => return Err("Peça de promoção inválida.".into()),
                None => return Err("Escolha uma peça para a promoção.".into()),
            }
        } else {
            None
        };

        let mut captured = self.board[tr][tf];
        if is_en_passant {
            captured = self.board[fr][tf];
            self.board[fr][tf] = None;
        }

        self.history.push(format_move(
            piece,
            ff,
            tr,
            tf,
            captured.is_some(),
            is_en_passant,
            promoted_type,
        ));

        self.board[fr][ff] = None;
        self.board[tr][tf] = Some(Piece {
            color: piece.color,
            piece_type: promoted_type.unwrap_or(piece.piece_type),
        });

        self.update_castling_rights(piece, fr, ff, tr, tf);
        self.en_passant_target = Self::compute_en_passant_target(piece, fr, ff, tr);

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

    fn castle(&mut self, color: Color, to_file: usize) -> Result<(), String> {
        let kingside = to_file == 6;
        let queenside = to_file == 2;
        if !kingside && !queenside {
            return Err("Movimento de roque inválido.".into());
        }
        if !self.can_castle(color, kingside) {
            return Err("O roque não é permitido nessa posição.".into());
        }

        let rank = if color == Color::White { 0 } else { 7 };
        let rook_file = if kingside { 7 } else { 0 };
        let new_king_file = if kingside { 6 } else { 2 };
        let new_rook_file = if kingside { 5 } else { 3 };

        self.board[rank][4] = None;
        self.board[rank][rook_file] = None;
        self.board[rank][new_king_file] = Some(Piece {
            color,
            piece_type: PieceType::King,
        });
        self.board[rank][new_rook_file] = Some(Piece {
            color,
            piece_type: PieceType::Rook,
        });

        match color {
            Color::White => {
                self.castling.white_king_side = false;
                self.castling.white_queen_side = false;
            }
            Color::Black => {
                self.castling.black_king_side = false;
                self.castling.black_queen_side = false;
            }
        }

        self.history
            .push(if kingside { "O-O".into() } else { "O-O-O".into() });

        self.en_passant_target = None;
        self.turn = self.turn.opposite();
        Ok(())
    }

    fn can_castle(&self, color: Color, kingside: bool) -> bool {
        let right_ok = match (color, kingside) {
            (Color::White, true) => self.castling.white_king_side,
            (Color::White, false) => self.castling.white_queen_side,
            (Color::Black, true) => self.castling.black_king_side,
            (Color::Black, false) => self.castling.black_queen_side,
        };
        if !right_ok {
            return false;
        }

        let rank = if color == Color::White { 0 } else { 7 };
        let king_ok = matches!(
            self.board[rank][4],
            Some(p) if p.piece_type == PieceType::King && p.color == color
        );
        let rook_file = if kingside { 7 } else { 0 };
        let rook_ok = matches!(
            self.board[rank][rook_file],
            Some(p) if p.piece_type == PieceType::Rook && p.color == color
        );
        if !king_ok || !rook_ok {
            return false;
        }

        let empty_files: &[usize] = if kingside { &[5, 6] } else { &[1, 2, 3] };
        if empty_files.iter().any(|&f| self.board[rank][f].is_some()) {
            return false;
        }

        // O rei não pode estar em xeque, passar por, ou pousar em uma casa atacada.
        let king_path: &[usize] = if kingside { &[4, 5, 6] } else { &[4, 3, 2] };
        let opponent = color.opposite();
        if king_path
            .iter()
            .any(|&f| self.is_square_attacked(rank, f, opponent))
        {
            return false;
        }

        true
    }

    fn is_square_attacked(&self, r: usize, f: usize, by: Color) -> bool {
        for pr in 0..8 {
            for pf in 0..8 {
                if let Some(p) = self.board[pr][pf] {
                    if p.color != by {
                        continue;
                    }
                    let attacks = if p.piece_type == PieceType::Pawn {
                        pawn_attacks(p.color, pr, pf, r, f)
                    } else {
                        self.validate_piece_move(p, pr, pf, r, f).is_ok()
                    };
                    if attacks {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn update_castling_rights(&mut self, piece: Piece, fr: usize, ff: usize, tr: usize, tf: usize) {
        if piece.piece_type == PieceType::King {
            match piece.color {
                Color::White => {
                    self.castling.white_king_side = false;
                    self.castling.white_queen_side = false;
                }
                Color::Black => {
                    self.castling.black_king_side = false;
                    self.castling.black_queen_side = false;
                }
            }
        }
        self.revoke_corner_right(fr, ff);
        self.revoke_corner_right(tr, tf);
    }

    fn revoke_corner_right(&mut self, r: usize, f: usize) {
        match (r, f) {
            (0, 0) => self.castling.white_queen_side = false,
            (0, 7) => self.castling.white_king_side = false,
            (7, 0) => self.castling.black_queen_side = false,
            (7, 7) => self.castling.black_king_side = false,
            _ => {}
        }
    }

    fn compute_en_passant_target(
        piece: Piece,
        fr: usize,
        ff: usize,
        tr: usize,
    ) -> Option<(usize, usize)> {
        if piece.piece_type != PieceType::Pawn {
            return None;
        }
        if (tr as i32 - fr as i32).abs() == 2 {
            let mid = ((fr as i32 + tr as i32) / 2) as usize;
            Some((mid, ff))
        } else {
            None
        }
    }

    fn validate_en_passant_move(
        &self,
        color: Color,
        fr: usize,
        ff: usize,
        tr: usize,
        tf: usize,
    ) -> Result<(), String> {
        let dir: i32 = if color == Color::White { 1 } else { -1 };
        let dr = tr as i32 - fr as i32;
        let df = tf as i32 - ff as i32;
        if dr == dir && df.abs() == 1 {
            Ok(())
        } else {
            Err("Movimento de en passant inválido.".into())
        }
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

fn format_move(
    piece: Piece,
    ff: usize,
    tr: usize,
    tf: usize,
    is_capture: bool,
    is_en_passant: bool,
    promoted: Option<PieceType>,
) -> String {
    let dest = square_name(tr, tf);
    let mut s = String::new();

    if piece.piece_type == PieceType::Pawn {
        if is_capture {
            s.push((b'a' + ff as u8) as char);
            s.push('x');
        }
        s.push_str(&dest);
        if let Some(promo) = promoted {
            s.push('=');
            s.push_str(piece_letter(promo));
        }
        if is_en_passant {
            s.push_str(" e.p.");
        }
    } else {
        s.push_str(piece_letter(piece.piece_type));
        if is_capture {
            s.push('x');
        }
        s.push_str(&dest);
    }

    s
}

fn piece_letter(piece_type: PieceType) -> &'static str {
    match piece_type {
        PieceType::Knight => "N",
        PieceType::Bishop => "B",
        PieceType::Rook => "R",
        PieceType::Queen => "Q",
        PieceType::King => "K",
        PieceType::Pawn => "",
    }
}

fn pawn_attacks(color: Color, fr: usize, ff: usize, tr: usize, tf: usize) -> bool {
    let dir: i32 = if color == Color::White { 1 } else { -1 };
    let dr = tr as i32 - fr as i32;
    let df = tf as i32 - ff as i32;
    dr == dir && df.abs() == 1
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
pub fn make_move(
    from: String,
    to: String,
    promotion: Option<PieceType>,
    state: State<AppState>,
) -> Result<GameStateDto, String> {
    let mut game = state.0.lock().unwrap();
    game.make_move(&from, &to, promotion)?;
    Ok(game.to_dto())
}

#[tauri::command]
pub fn legal_moves(from: String, state: State<AppState>) -> Result<Vec<String>, String> {
    let game = state.0.lock().unwrap();
    game.legal_moves(&from)
}
