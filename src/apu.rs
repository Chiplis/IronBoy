mod oscillators {
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    pub struct SquareWaveGenerator {
        frequency: u16,
        sample_rate: u32,
        position: u32,
        duty: u8,
    }

    impl SquareWaveGenerator {
        pub fn new(sample_rate: u32) -> SquareWaveGenerator {
            SquareWaveGenerator {frequency: 1917, sample_rate: sample_rate, position: 0, duty: 2}
        }

        pub fn write_reg(&mut self, reg: usize, val: u8) {
            match reg {
                0 => {

                }

                //Duty
                1 => {
                    let new_duty = val >> 6;
                    self.set_duty(new_duty);
                }

                2 => {

                }

                //Frequency 8 least significant bits
                3 => {
                    let new_frequency = (val as u16 & 0xFF) | (self.frequency & 0xFF00);
                    self.set_frequency(new_frequency);
                }

                //Frequency 3 most significant bits
                4 => {
                    let msb = ((val as u16 & 0x7) << 8) | 0xFF;
                    let new_frequency = (msb & 0xFF00) | (self.frequency & 0xFF);

                    self.set_frequency(new_frequency);
                }

                _ => {
                    println!("Osc 1: Unrecognised register");
                }
            }
        }

        fn set_frequency(&mut self, new_frequency: u16) {
            if self.frequency != new_frequency {
                self.frequency = new_frequency;
                self.position = 0;
            }
        }

        fn set_duty(&mut self, new_duty: u8) {
            if(self.duty != new_duty) {
                self.duty = new_duty;
                self.position = 0;
            }
        }

        pub fn generate_sample(&mut self) -> f32 {
            let period = 1.0 / (131072.0 / (2048 - self.frequency as u32) as f32);
            let period_samples = (period * self.sample_rate as f32) as u32 * 2;

            let mut output_sample = 1.0;

            match self.duty {
                //12.5%
                0 => {
                    if self.position < period_samples / 8 {
                        output_sample = -1.0;
                    }
                }

                //25%
                1 => {
                    if self.position < period_samples / 4 {
                        output_sample = -1.0;
                    }
                }

                //50%
                2 => {
                    if self.position < period_samples / 2 {
                        output_sample = -1.0;
                    }
                }

                //75%
                3 => {
                    if self.position >= period_samples / 4 {
                        output_sample = -1.0;
                    }
                }
                _ => {

                }
            }

            self.position += 1;

            if self.position == period_samples {
                self.position = 0;
            }

            output_sample
        }
    }
}

use std::sync::{Arc, Mutex};

use cpal::{traits::{HostTrait, DeviceTrait}, StreamConfig, OutputCallbackInfo, StreamError};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct AudioProcessingState {
    sample_rate: u32,
    osc_1: Mutex<oscillators::SquareWaveGenerator>,
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

        let processor = Arc::new(AudioProcessingState{sample_rate: config.sample_rate().0, osc_1: Mutex::new(oscillators::SquareWaveGenerator::new(config.sample_rate().0))});

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
                    match self.osc_1.lock() {
                        Ok(mut osc) => {
                            osc.write_reg(reg, value);
                        }
                        Err(error) => {
                            println!("Unable to acquire oscillator lock");
                        }
                    }
                }

                1 => {

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
        let mut mixed_sample = 0.0;

        match self.osc_1.try_lock() {
            Ok(mut osc) => {
                mixed_sample = osc.generate_sample();
            }
            Err(_error) => {
                println!("Missed the sample");
            }
        }

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