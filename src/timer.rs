use std::time;

pub struct AvgPerformanceTimer {
    avg_us : u128,
    instant: time::Instant,
    current_tick: u128,
    print_every: u128,
}

impl AvgPerformanceTimer {
    pub fn new(print_every: u128) -> Self {
        AvgPerformanceTimer {
            avg_us: 0,
            instant: time::Instant::now(),
            current_tick: 0,
            print_every: print_every
        }
    }

    pub fn begin(&mut self) {
        self.instant = time::Instant::now();
    }

    pub fn end(&mut self) {
        let elapsed_us = self.instant.elapsed().as_micros();
        if self.avg_us > 0 {
            self.avg_us = (self.avg_us + elapsed_us) / 2;
        } else {
            self.avg_us = elapsed_us;
        }

        if self.current_tick == self.print_every { // Every second
            println!("{} us", self.avg_us);
            self.current_tick = 0;
        } else {
            self.current_tick += 1;
        }
    }
}
