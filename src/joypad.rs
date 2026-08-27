use crate::mmu::MemoryArea;

use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;
use winit::keyboard::KeyCode::{ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Backspace, Enter, KeyC, KeyZ};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub struct Joypad {
    select: u8,
    action_buttons: u8,
    direction_buttons: u8,
    #[serde(skip)]
    pub(crate) held_action: Vec<KeyCode>,
    #[serde(skip)]
    pub(crate) held_direction: Vec<KeyCode>,
}

impl MemoryArea for Joypad {
    fn read(&self, address: usize) -> Option<u8> {
        match address {
            0xFF00 => Some(0xC0 | self.select | self.buttons()),
            _ => None,
        }
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0xFF00 => {
                self.select = value & 0x30;
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
            select: 0x30,
            held_direction: vec![],
            held_action: vec![],
        }
    }

    pub fn machine_cycle(&mut self) -> bool {
        let previous_buttons = self.buttons();

        self.action_buttons = Self::map_buttons([KeyZ, KeyC, Backspace, Enter], &self.held_action);
        self.direction_buttons = Self::map_buttons([ArrowRight, ArrowLeft, ArrowUp, ArrowDown], &self.held_direction);

        self.buttons() != previous_buttons
    }

    fn map_buttons(buttons: [KeyCode; 4], held: &[KeyCode]) -> u8 {
        !buttons
            .iter()
            .enumerate()
            .map(|(i, button)| u8::from(held.contains(button)) * 2u8.pow(i as u32))
            .sum::<u8>()
            & 0x0F
    }

    fn buttons(&self) -> u8 {
        match self.select {
            0x10 => self.action_buttons,
            0x20 => self.direction_buttons,
            0x00 => self.action_buttons & self.direction_buttons,
            _ => 0x0F,
        }
    }
}
