mod oscillators {
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    pub struct SquareWaveGenerator {
        frequency: u32,
        sample_rate: u32,
        position: u32,
    }

    impl SquareWaveGenerator {
        pub const fn new(sample_rate: u32) -> SquareWaveGenerator
        {
            SquareWaveGenerator {frequency: 1000, sample_rate: sample_rate, position: 0}
        }

        pub fn generate_sample(&mut self) -> f32
        {
            let period = self.sample_rate / self.frequency;

            let mut output_sample = 1.0;

            let current_position = self.position;

            if self.position < period / 2{
                output_sample = 0.0
            }

            self.position += 1;

            if(self.position == period)
            {
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
pub struct AudioProcessingUnit {
    sample_rate: u32,

    //These are mutexes to satisfy 
    osc_1: Mutex<oscillators::SquareWaveGenerator>,
}

//type AudioProcessingUnitRef = Arc<RefCell<AudioProcessingUnit>>;

impl AudioProcessingUnit {
    pub fn new() -> Arc<AudioProcessingUnit> {
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

        let processor = Arc::new(AudioProcessingUnit{sample_rate: config.sample_rate().0, osc_1: Mutex::new(oscillators::SquareWaveGenerator::new(config.sample_rate().0))});

        let audio_callback_ref = processor.clone();
        let audio_error_ref = processor.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio: &mut [f32], info: &OutputCallbackInfo| audio_callback_ref.clone().audio_block_f32(audio, info), move |stream_error| audio_error_ref.clone().audio_error(stream_error)),
            cpal::SampleFormat::I16 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio: &mut [i16], info: &OutputCallbackInfo| audio_callback_ref.clone().audio_block_i16(audio, info), move |stream_error| audio_error_ref.clone().audio_error(stream_error)),                
            cpal::SampleFormat::U16 => out_dev.build_output_stream(&StreamConfig::from(config), move |audio: &mut [u16], info: &OutputCallbackInfo| audio_callback_ref.clone().audio_block_u16(audio, info), move |stream_error| audio_error_ref.clone().audio_error(stream_error))
        };

        match stream {
            Ok(_) => println!("Stream Created"),
            Err(_) => println!("Stream Failed")
        }

        processor
    }

    pub fn write_register(&self, address: usize, value: u8) {
        if address < 0xFF24 {
            let rel_address = address - 0xFF10;

            let osc = rel_address / 5;
            let reg = rel_address % 5;

            //println!("Osc: {}, Reg: {}", osc, reg);
        }   
    }

    fn audio_block_f32(&self, audio: &mut [f32], _info: &OutputCallbackInfo) {
        println!("Audio");
        for sample in audio.iter_mut() {
            *sample = self.generate_sample();
        }
    }
    
    fn audio_block_i16(&self, audio: &mut [i16], _info: &OutputCallbackInfo) {
        println!("Audio");
        for sample in audio.iter_mut() {
            let f32_sample = self.generate_sample();
            *sample = (f32_sample * i16::MAX as f32) as i16;
        }
    }
    
    fn audio_block_u16(&self, audio: &mut [u16], _info: &OutputCallbackInfo) {
        println!("Audio");
        for sample in audio.iter_mut() {
            let f32_sample = self.generate_sample();
            *sample = ((f32_sample + 1.0) * i16::MAX as f32) as u16;
        }
    }
    
    fn audio_error(&self, error: StreamError) {
        println!("Audio Error");
    }

    fn generate_sample(&self) -> f32 {
        self.osc_1.lock().unwrap().generate_sample()

        //self.osc_1.generate_sample()
    }
}