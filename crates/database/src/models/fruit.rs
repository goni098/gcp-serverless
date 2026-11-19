use jiff::Timestamp;
use serde::Serialize;
use toasty::Deferred;

use super::Cake;

#[derive(Debug, toasty::Model, Serialize)]
#[table = "fruit"]
pub struct Fruit {
    #[key]
    #[auto]
    pub id: i32,

    #[default(Timestamp::now())]
    pub created_at: Timestamp,

    #[column(type = varchar(255))]
    pub name: String,

    #[index]
    pub cake_id: Option<i32>,

    #[belongs_to(key = cake_id, references = id)]
    pub cake: Deferred<Option<Cake>>,
}
