pub mod topics;

use esp_idf_svc::mqtt::client::{EspMqttClient, EventPayload, MqttClientConfiguration, QoS};
use esp_idf_svc::sys::EspError;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use topics::{SubscribedTopics, TOPICS_TO_SUBSCRIBE};

const MQTT_BROKER: &str = env!("MQTT_BROKER");
const USERNAME: &str = env!("MQTT_USERNAME");

type RequestReceiver = Receiver<(SubscribedTopics, Vec<u8>)>;
type Client = Arc<Mutex<EspMqttClient<'static>>>;

pub fn start_mqtt() -> Result<(Client, RequestReceiver), EspError> {
    let connected = Arc::new(AtomicBool::new(false));
    let subscribed = Arc::new(AtomicUsize::new(0));
    let (request_tx, request_rx) = channel();

    let client = {
        let connected = connected.clone();
        let subscribed = subscribed.clone();

        Arc::new(Mutex::new(EspMqttClient::new_cb(
            MQTT_BROKER,
            &MqttClientConfiguration {
                username: Some(USERNAME),
                ..Default::default()
            },
            move |event| match event.payload() {
                EventPayload::Error(e) => log::error!("Error: {:?}", e),

                EventPayload::Connected(_) => {
                    connected.store(true, Ordering::Relaxed);
                }

                EventPayload::Subscribed(_) => {
                    subscribed.fetch_add(1, Ordering::Relaxed);
                }

                EventPayload::Received { topic, data, .. } => match topic {
                    None => {
                        log::error!("Received message without topic");
                    }

                    Some(topic) => {
                        let result: Result<SubscribedTopics, ()> = topic.try_into();

                        match result {
                            Ok(topic) => request_tx.send((topic, data.to_vec())).unwrap(),
                            Err(()) => log::error!("Received message on unknown topic: {}", topic),
                        }
                    }
                },

                _ => {}
            },
        )?))
    };

    // Wait for connection
    while !connected.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    // Subscribe to topics
    for topic in TOPICS_TO_SUBSCRIBE {
        while let Err(e) = client.lock().unwrap().subscribe(topic, QoS::AtMostOnce) {
            log::error!("Error subscribing to topic: {:?}", e);
            thread::sleep(Duration::from_millis(500));
        }
    }

    // Wait for subscriptions
    while subscribed.load(Ordering::Relaxed) < TOPICS_TO_SUBSCRIBE.len() {
        thread::sleep(Duration::from_millis(100));
    }

    Ok((client, request_rx))
}
