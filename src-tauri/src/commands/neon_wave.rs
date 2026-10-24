use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Emitter;
struct Worker {
    session: String,
    cancel: Arc<AtomicBool>,
}
static WORKER: Mutex<Option<Worker>> = Mutex::new(None);
#[derive(Clone, Serialize)]
struct Bands {
    session: String,
    bass: f32,
    mid: f32,
    treble: f32,
}
#[derive(Clone, Serialize)]
struct AudioError {
    session: String,
    error: String,
}
#[tauri::command]
pub fn set_neon_wave_audio(
    app: tauri::AppHandle,
    enabled: bool,
    session: String,
) -> Result<(), String> {
    if session.is_empty() || session.len() > 80 {
        return Err("Sessão de visualização inválida.".into());
    }
    let mut worker = WORKER.lock().map_err(|e| e.to_string())?;
    if !enabled {
        if worker.as_ref().is_some_and(|old| old.session == session) {
            if let Some(old) = worker.take() {
                old.cancel.store(true, Ordering::Release);
            }
        }
        return Ok(());
    }
    if worker
        .as_ref()
        .is_some_and(|old| old.session == session && !old.cancel.load(Ordering::Acquire))
    {
        return Ok(());
    }
    if let Some(old) = worker.take() {
        old.cancel.store(true, Ordering::Release);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let id = session.clone();
    std::thread::Builder::new()
        .name("neon-wave-audio".into())
        .spawn(move || {
            #[cfg(target_os = "linux")]
            let result = crate::core::neon_wave_audio::monitor(&stop, |values| {
                if !stop.load(Ordering::Acquire) {
                    let _ = app.emit(
                        "neon-wave-bands",
                        Bands {
                            session: id.clone(),
                            bass: values[0],
                            mid: values[1],
                            treble: values[2],
                        },
                    );
                }
            });
            #[cfg(not(target_os = "linux"))]
            let result: Result<(), String> =
                Err("O monitor de áudio deste tema está disponível no Linux.".into());
            if let Err(error) = result {
                if !stop.load(Ordering::Acquire) {
                    let _ = app.emit("neon-wave-audio-error", AudioError { session: id, error });
                }
            }
            stop.store(true, Ordering::Release);
        })
        .map_err(|e| e.to_string())?;
    *worker = Some(Worker { session, cancel });
    Ok(())
}
