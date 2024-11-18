use bstr::ByteSlice;
use esp32_nimble::{uuid128, BLEClient, BLEDevice, BLEScan};
use esp_idf_svc::hal::{
    prelude::Peripherals,
    task::block_on,
    timer::{TimerConfig, TimerDriver},
};

pub fn ble_stuff() -> Result<()> {
    let peripherals = Peripherals::take()?;
    let mut timer = TimerDriver::new(peripherals.timer00, &TimerConfig::new())?;
    block_on(async {
        let ble_device = BLEDevice::take();
        let mut ble_scan = BLEScan::new();
        let device = ble_scan
            .active_scan(true)
            .interval(100)
            .window(99)
            .start(ble_device, 10000, |device, data| {
                if let Some(name) = data.name() {
                    if name.contains_str("Polar H10") {
                        return Some(*device);
                    }
                }
                None
            })
            .await?;

        if let Some(device) = device {
            let mut client = BLEClient::new();
            client.on_connect(|client| {
                client.update_conn_params(120, 120, 0, 60).unwrap();
            });
            client.connect(&device.addr()).await?;

            let service = client
                .get_service(uuid128!("0000180f-0000-1000-8000-00805f9b34fb"))
                .await?;

            let uuid = uuid128!("00002a19-0000-1000-8000-00805f9b34fb");
            let characteristic = service.get_characteristic(uuid).await?;
            let value = characteristic.read_value().await?;
            ::log::info!("{} value: {:?}", characteristic, value);

            let service = client
                .get_service(uuid128!("0000180d-0000-1000-8000-00805f9b34fb"))
                .await?;

            let uuid = uuid128!("00002a37-0000-1000-8000-00805f9b34fb");
            let characteristic = service.get_characteristic(uuid).await?;

            if !characteristic.can_notify() {
                ::log::error!("characteristic can't notify: {}", characteristic);
                return anyhow::Ok(());
            }

            ::log::info!("subscribe to {}", characteristic);
            characteristic
                .on_notify(|data| {
                    ::log::info!("{:?}", data);
                })
                .subscribe_notify(false)
                .await?;

            timer.delay(timer.tick_hz() * 10).await?;

            client.disconnect()?;
        }

        anyhow::Ok(())
    })
}
