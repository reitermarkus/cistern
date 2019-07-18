#![feature(proc_macro_hygiene, decl_macro)]

use std::env;
use std::sync::{mpsc::channel, Arc, RwLock};
use std::thread;

use embedded_hal::adc::OneShot;
use nb::block;
use ads1x1x::{channel, Ads1x1x, FullScaleRange::Within6_144V};
use linux_embedded_hal::I2cdev;
use measurements::Length;
use medianheap::MedianHeap;
use serde_json::json;
use simple_signal::{self, Signal};
use rocket::{self, get, routes, State};
use rocket_contrib::json::{Json};

const MAX_WATER_LEVEL: u16 = 156 - 21; // This is the height of the drain pipe.
const MAX_VOLUME: u16 = 1600;

#[get("/")]
fn cistern(heap: State<Arc<RwLock<MedianHeap<i16>>>>) -> Option<Json<serde_json::value::Value>> {
  heap.read().unwrap().median().map(|value| {
    let adjusted_value = if value < 0 { 0.0 } else { f64::from(value) * 6.144 / f64::from(2i16.pow(15) - 1) };

    let height = Length::from_millimeters((adjusted_value * 1000.0).round());

    let cm = height.as_centimeters();
    let percentage = if cm > f64::from(MAX_WATER_LEVEL) { 1.0 } else { cm / f64::from(MAX_WATER_LEVEL) };
    let liters = f64::from(MAX_VOLUME) * percentage;

    Json(json!({
      "fill_height": cm,
      "percentage": percentage * 100.0,
      "volume": liters,
    }))
  })
}

fn main() {
  let dev = I2cdev::new(env::var("I2C_DEVICE").expect("I2C_DEVICE is not set")).expect("Failed to open I2C device");

  let mut adc = Ads1x1x::new_ads1115(dev, Default::default());
  adc.set_full_scale_range(Within6_144V).expect("Failed to set voltage range");

  let heap = Arc::new(RwLock::new(MedianHeap::with_max_size(10000)));
  let heap_clone = heap.clone();

  thread::spawn(move || {
    let (sig_tx, sig_rx) = channel();

    simple_signal::set_handler(&[Signal::Int], move |_| {
      sig_tx.send(true).unwrap();
    });

    let heap = heap_clone;

    loop {
      if sig_rx.try_recv().unwrap_or(false) {
        break
      }

      if let Ok(voltage) = block!(adc.read(&mut channel::SingleA0)) {
        let mut heap = heap.write().unwrap();
        heap.push(voltage);
      }
    }

    std::process::exit(1);
  });

  rocket::ignite()
    .manage(heap.clone())
    .mount("/", routes![cistern])
    .launch();
}
