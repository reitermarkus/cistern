#![feature(proc_macro_hygiene, decl_macro)]

use std::env;
use std::sync::{mpsc::channel, Arc, RwLock};
use std::thread;
use std::sync::Weak;

use linux_embedded_hal::I2cdev;
use simple_signal::{self, Signal};
use serde_json::json;
use webthing::{Thing, ThingsType, Action, server::ActionGenerator, WebThingServer};

use cistern::Cistern;

struct Generator;

impl ActionGenerator for Generator {
  fn generate(
    &self,
    _thing: Weak<RwLock<Box<dyn Thing>>>,
    _name: String,
    _input: Option<&serde_json::Value>,
  ) -> Option<Box<dyn Action>> {
    None
  }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
  env_logger::init();

  let dev = I2cdev::new(env::var("I2C_DEVICE").expect("I2C_DEVICE is not set")).expect("Failed to open I2C device");

  let cistern = Cistern::new(dev);
  let thing: Arc<RwLock<Box<dyn Thing + 'static>>> = Arc::new(RwLock::new(Box::new(cistern.to_thing())));
  let thing_clone = thing.clone();

  let cistern = Arc::new(RwLock::new(cistern));

  thread::spawn(move || {
    let (sig_tx, sig_rx) = channel();

    simple_signal::set_handler(&[Signal::Int], move |_| {
      sig_tx.send(true).unwrap();
    });

    let mut i = 0;
    loop {
      if sig_rx.try_recv().unwrap_or(false) {
        break
      }

      let mut cistern = cistern.write().unwrap();
      match cistern.measure() {
        Ok(_) => {
          i += 1;

          // Don't update value after every measurement.
          if i >= 1000 {
            i = 0;

            let (height, percentage, volume) = cistern.level().unwrap();

            let mut thing = thing.write().unwrap();
            let mut change_value = |property_name: &str, value| {
              let property_name = property_name.to_owned();
              let prop = thing.find_property(&property_name).unwrap();
              let value = json!(value);
              prop.set_cached_value(value.clone()).unwrap();
              thing.property_notify(property_name, value);
            };
            change_value("height", height);
            change_value("percentage", percentage * 100.0);
            change_value("volume", volume);
          }
        },
        Err(e) => log::error!("Failed to measure: {:?}", e),
      }
    }

    std::process::exit(1);
  });

  let mut server = WebThingServer::new(
      ThingsType::Single(thing_clone),
      Some(8888),
      None,
      None,
      Box::new(Generator),
      None,
      None,
  );

  server.start(None).await
}
