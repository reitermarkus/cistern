use ads1x1x::{channel, Ads1x1x, FullScaleRange::Within6_144V};
use ads1x1x::interface::I2cInterface;
use ads1x1x::ic::{Ads1115, Resolution16Bit};
use measurements::Length;
use medianheap::MedianHeap;
use nb::block;
use embedded_hal::adc::OneShot;
use embedded_hal::blocking::i2c::WriteRead;
use embedded_hal::blocking::i2c::Write;

pub struct Cistern<I> {
  heap: MedianHeap<i16>,
  adc: Ads1x1x<I2cInterface<I>, Ads1115, Resolution16Bit, ads1x1x::mode::OneShot>,
}

impl<I, E> Cistern<I>
  where
    I: Write<Error = E> + WriteRead<Error = E>,
    E: std::fmt::Debug {
  const MAX_WATER_LEVEL: u16 = 156 - 21; // This is the height of the drain pipe.
  const MAX_VOLUME: u16 = 1600;

  pub fn new(dev: I) -> Self {
    let mut adc = Ads1x1x::new_ads1115(dev, Default::default());
    adc.set_full_scale_range(Within6_144V).expect("Failed to set voltage range");

    Self {
      heap: MedianHeap::with_max_size(10000),
      adc,
    }
  }

  pub fn measure(&mut self) -> Result<(), ads1x1x::Error<E>> {
    let value = block!(self.adc.read(&mut channel::SingleA0))?;
    self.heap.push(value);
    Ok(())
  }

  pub fn level(&self) -> Option<(f64, f64, f64)> {
    self.heap.median().map(|value| {
      let adjusted_value = if value < 0 {
        0.0
      } else {
        f64::from(value) * 6.144 / f64::from(2i16.pow(15) - 1)
      };

      let height = Length::from_millimeters((adjusted_value * 1000.0).round());

      let height = height.as_centimeters();
      let percent = if height > f64::from(Self::MAX_WATER_LEVEL) {
        1.0
      } else {
        height / f64::from(Self::MAX_WATER_LEVEL)
      };
      let volume = f64::from(Self::MAX_VOLUME) * percent;

      (height, percent, volume)
    })
  }
}
