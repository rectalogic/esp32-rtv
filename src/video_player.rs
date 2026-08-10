use esp_idf_svc::sys::{
    EspError, esp,
    video_player::{
        deinitialize_video_system, esp_player_err_t, hilite_video, initialize_video_system,
        play_url,
    },
};
use std::ffi::CString;

pub struct VideoPlayer {
    _private: (),
}

impl VideoPlayer {
    pub fn new() -> Result<Self, Error> {
        let result = Error::from(unsafe { initialize_video_system() });
        if result == Error::Ok {
            Ok(Self { _private: () })
        } else {
            Err(result)
        }
    }

    pub fn play(&self, url: &str) -> Result<Status, Error> {
        let c_url = CString::new(url).map_err(|_| Error::InvalidArg)?;
        esp_err_to_result(unsafe { play_url(c_url.as_ptr()) })
    }

    pub fn hilite_video(&self, hilite: bool) -> Result<(), EspError> {
        esp!(unsafe { hilite_video(hilite) })
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        unsafe { deinitialize_video_system() };
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Copy, Clone)]
pub enum Error {
    #[error("Operation successful")]
    Ok,
    #[error("End of Stream (reached end of file)")]
    Eos,
    #[error("Operation failed (generic error)")]
    Fail,
    #[error("Invalid parameter error")]
    InvalidArg,
    #[error("Out of memory error")]
    NoMem,
    #[error("Operation timeout error")]
    Timeout,
    #[error("Unsupported feature or format")]
    NotSupport,
    #[error("Operation not allowed in current player state")]
    InvalidState,
    #[error("Unknown: {0}")]
    Unknown(esp_player_err_t),
}

pub enum Status {
    Ok,
    Eos,
}

impl Error {
    fn into_result(self) -> Result<Status, Self> {
        match self {
            Error::Ok => Ok(Status::Ok),
            Error::Eos => Ok(Status::Eos),
            err => Err(err),
        }
    }
}

fn esp_err_to_result(code: esp_player_err_t) -> Result<Status, Error> {
    Error::from(code).into_result()
}

impl From<esp_player_err_t> for Error {
    fn from(err: esp_player_err_t) -> Self {
        match err {
            0 /* esp_player_err_t_ESP_PLAYER_ERR_OK */ => Error::Ok,
            1 /* esp_player_err_t_ESP_PLAYER_ERR_EOS */ => Error::Eos,
            -1 /* esp_player_err_t_ESP_PLAYER_ERR_FAIL */ => Error::Fail,
            -2 /* esp_player_err_t_ESP_PLAYER_ERR_INVALID_ARG */ => Error::InvalidArg,
            -3 /* esp_player_err_t_ESP_PLAYER_ERR_NO_MEM */ => Error::NoMem,
            -4 /* esp_player_err_t_ESP_PLAYER_ERR_TIMEOUT */ => Error::Timeout,
            -5 /* esp_player_err_t_ESP_PLAYER_ERR_NOT_SUPPORT */ => Error::NotSupport,
            -6 /* esp_player_err_t_ESP_PLAYER_ERR_INVALID_STATE */ => Error::InvalidState,
            unknown => Error::Unknown(unknown),
        }
    }
}
