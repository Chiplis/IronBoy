mod oscillators {
    use rand::seq::index::sample;
    use serde::{Serialize, Deserialize};
    use winit::event::ElementState;
    use std::sync::{atomic::{AtomicU16, AtomicU8, Ordering, AtomicBool, AtomicU32}, RwLock, Mutex};

    #[derive(Serialize, Deserialize)]
    struct VolumeEnvelopeParams {
        add_mode: bool,
        period: u32,
        current_level: u8,
        sample_counter: u32,
    }

    #[derive(Serialize, Deserialize)]
    struct VolumeEnvelope {
        sample_rate: u32,
        params: Mutex<VolumeEnvelopeParams>,
        last_val: AtomicU8, 
    }

    impl VolumeEnvelope {
        pub fn new(sample_rate: u32) -> VolumeEnvelope {
            VolumeEnvelope {sample_rate: sample_rate,
                            params: Mutex::new(VolumeEnvelopeParams{
                                add_mode: false, 
                                period: 0, 
                                current_level: 0,
                                sample_counter: 0,}
                            ),
                            last_val: AtomicU8::new(0),}
        }

        pub fn write_settings(&self, val: u8) {
            let starting_vol = val >> 4;
            let add_mode = ((val & 0x08) >> 3) > 0;
            let period = (self.sample_rate / 64) * ((val & 0x07) as u32);

            //Get the lock for all items
            match self.params.lock() {
                Ok(mut params) => {
                    params.current_level = starting_vol;
                    params.add_mode = add_mode;
                    params.period = period;
                    params.sample_counter = 0;
                }
                Err(error) => {
                    println!("Could not obtain envelope data lock");
                }
            }
        }

        pub fn generate_sample(&self) -> f32 {
            match self.params.lock() {
                Ok(mut params) => {
                    self.last_val.store(params.current_level, Ordering::Relaxed);
                    let output_sample = params.current_level as f32 / 15.0;

                    //Apply envelope
                    if params.period > 0 {

                        //Check if level change is needed
                        if params.period == params.sample_counter {
                            if params.add_mode && params.current_level < 15 {
                                params.current_level +=  1;
                            }
                            else if !params.add_mode && params.current_level > 0 {
                                params.current_level -= 1;
                            }

                            params.sample_counter = 0;
                        }
                        else {
                            params.sample_counter += 1;
                        }
                    }

                    return output_sample;
                }

                Err(error) => {
                    println!("missed vol env sample");
                    return self.last_val.load(Ordering::Relaxed) as f32 / 15.0;
                }
            }
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct SquareWaveGenerator {
        frequency: AtomicU16,
        frequency_timer: AtomicU32,
        sample_rate: u32,
        sweep: bool,
        position: AtomicU8,
        duty: AtomicU8,
        enabled: AtomicBool,
        length_counter: RwLock<u32>,
        length_enabled: AtomicBool,
        env: VolumeEnvelope,
        sweep_time_samples: AtomicU32,
        sweep_negate: AtomicBool,
        sweep_shift: AtomicU8,
        sweep_sample_counter: AtomicU32,
    }

    impl SquareWaveGenerator {
        pub fn new(sample_rate: u32, sweep: bool) -> SquareWaveGenerator {
            SquareWaveGenerator {frequency: AtomicU16::new(0), 
                                 frequency_timer: AtomicU32::new(0),
                                 sample_rate: sample_rate, 
                                 sweep: sweep,
                                 position: AtomicU8::new(0),
                                 duty: AtomicU8::new(2), 
                                 enabled: AtomicBool::new(false),
                                 length_counter: RwLock::new(0), 
                                 length_enabled: AtomicBool::new(false),
                                 env: VolumeEnvelope::new(sample_rate),
                                 sweep_time_samples: AtomicU32::new(0),
                                 sweep_negate: AtomicBool::new(false),
                                 sweep_shift: AtomicU8::new(0),
                                 sweep_sample_counter: AtomicU32::new(0),}
        }

        pub fn write_reg(&self, reg: usize, val: u8) {
            match reg {
                0 => {
                    if self.sweep {
                        let period = (val & 0x70) >> 4;
                        let negate = (val & 0x08) > 0;
                        let shift = val & 0x07;

                        let sweep_time_samples: u32 = if period == 0 {
                            0
                        }
                        else
                        {
                            (period as f32 / 128 as f32) as u32 * self.sample_rate
                        };

                        self.sweep_negate.store(negate, Ordering::Relaxed);
                        self.sweep_shift.store(shift, Ordering::Relaxed);
                        self.sweep_time_samples.store(sweep_time_samples, Ordering::Relaxed);
                        self.sweep_sample_counter.store(0, Ordering::Relaxed);
                    }
                }

                //Duty and length
                1 => {
                    let new_duty = val >> 6;
                    self.duty.store(new_duty, Ordering::Relaxed);

                    let length_256hz = 64 - (val & 0x3F);
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;

                    //Here we set the length counter making sure nothing can use it while it is set
                    match self.length_counter.write() {
                        Ok(mut length_counter) => {
                            *length_counter = length_samples;
                        }
                        Err(error) => {
                            println!("Could not set square wave length");
                        }
                    }
                }

                //Volume envelope
                2 => {
                    self.env.write_settings(val);
                }

                //Frequency 8 least significant bits
                3 => {
                    //No need to worry about safety since this thread is the only one which will ever change frequency
                    let new_frequency = (val as u16 & 0xFF) | (self.frequency.load(Ordering::Relaxed) & 0xFF00);
                    self.frequency.store(new_frequency, Ordering::Relaxed);
                }

                //Frequency 3 most significant bits and Trigger
                4 => {

                    //No need to worry about safety since this thread is the only one which will ever change frequency
                    let msb = ((val as u16 & 0x7) << 8) | 0xFF;
                    let new_frequency = (msb & 0xFF00) | (self.frequency.load(Ordering::Relaxed) & 0xFF);
                    self.frequency.store(new_frequency, Ordering::Relaxed);

                    self.length_enabled.store((val & 0x40) > 0, Ordering::Relaxed);

                    let trigger = val & 0x80;
                    if trigger > 0 {
                        //If length == 0 reset it to 64
                        match self.length_counter.write() {
                            Ok(mut length_counter) => {
                                if *length_counter == 0 {
                                    *length_counter = ((self.sample_rate as f32 / 256.0) * 64.0).ceil() as u32;
                                }
                            }
                            Err(error) => {
                                println!("Could not set square wave length");
                            }
                        }

                        //Reset frequency
                        let cycles_till_next = (2048 - self.frequency.load(Ordering::Relaxed) as u32) * 4;
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;
                        self.frequency_timer.store(samples_till_next as u32, Ordering::Relaxed);

                        //Set enabled
                        self.enabled.store(true, Ordering::Relaxed);
                    }
                }

                _ => {
                    println!("Square Wave Osc: Unrecognised register");
                }
            }
        }

        pub fn generate_sample(&self) -> f32 {
            let mut output_sample = 0.0;

            if !self.enabled.load(Ordering::Relaxed) {
                return output_sample;
            }

            if self.frequency_timer.load(Ordering::Relaxed) <= 0
            {
                let cycles_till_next = (2048 - self.frequency.load(Ordering::Relaxed) as u32) * 4;
                let samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;

                self.frequency_timer.store(samples_till_next as u32, Ordering::Relaxed);
                
                let current_position = self.position.load(Ordering::Relaxed);

                let mut new_position = current_position + 1;
                if new_position >= 8 {
                    new_position = 0;
                }

                self.position.store(new_position, Ordering::Relaxed);
            }

            self.frequency_timer.store(self.frequency_timer.load(Ordering::Relaxed) - 1, Ordering::Relaxed);

            let envelope_sample = self.env.generate_sample();

            match self.duty.load(Ordering::Relaxed) {
                //12.5%
                0 => {
                    if self.position.load(Ordering::Relaxed) == 7 {
                        output_sample = 1.0;
                    }
                    else {
                        output_sample = 0.0;
                    }
                }

                //25%
                1 => {
                    if self.position.load(Ordering::Relaxed) >= 6 {
                        output_sample = 1.0;
                    }
                    else {
                        output_sample = 0.0;
                    }
                }

                //50%
                2 => {
                    if self.position.load(Ordering::Relaxed) >= 4 {
                        output_sample = 1.0;
                    }
                    else {
                        output_sample = 0.0;
                    }
                }

                //75%
                3 => {
                    if self.position.load(Ordering::Relaxed) >= 6 {
                        output_sample = 0.0;
                    }
                    else {
                        output_sample = 1.0;
                    }
                }

                _ => {}
            }

            if self.length_enabled.load(Ordering::Relaxed) {
                //Try and decrement the length counter, if we can't get access to it that means it's being reset and we don't want to decrement it anyway
                match self.length_counter.try_write() {
                    Ok(mut length_counter) => {

                        //Just in case there's an underflow
                        let new_length = match length_counter.checked_sub(1) {
                            Some(val) => {
                                val
                            }
                            None => {
                                0
                            }
                        };

                        *length_counter = new_length;

                        //If we've reached the end of the current length disable the channel
                        if *length_counter <= 0 {
                            self.enabled.store(false, Ordering::Relaxed);
                        }
                    }
                    Err(_error) => {

                    }
                }
            }
            output_sample * envelope_sample
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct WaveTable {
        sample_rate: u32,
        sound_data: [AtomicU8; 32],
        frequency: AtomicU16,
        position_counter: AtomicU32,
        samples_at_position: AtomicU32,
        enabled: AtomicBool, 
        last_set_length: AtomicU32, 
        length_counter: AtomicU32,
        volume_code: AtomicU8,
    }

    impl WaveTable {
        pub fn new(sample_rate: u32) -> WaveTable {

            const GENERATOR: AtomicU8 = AtomicU8::new(0);

            WaveTable { sample_rate: sample_rate,
                        sound_data: [GENERATOR; 32], 
                        frequency: AtomicU16::new(0),
                        position_counter: AtomicU32::new(0),
                        samples_at_position: AtomicU32::new(0),
                        enabled: AtomicBool::new(false), 
                        last_set_length: AtomicU32::new(0), 
                        length_counter: AtomicU32::new(0),
                        volume_code: AtomicU8::new(0),}
        }

        pub fn write_reg(&self, reg: usize, val: u8) {
            match reg {
                0 => {

                }
                1 => {
                    let new_length = (self.sample_rate / 256) * 64 - val as u32;
                    self.last_set_length.store(new_length, Ordering::Relaxed);
                }

                2 => {
                    self.volume_code.store((val & 60) >> 5, Ordering::Relaxed);
                }

                //Frequency 8 least significant bits
                3 => {

                    //No need to worry about safety since this thread is the only one which will ever change frequency
                    let new_frequency = (val as u16 & 0xFF) | (self.frequency.load(Ordering::Relaxed) & 0xFF00);
                    self.frequency.store(new_frequency, Ordering::Relaxed);
                }

                //Frequency 3 most significant bits and Trigger
                4 => {

                    //No need to worry about safety since this thread is the only one which will ever change frequency
                    let msb = ((val as u16 & 0x7) << 8) | 0xFF;
                    let new_frequency = (msb & 0xFF00) | (self.frequency.load(Ordering::Relaxed) & 0xFF);
                    self.frequency.store(new_frequency, Ordering::Relaxed);

                    let trigger = val & 0x80;
                    if trigger > 0 {
                        self.enabled.store(true, Ordering::Relaxed);
                        self.length_counter.store(self.last_set_length.load(Ordering::Relaxed), Ordering::Relaxed);
                    }
                }

                _ => {

                }
            }
        }

        pub fn write_sound_data(&self, address: usize, val: u8) {
            let rel_address = address - 0xFF30;

            let start_sample = rel_address * 2;

            self.sound_data[start_sample].store(val >> 4, Ordering::Relaxed);
            self.sound_data[start_sample + 1].store(val & 0x0F, Ordering::Relaxed);
        }

        pub fn generate_sample(&self) -> f32 {
            
            let mut output_sample = 0.0;

            if !self.enabled.load(Ordering::Relaxed)  || self.length_counter.load(Ordering::Relaxed) <= 0 || self.frequency.load(Ordering::Relaxed) <= 0 {
                return output_sample;
            }

            let current_position = self.position_counter.load(Ordering::Relaxed);

            output_sample = self.sound_data[current_position as usize].load(Ordering::Relaxed) as f32 / 15.0;

            let change_time_samples = self.sample_rate / self.frequency.load(Ordering::Relaxed) as u32;

            if self.samples_at_position.load(Ordering::Relaxed) >= change_time_samples {
                let new_position = if current_position == 31 {
                    0
                }
                else {
                    current_position + 1
                };

                self.position_counter.store(new_position, Ordering::Relaxed);
                self.samples_at_position.store(0, Ordering::Relaxed);
            }
            else {
                self.samples_at_position.store(self.samples_at_position.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
            }

            let volume = match self.volume_code.load(Ordering::Relaxed) {
                0 => {
                    0.0
                }

                1 => {
                    1.0
                }

                2 => {
                    0.5
                }

                3 => {
                    0.25
                }

                _ => {
                    println!("Wave table: unexpected volume code");
                    1.0
                }
            };

            output_sample *= volume;

            //Decrement the length counter making sure no underflow happens if length changed during that
            let new_length = match self.length_counter.load(Ordering::Relaxed).checked_sub(1) {
                Some(val) => {
                    val
                }
                None => {
                    0
                }
            };

            self.length_counter.store(new_length, Ordering::Relaxed);

            if self.length_counter.load(Ordering::Relaxed) <= 0 {
                self.enabled.store(false, Ordering::Relaxed);
            }
            
            output_sample
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct NoiseGenerator {
        sample_rate: u32,
        env: VolumeEnvelope,
        divisor : AtomicU8,
        clock_shift: AtomicU8,
        frequency_timer: AtomicU32,
        sample_counter: AtomicU32,
        LFSR: Mutex<[bool; 15]>,
        width: AtomicBool,
        enabled: AtomicBool,
        length_counter: RwLock<u32>,
        length_enabled: AtomicBool,
    }

    impl NoiseGenerator {
        pub fn new(sample_rate: u32) -> NoiseGenerator {
            NoiseGenerator { sample_rate: sample_rate,
                             env: VolumeEnvelope::new(sample_rate),
                             divisor: AtomicU8::new(0),
                             clock_shift: AtomicU8::new(0),
                             frequency_timer: AtomicU32::new(0),
                             sample_counter: AtomicU32::new(0),
                             LFSR: Mutex::new([true; 15]),
                             width: AtomicBool::new(false),
                             enabled: AtomicBool::new(false), 
                             length_counter: RwLock::new(0),
                             length_enabled: AtomicBool::new(false),}
        }

        pub fn write_reg(&self, reg: usize, val: u8) {
            match reg {
                0 => {
                    
                }

                1 => {
                    let length_256hz = 64 - (val & 0x3F);
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;

                    //Here we set the length counter making sure nothing can use it while it is set
                    match self.length_counter.write() {
                        Ok(mut length_counter) => {
                            *length_counter = length_samples;
                        }
                        Err(error) => {
                            println!("Could not set square wave length");
                        }
                    }
                }

                2 => {
                    self.env.write_settings(val);
                }

                3 => {
                    let clock_shift = val >> 4;
                    let width = (val & 0x08) >> 3;
                    let divisor_code = val & 0x07;

                    let divisor = if divisor_code == 0 {
                        8
                    }
                    else {
                        divisor_code * 16
                    };

                    self.divisor.store(divisor, Ordering::Relaxed);
                    self.clock_shift.store(clock_shift, Ordering::Relaxed);

                    self.width.store(width != 0, Ordering::Relaxed);
                }

                4 => {
                    self.length_enabled.store((val & 0x40) > 0, Ordering::Relaxed);

                    let trigger = val & 0x80;
                    if trigger > 0 {
                        //If length == 0 reset it to 64
                        match self.length_counter.write() {
                            Ok(mut length_counter) => {
                                if *length_counter == 0 {
                                    *length_counter = ((self.sample_rate as f32 / 256.0) * 64.0).ceil() as u32;
                                }
                            }
                            Err(error) => {
                                println!("Could not set square wave length");
                            }
                        }

                        //Fill LFSR with 1s
                        match self.LFSR.lock() {
                            Ok(mut LFSR) => {
                                for bit in LFSR.iter_mut() {
                                    *bit = true;
                                }
                            }
                            Err(error) => {
                                println!("Could not obtain LFSR Mutex");
                            }
                        }

                        //Set frequency timer
                        let frequency = (self.divisor.load(Ordering::Relaxed) as u32) << (self.clock_shift.load(Ordering::Relaxed) as u32);
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * frequency as f32;
                        self.frequency_timer.store(samples_till_next as u32, Ordering::Relaxed);

                        self.enabled.store(true, Ordering::Relaxed);
                    }
                }

                _ => {
                    println!("Noise Osc: Unrecognised register");
                }
            }
        }

        pub fn generate_sample(&self) -> f32 {
            let env_sample = self.env.generate_sample();
            let mut output_sample = 0.0;

            if !self.enabled.load(Ordering::Relaxed) {
                return output_sample;
            }

            match self.LFSR.lock() {
                Ok(mut LFSR) => {
                    if self.frequency_timer.load(Ordering::Relaxed) <= 0 {
                        //Reset frequency timer
                        let frequency = (self.divisor.load(Ordering::Relaxed) as u32) << (self.clock_shift.load(Ordering::Relaxed) as u32);
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * frequency as f32;
                        self.frequency_timer.store(samples_till_next.ceil() as u32, Ordering::Relaxed);

                        //Move LFSR on
                        let new_val = LFSR[0] != LFSR[1];
                        LFSR.rotate_left(1);

                        let write_pos = if self.width.load(Ordering::Relaxed) {
                            6
                        }
                        else {
                            14
                        };

                        LFSR[write_pos] = new_val;
                    }

                    self.frequency_timer.store(self.frequency_timer.load(Ordering::Relaxed) - 1, Ordering::Relaxed);

                    output_sample = if LFSR[0] == true {
                        1.0
                    }
                    else {
                        0.0
                    };
                }
                //This should never happen
                Err(error) => {
                    println!("Could not obtain LFSR lock");
                }
            }

            if self.length_enabled.load(Ordering::Relaxed) {
                //Try and decrement the length counter, if we can't get access to it that means it's being reset and we don't want to decrement it anyway
                match self.length_counter.try_write() {
                    Ok(mut length_counter) => {

                        //Just in case there's an underflow
                        let new_length = match length_counter.checked_sub(1) {
                            Some(val) => {
                                val
                            }
                            None => {
                                0
                            }
                        };

                        *length_counter = new_length;

                        //If we've reached the end of the current length disable the channel
                        if *length_counter <= 0 {
                            self.enabled.store(false, Ordering::Relaxed);
                        }
                    }
                    Err(_error) => {

                    }
                }
            }
            env_sample * output_sample
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SineGen {
    sample_rate: u32,
    phase: std::sync::Mutex<f32>,
    phase_step: f32,
}

impl SineGen {
    fn new(sample_rate: u32) -> SineGen {
        SineGen { sample_rate: sample_rate, phase: std::sync::Mutex::new(0.0), phase_step: 1000.0 / sample_rate as f32 }
    }

    fn generate_sample(&self) -> f32 {
        match self.phase.lock() {
            Ok(mut phase) => {
                let sample = (*phase * 2.0 * std::f32::consts::PI).sin();
                *phase = (*phase + self.phase_step) % 1.0;
                sample
            }
            Err(error) => {
                0.0
            }
        }
    }
}

use std::sync::{Arc, atomic::{AtomicBool, AtomicU8, Ordering}};

use cpal::{traits::{HostTrait, DeviceTrait}, StreamConfig, OutputCallbackInfo, StreamError};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct AudioProcessingState {
    sample_rate: u32,
    num_channels: u16,
    osc_1: oscillators::SquareWaveGenerator,
    osc_2: oscillators::SquareWaveGenerator,
    osc_3: oscillators::WaveTable,
    osc_4: oscillators::NoiseGenerator,

    left_osc_1_enable: AtomicBool,
    left_osc_2_enable: AtomicBool,
    left_osc_3_enable: AtomicBool,
    left_osc_4_enable: AtomicBool,
    right_osc_1_enable: AtomicBool,
    right_osc_2_enable: AtomicBool,
    right_osc_3_enable: AtomicBool,
    right_osc_4_enable: AtomicBool,

    left_master_vol: AtomicU8,
    right_master_vol: AtomicU8,

    test_osc: SineGen,
}

impl AudioProcessingState {
    pub fn new() -> (Arc<AudioProcessingState>, cpal::Stream) {
        //Setup audio interfacing
        let out_dev = cpal::default_host().default_output_device().expect("No available output device found");

        //Display device name
        match out_dev.name() {
            Ok(name) => {
                println!("Using {}", name);
            }
            Err(_) => {}
        }

        let mut supported_configs_range = out_dev.supported_output_configs().expect("Could not obtain device configs");
        let config = supported_configs_range.next().expect("No available configs").with_max_sample_rate();

        let processor = Arc::new(AudioProcessingState{sample_rate: config.sample_rate().0, 
                                                                                       num_channels: config.channels(),
                                                                                       osc_1: oscillators::SquareWaveGenerator::new(config.sample_rate().0, true), 
                                                                                       osc_2: oscillators::SquareWaveGenerator::new(config.sample_rate().0, false),
                                                                                       osc_3: oscillators::WaveTable::new(config.sample_rate().0),
                                                                                       osc_4: oscillators::NoiseGenerator::new(config.sample_rate().0),
                                                                                       left_osc_1_enable: AtomicBool::new(false),
                                                                                       left_osc_2_enable: AtomicBool::new(false),
                                                                                       left_osc_3_enable: AtomicBool::new(false),
                                                                                       left_osc_4_enable: AtomicBool::new(false),
                                                                                       right_osc_1_enable: AtomicBool::new(false),
                                                                                       right_osc_2_enable: AtomicBool::new(false),
                                                                                       right_osc_3_enable: AtomicBool::new(false),
                                                                                       right_osc_4_enable: AtomicBool::new(false),
                                                                                       left_master_vol: AtomicU8::new(0),
                                                                                       right_master_vol: AtomicU8::new(0),
                                                                                       test_osc: SineGen::new(config.sample_rate().0),
                                                                                       });

        let audio_callback_ref = processor.clone();
        let audio_error_ref = processor.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio: &mut [f32], info: &OutputCallbackInfo| audio_callback_ref.audio_block_f32(audio, info), move |stream_error| audio_error_ref.audio_error(stream_error)),
            cpal::SampleFormat::I16 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio: &mut [i16], info: &OutputCallbackInfo| audio_callback_ref.audio_block_i16(audio, info), move |stream_error| audio_error_ref.audio_error(stream_error)),                
            cpal::SampleFormat::U16 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio: &mut [u16], info: &OutputCallbackInfo| audio_callback_ref.audio_block_u16(audio, info), move |stream_error| audio_error_ref.audio_error(stream_error))
        };

        match stream {
            Ok(_) => println!("Stream Created"),
            Err(_) => println!("Stream Failed")
        }

        (processor, stream.unwrap())
    }

    pub fn write_register(&self, address: usize, value: u8) {
        if address < 0xFF24 {
            let rel_address = address - 0xFF10;

            let osc = rel_address / 5;
            let reg = rel_address % 5;

            match osc {
                0 => {
                    self.osc_1.write_reg(reg, value);
                }

                1 => {
                    self.osc_2.write_reg(reg, value);
                }

                2 => {
                    self.osc_3.write_reg(reg, value);
                }

                3 => {
                    self.osc_4.write_reg(reg, value);
                }
                _ => {
                    println!("Unrecognised oscillator number");
                }
            }
        }
        else if address >= 0xFF30 && address <= 0xFF3F {
            self.osc_3.write_sound_data(address, value);
        }  
        else {
            match address {
                0xFF24 => {
                    let left_vol = (value & 0x70) >> 4;
                    let right_vol = value & 0x07;

                    self.left_master_vol.store(left_vol, Ordering::Relaxed);
                    self.right_master_vol.store(right_vol, Ordering::Relaxed);
                }

                0xFF25 => {
                    self.left_osc_4_enable.store((value >> 7) > 0, Ordering::Relaxed);
                    self.left_osc_3_enable.store(((value & 0x40) >> 6) > 0, Ordering::Relaxed);
                    self.left_osc_2_enable.store(((value & 0x20) >> 5) > 0, Ordering::Relaxed);
                    self.left_osc_1_enable.store(((value & 0x10) >> 4) > 0, Ordering::Relaxed);

                    self.right_osc_4_enable.store(((value & 0x08) >> 3) > 0, Ordering::Relaxed);
                    self.right_osc_3_enable.store(((value & 0x04) >> 2) > 0, Ordering::Relaxed);
                    self.right_osc_2_enable.store(((value & 0x02) >> 1) > 0, Ordering::Relaxed);
                    self.right_osc_1_enable.store((value & 0x01) > 0, Ordering::Relaxed);
                }

                0xFF26 => {

                }

                _ => {
                    println!("Audio: Unrecognised address: {}", address);
                }
            }
        } 
    }

    fn audio_block_f32(&self, audio: &mut [f32], info: &OutputCallbackInfo) {
        for chunk in audio.chunks_mut(self.num_channels as usize) {
            let generated_sample = self.generate_sample();

            for sample in chunk.iter_mut() {
                *sample = generated_sample;
            }
        }
    }
    
    fn audio_block_i16(&self, audio: &mut [i16], _info: &OutputCallbackInfo) {
        for chunk in audio.chunks_mut(self.num_channels as usize) {
            let f32_sample = self.generate_sample();
            let i16_sample = (f32_sample * i16::MAX as f32) as i16;

            for sample in chunk.iter_mut() {
                *sample = i16_sample;
            }
        }
    }
    
    fn audio_block_u16(&self, audio: &mut [u16], _info: &OutputCallbackInfo) {
        for chunk in audio.chunks_mut(self.num_channels as usize) {
            let f32_sample = self.generate_sample();
            let u16_sample = ((f32_sample + 1.0) * i16::MAX as f32) as u16;

            for sample in chunk.iter_mut() {
                *sample = u16_sample;
            }
        }
    }
    
    fn audio_error(&self, error: StreamError) {
        println!("Audio Error");
    }

    fn generate_sample(&self) -> f32 {
        let mut mixed_sample = 0.0;

        //Only doing left channel at the moment
        let osc_1_sample = self.osc_1.generate_sample();
        if self.left_osc_1_enable.load(Ordering::Relaxed) {
            mixed_sample += osc_1_sample;
        }

        let osc_2_sample = self.osc_2.generate_sample();
        if self.left_osc_2_enable.load(Ordering::Relaxed) {
            mixed_sample += osc_2_sample;
        }

        let osc_3_sample = self.osc_3.generate_sample();
        if self.left_osc_3_enable.load(Ordering::Relaxed) {
            //mixed_sample += osc_3_sample;
        }

        let osc_4_sample = self.osc_4.generate_sample();
        if self.left_osc_4_enable.load(Ordering::Relaxed) {
            mixed_sample += osc_4_sample;
        }

        mixed_sample *= (self.left_master_vol.load(Ordering::Relaxed) as f32) / 7.0;

        return mixed_sample * 0.3;
    }
}

#[derive(Serialize, Deserialize)]
pub struct AudioProcessingUnit {
    state: Arc<AudioProcessingState>,

    #[serde(skip)]
    stream: Option<cpal::Stream>,
}

impl AudioProcessingUnit {
    pub fn new() -> AudioProcessingUnit {
        let temp = AudioProcessingState::new();

        AudioProcessingUnit { state: temp.0, stream: Some(temp.1) }
    }

    pub fn write_register(&self, address: usize, value: u8) {
        self.state.write_register(address, value);
    }
}