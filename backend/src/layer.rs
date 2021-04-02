use std::cell::Cell;

use crate::{DaemonBoard, DaemonBoardWeak, Hs, Mode};

#[derive(Debug)]
pub struct Layer {
    layer: u8,
    index: u8,
    board: DaemonBoardWeak,
    mode: Cell<Option<(u8, u8)>>,
    brightness: Cell<i32>,
    color: Cell<Hs>,
}

impl Layer {
    pub(crate) fn new(board: &DaemonBoard, layer: u8) -> Self {
        let index = if board.layout().meta.has_per_layer {
            0xf0 + layer
        } else {
            0xff
        };
        let mode = if board.layout().meta.has_mode {
            board
                .0
                .daemon
                .mode(board.0.board, layer)
                .map(Some)
                .unwrap_or_else(|err| {
                    error!("Error getting layer mode: {}", err);
                    None
                })
        } else {
            None
        };
        let brightness = board
            .0
            .daemon
            .brightness(board.0.board, index)
            .unwrap_or_else(|err| {
                error!("error getting layer brightness: {}", err);
                0
            });
        let color = board
            .0
            .daemon
            .color(board.0.board, index)
            .unwrap_or_else(|err| {
                error!("error getting layer color: {}", err);
                Hs::new(0., 0.)
            });
        Self {
            layer,
            index,
            board: board.downgrade(),
            mode: Cell::new(mode),
            brightness: Cell::new(brightness),
            color: Cell::new(color),
        }
    }

    fn board(&self) -> DaemonBoard {
        self.board.upgrade().unwrap()
    }

    pub fn mode(&self) -> Option<(&'static Mode, u8)> {
        let (index, speed) = self.mode.get()?;
        Some((Mode::from_index(index)?, speed))
    }

    pub fn set_mode(&self, mode: &Mode, speed: u8) -> Result<(), String> {
        let board = self.board();
        board
            .0
            .daemon
            .set_mode(board.0.board, self.layer, mode.index, speed)?;
        self.mode.set(Some((mode.index, speed)));
        board.0.leds_changed.set(true);
        Ok(())
    }

    pub fn brightness(&self) -> i32 {
        self.brightness.get()
    }

    pub fn set_brightness(&self, brightness: i32) -> Result<(), String> {
        let board = self.board();
        board
            .0
            .daemon
            .set_brightness(board.0.board, self.index, brightness)?;
        self.brightness.set(brightness);
        board.0.leds_changed.set(true);
        Ok(())
    }

    pub fn color(&self) -> Hs {
        self.color.get()
    }

    pub fn set_color(&self, color: Hs) -> Result<(), String> {
        let board = self.board();
        board.0.daemon.set_color(board.0.board, self.index, color)?;
        self.color.set(color);
        board.0.leds_changed.set(true);
        Ok(())
    }
}
