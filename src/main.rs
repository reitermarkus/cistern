#![feature(proc_macro_hygiene, decl_macro)]

use std::env;
use std::sync::{mpsc::channel, Arc, RwLock};
use std::thread;

use linux_embedded_hal::I2cdev;
use simple_signal::{self, Signal};
use rocket::{self, get, routes, State};
use rocket_contrib::json::JsonValue;
use rocket_contrib::json;

#[get("/")]
fn cistern(cistern: State<Arc<RwLock<Cistern<I2cdev>>>>) -> Option<JsonValue> {
  cistern.read().unwrap().level().map(|(height, percent, volume)| {
    json!({
      "fill_height": height,
      "percentage": percent * 100.0,
      "volume": volume,
    })
  })
}

use cistern::Cistern;

fn main() {
  let dev = I2cdev::new(env::var("I2C_DEVICE").expect("I2C_DEVICE is not set")).expect("Failed to open I2C device");

  let cistern = Arc::new(RwLock::new(Cistern::new(dev)));
  let cistern_clone = cistern.clone();

  thread::spawn(move || {
    let (sig_tx, sig_rx) = channel();

    simple_signal::set_handler(&[Signal::Int], move |_| {
      sig_tx.send(true).unwrap();
    });

    loop {
      if sig_rx.try_recv().unwrap_or(false) {
        break
      }

      if let Err(e) = cistern.write().unwrap().measure() {
        eprintln!("Failed to measure: {:?}", e);
      }
    }

    std::process::exit(1);
  });

  rocket::ignite()
    .manage(cistern_clone)
    .mount("/", routes![cistern])
    .launch();
}
