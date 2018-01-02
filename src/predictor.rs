

pub trait Predictor {
    fn predict_next(&self) -> u64;
    fn update(&mut self, value: u64);
}

pub struct SimplePredictor {
    next_value:u64,
}

impl SimplePredictor {
    pub fn new() -> Self {
        SimplePredictor { next_value: 0 }
    }
}

impl Predictor for SimplePredictor {
    fn predict_next(&self) -> u64 {
        self.next_value
        }
    fn update(&mut self, value: u64) {
        self.next_value = value;
    }
}

pub struct FcmPredictor {
    table:Vec<u64>,
    last_hash:u64,
    mask:u64,
}

impl FcmPredictor {
    pub fn new(size: usize) -> Self {
        FcmPredictor {
            table: vec![0; size],
            last_hash: 0,
            mask:(size-1) as _,
        }
    }
}

impl Predictor for FcmPredictor {
    fn predict_next(&self) -> u64 {
        self.table[self.last_hash as usize]
    }
    fn update(&mut self, value: u64) {
        self.table[self.last_hash as usize] = value;
        self.last_hash = ((self.last_hash << 5) ^ (value >> 50)) & self.mask;
    }
}

pub struct DfcmPredictor {
    table: Vec<u64>,
    last_hash: u64,
    last_value: u64,
    mask:u64,
}

impl DfcmPredictor {
    pub fn new(size: usize) -> Self {
        DfcmPredictor {
            table: vec![0; size],
            last_hash: 0,
            last_value: 0,
            mask:(size-1) as _,
        }
    }
}

impl Predictor for DfcmPredictor {
    fn predict_next(&self) -> u64 {
        self.table[self.last_hash as usize] + self.last_value
    }
    fn update(&mut self, value: u64) {
        self.table[self.last_hash as usize] = value - self.last_value;
        self.last_hash = ((self.last_hash << 5) ^ ((value - self.last_value) >> 50)) & self.mask;
        self.last_value = value;
    }
}