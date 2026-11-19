use jiff::Timestamp;
use serde::Serialize;
use toasty::Deferred;

use super::Fruit;

#[derive(Debug, toasty::Model, Serialize)]
#[table = "cake"]
pub struct Cake {
    #[key]
    #[auto]
    pub id: i32,

    #[default(Timestamp::now())]
    pub created_at: Timestamp,

    #[column(type = varchar(255))]
    pub name: String,

    #[has_many]
    pub fruits: Deferred<Vec<Fruit>>,
}
