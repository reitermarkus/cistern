use ads1x1x::{FullScaleRange::Within6_144V, ChannelSelection};
use ads1x1x::{Ads1115};
use measurements::Length;
use medianheap::{Median, MedianHeap};
use nb::block;
use embedded_hal::i2c::I2c;
use webthing::{BaseThing, Thing, BaseProperty};
use serde_json::json;

pub struct Cistern<I> {
  heap: MedianHeap<i16>,
  adc: Ads1115<I, ads1x1x::mode::OneShot>,
}

impl<I, E> Cistern<I>
where
  I: I2c<Error = E>,
  E: std::fmt::Debug
{
  const MAX_WATER_LEVEL: u16 = 156 - 21; // This is the height of the drain pipe.
  const MAX_VOLUME: u16 = 1600;

  pub fn new(i2c: I) -> Self {
    let mut adc = Ads1115::new(i2c, Default::default());
    adc.set_full_scale_range(Within6_144V).expect("Failed to set voltage range");

    Self { heap: MedianHeap::with_max_size(10000), adc }
  }

  pub fn measure(&mut self) -> Result<Median<&i16>, E> {
    let value = block!(self.adc.read(ChannelSelection::SingleA0))?;
    self.heap.push(value);

    Ok(self.heap.median().unwrap())
  }

  pub fn level(&self) -> Option<(f64, f64, f64)> {
    self.heap.median_with(|v1, v2| (v1 + v2) / 2).map(|value| {
      let adjusted_value = if value < 0 {
        0
      } else {
        i32::from(value) * 6_144 / (2i32.pow(15) - 1)
      };

      // Measurement range is 0-5 V for 5 m, so 1 mV is exactly 1 mm.
      let height = Length::from_millimeters(adjusted_value as f64);

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

  pub fn to_thing(&self) -> BaseThing {
    let mut thing = BaseThing::new(
      "urn:dev:ops:cistern-level-sensor-1234".to_owned(),
      "Cistern Level Sensor".to_owned(),
      Some(vec!["MultiLevelSensor".to_owned()]),
      Some("A web connected cistern level sensor".to_owned()),
    );

    let level = BaseProperty::new(
      "percentage".to_owned(),
      json!(null),
      None,
      Some(
        json!({
          "@type": "LevelProperty",
          "title": "Level",
          "type": "number",
          "description": "The current fill level in percent",
          "minimum": 0,
          "maximum": 100,
          "unit": "percent",
          "readOnly": true
        })
        .as_object()
        .unwrap()
        .clone()
      ),
    );
    let height = BaseProperty::new(
      "height".to_owned(),
      json!(null),
      None,
      Some(
        json!({
          "@type": "LevelProperty",
          "title": "Height",
          "type": "number",
          "description": "The current fill height in centimeters",
          "minimum": 0,
          "maximum": Self::MAX_WATER_LEVEL,
          "unit": "centimeter",
          "readOnly": true
        })
        .as_object()
        .unwrap()
        .clone()),
    );
    let volume = BaseProperty::new(
      "volume".to_owned(),
      json!(null),
      None,
      Some(
        json!({
          "@type": "LevelProperty",
          "title": "Volume",
          "type": "number",
          "description": "The current fill volume in liters",
          "minimum": 0,
          "maximum": Self::MAX_VOLUME,
          "unit": "liter",
          "readOnly": true
        })
        .as_object()
        .unwrap()
        .clone()
      ),
    );

    thing.add_property(Box::new(level));
    thing.add_property(Box::new(height));
    thing.add_property(Box::new(volume));

    thing
  }
}
