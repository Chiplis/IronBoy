pub struct TimerInterrupt();

pub struct Timer {
    divider: u8,
    tima: u8,
    tma: u8,
    tac: u8,
    old_tma: u8,
    timer_ticks: usize,
    divider_ticks: usize,
}

impl Timer {
    const DIVIDER: usize = 0xFF04;
    const TIMA: usize = 0xFF05;
    const TMA: usize = 0xFF06;
    const TAC: usize = 0xFF07;

    pub fn new() -> Self {
        Self {
            divider: 0xAB,
            tima: 0,
            tma: 0,
            tac: 0,
            timer_ticks: 0,
            divider_ticks: 0,
            old_tma: 0,
        }
    }

    pub fn timer_cycle(&mut self, cpu_cycles: usize) -> Option<TimerInterrupt> {
        self.divider_ticks += cpu_cycles * 4;
        let mut interrupt = None;

        if self.divider_ticks >= 256 {
            self.divider_ticks -= 256;
            self.divider = self.divider.wrapping_add(1);
        }

        if self.timer_enabled() {
            self.timer_ticks += cpu_cycles * 4;
            while self.timer_ticks >= self.timer_frequency() {
                self.timer_ticks -= self.timer_frequency();
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = if overflow { self.old_tma } else { new_tima };
                if overflow { interrupt = Some(TimerInterrupt()) }
            }
        }

        self.old_tma = self.tma;

        interrupt
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        match address {
            Timer::DIVIDER => {
                Some(self.divider)
            },
            Timer::TIMA => Some(self.tima),
            Timer::TMA => Some(self.tma),
            Timer::TAC => Some(self.tac),
            _ => None
        }
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            Timer::DIVIDER => {
                self.divider_ticks = 0x00;
                self.divider = 0x00;
            },
            Timer::TIMA => self.tima = value,
            Timer::TMA => {
                self.old_tma = self.tma;
                self.tma = value
            }
            Timer::TAC => self.tac = value,
            _ => return false
        };
        true
    }

    fn timer_enabled(&self) -> bool { self.tac & 0x04 != 0 }

    fn timer_frequency(&self) -> usize {
        match self.tac & 0x03 {
            0x03 => 256,
            0x02 => 64,
            0x01 => 16,
            _ => 1024,
        }
    }
}