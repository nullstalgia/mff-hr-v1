use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, Sender, SyncSender},
};

use crate::errors::Result;
use bstr::ByteSlice;
use esp32_nimble::{utilities::BleUuid, uuid128, BLEAddress, BLEClient, BLEDevice, BLEScan};
use esp_idf_hal::delay::Delay;
use esp_idf_svc::hal::{
    prelude::Peripherals,
    task::block_on,
    timer::{TimerConfig, TimerDriver},
};
use log::info;
use serde_derive::{Deserialize, Serialize};

const BATTERY_SERVICE_UUID: BleUuid = uuid128!("0000180f-0000-1000-8000-00805f9b34fb");
const BATTERY_CHAR_UUID: BleUuid = uuid128!("00002a19-0000-1000-8000-00805f9b34fb");

const HR_SERVICE_UUID: BleUuid = uuid128!("0000180d-0000-1000-8000-00805f9b34fb");
const HR_CHAR_UUID: BleUuid = uuid128!("00002a37-0000-1000-8000-00805f9b34fb");

pub type BleMacLe = [u8; 6];
pub type Monitors = BTreeMap<BleMacLe, String>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BatteryLevel {
    #[default]
    Unknown,
    NotReported,
    Level(u8),
}

impl From<BatteryLevel> for u8 {
    fn from(level: BatteryLevel) -> Self {
        match level {
            BatteryLevel::Level(battery) => battery,
            _ => 0,
        }
    }
}
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct BleIdents {
    pub mac: [u8; 6],
    pub name: String,
}

impl std::fmt::Display for BleIdents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name_display = if self.name.is_empty() {
            "Unknown".to_string()
        } else {
            self.name.clone()
        };
        write!(
            f,
            "{}\n({:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X})",
            name_display,
            self.mac[0],
            self.mac[1],
            self.mac[2],
            self.mac[3],
            self.mac[4],
            self.mac[5],
        )
    }
}

// impl BleIdents {
//     pub fn to_string_all(&self) -> String {
//         let name_display = if self.name.is_empty() {
//             "Unknown".to_string()
//         } else {
//             self.name.clone()
//         };
//         format!(
//             "{}\n({:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X})",
//             name_display,
//             self.mac[0],
//             self.mac[1],
//             self.mac[2],
//             self.mac[3],
//             self.mac[4],
//             self.mac[5],
//         )
//     }
// }

// impl std::fmt::Display for BleIdents {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         if !self.name.is_empty() {
//             write!(f, "{}", self.name)
//         } else {
//             write!(
//                 f,
//                 "MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
//                 self.mac[0], self.mac[1], self.mac[2], self.mac[3], self.mac[4], self.mac[5],
//             )
//         }
//     }
// }

pub struct MonitorStatus {
    pub heart_rate_bpm: u16,
    pub rr_intervals: Vec<std::time::Duration>,
    pub battery_level: BatteryLevel,
    // Twitches are calculated by HR sources so that
    // all listeners see twitches at the same time
    pub twitch_up: bool,
    pub twitch_down: bool,
}

pub struct BleStuff<'a> {
    host_device: &'a mut BLEDevice,
    pub discovered: Monitors,
    pub chosen_discovered: usize,
    pub monitor: BLEClient,
    // discovered_rx: Option<Receiver<BleIdents>>,
}
impl<'a> BleStuff<'a> {
    pub fn build() -> Self {
        let mut monitor = BLEClient::new();
        monitor.on_connect(|client| {
            client.update_conn_params(120, 120, 0, 60).unwrap();
        });
        Self {
            host_device: BLEDevice::take(),
            discovered: Monitors::new(),
            chosen_discovered: 0,
            monitor,
            // discovered_rx: None,
        }
    }
    pub async fn scan_for_select(&self) -> Result<Monitors> {
        let mut ble_scan = BLEScan::new();
        let mut devices = Monitors::new();
        let _: Option<()> = ble_scan
            .active_scan(true)
            .interval(100)
            .window(99)
            .filter_duplicates(false)
            .start(&self.host_device, 10000, |device, data| {
                // info!("{device:#?}\n{data:#?}");
                let address = device.addr().as_be_bytes();

                if data
                    .service_uuids()
                    .find(|s| *s == HR_SERVICE_UUID)
                    .is_some()
                {
                    // I think this will always give me a blank name if it had the services?
                    // Unsure, need to look into how BLE advertising works.
                    devices.insert(address, data.name().unwrap_or_default().to_string());
                }

                // Populate the discovered monitors's name in the map if it's empty
                match (devices.get(&address), data.name()) {
                    (Some(current_name), Some(device_name)) if current_name.is_empty() => {
                        devices.insert(address, device_name.to_string());
                    }
                    _ => (),
                }

                None
            })
            .await?;

        Ok(devices)
    }
    pub async fn connect_to_monitor(&mut self, addr: BleMacLe) -> Result<()> {
        let addr = BLEAddress::from_be_bytes(addr, esp32_nimble::BLEAddressType::Random);
        self.monitor.connect(&addr).await?;

        let service = self.monitor.get_service(BATTERY_SERVICE_UUID).await?;

        let uuid = BATTERY_CHAR_UUID;
        let characteristic = service.get_characteristic(uuid).await?;
        let value = characteristic.read_value().await?;
        ::log::info!("Battery value: {:?}", value);

        let service = self.monitor.get_service(HR_SERVICE_UUID).await?;

        let uuid = HR_CHAR_UUID;
        let characteristic = service.get_characteristic(uuid).await?;

        if !characteristic.can_notify() {
            ::log::error!("characteristic can't notify: {}", characteristic);
            return Ok(());
        }

        ::log::info!("subscribe to {}", characteristic);
        characteristic
            .on_notify(|data| {
                ::log::info!("{:?}", data);
            })
            // Dunno yet why this is `false`
            .subscribe_notify(false)
            .await?;
        Ok(())
    }
}

// #[derive(Debug)]
// pub enum BleHrCommand {
//     Scan,
//     Connect,
//     // Disconnect
// }

// pub enum BleHrReply {
//     ScannedDevice(BleIdents),
//     MonitorStatus(MonitorStatus),
//     Error,
// }

// pub struct BleHrHandle {
//     reply_rx: Receiver<BleHrReply>,
//     command_tx: SyncSender<BleHrCommand>,
// }

// impl BleHrHandle {
//     pub fn build() -> Result<Self> {
//         let (command_tx, command_rx) = mpsc::sync_channel::<BleHrCommand>(5);
//         let (reply_tx, reply_rx) = mpsc::sync_channel::<BleHrReply>(5);

//         // let actor = BleHrActor::build(command_rx, reply_tx)?;

//         // std::thread::Builder::new()
//         //     .name("ble-hr".to_string())
//         //     .stack_size(10000)
//         //     .spawn(move || {
//         //         block_on(async {
//         //             // ble_stuff().await.unwrap()

//         //             while let Ok(command) = actor.command_rx.recv() {
//         //                 info!("ble-hr: {command:?}");
//         //                 match command {
//         //                     BleHrCommand::Scan => {
//         //                         actor.scan_for_select().await.unwrap();
//         //                     }
//         //                     BleHrCommand::Connect => (),
//         //                 }
//         //             }
//         //         });
//         //     })?;

//         Ok(Self {
//             reply_rx,
//             command_tx,
//         })
//     }
//     pub fn test(&self) {
//         self.command_tx.send(BleHrCommand::Scan).unwrap();
//     }
// }

// struct BleHrActor<'a> {
//     command_rx: Receiver<BleHrCommand>,
//     reply_tx: SyncSender<BleHrReply>,
//     device: &'a mut BLEDevice,
//     // delay: Delay,
// }

// impl<'a> BleHrActor<'a> {
//     pub fn build(
//         command_rx: Receiver<BleHrCommand>,
//         reply_tx: SyncSender<BleHrReply>,
//         // delay: Delay,
//     ) -> Result<Self> {
//         let device = BLEDevice::take();
//         Ok(Self {
//             command_rx,
//             reply_tx,
//             device,
//             // delay,
//         })
//     }

//     pub async fn scan_for_select(&self) -> Result<()> {
//         let mut ble_scan = BLEScan::new();
//         let _: Option<()> = ble_scan
//             // .active_scan(true)
//             .interval(100)
//             .window(99)
//             .start(&self.device, 10000, |device, data| {
//                 info!("{device:#?}\n{data:#?}");
//                 None
//             })
//             .await?;

//         Ok(())
//     }

//     pub async fn scan_for_connect(&self, desired_device: Option<&BleIdents>) -> Result<()> {
//         return todo!();
//         let known_mac: Option<[u8; 6]> = desired_device.map(|d| d.mac);
//         let mut ble_scan = BLEScan::new();
//         let device = ble_scan
//             // .active_scan(true)
//             .interval(100)
//             .window(99)
//             .start(&self.device, 10000, |device, data| {
//                 // If we supplied a saved device
//                 if let Some(saved) = desired_device {
//                     // Check if the MAC or name matches
//                     if let Some(addr) = known_mac.as_ref() {
//                         if device.addr().as_le_bytes() == *addr {
//                             return Some(*device);
//                         }
//                     } else if let Some(name) = data.name() {
//                         if name == saved.name {
//                             return Some(*device);
//                         }
//                     }
//                 }

//                 None
//             })
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
