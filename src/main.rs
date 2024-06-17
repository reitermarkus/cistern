use std::env;
use std::sync::{Arc, RwLock};

use anyhow::Context;
use linux_embedded_hal::I2cdev;
use serde_json::json;
use tokio::{sync::oneshot, task};
use webthing::{BaseActionGenerator, Thing, ThingsType, WebThingServer};

use cistern::Cistern;

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
  env_logger::init();

  println!("RUST_LOG={}", env::var("RUST_LOG").unwrap_or_default());

  let i2c_device = env::var("I2C_DEVICE").context("I2C_DEVICE is not set")?;
  let i2c = I2cdev::new(i2c_device).context("Failed to open I2C device")?;
  let cistern = Cistern::new(i2c);

  let thing: Arc<RwLock<Box<dyn Thing + 'static>>> = Arc::new(RwLock::new(Box::new(cistern.to_thing())));
  let thing_clone = thing.clone();

  let cistern = Arc::new(tokio::sync::RwLock::new(cistern));

  let (measurement_loop_stop_tx, mut measurement_loop_stop_rx) = oneshot::channel();

  let measurement_loop = async {
    task::spawn(async move {
      log::info!("Starting measurement loop…");

      let mut i = 0;
      loop {
        if measurement_loop_stop_rx.try_recv().unwrap_or(false) {
          log::info!("Stopping measurement loop.");
          break Ok(());
        }

        let mut cistern = cistern.write().await;
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
          Err(e) => break Err(e).context("Measurement failed"),
        }

        tokio::task::yield_now().await;
      }
    })
    .await
    .context("Failed to start measurement loop")
  };

  let webthing_server = async {
    let port = env::var("PORT").map(|p| p.parse::<u16>().expect("Failed to parse PORT")).unwrap_or(8888);

    let mut server = WebThingServer::new(
      ThingsType::Single(thing_clone),
      Some(port),
      None,
      None,
      Box::new(BaseActionGenerator),
      None,
      Some(true),
    );

    log::info!("Starting WebThing server on port {port}…");
    let res = server.start(None).await;

    measurement_loop_stop_tx.send(true).unwrap();

    res.context("WebThing server failed")
  };

  tokio::try_join!(measurement_loop, webthing_server)?.0?;

  Ok(())
}
