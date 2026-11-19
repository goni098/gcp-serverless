use std::borrow::Cow;
use std::fmt;
use std::panic::Location;

pub type R<A> = Result<A, AppError>;

#[derive(thiserror::Error)]
pub struct AppError {
    #[source]
    context: AnyError,
    location: &'static Location<'static>,
}

impl AppError {
    #[track_caller]
    pub fn cust<E: Into<Cow<'static, str>>>(msg: E) -> Self {
        Self {
            context: AnyError::Cust(msg.into()),
            location: Location::caller(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.context.fmt(formatter)
    }
}

impl fmt::Debug for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "Error: {:?}", self.context)?;
        writeln!(formatter, "Location: {}", self.location)?;

        Ok(())
    }
}

impl<E> From<E> for AppError
where
    E: Into<AnyError>,
{
    #[track_caller]
    fn from(value: E) -> Self {
        Self {
            context: value.into(),
            location: core::panic::Location::caller(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum AnyError {
    #[error(transparent)]
    Toasty(#[from] toasty_core::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("{0}")]
    Cust(Cow<'static, str>),

    #[error(transparent)]
    VarEnv(#[from] std::env::VarError),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Jiff(#[from] jiff::Error),

    #[error(transparent)]
    PubSubClientBuilder(#[from] google_cloud_gax::client_builder::Error),

    #[error(transparent)]
    PubSubPublish(#[from] google_cloud_pubsub::error::PublishError),
}
