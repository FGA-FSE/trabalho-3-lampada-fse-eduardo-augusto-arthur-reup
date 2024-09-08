mod lamp_control;
mod model;
mod mqtt;
mod wifi;

use std::thread;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::mqtt::client::QoS;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys;
use lamp_control::LampControl;
use model::RequestPayload;
use mqtt::topics::SubscribedTopics;

fn main() {
    sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    // Wifi configuration
    let _wifi =
        wifi::start_wifi(peripherals.modem, sys_loop, nvs.clone()).expect("Error starting wifi");

    // MQTT configuration
    let (client, rx) = mqtt::start_mqtt().expect("Failed to start mqtt");

    log::info!("Wifi and MQTT started");

    // Handler thread
    thread::spawn(move || {
        let mut lamp_control = LampControl::new(
            PinDriver::output(peripherals.pins.gpio23).unwrap(),
            PinDriver::input(peripherals.pins.gpio22).unwrap(),
            nvs,
            client.clone(),
        );

        log::info!("Lamp control initialized");

        while let Ok((topic, payload)) = rx.recv() {
            match topic {
                SubscribedTopics::Request(_) => {
                    // Deserialize request
                    let request: RequestPayload = match serde_json::from_slice(&payload) {
                        Ok(request) => request,
                        Err(e) => {
                            log::error!("Error deserializing request: {:?}", e);
                            continue;
                        }
                    };

                    if request.method != "setValue" {
                        log::warn!("Received request with method: {}", request.method);
                        continue;
                    }

                    if request.params.is_none() {
                        log::error!("Request params are missing");
                        continue;
                    }

                    let params = request.params.unwrap();

                    // if type is lamp, toggle lamp
                    match params.request_type.as_str() {
                        "lamp_state" => {
                            lamp_control.set_lamp(params.value);
                        }
                        "sensor_state" => {
                            lamp_control.set_sensor(params.value);
                        }
                        _ => {
                            log::error!("Unknown request type: {}", params.request_type);
                            continue;
                        }
                    }

                    // Respond to request
                    let topic: String = topic.into();

                    match client
                        .lock()
                        .unwrap()
                        .publish(&topic, QoS::AtMostOnce, false, &payload)
                    {
                        Ok(_) => (),
                        Err(e) => log::error!("Error sending response: {:?}", e),
                    }
                }
            }
        }
    })
    .join()
    .unwrap();
}
