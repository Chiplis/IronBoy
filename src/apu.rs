mod oscillators {
    use serde::{Serialize, Deserialize};
    use std::{sync::{atomic::{AtomicU16, AtomicU8, Ordering, AtomicBool, AtomicU32}, RwLock, Mutex}};

    #[derive(Serialize, Deserialize)]
    struct VolumeEnvelopeParams {
        add_mode: bool,
        period: u8,
        current_level: u8,
        frequency_timer: u32,
    }

    #[derive(Serialize, Deserialize)]
    struct VolumeEnvelope {
        sample_rate: u32,
        params: Mutex<VolumeEnvelopeParams>,
        last_val: AtomicU8,
        current_settings: AtomicU8,
    }

    impl VolumeEnvelope {
        pub fn new(sample_rate: u32) -> VolumeEnvelope {
            VolumeEnvelope {sample_rate: sample_rate,
                            params: Mutex::new(VolumeEnvelopeParams{
                                add_mode: false, 
                                period: 0, 
                                current_level: 0,
                                frequency_timer: 0,}
                            ),
                            last_val: AtomicU8::new(0),
                            current_settings: AtomicU8::new(0),}
        }

        pub fn write_settings(&self, val: u8) {
            let starting_vol = val >> 4;
            let add_mode = ((val & 0x08) >> 3) > 0;
            let period = val & 0x07;

            //Get the lock for all items
            match self.params.lock() {
                Ok(mut params) => {
                    params.current_level = starting_vol;
                    params.add_mode = add_mode;
                    params.period = period;
                    params.frequency_timer = (self.sample_rate / 64) * ((period) as u32);
                }
                Err(_error) => {
                    println!("Could not obtain envelope data lock");
                }
            }

            self.current_settings.store(val, Ordering::Relaxed);
        }

        pub fn read_settings(&self) -> u8 {
            self.current_settings.load(Ordering::Relaxed)
        }

        pub fn generate_sample(&self) -> u8 {
            match self.params.lock() {
                Ok(mut params) => {
                    self.last_val.store(params.current_level, Ordering::Relaxed);
                    let output_sample = params.current_level as u8;

                    //Apply envelope
                    if params.period > 0 {
                        //Check if level change is needed
                        if params.frequency_timer <= 0 {

                            params.frequency_timer = (self.sample_rate / 64) * ((params.period) as u32);

                            if params.add_mode && params.current_level < 15 {
                                params.current_level +=  1;
                            }
                            else if !params.add_mode && params.current_level > 0 {
                                params.current_level -= 1;
                            }
                        }

                        params.frequency_timer -= 1
                    }

                    return output_sample;
                }

                Err(_error) => {
                    println!("missed vol env sample");
                    return self.last_val.load(Ordering::Relaxed) as u8;
                }
            }
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct SquareWaveGenerator {
        frequency: AtomicU16,
        frequency_timer: AtomicU32,
        timer_leftover: RwLock<f32>,
        sample_rate: u32,
        sweep: bool,
        position: AtomicU8,
        duty: AtomicU8,
        trigger: AtomicU8,
        enabled: AtomicBool,
        length: AtomicU8,
        length_counter: RwLock<u32>,
        length_enabled: AtomicBool,
        env: VolumeEnvelope,

        sweep_period: AtomicU8,
        sweep_timer: AtomicU32,
        sweep_negate: AtomicBool,
        sweep_shift: AtomicU8,
        sweep_enabled: AtomicBool,
        sweep_frequency: AtomicU16,
    }

    impl SquareWaveGenerator {
        pub fn new(sample_rate: u32, sweep: bool) -> SquareWaveGenerator {
            SquareWaveGenerator {frequency: AtomicU16::new(0), 
                                 frequency_timer: AtomicU32::new(0),
                                 timer_leftover: RwLock::new(0.0),
                                 sample_rate: sample_rate, 
                                 sweep: sweep,
                                 position: AtomicU8::new(0),
                                 duty: AtomicU8::new(2), 
                                 trigger: AtomicU8::new(0),
                                 enabled: AtomicBool::new(false),
                                 length: AtomicU8::new(0),
                                 length_counter: RwLock::new(0), 
                                 length_enabled: AtomicBool::new(false),
                                 env: VolumeEnvelope::new(sample_rate),

                                 sweep_period: AtomicU8::new(0),
                                 sweep_timer: AtomicU32::new(0),
                                 sweep_negate: AtomicBool::new(false),
                                 sweep_shift: AtomicU8::new(0),
                                 sweep_enabled: AtomicBool::new(false),
                                 sweep_frequency: AtomicU16::new(0),}
        }

        pub fn write_reg(&self, reg: usize, val: u8) {
            match reg {
                0 => {
                    if self.sweep {
                        let period = (val & 0x70) >> 4;
                        let negate = (val & 0x08) > 0;
                        let shift = val & 0x07;

                        self.sweep_period.store(period, Ordering::Relaxed);
                        self.sweep_negate.store(negate, Ordering::Relaxed);
                        self.sweep_shift.store(shift, Ordering::Relaxed);
                    }
                }

                //Duty and length
                1 => {
                    let new_duty = val >> 6;
                    self.duty.store(new_duty, Ordering::Relaxed);

                    let length = val & 0x3F;
                    self.length.store(length, Ordering::Relaxed);

                    let length_256hz = 64 - length;
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;

                    //Here we set the length counter making sure nothing can use it while it is set
                    match self.length_counter.write() {
                        Ok(mut length_counter) => {
                            *length_counter = length_samples;
                        }
                        Err(_error) => {
                            println!("Could not set square wave length");
                        }
                    }
                }

                //Volume envelope
                2 => {
                    self.env.write_settings(val);

                    //Disable channel if no DAC power
                    if val & 0xF0 == 0 {
                        self.enabled.store(false, Ordering::Relaxed);
                    }
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
                    self.trigger.store(trigger, Ordering::Relaxed);

                    if trigger > 0 {
                        //If length == 0 reset it to 64
                        match self.length_counter.write() {
                            Ok(mut length_counter) => {
                                if *length_counter == 0 {
                                    *length_counter = ((self.sample_rate as f32 / 256.0) * 64.0).ceil() as u32;
                                }
                            }
                            Err(_error) => {
                                println!("Could not set square wave length");
                            }
                        }

                        //Sweep data
                        if self.sweep {
                            //Copy frequency to shadow register
                            self.sweep_frequency.store(new_frequency, Ordering::Relaxed);

                            let sweep_period = self.sweep_period.load(Ordering::Relaxed);
                            let sweep_shift = self.sweep_shift.load(Ordering::Relaxed);

                            //Reload sweep timer
                            let sweep_num_samples = ((self.sample_rate as f32 / 128.0) * sweep_period as f32) as u32;
                            self.sweep_timer.store(sweep_num_samples, Ordering::Relaxed);

                            //Set sweep enabled flag
                            let sweep_enabled = sweep_period != 0 && sweep_shift != 0;
                            self.sweep_enabled.store(sweep_enabled, Ordering::Relaxed);

                            //Perform frequency and overflow calcs
                            if sweep_enabled {
                                let (overflow, new_sweep_freq) = self.calculate_sweep_freq();

                                if overflow {
                                    self.enabled.store(false, Ordering::Relaxed);
                                    return;
                                }

                                self.sweep_frequency.store(new_sweep_freq, Ordering::Relaxed);
                                self.frequency.store(new_sweep_freq, Ordering::Relaxed);

                                let (overflow_2, _) = self.calculate_sweep_freq();

                                if overflow_2 {
                                    self.enabled.store(false, Ordering::Relaxed);
                                    return;
                                }
                            }
                        }

                        //Reset frequency timer and timer leftover
                        let cycles_till_next = (2048 - self.frequency.load(Ordering::Relaxed) as u32) * 4;
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;
                        self.frequency_timer.store(samples_till_next.floor() as u32, Ordering::Relaxed);

                        //Store the remainder from the conversion from length in cycles to samples in timer leftover
                        match self.timer_leftover.write() {
                            Ok(mut timer_leftover) => {
                                *timer_leftover = samples_till_next - samples_till_next.floor();
                            }
                            Err(_) => {
                                println!("Square Wave: Could not write to timer leftover")
                            }
                        }

                        //Set enabled
                        self.enabled.store(true, Ordering::Relaxed);
                    }
                }

                _ => {
                    println!("Square Wave Osc: Unrecognised register");
                }
            }
        }

        pub fn is_enabled(&self) -> bool {
            self.enabled.load(Ordering::Relaxed)
        }

        pub fn read_reg(&self, reg: usize) -> u8 {
            match reg {
                0 => {
                    if self.sweep {
                        let mut reg_val = self.sweep_period.load(Ordering::Relaxed) << 4;
                        reg_val |= (self.sweep_negate.load(Ordering::Relaxed) as u8) << 3;
                        reg_val |= self.sweep_shift.load(Ordering::Relaxed);
                        return reg_val;
                    }
                    else {
                        return 0x00;
                    }
                }

                1 => {
                    let mut reg_value = self.duty.load(Ordering::Relaxed) << 6;
                    reg_value |= self.length.load(Ordering::Relaxed);
                    return reg_value;
                }

                2 => {
                    self.env.read_settings()
                }

                3 => {
                    return (self.frequency.load(Ordering::Relaxed) & 0x00FF) as u8;
                }

                4 => {
                    let mut reg_value = self.trigger.load(Ordering::Relaxed);
                    reg_value |= (self.length_enabled.load(Ordering::Relaxed) as u8) << 6;
                    reg_value |= ((self.frequency.load(Ordering::Relaxed) & 0x0F00) >> 8) as u8;
                    return reg_value;
                }

                _ => {
                    return 0x00;
                }
            }
        }

        pub fn generate_sample(&self) -> f32 {
            if !self.enabled.load(Ordering::Relaxed) {
                return 0.0;
            }

            if self.frequency_timer.load(Ordering::Relaxed) <= 0 {
                //Reset frequency timer
                let cycles_till_next = (2048 - self.frequency.load(Ordering::Relaxed) as u32) * 4;
                let mut samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;

                //If leftover plus current remainder is more than one we should make this period another sample long to make up for the lost time
                match self.timer_leftover.write() {
                    Ok(mut timer_leftover) => {
                        *timer_leftover += samples_till_next - samples_till_next.floor();

                        if *timer_leftover > 1.0 {
                            *timer_leftover -= 1.0;
                            samples_till_next += 1.0;
                        }
                    }
                    Err(_) => {
                        println!("Square Wave: Could not write to timer leftover");
                    }

                }

                self.frequency_timer.store(samples_till_next.floor() as u32, Ordering::Relaxed);

                let current_position = self.position.load(Ordering::Relaxed);

                let mut new_position = current_position + 1;
                if new_position >= 8 {
                    new_position = 0;
                }

                self.position.store(new_position, Ordering::Relaxed);
            }

            self.frequency_timer.store(self.frequency_timer.load(Ordering::Relaxed) - 1, Ordering::Relaxed);

            if self.sweep {
                if self.sweep_timer.load(Ordering::Relaxed) <= 0 && self.sweep_enabled.load(Ordering::Relaxed) && self.sweep_period.load(Ordering::Relaxed) > 0 {
                    //Reload sweep timer
                    let sweep_num_samples = ((self.sample_rate as f32 / 128.0) * self.sweep_period.load(Ordering::Relaxed) as f32) as u32;
                    self.sweep_timer.store(sweep_num_samples, Ordering::Relaxed);

                    let (overflow, new_sweep_freq) = self.calculate_sweep_freq();

                    if overflow {
                        self.enabled.store(false, Ordering::Relaxed);
                        return 0.0;
                    }

                    self.sweep_frequency.store(new_sweep_freq, Ordering::Relaxed);
                    self.frequency.store(new_sweep_freq, Ordering::Relaxed);

                    let (overflow_2, _) = self.calculate_sweep_freq();

                    if overflow_2 {
                        self.enabled.store(false, Ordering::Relaxed);
                        return 0.0;
                    }
                }

                self.sweep_timer.store(self.sweep_timer.load(Ordering::Relaxed) - 1, Ordering::Relaxed);
            }

            let mut wave_sample = 0;
            let envelope_sample = self.env.generate_sample();

            match self.duty.load(Ordering::Relaxed) {
                //12.5%
                0 => {
                    if self.position.load(Ordering::Relaxed) == 7 {
                        wave_sample = 1;
                    }
                }

                //25%
                1 => {
                    if self.position.load(Ordering::Relaxed) >= 6 {
                        wave_sample = 1;
                    }
                }

                //50%
                2 => {
                    if self.position.load(Ordering::Relaxed) >= 4 {
                        wave_sample = 1;
                    }
                }

                //75%
                3 => {
                    if self.position.load(Ordering::Relaxed) < 6 {
                        wave_sample = 1;
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

            let dac_input_sample = if wave_sample != 0 {
                envelope_sample
            }
            else {
                0
            };

            return dac_input_sample as f32 / 15.0;
        }

        fn calculate_sweep_freq(&self) -> (bool, u16) {
            let offset = (self.sweep_frequency.load(Ordering::Relaxed) >> self.sweep_shift.load(Ordering::Relaxed)) as u16;

            let new_freq : u32;

            if self.sweep_negate.load(Ordering::Relaxed) {
                //Check for underflow
                new_freq = match self.sweep_frequency.load(Ordering::Relaxed).checked_sub(offset) {
                    Some(res) => {
                        res.into()
                    }
                    None => {
                        0
                    }
                }
            }
            else {
                new_freq = self.sweep_frequency.load(Ordering::Relaxed) as u32 + offset as u32;
            }

            //Overflow check
            if new_freq > 2047 {
                return (true, new_freq as u16);
            }

            return (false, new_freq as u16);
        }
    }


    #[derive(Serialize, Deserialize)]
    pub struct WaveTable {
        sample_rate: u32,
        sound_data: [AtomicU8; 32],
        frequency: AtomicU16,
        frequency_timer: AtomicU32,
        timer_leftover: RwLock<f32>,
        position: AtomicU8,
        trigger: AtomicU8,
        enabled: AtomicBool,
        length: AtomicU8,
        length_counter: RwLock<u32>,
        length_enabled: AtomicBool,
        volume_code: AtomicU8,
    }

    impl WaveTable {
        pub fn new(sample_rate: u32) -> WaveTable {

            const GENERATOR: AtomicU8 = AtomicU8::new(0);

            WaveTable { sample_rate: sample_rate,
                        sound_data: [GENERATOR; 32], 
                        frequency: AtomicU16::new(0),
                        frequency_timer: AtomicU32::new(0),
                        timer_leftover: RwLock::new(0.0),
                        position: AtomicU8::new(0),
                        trigger: AtomicU8::new(0),
                        enabled: AtomicBool::new(false),
                        length: AtomicU8::new(0),
                        length_counter: RwLock::new(0),
                        length_enabled: AtomicBool::new(false),
                        volume_code: AtomicU8::new(0),}
        }

        pub fn write_reg(&self, reg: usize, val: u8) {
            match reg {
                0 => {
                    if val == 0x00 {
                        self.enabled.store(false, Ordering::Relaxed);
                    }
                }
                1 => {
                    self.length.store(val, Ordering::Relaxed);
                    let length_256hz = 256 - val as u32;
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;

                    //Here we set the length counter making sure nothing can use it while it is set
                    match self.length_counter.write() {
                        Ok(mut length_counter) => {
                            *length_counter = length_samples;
                        }
                        Err(_error) => {
                            println!("Could not set wave table length");
                        }
                    }
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

                    self.length_enabled.store((val & 0x40) > 0, Ordering::Relaxed);

                    let trigger = val & 0x80;
                    self.trigger.store(trigger, Ordering::Relaxed);

                    if trigger > 0 {
                        //If length == 0 reset it to 256
                        match self.length_counter.write() {
                            Ok(mut length_counter) => {
                                if *length_counter == 0 {
                                    *length_counter = ((self.sample_rate as f32 / 256.0) * 256.0).ceil() as u32;
                                }
                            }
                            Err(_error) => {
                                println!("Could not set square wave length");
                            }
                        }

                        //Reset frequency timer
                        let cycles_till_next = (2048 - self.frequency.load(Ordering::Relaxed) as u32) * 2;
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;
                        self.frequency_timer.store(samples_till_next as u32, Ordering::Relaxed);

                        //See square wave for an explanation on timer leftover
                        match self.timer_leftover.write() {
                            Ok(mut timer_leftover) => {
                                *timer_leftover = samples_till_next - samples_till_next.floor();
                            }
                            Err(_) => {
                                println!("Wave table: Could not write to timer leftover")
                            }
                        }

                        self.position.store(0, Ordering::Relaxed);

                        self.enabled.store(true, Ordering::Relaxed);
                    }
                }

                _ => {

                }
            }
        }

        pub fn is_enabled(&self) -> bool {
            self.enabled.load(Ordering::Relaxed)
        }

        pub fn read_reg(&self, reg: usize) -> u8 {
            match reg {
                0 => {
                    0x00
                }

                1 => {
                    return self.length.load(Ordering::Relaxed);
                }

                2 => {
                    return self.volume_code.load(Ordering::Relaxed) << 6;
                }

                3 => {
                    return (self.frequency.load(Ordering::Relaxed) & 0x00FF) as u8;
                }

                4 => {
                    let mut reg_value = self.trigger.load(Ordering::Relaxed);
                    reg_value |= (self.length_enabled.load(Ordering::Relaxed) as u8) << 6;
                    reg_value |= ((self.frequency.load(Ordering::Relaxed) & 0x0F00) >> 8) as u8;
                    return reg_value;
                }

                _ => {
                    return 0x00;
                }
            }
        }

        pub fn write_sound_data(&self, address: usize, val: u8) {
            let rel_address = address - 0xFF30;
            let start_sample = rel_address * 2;

            self.sound_data[start_sample].store(val >> 4, Ordering::Relaxed);
            self.sound_data[start_sample + 1].store(val & 0x0F, Ordering::Relaxed);
        }

        pub fn read_sound_data(&self, address: usize) -> u8 {
            let rel_address = address - 0xFF30;
            let start_sample = rel_address * 2;

            let mut reg_val = 0x00;
            reg_val |= self.sound_data[start_sample].load(Ordering::Relaxed) << 4;
            reg_val |= self.sound_data[start_sample + 1].load(Ordering::Relaxed);
            return reg_val;
        }

        pub fn generate_sample(&self) -> f32 {
            if !self.enabled.load(Ordering::Relaxed) {
                return 0.0;
            }

            let mut current_position = self.position.load(Ordering::Relaxed);

            if self.frequency_timer.load(Ordering::Relaxed) <= 0 {

                //Reset frequency timer
                let cycles_till_next = (2048 - self.frequency.load(Ordering::Relaxed) as u32) * 2;
                let mut samples_till_next = (self.sample_rate as f32 / 4194304.0) * cycles_till_next as f32;

                //See square wave for explanation on timer leftover
                match self.timer_leftover.write() {
                    Ok(mut timer_leftover) => {
                        *timer_leftover += samples_till_next - samples_till_next.floor();

                        if *timer_leftover > 1.0 {
                            *timer_leftover -= 1.0;
                            samples_till_next += 1.0;
                        }
                    }
                    Err(_) => {
                        println!("Wave table: Could not write to timer leftover");
                    }
                }

                self.frequency_timer.store(samples_till_next as u32, Ordering::Relaxed);
                
                //Move one position forward
                let new_position = if current_position == 31 {
                    0
                }
                else {
                    current_position + 1
                };

                self.position.store(new_position, Ordering::Relaxed);
                current_position = new_position;
            }

            self.frequency_timer.store(self.frequency_timer.load(Ordering::Relaxed) - 1, Ordering::Relaxed);

            let mut wave_sample = self.sound_data[current_position as usize].load(Ordering::Relaxed);

            let volume_shift = match self.volume_code.load(Ordering::Relaxed) {
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
                    println!("Wave table: unexpected volume code");
                    4
                }
            };

            wave_sample >>= volume_shift;

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
            
            wave_sample as f32 / 15.0
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct NoiseGenerator {
        sample_rate: u32,
        env: VolumeEnvelope,
        divisor_code: AtomicU8,
        divisor: AtomicU8,
        clock_shift: AtomicU8,
        frequency_timer: AtomicU32,
        timer_leftover: RwLock<f32>,
        sample_counter: AtomicU32,
        lfsr: Mutex<[bool; 15]>,
        width: AtomicBool,
        trigger: AtomicU8,
        enabled: AtomicBool,
        length: AtomicU8,
        length_counter: RwLock<u32>,
        length_enabled: AtomicBool,
    }

    impl NoiseGenerator {
        pub fn new(sample_rate: u32) -> NoiseGenerator {
            NoiseGenerator { sample_rate: sample_rate,
                             env: VolumeEnvelope::new(sample_rate),
                             divisor_code: AtomicU8::new(0),
                             divisor: AtomicU8::new(0),
                             clock_shift: AtomicU8::new(0),
                             frequency_timer: AtomicU32::new(0),
                             timer_leftover: RwLock::new(0.0),
                             sample_counter: AtomicU32::new(0),
                             lfsr: Mutex::new([true; 15]),
                             width: AtomicBool::new(false),
                             trigger: AtomicU8::new(0),
                             enabled: AtomicBool::new(false), 
                             length: AtomicU8::new(0),
                             length_counter: RwLock::new(0),
                             length_enabled: AtomicBool::new(false),}
        }

        pub fn write_reg(&self, reg: usize, val: u8) {
            match reg {
                0 => {
                    
                }

                1 => {
                    let length = val & 0x3F;
                    let length_256hz = 64 - length;
                    let length_samples = ((self.sample_rate as f32 / 256.0) * length_256hz as f32).ceil() as u32;
                    self.length.store(length, Ordering::Relaxed);

                    //Here we set the length counter making sure nothing can use it while it is set
                    match self.length_counter.write() {
                        Ok(mut length_counter) => {
                            *length_counter = length_samples;
                        }
                        Err(_error) => {
                            println!("Could not set noise generator length");
                        }
                    }
                }

                2 => {
                    self.env.write_settings(val);

                    //Disable channel if no DAC power
                    if val & 0xF0 == 0 {
                        self.enabled.store(false, Ordering::Relaxed);
                    }
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

                    self.divisor_code.store(divisor_code, Ordering::Relaxed);
                    self.divisor.store(divisor, Ordering::Relaxed);
                    self.clock_shift.store(clock_shift, Ordering::Relaxed);

                    self.width.store(width != 0, Ordering::Relaxed);
                }

                4 => {
                    self.length_enabled.store((val & 0x40) > 0, Ordering::Relaxed);

                    let trigger = val & 0x80;
                    self.trigger.store(trigger, Ordering::Relaxed);

                    if trigger > 0 {
                        //If length == 0 reset it to 64
                        match self.length_counter.write() {
                            Ok(mut length_counter) => {
                                if *length_counter == 0 {
                                    *length_counter = ((self.sample_rate as f32 / 256.0) * 64.0).ceil() as u32;
                                }
                            }
                            Err(_error) => {
                                println!("Could not set square wave length");
                            }
                        }

                        //Fill LFSR with 1s
                        match self.lfsr.lock() {
                            Ok(mut lfsr) => {
                                for bit in lfsr.iter_mut() {
                                    *bit = true;
                                }
                            }
                            Err(_error) => {
                                println!("Could not obtain LFSR Mutex");
                            }
                        }

                        //Set frequency timer
                        let frequency = (self.divisor.load(Ordering::Relaxed) as u32) << (self.clock_shift.load(Ordering::Relaxed) as u32);
                        let samples_till_next = (self.sample_rate as f32 / 4194304.0) * frequency as f32;
                        self.frequency_timer.store(samples_till_next as u32, Ordering::Relaxed);

                        //See square wave for an explanation on timer leftover
                        match self.timer_leftover.write() {
                            Ok(mut timer_leftover) => {
                                *timer_leftover = samples_till_next - samples_till_next.floor();
                            }
                            Err(_) => {
                                println!("Noise osc: Could not write to timer leftover")
                            }
                        }

                        self.enabled.store(true, Ordering::Relaxed);
                    }
                }

                _ => {
                    println!("Noise Osc: Unrecognised register");
                }
            }
        }

        pub fn is_enabled(&self) -> bool {
            self.enabled.load(Ordering::Relaxed)
        }

        pub fn read_reg(&self, reg: usize) -> u8 {
            match reg {
                0 => {
                    return 0x00;
                }

                1 => {
                    return self.length.load(Ordering::Relaxed);
                }

                2 => {
                    return self.env.read_settings();
                }

                3 => {
                    let mut reg_val = 0x00;
                    reg_val |= self.clock_shift.load(Ordering::Relaxed) << 4;
                    reg_val |= (self.width.load(Ordering::Relaxed) as u8) << 3;
                    reg_val |= self.divisor_code.load(Ordering::Relaxed);
                    return reg_val;
                }

                4 => {
                    let mut reg_value = self.trigger.load(Ordering::Relaxed);
                    reg_value |= (self.length_enabled.load(Ordering::Relaxed) as u8) << 6;
                    return reg_value;
                }

                _ => {
                    return 0x00;
                }
            }
        }

        pub fn generate_sample(&self) -> f32 {
            if !self.enabled.load(Ordering::Relaxed) {
                return 0.0;
            }

            let env_sample = self.env.generate_sample();
            let mut noise_sample = 0;

            match self.lfsr.lock() {
                Ok(mut lfsr) => {
                    if self.frequency_timer.load(Ordering::Relaxed) <= 0 {
                        //Reset frequency timer
                        let frequency = (self.divisor.load(Ordering::Relaxed) as u32) << (self.clock_shift.load(Ordering::Relaxed) as u32);
                        let mut samples_till_next = (self.sample_rate as f32 / 4194304.0) * frequency as f32;

                        //See square wave for explanation on timer leftover
                        match self.timer_leftover.write() {
                            Ok(mut timer_leftover) => {
                                *timer_leftover += samples_till_next - samples_till_next.floor();
        
                                if *timer_leftover > 1.0 {
                                    *timer_leftover -= 1.0;
                                    samples_till_next += 1.0;
                                }
                            }
                            Err(_) => {
                                println!("Square Wave: Could not write to timer leftover");
                            }
                        }

                        self.frequency_timer.store(samples_till_next.ceil() as u32, Ordering::Relaxed);

                        //Move LFSR on
                        let new_val = lfsr[0] != lfsr[1];
                        lfsr.rotate_left(1);

                        lfsr[14] = new_val;

                        if self.width.load(Ordering::Relaxed) {
                            lfsr[6] = new_val;
                        }
                    }

                    self.frequency_timer.store(self.frequency_timer.load(Ordering::Relaxed) - 1, Ordering::Relaxed);

                    noise_sample = if lfsr[0] == true {
                        1
                    }
                    else {
                        0
                    };
                }
                //This should never happen
                Err(_error) => {
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
            
            let dac_input_sample = if noise_sample != 0 {
                env_sample
            }
            else {
                0
            };

            return dac_input_sample as f32 / 15.0;
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

    power_control: AtomicBool,
}

impl AudioProcessingState {
    pub fn new() -> (Arc<AudioProcessingState>, cpal::Stream) {
        //Setup audio interfacing
        let out_dev = cpal::default_host().default_output_device().expect("No available output device found");

        let mut supported_configs_range = out_dev.supported_output_configs().expect("Could not obtain device configs");

        let config = supported_configs_range.find(|c| {
            c.max_sample_rate() >= cpal::SampleRate(44100)
        }).expect("Audio device does not support sample rate (44100)").with_sample_rate(cpal::SampleRate(44100));

        //Display device name
        match out_dev.name() {
            Ok(name) => {
                println!("Using {} at {}Hz with {} channels", name, config.sample_rate().0, config.channels());
            }
            Err(_) => {}
        }

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
                                                                                       power_control: AtomicBool::new(false),
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
                    println!("APU Write: Unrecognised oscillator number");
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
                    self.power_control.store((value >> 7) > 0, Ordering::Relaxed);
                }

                _ => {
                    println!("APU Write: Unrecognised address: {}", address);
                }
            }
        } 
    }

    pub fn read_register(&self, address: usize) -> u8 {
        if address < 0xFF24 {
            let rel_address = address - 0xFF10;

            let osc = rel_address / 5;
            let reg = rel_address % 5;

            match osc {
                0 => {
                    return self.osc_1.read_reg(reg);
                }

                1 => {
                    return self.osc_2.read_reg(reg);
                }

                2 => {
                    return self.osc_3.read_reg(reg);
                }

                3 => {
                    return self.osc_4.read_reg(reg);
                }
                _ => {
                    println!("APU Read: Unrecognised oscillator number");
                    return 0x00;
                }
            }
        }
        else if address >= 0xFF30 && address <= 0xFF3F {
            return self.osc_3.read_sound_data(address);
        }  
        else {
            match address {
                0xFF24 => {
                    let mut reg_val = 0x00;
                    reg_val |= self.left_master_vol.load(Ordering::Relaxed) << 4;
                    reg_val |= self.right_master_vol.load(Ordering::Relaxed);
                    return reg_val;
                }
                0xFF25 => {
                    let mut reg_val = 0x00;
                    reg_val |= (self.left_osc_4_enable.load(Ordering::Relaxed) as u8) << 7;
                    reg_val |= (self.left_osc_3_enable.load(Ordering::Relaxed) as u8) << 6;
                    reg_val |= (self.left_osc_2_enable.load(Ordering::Relaxed) as u8) << 5;
                    reg_val |= (self.left_osc_1_enable.load(Ordering::Relaxed) as u8) << 4;

                    reg_val |= (self.right_osc_4_enable.load(Ordering::Relaxed) as u8) << 3;
                    reg_val |= (self.right_osc_3_enable.load(Ordering::Relaxed) as u8) << 2;
                    reg_val |= (self.right_osc_2_enable.load(Ordering::Relaxed) as u8) << 1;
                    reg_val |= self.right_osc_1_enable.load(Ordering::Relaxed) as u8;

                    return reg_val;
                }
                0xFF26 => {
                    let mut reg_val = (self.power_control.load(Ordering::Relaxed) as u8) << 7;
                    reg_val |= (self.osc_4.is_enabled() as u8) << 3;
                    reg_val |= (self.osc_3.is_enabled() as u8) << 2;
                    reg_val |= (self.osc_2.is_enabled() as u8) << 1;
                    reg_val |= self.osc_1.is_enabled() as u8;
                    return reg_val;
                }
                _ => {
                    println!("APU Read: Unrecognised address");
                    return 0x00;
                }
            }
        }
    }

    fn audio_block_f32(&self, audio: &mut [f32], _info: &OutputCallbackInfo) {
        let num_samples = audio.len() / self.num_channels as usize;

        for sample_index in 0..num_samples {
            let generated_samples = self.generate_samples();

            let first_channel_index = sample_index * self.num_channels as usize;

            audio[first_channel_index] = generated_samples.0;

            if self.num_channels > 1 {
                audio[first_channel_index + 1] = generated_samples.1;
            }
        }
    }
    
    fn audio_block_i16(&self, audio: &mut [i16], _info: &OutputCallbackInfo) {
        let num_samples = audio.len() / self.num_channels as usize;

        for sample_index in 0..num_samples {
            let f32_samples = self.generate_samples();

            let first_channel_index = sample_index * self.num_channels as usize;

            audio[first_channel_index] = (f32_samples.0 * i16::MAX as f32) as i16;

            if self.num_channels > 1 {
                audio[first_channel_index + 1] = (f32_samples.1 * i16::MAX as f32) as i16;
            }
        }
    }
    
    fn audio_block_u16(&self, audio: &mut [u16], _info: &OutputCallbackInfo) {
        let num_samples = audio.len() / self.num_channels as usize;

        for sample_index in 0..num_samples {
            let f32_samples = self.generate_samples();

            let first_channel_index = sample_index * self.num_channels as usize;

            audio[first_channel_index] = ((f32_samples.0 + 1.0) * u16::MAX as f32) as u16;

            if self.num_channels > 1 {
                audio[first_channel_index + 1] = ((f32_samples.1 + 1.0) * u16::MAX as f32) as u16;
            }
        }
    }
    
    fn audio_error(&self, _error: StreamError) {
        println!("Audio Error");
    }

    fn generate_samples(&self) -> (f32, f32) {

        if !self.power_control.load(Ordering::Relaxed) {
            return (0.0, 0.0);
        }

        let mut mixed_left_sample = 0.0;
        let mut mixed_right_sample = 0.0;

        let osc_1_sample = self.osc_1.generate_sample();
        if self.left_osc_1_enable.load(Ordering::Relaxed) {
            mixed_left_sample += osc_1_sample;
        }
        if self.right_osc_1_enable.load(Ordering::Relaxed) {
            mixed_right_sample += osc_1_sample;
        }

        let osc_2_sample = self.osc_2.generate_sample();
        if self.left_osc_2_enable.load(Ordering::Relaxed) {
            mixed_left_sample += osc_2_sample;
        }
        if self.right_osc_2_enable.load(Ordering::Relaxed) {
            mixed_right_sample += osc_2_sample;
        }

        let osc_3_sample = self.osc_3.generate_sample();
        if self.left_osc_3_enable.load(Ordering::Relaxed) {
            mixed_left_sample += osc_3_sample;
        }
        if self.right_osc_3_enable.load(Ordering::Relaxed) {
            mixed_right_sample += osc_3_sample;
        }

        let osc_4_sample = self.osc_4.generate_sample();
        if self.left_osc_4_enable.load(Ordering::Relaxed) {
            mixed_left_sample += osc_4_sample;
        }
        if self.right_osc_4_enable.load(Ordering::Relaxed) {
            mixed_right_sample += osc_4_sample;
        }

        mixed_left_sample /= 4.0;
        mixed_right_sample /= 4.0;

        //These volumes seem to be really loud on their own so I have scaled them down
        //mixed_left_sample *= (self.left_master_vol.load(Ordering::Relaxed) + 1) as f32 / 8.0;
        //mixed_right_sample *= (self.right_master_vol.load(Ordering::Relaxed) + 1) as f32 / 8.0;

        return (mixed_left_sample, mixed_right_sample);
    }
}

#[allow(dead_code)]
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

    pub fn read_register(&self, address: usize) -> u8 {
        self.state.read_register(address)
    }
}