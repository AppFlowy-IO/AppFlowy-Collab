mod database_id;

use lazy_static::lazy_static;
use parking_lot::Mutex;

use database_id::DatabaseIDGen;
lazy_static! {
  pub static ref ID_GEN: Mutex<DatabaseIDGen> = Mutex::new(DatabaseIDGen::new(1));
}
