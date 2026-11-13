pub mod selection;
pub mod shortcuts;
pub mod key;
use serde::{Serialize,Deserialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CaptureStatus {
    pub phase: String,
    pub game_key: Option<String>,
    pub game_name: Option<String>,
    pub recording: bool,
    pub replay: bool,
    pub last_file: Option<String>,
    pub error: Option<String>,
}
