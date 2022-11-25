use crate::joypad::SelectedButtons::{Action, Direction};
use crate::mmu::MemoryArea;

use serde::{Deserialize, Serialize};
use winit::event::VirtualKeyCode;
use winit::event::VirtualKeyCode::*;

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub enum SelectedButtons {
    Action = 0x10,
    Direction = 0x20,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub struct Joypad {
    selected_buttons: SelectedButtons,
    action_buttons: u8,
    direction_buttons: u8,
    #[serde(skip)]
    pub(crate) held_action: Vec<VirtualKeyCode>,
    #[serde(skip)]
    pub(crate) held_direction: Vec<VirtualKeyCode>,
}

impl MemoryArea for Joypad {
    fn read(&self, address: usize) -> Option<u8> {
        let value = self.selected_buttons as u8 | self.buttons();
        match address {
            0xFF00 => Some(value),
            _ => None,
        }
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
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

impl Joypad {
    pub fn new() -> Self {
        Self {
            action_buttons: 0x0F,
            direction_buttons: 0x0F,
            selected_buttons: Action,
            held_direction: vec![],
            held_action: vec![],
        }
    }

    pub fn machine_cycle(&mut self) -> bool {
        let previous_buttons = self.buttons();

        self.action_buttons = Self::map_buttons([Z, C, Back, Return], &self.held_action);
        self.direction_buttons = Self::map_buttons([Right, Left, Up, Down], &self.held_direction);

        self.buttons() != previous_buttons
    }

    fn map_buttons(buttons: [VirtualKeyCode; 4], held: &[VirtualKeyCode]) -> u8 {
        !buttons
            .iter()
            .enumerate()
            .map(|(i, button)| u8::from(held.contains(button)) * 2u8.pow(i as u32))
            .sum::<u8>()
            & 0x0F
    }

    fn buttons(&self) -> u8 {
        if self.selected_buttons == Action {
            self.action_buttons
        } else {
            self.direction_buttons
        }
    }
}
