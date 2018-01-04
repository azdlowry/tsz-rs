// MIT License

// Copyright (c) 2016 Jerome Froelich

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! A crate for time series compression based upon Facebook's white paper
//! [Gorilla: A Fast, Scalable, In-Memory Time Series Database](http://www.vldb.org/pvldb/vol8/p1816-teller.pdf).
//! `tsz` provides functionality for compressing a stream of `DataPoint`s, which are composed of a
//! time and value, into bytes, and decompressing a stream of bytes into `DataPoint`s.
//!
//! ## Example
//!
//! Below is a simple example of how to interact with `tsz` to encode and decode `DataPoint`s.
//!
//! ```rust,no_run
//! extern crate tsz;
//!
//! use std::vec::Vec;
//! use tsz::{DataPoint, Encode, Decode, StdEncoder, StdDecoder, SimplePredictor};
//! use tsz::stream::{BufferedReader, BufferedWriter};
//! use tsz::decode::Error;
//!
//! const DATA: &'static str = "1482892270,1.76
//! 1482892280,7.78
//! 1482892288,7.95
//! 1482892292,5.53
//! 1482892310,4.41
//! 1482892323,5.30
//! 1482892334,5.30
//! 1482892341,2.92
//! 1482892350,0.73
//! 1482892360,-1.33
//! 1482892370,-1.78
//! 1482892390,-12.45
//! 1482892401,-34.76
//! 1482892490,78.9
//! 1482892500,335.67
//! 1482892800,12908.12
//! ";
//!
//! fn main() {
//!     let p = SimplePredictor::new();
//!     let w = BufferedWriter::new();
//!
//!     // 1482892260 is the Unix timestamp of the start of the stream
//!     let mut encoder = StdEncoder::new(1482892260, w, p);
//!
//!     let mut actual_datapoints = Vec::new();
//!
//!     for line in DATA.lines() {
//!         let substrings: Vec<&str> = line.split(",").collect();
//!         let t = substrings[0].parse::<u64>().unwrap();
//!         let v = substrings[1].parse::<f64>().unwrap();
//!         let dp = DataPoint::new(t, v);
//!         actual_datapoints.push(dp);
//!     }
//!
//!     for dp in &actual_datapoints {
//!         encoder.encode(*dp);
//!     }
//!
//!     let bytes = encoder.close();
//!     let r = BufferedReader::new(bytes);
//!     let mut decoder = StdDecoder::new(r);
//!
//!     let mut expected_datapoints = Vec::new();
//!
//!     let mut done = false;
//!     loop {
//!         if done {
//!             break;
//!         }
//!
//!         match decoder.next() {
//!             Ok(dp) => expected_datapoints.push(dp),
//!             Err(err) => {
//!                 if err == Error::EndOfStream {
//!                     done = true;
//!                 } else {
//!                     panic!("Received an error from decoder: {:?}", err);
//!                 }
//!             }
//!         };
//!     }
//!
//!     println!("actual datapoints: {:?}", actual_datapoints);
//!     println!("expected datapoints: {:?}", expected_datapoints);
//! }
//! ```

/// Bit
///
/// An enum used to represent a single bit, can be either `Zero` or `One`.
#[derive(Debug, PartialEq)]
pub enum Bit {
    Zero,
    One,
}

impl Bit {
    /// Convert a bit to u64, so `Zero` becomes 0 and `One` becomes 1.
    pub fn to_u64(self) -> u64 {
        match self {
            Bit::Zero => 0,
            Bit::One => 1,
        }
    }
}

/// DataPoint
///
/// Struct used to represent a single datapoint. Consists of a time and value.
#[derive(Debug, PartialEq, Copy)]
pub struct DataPoint {
    time: u64,
    value: i64,
}

impl Clone for DataPoint {
    fn clone(&self) -> DataPoint {
        *self
    }
}

impl DataPoint {
    // Create a new DataPoint from a time and value.
    pub fn new(time: u64, value: i64) -> Self {
        DataPoint {
            time: time,
            value: value,
        }
    }
}

pub mod stream;

pub mod predictor;
pub use self::predictor::Predictor;
pub use self::predictor::{SimplePredictor, FcmPredictor, DfcmPredictor};

pub mod encode;
pub use self::encode::Encode;
pub use self::encode::std_encoder::StdEncoder;

pub mod decode;
pub use self::decode::Decode;
pub use self::decode::std_decoder::StdDecoder;

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::{DataPoint, Encode, Decode, StdEncoder, StdDecoder, SimplePredictor};
    use super::stream::{BufferedReader, BufferedWriter};
    use super::decode::Error;

    const DATA: &'static str = "1482892270,176
1482892280,778
1482892288,795
1482892292,553
1482892310,441
1482892323,530
1482892334,530
1482892341,292
1482892350,073
1482892360,-133
1482892370,-178
1482892390,-1245
1482892401,-3476
1482892490,789
1482892500,33567
1482892800,1290812
";

    #[test]
    fn integration_test() {
        let w = BufferedWriter::new();
        let p = SimplePredictor::new();
        let mut encoder = StdEncoder::new(1482892260, w, p);

        let mut original_datapoints = Vec::new();

        for line in DATA.lines() {
            let substrings: Vec<&str> = line.split(",").collect();
            let t = substrings[0].parse::<u64>().unwrap();
            let v = substrings[1].parse::<i64>().unwrap();
            let dp = DataPoint::new(t, v);
            original_datapoints.push(dp);
        }

        for dp in &original_datapoints {
            encoder.encode(*dp);
        }

        let bytes = encoder.close();
        let r = BufferedReader::new(bytes);
        let p = SimplePredictor::new();
        let mut decoder = StdDecoder::new(r, p);

        let mut new_datapoints = Vec::new();

        let mut done = false;
        loop {
            if done {
                break;
            }

            match decoder.next() {
                Ok(dp) => new_datapoints.push(dp),
                Err(err) => {
                    if err == Error::EndOfStream {
                        done = true;
                    } else {
                        panic!("Received an error from decoder: {:?}", err);
                    }
                }
            };
        }

        assert_eq!(original_datapoints, new_datapoints);
    }
}