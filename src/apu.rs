mod oscillators {
    use serde::{Serialize, Deserialize};
    use std::sync::{RwLock};
    use crate::logger::Logger;

    #[derive(Default, Serialize, Deserialize)]
    struct VolumeEnvelopeParams {
        add_mode: bool,
        period: u8,
        current_level: u8,
        frequency_timer: u32,
    }

    #[derive(Default, Serialize, Deserialize)]
    struct VolumeEnvelope {
        sample_rate: u32,
        params: VolumeEnvelopeParams,
        last_val: u8,
        current_settings: u8,
    }

    impl VolumeEnvelope {
        pub(crate) fn new(sample_rate: u32) -> VolumeEnvelope {
            VolumeEnvelope { sample_rate, ..Default::default() }
        }

        pub(crate) fn write_settings(&mut self, val: u8) {
            let starting_vol = val >> 4;
            let add_mode = ((val & 0x08) >> 3) > 0;
            let period = val & 0x07;

            // Get the lock for all items
            self.params.current_level = starting_vol;
            self.params.add_mode = add_mode;
            self.params.period = period;
            self.params.frequency_timer = (self.sample_rate / 64) * ((period) as u32);

            self.current_settings = val;
        }

        pub(crate) fn read_settings(&self) -> u8 {
            self.current_settings
        }

        pub(crate) fn generate_sample(&mut self) -> u8 {
            self.last_val = self.params.current_level;
            let output_sample = self.params.current_level;
            if self.params.period == 0 {
                return output_sample;
            }
            // Apply envelope
            // Check if level change is needed
            if self.params.frequency_timer == 0 {
                self.params.frequency_timer = (self.sample_rate / 64) * ((self.params.period) as u32);

                if self.params.add_mode && self.params.current_level < 15 {
                    self.params.current_level += 1;
                } else if !self.params.add_mode && self.params.current_level > 0 {
                    self.params.current_level -= 1;
                }
            }
            self.params.frequency_timer -= 1;
            output_sample
        }
    }

    #[derive(Default, Serialize, Deserialize)]
    pub struct SquareWaveGenerator {
        frequency: u16,

        frequency_timer: u32,
        timer_leftover: RwLock<f32>,
        sample_rate: u32,
        sweep: bool,
        position: u8,
        duty: u8,
        trigger: u8,
        enabled: bool,
        length: u8,
        length_counter: u32,
        length_enabled: bool,
        env: VolumeEnvelope,
        sweep_period: u8,
        sweep_timer: u32,
        sweep_negate: bool,
        sweep_shift: u8,
        sweep_enabled: bool,
        sweep_frequency: u16,
    }

    impl SquareWaveGenerator {
        pub(crate) fn new(sample_rate: u32, sweep: bool) -> SquareWaveGenerator {
            SquareWaveGenerator {
                sample_rate,
                sweep,
                duty: 2,
                env: VolumeEnvelope::new(sample_rate),
                ..Default::default()
            }
        }

        pub(crate) fn write_reg(&mut self, reg: usize, val: u8) {
            match reg {
                0 => {
                    if self.sweep {
                        let period = (val & 0x70) >> 4;
                        let negate = (val & 0x08) > 0;
                        let shift = val & 0x07;

                        self.sweep_period = period;
                        self.sweep_negate = negate;
                        self.sweep_shift = shift;
                    }
                }

                // Duty and length
                1 => {
                    let new_duty = val >> 6;
                    self.duty = new_duty;

                    let length = val & 0x3F;
                    self.length = length;

                    let length_256hz = 64 - length;
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;

                    // Here we set the length counter making sure nothing can use it while it is set
                    self.length_counter = length_samples;
                }

                // Volume envelope
                2 => {
                    self.env.write_settings(val);

                    // Disable channel if no DAC power
                    if val & 0xF0 == 0 {
                        self.enabled = false;
                    }
                }

                // Frequency 8 least significant bits
                3 => {
                    // No need to worry about safety since this thread is the only one which will ever change frequency
                    let new_frequency = (val as u16 & 0xFF) | (self.frequency & 0xFF00);
                    self.frequency = new_frequency;
                }

                // Frequency 3 most significant bits and trigger
                4 => {

                    // No need to worry about safety since this thread is the only one which will ever change frequency
                    let msb = ((val as u16 & 0x7) << 8) | 0xFF;
                    let new_frequency = (msb & 0xFF00) | (self.frequency & 0xFF);
                    self.frequency = new_frequency;

                    self.length_enabled = (val & 0x40) > 0;

                    let trigger = val & 0x80;
                    self.trigger = trigger;

                    if trigger == 0 {
                        return;
                    }

                    // If length == 0 reset it to 64
                    if self.length_counter == 0 {
                        self.length_counter = ((self.sample_rate as f32 / 256.0) * 64.0).ceil() as u32;
                    }

                    // Sweep data
                    if self.sweep {
                        // Copy frequency to shadow register
                        self.sweep_frequency = new_frequency;

                        let sweep_period = self.sweep_period;
                        let sweep_shift = self.sweep_shift;

                        // Reload sweep timer
                        let sweep_num_samples = ((self.sample_rate as f32 / 128.0) * sweep_period as f32) as u32;
                        self.sweep_timer = sweep_num_samples;

                        // Set sweep enabled flag
                        let sweep_enabled = sweep_period != 0 && sweep_shift != 0;
                        self.sweep_enabled = sweep_enabled;

                        // Perform frequency and overflow calcs
                        if sweep_enabled {
                            let (overflow, new_sweep_freq) = self.calculate_sweep_freq();

                            if overflow {
                                self.enabled = false;
                                return;
                            }

                            self.sweep_frequency = new_sweep_freq;
                            self.frequency = new_sweep_freq;

                            let (overflow_2, _) = self.calculate_sweep_freq();

                            if overflow_2 {
                                self.enabled = false;
                                return;
                            }
                        }
                    }

                    // Reset frequency timer and timer leftover
                    let cycles_till_next = (2048 - self.frequency as u32) * 4;
                    let samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;
                    self.frequency_timer = samples_till_next.floor() as u32;

                    // Store the remainder from the conversion from length in cycles to samples in timer leftover
                    match self.timer_leftover.write() {
                        Ok(mut timer_leftover) => {
                            *timer_leftover = samples_till_next - samples_till_next.floor();
                        }
                        Err(error) => Logger::error(format!("Square Wave: Could not write to timer leftover: {error}")),
                    }

                    // Set enabled
                    self.enabled = true;
                }

                reg => Logger::error(format!("Square Wave Osc: Unrecognised register ({reg})")),
            }
        }

        pub(crate) fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub(crate) fn read_reg(&self, reg: usize) -> u8 {
            match reg {
                0 => {
                    if self.sweep {
                        let mut reg_val = self.sweep_period << 4;
                        reg_val |= (self.sweep_negate as u8) << 3;
                        reg_val |= self.sweep_shift;
                        reg_val
                    } else {
                        0x00
                    }
                }

                1 => {
                    let mut reg_value = self.duty << 6;
                    reg_value |= self.length;
                    reg_value
                }

                2 => self.env.read_settings(),

                3 => (self.frequency & 0x00FF) as u8,

                4 => {
                    let mut reg_value = self.trigger;
                    reg_value |= (self.length_enabled as u8) << 6;
                    reg_value |= ((self.frequency & 0x0F00) >> 8) as u8;
                    reg_value
                }

                _ => 0x00
            }
        }

        pub(crate) fn generate_sample(&mut self) -> f32 {
            if !self.enabled {
                return 0.0;
            }

            if self.frequency_timer == 0 {
                // Reset frequency timer
                let cycles_till_next = (2048 - self.frequency as u32) * 4;
                let mut samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;

                // If leftover plus current remainder is more than one we should make this period another sample long to make up for the lost time
                match self.timer_leftover.write() {
                    Ok(mut timer_leftover) => {
                        *timer_leftover += samples_till_next - samples_till_next.floor();

                        if *timer_leftover > 1.0 {
                            *timer_leftover -= 1.0;
                            samples_till_next += 1.0;
                        }
                    }
                    Err(error) => {
                        Logger::error(format!("Square Wave - Could not write to timer leftover: {error}"));
                    }
                }

                self.frequency_timer = samples_till_next.floor() as u32;

                let current_position = self.position;

                let mut new_position = current_position + 1;
                if new_position >= 8 {
                    new_position = 0;
                }

                self.position = new_position;
            }

            self.frequency_timer -= 1;

            if self.sweep {
                if self.sweep_timer == 0 && self.sweep_enabled && self.sweep_period > 0 {
                    // Reload sweep timer
                    let sweep_num_samples = ((self.sample_rate as f32 / 128.0) * self.sweep_period as f32) as u32;
                    self.sweep_timer = sweep_num_samples;

                    let (overflow, new_sweep_freq) = self.calculate_sweep_freq();

                    if overflow {
                        self.enabled = false;
                        return 0.0;
                    }

                    self.sweep_frequency = new_sweep_freq;
                    self.frequency = new_sweep_freq;

                    let (overflow_2, _) = self.calculate_sweep_freq();

                    if overflow_2 {
                        self.enabled = false;
                        return 0.0;
                    }
                }

                self.sweep_timer -= 1;
            }

            let mut wave_sample = 0;
            let envelope_sample = self.env.generate_sample();

            match self.duty {
                // 12.5%
                0 => {
                    if self.position == 7 {
                        wave_sample = 1;
                    }
                }

                // 25%
                1 => {
                    if self.position >= 6 {
                        wave_sample = 1;
                    }
                }

                // 50%
                2 => {
                    if self.position >= 4 {
                        wave_sample = 1;
                    }
                }

                // 75%
                3 => {
                    if self.position < 6 {
                        wave_sample = 1;
                    }
                }

                _ => {}
            }

            if self.length_enabled {
                // Try and decrement the length counter, if we can't get access to it that means it's being reset and we don't want to decrement it anyway
                self.length_counter = if let Some(val) = self.length_counter.checked_sub(1) { val } else { 0 };
                if self.length_counter == 0 {
                    self.enabled = false;
                }
            }

            let dac_input_sample = if wave_sample != 0 {
                envelope_sample
            } else {
                0
            };

            dac_input_sample as f32 / 15.0
        }

        fn calculate_sweep_freq(&self) -> (bool, u16) {
            let offset = self.sweep_frequency >> self.sweep_shift;

            let new_freq: u32 = if self.sweep_negate {
                // Check for underflow
                match self.sweep_frequency.checked_sub(offset) {
                    Some(res) => res.into(),
                    None => 0
                }
            } else {
                self.sweep_frequency as u32 + offset as u32
            };

            // Overflow check
            if new_freq > 2047 {
                return (true, new_freq as u16);
            }

            (false, new_freq as u16)
        }
    }


    #[derive(Default, Serialize, Deserialize)]
    pub struct WaveTable {
        sample_rate: u32,

        sound_data: [u8; 32],

        frequency: u16,

        frequency_timer: u32,
        timer_leftover: RwLock<f32>,

        position: u8,

        trigger: u8,

        enabled: bool,

        length: u8,
        length_counter: RwLock<u32>,

        length_enabled: bool,

        volume_code: u8,
    }

    impl WaveTable {
        pub(crate) fn new(sample_rate: u32) -> WaveTable {
            WaveTable { sample_rate, ..Default::default() }
        }

        pub(crate) fn write_reg(&mut self, reg: usize, val: u8) {
            match reg {
                0 => {
                    if val == 0x00 {
                        self.enabled = false;
                    }
                }
                1 => {
                    self.length = val;
                    let length_256hz = 256 - val as u32;
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;

                    // Here we set the length counter making sure nothing can use it while it is set
                    match self.length_counter.write() {
                        Ok(mut length_counter) => {
                            *length_counter = length_samples;
                        }
                        Err(error) => Logger::error(format!("Could not set wave table length: {error}")),
                    }
                }

                2 => {
                    self.volume_code = (val & 60) >> 5;
                }

                // Frequency 8 least significant bits
                3 => {
                    // No need to worry about safety since this thread is the only one which will ever change frequency
                    let new_frequency = (val as u16 & 0xFF) | (self.frequency & 0xFF00);
                    self.frequency = new_frequency;
                }

                // Frequency 3 most significant bits and Trigger
                4 => {

                    // No need to worry about safety since this thread is the only one which will ever change frequency
                    let msb = ((val as u16 & 0x7) << 8) | 0xFF;
                    let new_frequency = (msb & 0xFF00) | (self.frequency & 0xFF);
                    self.frequency = new_frequency;

                    self.length_enabled = (val & 0x40) > 0;

                    let trigger = val & 0x80;
                    self.trigger = trigger;

                    if trigger > 0 {
                        // If length == 0 reset it to 256
                        match self.length_counter.write() {
                            Ok(mut length_counter) => {
                                if *length_counter == 0 {
                                    *length_counter = ((self.sample_rate as f32 / 256.0) * 256.0).ceil() as u32;
                                }
                            }
                            Err(_error) => {
                                Logger::error("Could not set square wave length");
                            }
                        }

                        // Reset frequency timer
                        let cycles_till_next = (2048 - self.frequency as u32) * 2;
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;
                        self.frequency_timer = samples_till_next as u32;

                        // See square wave for an explanation on timer leftover
                        match self.timer_leftover.write() {
                            Ok(mut timer_leftover) => {
                                *timer_leftover = samples_till_next - samples_till_next.floor();
                            }
                            Err(_) => {
                                Logger::error("Wave table: Could not write to timer leftover")
                            }
                        }

                        self.position = 0;

                        self.enabled = true;
                    }
                }

                _ => {}
            }
        }

        pub(crate) fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub(crate) fn read_reg(&self, reg: usize) -> u8 {
            match reg {
                1 => self.length,

                2 => self.volume_code << 6,

                3 => (self.frequency & 0x00FF) as u8,

                4 => {
                    let mut reg_value = self.trigger;
                    reg_value |= (self.length_enabled as u8) << 6;
                    reg_value |= ((self.frequency & 0x0F00) >> 8) as u8;
                    reg_value
                }

                _ => 0x00,
            }
        }

        pub(crate) fn write_sound_data(&mut self, address: usize, val: u8) {
            let rel_address = address - 0xFF30;
            let start_sample = rel_address * 2;

            self.sound_data[start_sample] = val >> 4;
            self.sound_data[start_sample + 1] = val & 0x0F;
        }

        pub(crate) fn read_sound_data(&self, address: usize) -> u8 {
            let rel_address = address - 0xFF30;
            let start_sample = rel_address * 2;

            let mut reg_val = 0x00;
            reg_val |= self.sound_data[start_sample] << 4;
            reg_val |= self.sound_data[start_sample + 1];
            reg_val
        }

        pub(crate) fn generate_sample(&mut self) -> f32 {
            if !self.enabled {
                return 0.0;
            }

            let mut current_position = self.position;

            if self.frequency_timer == 0 {

                // Reset frequency timer
                let cycles_till_next = (2048 - self.frequency as u32) * 2;
                let mut samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;

                // See square wave for explanation on timer leftover
                match self.timer_leftover.write() {
                    Ok(mut timer_leftover) => {
                        *timer_leftover += samples_till_next - samples_till_next.floor();

                        if *timer_leftover > 1.0 {
                            *timer_leftover -= 1.0;
                            samples_till_next += 1.0;
                        }
                    }
                    Err(_) => {
                        Logger::error("Wave table: Could not write to timer leftover");
                    }
                }

                self.frequency_timer = samples_till_next as u32;

                // Move one position forward
                let new_position = if current_position == 31 {
                    0
                } else {
                    current_position + 1
                };

                self.position = new_position;
                current_position = new_position;
            }

            self.frequency_timer -= 1;

            let mut wave_sample = self.sound_data[current_position as usize];

            let volume_shift = match self.volume_code {
                0 => {
                    4
                }

                1 => {
                    0
                }

                2 => {
                    1
                }

                3 => {
                    2
                }

                _ => {
                    Logger::error("Wave table: unexpected volume code");
                    4
                }
            };

            wave_sample >>= volume_shift;

            if self.length_enabled {
                // Try and decrement the length counter, if we can't get access to it that means it's being reset and we don't want to decrement it anyway
                match self.length_counter.try_write() {
                    Ok(mut length_counter) => {

                        // Just in case there's an underflow
                        let new_length = match length_counter.checked_sub(1) {
                            Some(val) => {
                                val
                            }
                            None => {
                                0
                            }
                        };

                        *length_counter = new_length;

                        // If we've reached the end of the current length disable the channel
                        if *length_counter == 0 {
                            self.enabled = false;
                        }
                    }
                    Err(_error) => {}
                }
            }

            wave_sample as f32 / 15.0
        }
    }

    #[derive(Default, Serialize, Deserialize)]
    pub struct NoiseGenerator {
        sample_rate: u32,
        env: VolumeEnvelope,

        divisor_code: u8,

        divisor: u8,

        clock_shift: u8,

        frequency_timer: u32,
        timer_leftover: RwLock<f32>,
        lfsr: [bool; 15],

        width: bool,

        trigger: u8,

        enabled: bool,

        length: u8,
        length_counter: RwLock<u32>,

        length_enabled: bool,
    }

    impl NoiseGenerator {
        pub(crate) fn new(sample_rate: u32) -> NoiseGenerator {
            NoiseGenerator {
                sample_rate,
                env: VolumeEnvelope::new(sample_rate),
                lfsr: [true; 15],
                ..Default::default()
            }
        }

        pub(crate) fn write_reg(&mut self, reg: usize, val: u8) {
            match reg {
                0 => {}

                1 => {
                    let length = val & 0x3F;
                    let length_256hz = 64 - length;
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;
                    self.length = length;

                    // Here we set the length counter making sure nothing can use it while it is set
                    match self.length_counter.write() {
                        Ok(mut length_counter) => {
                            *length_counter = length_samples;
                        }
                        Err(_error) => {
                            Logger::error("Could not set noise generator length");
                        }
                    }
                }

                2 => {
                    self.env.write_settings(val);

                    // Disable channel if no DAC power
                    if val & 0xF0 == 0 {
                        self.enabled = false;
                    }
                }

                3 => {
                    let clock_shift = val >> 4;
                    let width = (val & 0x08) >> 3;
                    let divisor_code = val & 0x07;

                    let divisor = if divisor_code == 0 {
                        8
                    } else {
                        divisor_code * 16
                    };

                    self.divisor_code = divisor_code;
                    self.divisor = divisor;
                    self.clock_shift = clock_shift;

                    self.width = width != 0;
                }

                4 => {
                    self.length_enabled = (val & 0x40) > 0;

                    let trigger = val & 0x80;
                    self.trigger = trigger;

                    if trigger > 0 {
                        // If length == 0 reset it to 64
                        match self.length_counter.write() {
                            Ok(mut length_counter) => {
                                if *length_counter == 0 {
                                    *length_counter = ((self.sample_rate as f32 / 256.0) * 64.0).ceil() as u32;
                                }
                            }
                            Err(_error) => {
                                Logger::error("Could not set square wave length");
                            }
                        }

                        // Fill LFSR with 1s
                        for bit in self.lfsr.iter_mut() {
                            *bit = true;
                        }

                        // Set frequency timer
                        let frequency = (self.divisor as u32) << (self.clock_shift as u32);
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * frequency as f32;
                        self.frequency_timer = samples_till_next as u32;

                        // See square wave for an explanation on timer leftover
                        match self.timer_leftover.write() {
                            Ok(mut timer_leftover) => {
                                *timer_leftover = samples_till_next - samples_till_next.floor();
                            }
                            Err(_) => {
                                Logger::error("Noise osc: Could not write to timer leftover")
                            }
                        }

                        self.enabled = true;
                    }
                }

                _ => {
                    Logger::error("Noise Osc: Unrecognised register");
                }
            }
        }

        pub(crate) fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub(crate) fn read_reg(&self, reg: usize) -> u8 {
            match reg {
                1 => self.length,

                2 => self.env.read_settings(),

                3 => {
                    let mut reg_val = 0x00;
                    reg_val |= self.clock_shift << 4;
                    reg_val |= (self.width as u8) << 3;
                    reg_val |= self.divisor_code;
                    reg_val
                }

                4 => {
                    let mut reg_value = self.trigger;
                    reg_value |= (self.length_enabled as u8) << 6;
                    reg_value
                }

                _ => 0,
            }
        }

        pub(crate) fn generate_sample(&mut self) -> f32 {
            if !self.enabled {
                return 0.0;
            }

            let env_sample = self.env.generate_sample();
            if self.frequency_timer == 0 {
                // Reset frequency timer
                let frequency = (self.divisor as u32) << (self.clock_shift as u32);
                let mut samples_till_next = (self.sample_rate as f32 / 4194304.0) * frequency as f32;

                // See square wave for explanation on timer leftover
                match self.timer_leftover.write() {
                    Ok(mut timer_leftover) => {
                        *timer_leftover += samples_till_next - samples_till_next.floor();

                        if *timer_leftover > 1.0 {
                            *timer_leftover -= 1.0;
                            samples_till_next += 1.0;
                        }
                    }
                    Err(_) => {
                        Logger::error("Square Wave: Could not write to timer leftover");
                    }
                }

                self.frequency_timer = samples_till_next.ceil() as u32;

                // Move self.lfsr on
                let new_val = self.lfsr[0] != self.lfsr[1];
                self.lfsr.rotate_left(1);

                self.lfsr[14] = new_val;

                if self.width {
                    self.lfsr[6] = new_val;
                }
            }

            self.frequency_timer -= 1;

            let noise_sample = i32::from(self.lfsr[0]);

            if self.length_enabled {

                // Try and decrement the length counter, if we can't get access to it that means it's being reset and we don't want to decrement it anyway
                if let Ok(mut length_counter) = self.length_counter.try_write() {

                    // Just in case there's an underflow
                    let new_length = length_counter.checked_sub(1).unwrap_or(0);

                    *length_counter = new_length;

                    // If we've reached the end of the current length disable the channel
                    if *length_counter == 0 {
                        self.enabled = false;
                    }
                }
            }

            let dac_input_sample = if noise_sample != 0 {
                env_sample
            } else {
                0
            };

            dac_input_sample as f32 / 15.0
        }
    }
}

use std::cmp;
use std::cmp::min;
use std::sync::{Arc, Mutex};

use cpal::{traits::{HostTrait, DeviceTrait}, StreamConfig, StreamError, Stream, SupportedStreamConfig, SampleRate};
use serde::{Serialize, Deserialize};
use crate::logger::Logger;

#[derive(Default, Serialize, Deserialize)]
struct AudioProcessingState {
    sample_rate: u32,
    num_channels: u16,
    osc_1: oscillators::SquareWaveGenerator,
    osc_2: oscillators::SquareWaveGenerator,
    osc_3: oscillators::WaveTable,
    osc_4: oscillators::NoiseGenerator,

    left_osc_enable: [bool; 4],

    right_osc_enable: [bool; 4],

    left_master_vol: u8,

    right_master_vol: u8,

    power_control: bool,
}

impl AudioProcessingState {
    pub(crate) fn new() -> Arc<Mutex<AudioProcessingState>> {
        let config = Self::load_config();
        let sample_rate = config.sample_rate().0;
        let out_dev = cpal::default_host().default_output_device().expect("No available output device found");

        // Display device name
        if let Ok(name) = out_dev.name() {
            Logger::info(format!("Using {} at {}Hz with {} channels", name, sample_rate, config.channels()))
        }

        Arc::new(Mutex::new(AudioProcessingState {
            sample_rate,
            num_channels: config.channels(),
            osc_1: oscillators::SquareWaveGenerator::new(sample_rate, true),
            osc_2: oscillators::SquareWaveGenerator::new(sample_rate, false),
            osc_3: oscillators::WaveTable::new(sample_rate),
            osc_4: oscillators::NoiseGenerator::new(sample_rate),
            ..Default::default()
        }))
    }

    pub(crate) fn load_stream(processor: &Arc<Mutex<AudioProcessingState>>) -> Option<Stream> {
        let audio_callback_ref = processor.clone();
        let audio_error_ref = processor.clone();

        let config = Self::load_config();
        let out_dev = cpal::default_host().default_output_device().expect("No available output device found");

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio, _| audio_callback_ref.lock().unwrap().audio_block_f32(audio), move |stream_error| audio_error_ref.lock().unwrap().audio_error(stream_error), None),
            cpal::SampleFormat::I16 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio, _| audio_callback_ref.lock().unwrap().audio_block_i16(audio), move |stream_error| audio_error_ref.lock().unwrap().audio_error(stream_error), None),
            cpal::SampleFormat::U16 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio, _| audio_callback_ref.lock().unwrap().audio_block_u16(audio), move |stream_error| audio_error_ref.lock().unwrap().audio_error(stream_error), None),
            _unsupported => panic!("Unsupported stream format: {_unsupported}")
        };

        if let Err(ref error) = stream {
            Logger::error(format!("Error while building stream: {error}"));
        }

        stream.ok()
    }

    fn load_config() -> SupportedStreamConfig {
        // Setup audio interfacing
        let out_dev = cpal::default_host().default_output_device().expect("No available output device found");

        let mut supported_configs_range = out_dev.supported_output_configs().expect("Could not obtain device configs");

        supported_configs_range
            .find(|c| c.max_sample_rate() >= SampleRate(44100))
            .or(supported_configs_range.next())
            .map(|a| {
                let rate = a.max_sample_rate();
                a.with_sample_rate(min(SampleRate(44100), rate))
            }).expect("No valid audio config found.")
    }

    pub(crate) fn write_register(&mut self, address: usize, value: u8) {
        if address < 0xFF24 {
            let rel_address = address - 0xFF10;

            let osc = rel_address / 5;
            let reg = rel_address % 5;

            match osc {
                0 => self.osc_1.write_reg(reg, value),
                1 => self.osc_2.write_reg(reg, value),
                2 => self.osc_3.write_reg(reg, value),
                3 => self.osc_4.write_reg(reg, value),
                _ => Logger::error("APU Write: Unrecognised oscillator number"),
            }
        } else if (0xFF30..=0xFF3F).contains(&address) {
            self.osc_3.write_sound_data(address, value);
        } else {
            match address {
                0xFF24 => {
                    let left_vol = (value & 0x70) >> 4;
                    let right_vol = value & 0x07;

                    self.left_master_vol = left_vol;
                    self.right_master_vol = right_vol;
                }

                0xFF25 => {
                    self.left_osc_enable[3] = (value >> 7) > 0;
                    self.left_osc_enable[2] = ((value & 0x40) >> 6) > 0;
                    self.left_osc_enable[1] = ((value & 0x20) >> 5) > 0;
                    self.left_osc_enable[0] = ((value & 0x10) >> 4) > 0;

                    self.right_osc_enable[3] = ((value & 0x08) >> 3) > 0;
                    self.right_osc_enable[2] = ((value & 0x04) >> 2) > 0;
                    self.right_osc_enable[1] = ((value & 0x02) >> 1) > 0;
                    self.right_osc_enable[0] = (value & 0x01) > 0;
                }

                0xFF26 => {
                    self.power_control = (value >> 7) > 0;
                }

                _ => {
                    Logger::error(format!("APU Write: Unrecognised address: {}", address));
                }
            }
        }
    }

    pub(crate) fn read_register(&self, address: usize) -> u8 {
        if address < 0xFF24 {
            let rel_address = address - 0xFF10;

            let osc = rel_address / 5;
            let reg = rel_address % 5;

            match osc {
                0 => {
                    self.osc_1.read_reg(reg)
                }

                1 => {
                    self.osc_2.read_reg(reg)
                }

                2 => {
                    self.osc_3.read_reg(reg)
                }

                3 => {
                    self.osc_4.read_reg(reg)
                }
                _ => {
                    Logger::info("APU Read: Unrecognised oscillator number");
                    0x00
                }
            }
        } else if (0xFF30..=0xFF3F).contains(&address) {
            self.osc_3.read_sound_data(address)
        } else {
            match address {
                0xFF24 => {
                    let mut reg_val = 0x00;
                    reg_val |= self.left_master_vol << 4;
                    reg_val |= self.right_master_vol;
                    reg_val
                }
                0xFF25 => {
                    let mut reg_val = 0x00;
                    reg_val |= (self.left_osc_enable[3] as u8) << 7;
                    reg_val |= (self.left_osc_enable[2] as u8) << 6;
                    reg_val |= (self.left_osc_enable[1] as u8) << 5;
                    reg_val |= (self.left_osc_enable[0] as u8) << 4;

                    reg_val |= (self.right_osc_enable[3] as u8) << 3;
                    reg_val |= (self.right_osc_enable[2] as u8) << 2;
                    reg_val |= (self.right_osc_enable[1] as u8) << 1;
                    reg_val |= self.right_osc_enable[0] as u8;

                    reg_val
                }
                0xFF26 => {
                    let mut reg_val = (self.power_control as u8) << 7;
                    reg_val |= (self.osc_4.is_enabled() as u8) << 3;
                    reg_val |= (self.osc_3.is_enabled() as u8) << 2;
                    reg_val |= (self.osc_2.is_enabled() as u8) << 1;
                    reg_val |= self.osc_1.is_enabled() as u8;
                    reg_val
                }
                _ => {
                    Logger::error("APU Read: Unrecognised address");
                    0x00
                }
            }
        }
    }

    fn audio_block_f32(&mut self, audio: &mut [f32]) {
        let num_samples = audio.len() / self.num_channels as usize;

        for sample_index in 0..num_samples {
            let generated_samples = self.generate_samples();

            let first_channel_index = sample_index * self.num_channels as usize;

            match self.num_channels.cmp(&1) {
                cmp::Ordering::Equal => audio[first_channel_index] = (generated_samples.0 + generated_samples.1) / 2.0,
                cmp::Ordering::Greater => {
                    audio[first_channel_index] = generated_samples.0;
                    audio[first_channel_index + 1] = generated_samples.1;
                }
                cmp::Ordering::Less => (),
            }
        }
    }

    fn audio_block_i16(&mut self, audio: &mut [i16]) {
        let num_samples = audio.len() / self.num_channels as usize;

        for sample_index in 0..num_samples {
            let f32_samples = self.generate_samples();

            let left_sample = (f32_samples.0 * i16::MAX as f32) as i16;
            let right_sample = (f32_samples.1 * i16::MAX as f32) as i16;

            let first_channel_index = sample_index * self.num_channels as usize;

            match self.num_channels.cmp(&1) {
                cmp::Ordering::Equal => audio[first_channel_index] = (left_sample + right_sample) / 2,
                cmp::Ordering::Greater => {
                    audio[first_channel_index] = left_sample;
                    audio[first_channel_index + 1] = right_sample;
                }
                cmp::Ordering::Less => (),
            }
        }
    }

    fn audio_block_u16(&mut self, audio: &mut [u16]) {
        let num_samples = audio.len() / self.num_channels as usize;

        for sample_index in 0..num_samples {
            let f32_samples = self.generate_samples();

            let left_sample = ((f32_samples.0 + 1.0) * u16::MAX as f32) as u16;
            let right_sample = ((f32_samples.1 + 1.0) * u16::MAX as f32) as u16;

            let first_channel_index = sample_index * self.num_channels as usize;

            match self.num_channels.cmp(&1) {
                cmp::Ordering::Equal => audio[first_channel_index] = (left_sample + right_sample) / 2,
                cmp::Ordering::Greater => {
                    audio[first_channel_index] = left_sample;
                    audio[first_channel_index + 1] = right_sample;
                }
                cmp::Ordering::Less => (),
            }
        }
    }

    fn audio_error(&self, error: StreamError) {
        Logger::error(format!("Audio Error: {:?}", error));
    }

    fn generate_samples(&mut self) -> (f32, f32) {
        if !self.power_control {
            return (0.0, 0.0);
        }

        let mut mixed_left_sample = self.left_master_vol as f32 / 15.0;
        let mut mixed_right_sample = self.right_master_vol as f32 / 15.0;

        let osc_1_sample = self.osc_1.generate_sample();
        if self.left_osc_enable[0] {
            mixed_left_sample += osc_1_sample;
        }
        if self.right_osc_enable[0] {
            mixed_right_sample += osc_1_sample;
        }

        let osc_2_sample = self.osc_2.generate_sample();
        if self.left_osc_enable[1] {
            mixed_left_sample += osc_2_sample;
        }
        if self.right_osc_enable[1] {
            mixed_right_sample += osc_2_sample;
        }

        let osc_3_sample = self.osc_3.generate_sample();
        if self.left_osc_enable[2] {
            mixed_left_sample += osc_3_sample;
        }
        if self.right_osc_enable[2] {
            mixed_right_sample += osc_3_sample;
        }

        let osc_4_sample = self.osc_4.generate_sample();
        if self.left_osc_enable[3] {
            mixed_left_sample += osc_4_sample;
        }
        if self.right_osc_enable[3] {
            mixed_right_sample += osc_4_sample;
        }

        mixed_left_sample /= 4.0;
        mixed_right_sample /= 4.0;

        mixed_left_sample *= self.left_master_vol as f32 / 15.0;
        mixed_right_sample *= self.right_master_vol as f32 / 15.0;

        (mixed_left_sample, mixed_right_sample)
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Default)]
pub struct AudioProcessingUnit {
    state: Arc<Mutex<AudioProcessingState>>,
    #[serde(skip)]
    pub(crate) stream: Option<Stream>,
}

impl AudioProcessingUnit {
    pub(crate) fn new() -> AudioProcessingUnit {
        let state = AudioProcessingState::new();
        let stream = AudioProcessingState::load_stream(&state);
        AudioProcessingUnit { state, stream }
    }

    pub(crate) fn init(&mut self) {
        self.stream = AudioProcessingState::load_stream(&self.state);
    }

    pub(crate) fn write(&mut self, address: usize, value: u8) -> bool {
        if !(0xFF10..=0xFF3F).contains(&address) {
            false
        } else {
            self.state.lock().unwrap().write_register(address, value);
            true
        }
    }

    pub(crate) fn read(&self, address: usize) -> Option<u8> {
        if !(0xFF10..=0xFF3F).contains(&address) {
            None
        } else {
            Some(self.state.lock().unwrap().read_register(address))
        }
    }
}