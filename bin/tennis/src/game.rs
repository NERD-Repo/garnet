// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use fidl_fuchsia_game_tennis as fidl_tennis;
use fidl_fuchsia_game_tennis::GameState;
use fuchsia_syslog::{fx_log, fx_log_info};
use futures::prelude::*;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};

const BOARD_HEIGHT: f64 = 10.0;
const BOARD_WIDTH: f64 = 20.0;
const PADDLE_SPEED: f64 = 0.4; // distance paddle travels per step
const PADDLE_SIZE: f64 = 1.0; // vertical height of paddle
const BALL_SPEEDUP_MULTIPLIER: f64 = 1.05; // speed multiplier applied on every paddle bounce
const MAX_BOUNCE_ANGLE: f64 = 1.3; // in radians, bounce angle when hitting very top edge of paddle

pub struct Game {
    state: GameState,
    player_1: Option<Player>,
    player_2: Option<Player>,
    ball_dx: f64,
    ball_dy: f64,
}

#[derive(Clone)]
struct Player {
    pub name: String,
    pub state: Arc<Mutex<PlayerState>>,
}

#[derive(Clone)]
pub enum PlayerState {
    Up,
    Down,
    Stop,
}

fn calc_paddle_movement(pos: &mut f64, state: &PlayerState) {
    let player_delta = match state {
        PlayerState::Up => PADDLE_SPEED * -1.0,
        PlayerState::Down => PADDLE_SPEED,
        PlayerState::Stop => 0.0,
    };
    let new_paddle_location = *pos + player_delta;
    if new_paddle_location >= 0.0 && new_paddle_location < BOARD_HEIGHT {
        *pos = new_paddle_location;
    }
}

impl Game {
    /// return clone of internal state
    pub fn state(&self) -> GameState {
        fidl_tennis::GameState {
            ball_x: self.state.ball_x,
            ball_y: self.state.ball_y,
            player_1_y: self.state.player_1_y,
            player_2_y: self.state.player_2_y,
            player_1_score: self.state.player_1_score,
            player_2_score: self.state.player_2_score,
            player_1_name: self.state.player_1_name.clone(),
            player_2_name: self.state.player_2_name.clone(),
            time: self.state.time,
            game_num: self.state.game_num,
        }
    }
    pub fn new() -> Game {
        Game {
            player_1: None,
            player_2: None,
            ball_dx: 0.0,
            ball_dy: 0.0,
            state: GameState {
                ball_x: 0.0,
                ball_y: 0.0,
                game_num: 0,
                player_1_y: 0.0,
                player_2_y: 0.0,
                player_1_score: 0,
                player_2_score: 0,
                player_1_name: "".to_string(),
                player_2_name: "".to_string(),
                time: 0,
            },
        }
    }

    pub fn players_ready(&self) -> bool {
        return self.player_1.is_some() && self.player_2.is_some();
    }

    pub fn register_new_paddle(&mut self, player_name: String) -> Arc<Mutex<PlayerState>> {
        let paddle = Player {
            name: player_name.clone(),
            state: Arc::new(Mutex::new(PlayerState::Stop)),
        };
        let res = paddle.state.clone();
        if self.player_1.is_none() {
            self.player_1 = Some(paddle);
            self.state.player_1_name = player_name;
        } else if self.player_2.is_none() {
            self.player_2 = Some(paddle);
            self.state.player_2_name = player_name;
        } else {
            panic!("too many clients connected");
        }
        return res;
    }

    pub fn step(&mut self) {
        if self.players_ready() && self.state.game_num == 0 {
            self.new_game();
        } else if !self.players_ready() {
            // game has not started yet
            return;
        }

        fx_log_info!("new step");

        self.state.time += 1;

        calc_paddle_movement(
            &mut self.state.player_1_y,
            &self.player_1.as_mut().unwrap().state.lock(),
        );
        calc_paddle_movement(
            &mut self.state.player_2_y,
            &self.player_2.as_mut().unwrap().state.lock(),
        );

        let mut new_ball_x = self.state.ball_x + self.ball_dx;
        let mut new_ball_y = self.state.ball_y + self.ball_dy;

        // reflect off the top/bottom of the board
        if new_ball_y <= 0.0 || new_ball_y > BOARD_HEIGHT {
            self.ball_dy = -self.ball_dy;
            new_ball_y = self.state.ball_y;
            fx_log_info!("bounce off top or bottom");
        }

        // reflect off the left/right of the board, if a paddle is in the way
        if new_ball_x <= 0.0 {
            // we're about to go off of the left side
            if new_ball_y > self.state.player_1_y + (PADDLE_SIZE / 2.0)
                || new_ball_y < self.state.player_1_y - (PADDLE_SIZE / 2.0) {
                    // player 1 missed, so player 2 gets a point and we reset
                    self.state.player_2_score += 1;
                    self.new_game();
                    return;
            } else {
                self.ball_dx = -self.ball_dx;
                new_ball_x = self.state.ball_x;
                fx_log_info!("bounce off left");
            }
        }
        if new_ball_x > BOARD_WIDTH {
            // we're about to go off of the right side
            if new_ball_y > self.state.player_2_y + (PADDLE_SIZE / 2.0)
                || new_ball_y < self.state.player_2_y - (PADDLE_SIZE / 2.0) {
                    // player 2 missed, so player 1 gets a point and we reset
                    self.state.player_1_score += 1;
                    self.new_game();
                    return;
            } else {
                self.ball_dx = -self.ball_dx;
                new_ball_x = self.state.ball_x;
                fx_log_info!("bounce off right");
            }
        }

        self.state.ball_x = new_ball_x;
        self.state.ball_y = new_ball_y;
    }

    fn new_game(&mut self) {
        self.player_1.as_mut().map(|player| {
            *player.state.lock() = PlayerState::Stop;
        });
        self.player_2.as_mut().map(|player| {
            *player.state.lock() = PlayerState::Stop;
        });
        match self.state.game_num % 5 {
            0 => {
                self.ball_dx = 0.5;
                self.ball_dy = 0.5;
            },
            1 => {
                self.ball_dx = 0.5;
                self.ball_dy = -0.5;
            },
            2 => {
                self.ball_dx = -0.5;
                self.ball_dy = -0.5;
            },
            3 => {
                self.ball_dx = -0.5;
                self.ball_dy = 0.5;
            },
            weird_num => {
                panic!(format!("WTF game num: {}?", weird_num));
            },
        }
        //self.ball_dx = 0.5; // TODO randomize?
        //self.ball_dy = 0.5; // TODO randomize?
        self.state.ball_x = BOARD_WIDTH / 2.0;
        self.state.ball_y = BOARD_HEIGHT / 2.0;
        self.state.game_num += 1;
        //self.state.player_1_y = BOARD_HEIGHT / 2.0;
        //self.state.player_2_y = BOARD_HEIGHT / 2.0;
        self.state.time = 0;
    }
}
