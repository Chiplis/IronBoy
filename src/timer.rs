pub struct TimerInterrupt;

pub struct Timer {
    tima: u8,
    tma: u8,
    tac: u8,
    ticks: u16,
    interrupt: bool,
    interrupt_served: bool,
}

impl Timer {
    const DIVIDER: usize = 0xFF04;
    const TIMA: usize = 0xFF05;
    const TMA: usize = 0xFF06;
    const TAC: usize = 0xFF07;

    pub fn new() -> Self {
        Self {
            tima: 0,
            tma: 0,
            tac: 0,
            ticks: 0xABCC,
            interrupt: false,
            interrupt_served: false,
        }
    }

    pub fn machine_cycle(&mut self) -> Option<TimerInterrupt> {
        self.interrupt_served = false;
        let interrupt = if self.interrupt {
            self.tima = self.tma;
            self.interrupt_served = true;
            Some(TimerInterrupt)
        } else {
            None
        };

        self.interrupt = false;

        let old_ticks = self.ticks;
        self.ticks = self.ticks.wrapping_add(4);
        self.tima_increase(old_ticks);

        interrupt
    }

    fn tima_increase(&mut self, old_ticks: u16) {
        if self.timer_enabled() && self.timer_increase(old_ticks) {
            let (new_tima, overflow) = self.tima.overflowing_add(1);
            self.tima = new_tima;
            self.interrupt = overflow;
        }
    }

    fn timer_increase(&self, old_timer: u16) -> bool {
        old_timer & self.frequency() != 0 && self.ticks & self.frequency() == 0
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        match address {
            Timer::DIVIDER => Some(self.ticks.to_le_bytes()[1]),
            Timer::TIMA => Some(self.tima),
            Timer::TMA => Some(self.tma),
            Timer::TAC => Some(self.tac),
            _ => None,
        }
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            Timer::DIVIDER => {
                let old_ticks = self.ticks;
                self.ticks = 0x00;
                self.tima_increase(old_ticks);
            }
            Timer::TIMA => {
                if !self.interrupt_served {
                    self.tima = value;
                    self.interrupt = false;
                }
            }
            Timer::TMA => {
                self.tma = value;
                if self.interrupt_served {
                    self.tima = value
                }
            }
            Timer::TAC => self.tac = value,
            _ => return false,
        };
        true
    }

    fn timer_enabled(&self) -> bool {
        self.tac & 0x04 != 0
    }

    fn frequency(&self) -> u16 {
        2_u16.pow(match self.tac & 0x03 {
            0x03 => 7,
            0x02 => 5,
            0x01 => 3,
            0x00 => 9,
            _ => unreachable!(),
        })
    }
}
