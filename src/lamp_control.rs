use esp_idf_svc::hal::gpio::{Gpio22, Gpio23, Input, Level, Output, PinDriver};
use esp_idf_svc::mqtt::client::{EspMqttClient, QoS};
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use esp_idf_svc::sys::EspError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::model::AttributePayload;
use crate::mqtt::topics::PublishedTopics;

const NVS_NAMESPACE: &str = "lamp_control";

enum NvsKey {
    LampState,
    SensorState,
}

impl From<NvsKey> for &str {
    fn from(key: NvsKey) -> Self {
        match key {
            NvsKey::LampState => "lamp_state",
            NvsKey::SensorState => "sensor_state",
        }
    }
}

pub struct LampControl {
    lamp_state: Arc<AtomicBool>,
    sensor_state: Arc<AtomicBool>,
    relay_pin: Arc<Mutex<PinDriver<'static, Gpio23, Output>>>,
    nvs: Arc<Mutex<EspNvs<NvsDefault>>>,
    client: Arc<Mutex<EspMqttClient<'static>>>,
}

impl LampControl {
    pub fn new(
        mut relay_pin: PinDriver<'static, Gpio23, Output>,
        sensor_pin: PinDriver<'static, Gpio22, Input>,
        nvs: EspDefaultNvsPartition,
        client: Arc<Mutex<EspMqttClient<'static>>>,
    ) -> Self {
        let mut nvs = EspNvs::new(nvs, NVS_NAMESPACE, true).unwrap();

        let lamp_state = load_state(&mut nvs, NvsKey::LampState);
        let sensor_state = load_state(&mut nvs, NvsKey::SensorState);

        relay_pin
            .set_level(match lamp_state {
                true => Level::High,
                false => Level::Low,
            })
            .unwrap();

        let lamp_state = Arc::new(AtomicBool::new(lamp_state));
        let sensor_state = Arc::new(AtomicBool::new(sensor_state));
        let relay_pin = Arc::new(Mutex::new(relay_pin));
        let nvs = Arc::new(Mutex::new(nvs));

        {
            let lamp_state = lamp_state.clone();
            let sensor_state = sensor_state.clone();
            let relay_pin = relay_pin.clone();
            let nvs = nvs.clone();
            let client = client.clone();

            thread::spawn(move || loop {
                if sensor_state.load(Ordering::Relaxed) {
                    let sensor_pin = sensor_pin.get_level();

                    let level = match sensor_pin {
                        Level::High => true,
                        Level::Low => false,
                    };

                    if lamp_state.load(Ordering::Relaxed) != level {
                        relay_pin.lock().unwrap().set_level(sensor_pin).unwrap();
                        save_state(&mut nvs.lock().unwrap(), NvsKey::LampState, level).unwrap();

                        let payload = serde_json::to_vec(&AttributePayload {
                            lamp_state: level,
                            sensor_state: true,
                        })
                        .unwrap();

                        client
                            .lock()
                            .unwrap()
                            .publish(
                                PublishedTopics::Attributes.into(),
                                QoS::AtMostOnce,
                                false,
                                &payload,
                            )
                            .unwrap();

                        lamp_state.store(level, Ordering::Relaxed);
                    }
                }

                thread::sleep(Duration::from_millis(100));
            });

            log::info!("Sensor thread started");
        };

        Self {
            lamp_state,
            sensor_state,
            relay_pin,
            nvs,
            client,
        }
    }

    pub fn set_lamp(&mut self, state: bool) {
        // If sensor is on, do not allow lamp to be toggled
        if self.sensor_state.load(Ordering::Relaxed) {
            log::warn!("Cannot toggle lamp while sensor is on");
            return;
        }

        self.relay_pin
            .lock()
            .unwrap()
            .set_level(match state {
                true => Level::High,
                false => Level::Low,
            })
            .unwrap();

        self.lamp_state.store(state, Ordering::Relaxed);

        save_state(&mut self.nvs.lock().unwrap(), NvsKey::LampState, state).unwrap();

        let payload = serde_json::to_vec(&AttributePayload {
            lamp_state: state,
            sensor_state: self.sensor_state.load(Ordering::Relaxed),
        })
        .unwrap();

        self.client
            .lock()
            .unwrap()
            .publish(
                PublishedTopics::Attributes.into(),
                QoS::AtMostOnce,
                false,
                &payload,
            )
            .unwrap();
    }

    pub fn set_sensor(&mut self, state: bool) {
        self.sensor_state.store(state, Ordering::Relaxed);
        save_state(&mut self.nvs.lock().unwrap(), NvsKey::SensorState, state).unwrap();

        self.client
            .lock()
            .unwrap()
            .publish(
                PublishedTopics::Attributes.into(),
                QoS::AtMostOnce,
                false,
                &serde_json::to_vec(&AttributePayload {
                    sensor_state: state,
                    lamp_state: self.lamp_state.load(Ordering::Relaxed),
                })
                .unwrap(),
            )
            .unwrap();
    }
}

fn save_state(nvs: &mut EspNvs<NvsDefault>, key: NvsKey, state: bool) -> Result<bool, EspError> {
    nvs.set_raw(key.into(), &[state as u8])
}

fn load_state(nvs: &mut EspNvs<NvsDefault>, key: NvsKey) -> bool {
    match nvs.get_raw(key.into(), &mut [0; 1]) {
        Ok(Some(data)) => data[0] != 0,
        _ => false,
    }
}
