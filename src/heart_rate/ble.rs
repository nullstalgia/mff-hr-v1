use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    time::Duration,
};

use crate::errors::{AppError, Result};
use bstr::ByteSlice;
use log::info;
use serde_derive::{Deserialize, Serialize};
use takeable::Takeable;

use super::measurement::parse_hrm;

// Lots of logic yoinked from https://github.com/nullstalgia/iron-heart
#[derive(Debug, Default, Clone)]
pub struct MonitorStatus {
    pub heart_rate_bpm: u16,
    pub latest_rr: std::time::Duration,
    pub rr_intervals: Vec<std::time::Duration>,

    pub twitch_up: bool,
    pub twitch_down: bool,
    use_real_rr: bool,
}

// pub struct BleStuff<'a> {
//     host_device: &'a mut BLEDevice,
//     pub discovered: Monitors,
//     pub chosen_discovered: usize,
//     pub monitor: BLEClient,
//     // discovered_rx: Option<Receiver<BleIdents>>,
// }
// impl<'a> BleStuff<'a> {
//     pub fn build() -> Self {
//         let mut monitor = BLEClient::new();
//         monitor.on_connect(|client| {
//             client.update_conn_params(120, 120, 0, 60).unwrap();
//         });
//         Self {
//             host_device: BLEDevice::take(),
//             discovered: Monitors::new(),
//             chosen_discovered: 0,
//             monitor,
//             // discovered_rx: None,
//         }
//     }
//     pub async fn scan_for_connect(&self, ident: &BleIdents) -> Result<Option<BLEAddress>> {
//         let mut ble_scan = BLEScan::new();
//         let addr: Option<BLEAddress> = ble_scan
//             .active_scan(true)
//             .interval(100)
//             .window(99)
//             .filter_duplicates(false)
//             .start(&self.host_device, 10000, |device, data| {
//                 if device.addr().as_be_bytes() == ident.mac {
//                     Some(device.addr())
//                 } else if let Some(name) = data.name() {
//                     if name == ident.name {
//                         Some(device.addr())
//                     } else {
//                         None
//                     }
//                 } else {
//                     None
//                 }
//             })
//             .await?;

//         Ok(addr)
//     }
//     pub async fn scan_for_select(&self) -> Result<Monitors> {
//         let mut ble_scan = BLEScan::new();
//         let mut devices = Monitors::new();
//         let _: Option<()> = ble_scan
//             .active_scan(true)
//             .interval(100)
//             .window(99)
//             .filter_duplicates(false)
//             .start(&self.host_device, 10000, |device, data| {
//                 // info!("{device:#?}\n{data:#?}");
//                 let address = device.addr().as_be_bytes();

//                 if data
//                     .service_uuids()
//                     .find(|s| *s == HR_SERVICE_UUID)
//                     .is_some()
//                 {
//                     // I think this will always give me a blank name if it had the services?
//                     // Unsure, need to look into how BLE advertising works.
//                     devices.insert(address, data.name().unwrap_or_default().to_string());
//                     info!("Addr: {:?}", device.addr());
//                 }

//                 // Populate the discovered monitors's name in the map if it's empty
//                 match (devices.get(&address), data.name()) {
//                     (Some(current_name), Some(device_name)) if current_name.is_empty() => {
//                         devices.insert(address, device_name.to_string());
//                     }
//                     _ => (),
//                 }

//                 None
//             })
//             .await?;

//         Ok(devices)
//     }
//     // pub async fn is_monitor_present() -> Result<bool> {
//     //     let mut found = false;
//     //     Ok(found)
//     // }
//     // pub async fn connect_to_monitor(
//     //     &mut self,
//     //     addr: BleMacLe,
//     //     hr_tx: SyncSender<MonitorStatus>,
//     // ) -> Result<()> {
//     // }
// }

// #[derive(Debug)]
// pub enum BleHrCommand {
//     Scan,
//     Connect,
//     // Disconnect
// }

#[derive(Debug)]
pub enum MonitorReply {
    Connected,
    Error(AppError),
    // ScannedDevice(BleIdents),
    MonitorStatus(MonitorStatus),
    Disconnected,
}

pub struct MonitorHandle {
    pub reply_rx: Receiver<MonitorReply>,
}

impl MonitorHandle {
    pub fn build() -> Result<Self> {
        let (reply_tx, reply_rx) = mpsc::sync_channel::<MonitorReply>(5);

        std::thread::Builder::new()
            // .stack_size(4000)
            .spawn(move || {
                let mut x = 70;
                let mut up = true;
                loop {
                    match up {
                        true if x < 95 => {
                            x += 1;
                        }
                        true if x <= 95 => {
                            up = false;
                        }
                        false if x > 70 => {
                            x -= 1;
                        }
                        false if x >= 70 => {
                            up = true;
                        }
                        _ => unreachable!(),
                    };
                    reply_tx
                        .send(MonitorReply::MonitorStatus(MonitorStatus {
                            heart_rate_bpm: x,
                            ..Default::default()
                        }))
                        .unwrap();
                    std::thread::sleep(Duration::from_millis(500));
                }
                // let mut actor = MonitorActor::build(addr).unwrap();
                // block_on(async {
                //     let err_tx = reply_tx.clone();
                //     if let Err(e) = actor.connect(reply_tx).await {
                //         err_tx.send(MonitorReply::Error(e)).unwrap();
                //     };
                // });
                // loop {
                //     if !actor.client.connected() {
                //         break;
                //     }
                //     delay.delay_ms(1000);
                // }
            })?;

        Ok(Self {
            reply_rx,
            // command_tx,
        })
    }
    // pub fn test(&self) {
    //     self.command_tx.send(BleHrCommand::Scan).unwrap();
    // }
}

// struct MonitorActor {
//     // command_rx: Receiver<BleHrCommand>,
//     // reply_tx: Takeable<SyncSender<MonitorReply>>,
//     client: BLEClient,
//     address: BLEAddress,
//     // delay: Delay,
// }

// impl MonitorActor {
//     pub fn build(
//         // command_rx: Receiver<BleHrCommand>,
//         // reply_tx: SyncSender<MonitorReply>,
//         target_addr: BLEAddress, // delay: Delay,
//     ) -> Result<Self> {
//         let mut client = BLEClient::new();
//         client.on_connect(|client| {
//             client.update_conn_params(120, 120, 0, 60).unwrap();
//         });

//         Ok(Self {
//             // command_rx,
//             // reply_tx: Takeable::new(reply_tx),
//             client,
//             address: target_addr,
//             // delay,
//         })
//     }
//     async fn connect(&mut self, reply_tx: SyncSender<MonitorReply>) -> Result<()> {
//         if let Err(e) = self.client.connect(&self.address).await {
//             reply_tx.send(MonitorReply::Error(e.into())).unwrap();
//             return Ok(());
//         }

//         let mut status = MonitorStatus::default();

//         if let Ok(service) = self.client.get_service(BATTERY_SERVICE_UUID).await {
//             let characteristic = service.get_characteristic(BATTERY_CHAR_UUID).await?;
//             let value = characteristic.read_value().await?;
//             ::log::info!("Battery value: {:?}%", value[0]);
//             status.battery_level = value[0].into();
//         }

//         let hr_service = self.client.get_service(HR_SERVICE_UUID).await?;

//         let characteristic = hr_service.get_characteristic(HR_CHAR_UUID).await?;
//         if !characteristic.can_notify() {
//             ::log::error!("characteristic can't notify: {}", characteristic);
//             return Ok(());
//         }

//         ::log::info!("subscribe to {}", characteristic);
//         // let reply_tx = self.reply_tx.take();
//         characteristic
//             .on_notify(move |data| {
//                 status.update_from_slice(data);
//                 reply_tx
//                     .send(MonitorReply::MonitorStatus(status.clone()))
//                     .unwrap();
//                 ::log::info!("HR Notify: {:?}", data);
//             })
//             // Dunno yet why this is `false`
//             .subscribe_notify(false)
//             .await?;
//         Ok(())
//     }
// }

// pub async fn ble_stuff() -> Result<()> {
//     // block_on(async {

//     if let Some(device) = device {
//         let mut client = BLEClient::new();
//         client.on_connect(|client| {
//             client.update_conn_params(120, 120, 0, 60).unwrap();
//         });
//         client.connect(&device.addr()).await?;

//         let service = client
//             .get_service(uuid128!("0000180f-0000-1000-8000-00805f9b34fb"))
//             .await?;

//         let uuid = uuid128!("00002a19-0000-1000-8000-00805f9b34fb");
//         let characteristic = service.get_characteristic(uuid).await?;
//         let value = characteristic.read_value().await?;
//         ::log::info!("{} value: {:?}", characteristic, value);

//         let service = client
//             .get_service(uuid128!("0000180d-0000-1000-8000-00805f9b34fb"))
//             .await?;

//         let uuid = uuid128!("00002a37-0000-1000-8000-00805f9b34fb");
//         let characteristic = service.get_characteristic(uuid).await?;

//         if !characteristic.can_notify() {
//             ::log::error!("characteristic can't notify: {}", characteristic);
//             return Ok(());
//         }

//         ::log::info!("subscribe to {}", characteristic);
//         characteristic
//             .on_notify(|data| {
//                 ::log::info!("{:?}", data);
//             })
//             .subscribe_notify(false)
//             .await?;

//         // delay

//         client.disconnect()?;
//     }

//     Ok(())
//     // })
// }
