// Queue module for NATS JetStream integration

pub mod consumer;
pub mod nats;
pub mod publisher;

pub use consumer::{JobConsumer, JobHandler, NatsJobConsumer};
pub use nats::{NatsClient, NatsConfig};
pub use publisher::{JobMessage, JobPublisher, NatsJobPublisher};
