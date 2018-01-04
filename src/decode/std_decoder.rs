use std::mem;

use {Bit, DataPoint};
use stream::Read;
use decode::{Decode, Error};
use encode::std_encoder::{END_MARKER, END_MARKER_LEN};
use predictor::Predictor;

/// StdDecoder
///
/// StdDecoder is used to decode `DataPoint`s
#[derive(Debug)]
pub struct StdDecoder<T: Read, P: Predictor> {
    time: u64, // current time
    delta: u64, // current time delta
    predictor: P,

    leading_zeros: u32, // leading zeros
    //trailing_zeros: u32, // trailing zeros

    first: bool, // will next DataPoint be the first DataPoint decoded
    done: bool,

    r: T,
}

impl<T, P> StdDecoder<T, P>
    where T: Read, P: Predictor
{
    /// new creates a new StdDecoder which will read bytes from r
    pub fn new(r: T, p: P) -> Self {
        StdDecoder {
            time: 0,
            delta: 0,
            predictor: p,
            leading_zeros: 0,
            //trailing_zeros: 0,
            first: true,
            done: false,
            r: r,
        }
    }

    fn read_initial_timestamp(&mut self) -> Result<u64, Error> {
        self.r
            .read_bits(64)
            .map_err(|_| Error::InvalidInitialTimestamp)
            .map(|time| {
                self.time = time;
                time
            })
    }

    fn read_first_timestamp(&mut self) -> Result<u64, Error> {
        self.read_initial_timestamp()?;

        // sanity check to confirm that the stream contains more than just the initial timestamp
        let control_bit = self.r.peak_bits(1)?;
        if control_bit == 1 {
            return self.r
                .read_bits(END_MARKER_LEN)
                .map_err(|err| Error::Stream(err))
                .and_then(|marker| if marker == END_MARKER {
                    Err(Error::EndOfStream)
                } else {
                    Err(Error::InvalidEndOfStream)
                });
        }

        // stream contains datapoints so we can throw away the control bit
        self.r.read_bit()?;

        self.r
            .read_bits(14)
            .map(|delta| {
                self.delta = delta;
                self.time += delta;
            })?;

        Ok(self.time)
    }

    fn read_next_timestamp(&mut self) -> Result<u64, Error> {
        let mut control_bits = 0;
        for _ in 0..4 {
            let bit = self.r.read_bit()?;

            if bit == Bit::One {
                control_bits += 1;
            } else {
                break;
            }
        }

        let size = match control_bits {
            0 => {
                self.time += self.delta;
                return Ok(self.time);
            }
            1 => 7,
            2 => 9,
            3 => 12,
            4 => {
                return self.r
                    .read_bits(32)
                    .map_err(|err| Error::Stream(err))
                    .and_then(|dod| if dod == 0 {
                        Err(Error::EndOfStream)
                    } else {
                        Ok(dod)
                    });
            }
            _ => unreachable!(),
        };

        let mut dod = self.r.read_bits(size)?;

        // need to sign extend negative numbers
        if dod > (1 << (size - 1)) {
            let mask = u64::max_value() << size;
            dod = dod | mask;
        }

        // by performing a wrapping_add we can ensure that negative numbers will be handled correctly
        self.delta = self.delta.wrapping_add(dod);
        self.time = self.time.wrapping_add(self.delta);

        Ok(self.time)
    }

    fn read_first_value(&mut self) -> Result<u64, Error> {
        self.r
            .read_bits(64)
            .map_err(|err| Error::Stream(err))
            .map(|bits| {
                self.predictor.update(bits);
                println!("<- frist = {}", bits);
                bits
            })
    }

    fn read_next_value(&mut self) -> Result<u64, Error> {
        let contol_bit = self.r.read_bit()?;
        let predicted_value = self.predictor.predict_next();

        if contol_bit == Bit::Zero {
            println!("<- Bit::Zero = {}", predicted_value);
            return Ok(predicted_value);
        }

        let zeros_bit = self.r.read_bit()?;

        if zeros_bit == Bit::One {
            self.leading_zeros = self.r.read_bits(6).map(|n| n as u32)?;
            //let significant_digits = self.r.read_bits(6).map(|n| (n + 1) as u32)?;
            println!("<- significant_digits changed = 64 - {} = {}", self.leading_zeros, 64 - self.leading_zeros);
            //self.trailing_zeros = 64 - self.leading_zeros - significant_digits;
        }

        let size = 64 - self.leading_zeros;// - self.trailing_zeros;
        self.r
            .read_bits(size)
            .map_err(|err| Error::Stream(err))
            .map(|bits| {
                let value_bits = predicted_value ^ (bits);// << self.trailing_zeros);
                println!("<- {} = {} ^ {}", value_bits, predicted_value, bits);
                self.predictor.update(value_bits);
                value_bits
            })
    }
}

impl<T, P> Decode for StdDecoder<T, P>
    where T: Read, P: Predictor
{
    fn next(&mut self) -> Result<DataPoint, Error> {
        if self.done {
            return Err(Error::EndOfStream);
        }

        let time;
        let value_bits;

        if self.first {
            self.first = false;
            time = self.read_first_timestamp()
                .map_err(|err| {
                    if err == Error::EndOfStream {
                        self.done = true;
                    }
                    err
                })?;;
            value_bits = self.read_first_value()?;
        } else {
            time = self.read_next_timestamp()
                .map_err(|err| {
                    if err == Error::EndOfStream {
                        self.done = true;
                    }
                    err
                })?;;
            value_bits = self.read_next_value()?;
        }

        let value = unsafe { mem::transmute::<u64, i64>(value_bits) };

        Ok(DataPoint::new(time, value))
    }
}

#[cfg(test)]
mod tests {
    use {DataPoint, Decode};
    use stream::BufferedReader;
    use decode::Error;
    use super::StdDecoder;
    use predictor::SimplePredictor;

    #[test]
    fn create_new_decoder() {
        let bytes = vec![0, 0, 0, 0, 88, 89, 157, 151, 240, 0, 0, 0, 0];
        let r = BufferedReader::new(bytes.into_boxed_slice());
        let p = SimplePredictor::new();
        let mut decoder = StdDecoder::new(r, p);

        assert_eq!(decoder.next().err().unwrap(), Error::EndOfStream);
    }

    #[test]
    fn decode_datapoint() {
        let bytes = vec![0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 127, 231, 174, 20, 122, 225, 71,
                         175, 224, 0, 0, 0, 0];
        let r = BufferedReader::new(bytes.into_boxed_slice());
        let p = SimplePredictor::new();
        let mut decoder = StdDecoder::new(r, p);

        let expected_datapoint = DataPoint::new(1482268055 + 10, 124);

        assert_eq!(decoder.next().unwrap(), expected_datapoint);
        assert_eq!(decoder.next().err().unwrap(), Error::EndOfStream);
    }

    #[test]
    fn decode_multiple_datapoints() {
        let bytes = vec![0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 127, 231, 174, 20, 122, 225, 71,
                         174, 204, 207, 30, 71, 145, 228, 121, 30, 96, 88, 61, 255, 253, 91, 214,
                         245, 189, 111, 91, 3, 232, 1, 245, 97, 88, 86, 21, 133, 55, 202, 1, 17,
                         15, 92, 40, 245, 194, 151, 128, 0, 0, 0, 0];
        let r = BufferedReader::new(bytes.into_boxed_slice());
        let p = SimplePredictor::new();
        let mut decoder = StdDecoder::new(r, p);

        let first_expected_datapoint = DataPoint::new(1482268055 + 10, 124);
        let second_expected_datapoint = DataPoint::new(1482268055 + 20, 198);
        let third_expected_datapoint = DataPoint::new(1482268055 + 32, 237);
        let fourth_expected_datapoint = DataPoint::new(1482268055 + 44, -741);
        let fifth_expected_datapoint = DataPoint::new(1482268055 + 52, 10350);

        assert_eq!(decoder.next().unwrap(), first_expected_datapoint);
        assert_eq!(decoder.next().unwrap(), second_expected_datapoint);
        assert_eq!(decoder.next().unwrap(), third_expected_datapoint);
        assert_eq!(decoder.next().unwrap(), fourth_expected_datapoint);
        assert_eq!(decoder.next().unwrap(), fifth_expected_datapoint);
        assert_eq!(decoder.next().err().unwrap(), Error::EndOfStream);
    }
}