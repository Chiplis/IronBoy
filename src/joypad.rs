use crate::joypad::SelectedButtons::{Action, Direction};
use minifb::{Key, Window};
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

impl Joypad {
    pub fn new() -> Self {
        Self {
            action_buttons: 0x0F,
            direction_buttons: 0x0F,
            selected_buttons: Action,
        }
    }

    pub fn machine_cycle(&mut self, window: &Window) -> bool {
        let previous_buttons = self.buttons();

        self.action_buttons = Self::map_buttons([Z, C, Backspace, Enter], window);
        self.direction_buttons = Self::map_buttons([Right, Left, Up, Down], window);

        self.buttons() != previous_buttons
    }

    fn map_buttons(buttons: [Key; 4], window: &Window) -> u8 {
        let mut sum = 0;
        if window.is_key_down(buttons[0]) {
            sum += 1
        }
        if window.is_key_down(buttons[1]) {
            sum += 2
        }
        if window.is_key_down(buttons[2]) {
            sum += 4
        }
        if window.is_key_down(buttons[3]) {
            sum += 8
        }
        !sum & 0x0F
    }

    fn buttons(&self) -> u8 {
        if self.selected_buttons == Action {
            self.action_buttons
        } else {
            self.direction_buttons
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
