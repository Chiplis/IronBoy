mod oscillators {
    use serde::{Serialize, Deserialize};
    use std::sync::{atomic::{AtomicU16, AtomicU8, Ordering, AtomicBool, AtomicU32}, RwLock};

    #[derive(Serialize, Deserialize)]
    pub struct SquareWaveGenerator {
        frequency: AtomicU16,
        sample_rate: u32,
        position: RwLock<u32>,
        duty: AtomicU8,
        enabled: AtomicBool,
        last_set_length: AtomicU32,
        length_counter: AtomicU32,
    }

    /*Square Wave Generator Thread and Real-Time Safety
    Position must change whenever frequency or duty change and this must happen at the exact time of that change.
    Using a mutex round this object means that the generate sample function cannot run when frequency or duty are simply being read.
    The combination of frequency and duty being atomic and position being under a read/write lock allows this to happen.
    If frequency or duty want to be read they can be because they are atomic. 
    However if they want to be changed the position write lock must be used. This stops the generate sample function running at the same time.
    To maintain this, frequency and duty must only ever be changed using the set_freq or set_duty functions. If they are changed in other places the safety cannot be guaranteed
    */

    impl SquareWaveGenerator {
        pub fn new(sample_rate: u32) -> SquareWaveGenerator {
            SquareWaveGenerator {frequency: AtomicU16::new(1917), sample_rate: sample_rate, position: RwLock::new(0), duty: AtomicU8::new(2), enabled: AtomicBool::new(false), last_set_length: AtomicU32::new(0), length_counter: AtomicU32::new(0)}
        }

        pub fn write_reg(&self, reg: usize, val: u8) {
            match reg {
                0 => {

                }

                //Duty and length
                1 => {
                    let new_duty = val >> 6;
                    self.set_duty(new_duty);

                    let new_length = (self.sample_rate / 256) * 64 - (val & 0x3F) as u32;
                    self.last_set_length.store(new_length, Ordering::Relaxed);
                }

                2 => {

                }

                //Frequency 8 least significant bits
                3 => {

                    //No need to worry about safety since this thread is the only one which will ever change frequency
                    let new_frequency = (val as u16 & 0xFF) | (self.frequency.load(Ordering::Relaxed) & 0xFF00);
                    self.set_frequency(new_frequency);
                }

                //Frequency 3 most significant bits and Trigger
                4 => {

                    //No need to worry about safety since this thread is the only one which will ever change frequency
                    let msb = ((val as u16 & 0x7) << 8) | 0xFF;
                    let new_frequency = (msb & 0xFF00) | (self.frequency.load(Ordering::Relaxed) & 0xFF);
                    self.set_frequency(new_frequency);

                    let trigger = val & 0x80;
                    if trigger > 0 {
                        println!("Trigger");
                        self.enabled.store(true, Ordering::Relaxed);
                        self.length_counter.store(self.last_set_length.load(Ordering::Relaxed), Ordering::Relaxed);
                    }
                }

                _ => {
                    println!("Osc 1: Unrecognised register");
                }
            }
        }

        fn set_frequency(&self, new_frequency: u16) {
            if self.frequency.load(Ordering::Relaxed) != new_frequency {
                match self.position.write() {
                    Ok(mut position) => {
                        self.frequency.store(new_frequency, Ordering::Relaxed);
                        *position = 0;
                    }
                    Err(error) => {
                        println!("Could not obtain position write lock");
                    }
                }
            }
        }

        fn set_duty(&self, new_duty: u8) {
            if(self.duty.load(Ordering::Relaxed) != new_duty) {
                match self.position.write() {
                    Ok(mut position) => {
                        self.duty.store(new_duty, Ordering::Relaxed);
                        *position = 0;
                    }
                    Err(error) => {
                        println!("Could not obtain position write lock");
                    }
                }
            }
        }

        pub fn generate_sample(&self) -> f32 {

            let mut output_sample = 0.0;

            if !self.enabled.load(Ordering::Relaxed)  || self.length_counter.load(Ordering::Relaxed) <= 0 {
                return output_sample;
            }

            match self.position.try_write() {
                Ok(mut position) => {
                    //Can now be assured frequency and duty will not change during this buffer

                    let period = 1.0 / (131072.0 / (2048 - self.frequency.load(Ordering::Relaxed) as u32) as f32);
                    let period_samples = (period * self.sample_rate as f32) as u32 * 2;

                    match self.duty.load(Ordering::Relaxed) {
                        //12.5%
                        0 => {
                            if *position < period_samples / 8 {
                                output_sample = -1.0;
                            }
                        }

                        //25%
                        1 => {
                            if *position < period_samples / 4 {
                                output_sample = -1.0;
                            }
                        }

                        //50%
                        2 => {
                            if *position < period_samples / 2 {
                                output_sample = -1.0;
                            }
                        }

                        //75%
                        3 => {
                            if *position >= period_samples / 4 {
                                output_sample = -1.0;
                            }
                        }
                        _ => {

                        }
                    }

                    *position += 1;

                    if *position == period_samples {
                        *position = 0;
                    }
                }
                Err(error) => {
                    println!("Osc 1: Missed Buffer");
                }
            }

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
}

use std::sync::Arc;

use cpal::{traits::{HostTrait, DeviceTrait}, StreamConfig, OutputCallbackInfo, StreamError};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct AudioProcessingState {
    sample_rate: u32,
    osc_1: oscillators::SquareWaveGenerator,
    osc_2: oscillators::SquareWaveGenerator,
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

        let processor = Arc::new(AudioProcessingState{sample_rate: config.sample_rate().0, osc_1: oscillators::SquareWaveGenerator::new(config.sample_rate().0), osc_2: oscillators::SquareWaveGenerator::new(config.sample_rate().0)});

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

                }

                3 => {

                }
                _ => {
                    println!("Unrecognised oscillator number");
                }
            }

        }   
    }

    fn audio_block_f32(&self, audio: &mut [f32], _info: &OutputCallbackInfo) {
        //println!("audio");

        for sample in audio.iter_mut() {
            *sample = self.generate_sample();
        }
    }
    
    fn audio_block_i16(&self, audio: &mut [i16], _info: &OutputCallbackInfo) {
        for sample in audio.iter_mut() {
            let f32_sample = self.generate_sample();
            *sample = (f32_sample * i16::MAX as f32) as i16;
        }
    }
    
    fn audio_block_u16(&self, audio: &mut [u16], _info: &OutputCallbackInfo) {
        for sample in audio.iter_mut() {
            let f32_sample = self.generate_sample();
            *sample = ((f32_sample + 1.0) * i16::MAX as f32) as u16;
        }
    }
    
    fn audio_error(&self, error: StreamError) {
        println!("Audio Error");
    }

    fn generate_sample(&self) -> f32 {
        let mixed_sample = self.osc_1.generate_sample() + self.osc_2.generate_sample();

        //println!("{}", mixed_sample);

        return mixed_sample;
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