use crate::joypad::SelectedButtons::{Action, Direction};
use minifb::{Key, Window};
use std::ops::BitXor;
use Key::*;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum SelectedButtons {
    Action = 0x10,
    Direction = 0x20,
}

pub struct Joypad {
    selected_buttons: SelectedButtons,
    action_buttons: u8,
    direction_buttons: u8,
}

#[derive(Copy, Clone)]
pub struct InputInterrupt;

impl Joypad {
    pub fn new() -> Self {
        Self {
            action_buttons: 0x0F,
            direction_buttons: 0x0F,
            selected_buttons: Action,
        }
    }

    pub fn machine_cycle(&mut self, window: &Window) -> Vec<InputInterrupt> {
        let previous_buttons = *self.buttons();

        let map_buttons = |keys: [Key; 4]| {
            !(keys
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    if window.is_key_down(*k) {
                        2_u8.pow(i as u32)
                    } else {
                        0x00
                    }
                })
                .sum::<u8>())
                & 0x0F
        };

        self.action_buttons = map_buttons([Z, C, Backspace, Enter]);
        self.direction_buttons = map_buttons([Right, Left, Up, Down]);

        let size = self.buttons().bitxor(previous_buttons);
        vec![InputInterrupt; size as usize]
    }

    fn buttons(&self) -> &u8 {
        if self.selected_buttons == Action {
            &self.action_buttons
        } else {
            &self.direction_buttons
        }
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        let value = self.selected_buttons as u8 | self.buttons();
        match address {
            0xFF00 => Some(value),
            _ => None,
        }
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0xFF00 => {
                self.selected_buttons = match value & 0x30 {
                    0x20 | 0x30 => Direction,
                    0x10 => Action,
                    _ => self.selected_buttons,
                }
            }
            _ => return false,
        };
        true
    }
}
