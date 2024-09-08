pub const TOPICS_TO_SUBSCRIBE: [&str; 1] = ["v1/devices/me/rpc/request/+"];

pub enum SubscribedTopics {
    Request(u32),
}

impl TryFrom<&str> for SubscribedTopics {
    type Error = ();

    fn try_from(topic: &str) -> Result<Self, Self::Error> {
        // If the topic of type request is received, extract the request ID
        if let Some(id) = topic.strip_prefix("v1/devices/me/rpc/request/") {
            if let Ok(id) = id.parse::<u32>() {
                return Ok(SubscribedTopics::Request(id));
            }
        }

        Err(())
    }
}

impl TryFrom<String> for SubscribedTopics {
    type Error = ();

    fn try_from(topic: String) -> Result<Self, Self::Error> {
        Self::try_from(topic.as_str())
    }
}

impl From<SubscribedTopics> for String {
    fn from(topic: SubscribedTopics) -> Self {
        match topic {
            SubscribedTopics::Request(id) => format!("v1/devices/me/rpc/response/{}", id),
        }
    }
}

pub enum PublishedTopics {
    Attributes,
}

impl From<PublishedTopics> for &str {
    fn from(topic: PublishedTopics) -> Self {
        match topic {
            PublishedTopics::Attributes => "v1/devices/me/attributes",
        }
    }
}

impl From<PublishedTopics> for String {
    fn from(topic: PublishedTopics) -> Self {
        let str: &str = topic.into();
        str.to_string()
    }
}
