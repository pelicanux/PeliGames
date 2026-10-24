//! Output monitor only, through the PulseAudio ABI also served by pipewire-pulse.
//! No external executable, microphone fallback, audio files, or blocking reads.
use super::neon_wave_bands::BandMeter;
use std::{
    ffi::{c_char, c_int, c_void},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
type Handle = *mut c_void;
#[repr(C)]
struct SampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}
#[repr(C)]
struct BufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}
struct Library(Handle);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.0);
        }
    }
}
impl Library {
    unsafe fn symbol<T: Copy>(&self, name: &[u8]) -> Result<T, String> {
        let value = unsafe { libc::dlsym(self.0, name.as_ptr().cast()) };
        if value.is_null() {
            return Err("A biblioteca de áudio não oferece a API necessária.".into());
        }
        Ok(unsafe { std::mem::transmute_copy(&value) })
    }
}
macro_rules! pulse_api {
 ($($field:ident:$ty:ty),* $(,)?)=>{
  struct Pulse {$($field:$ty,)* _library:Library}
  impl Pulse {unsafe fn load()->Result<Self,String>{
   let handle=unsafe{libc::dlopen(c"libpulse.so.0".as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL)};
   if handle.is_null(){return Err("A biblioteca libpulse.so.0 não está disponível. O fundo continua no modo ambiente.".into());}
   let library=Library(handle);
   Ok(Self{$($field:unsafe{library.symbol(concat!(stringify!($field),"\0").as_bytes())?},)* _library:library})
  }}
 }
}
pulse_api! {
 pa_mainloop_new:unsafe extern "C" fn()->Handle,
 pa_mainloop_free:unsafe extern "C" fn(Handle),
 pa_mainloop_get_api:unsafe extern "C" fn(Handle)->Handle,
 pa_mainloop_iterate:unsafe extern "C" fn(Handle,c_int,*mut c_int)->c_int,
 pa_context_new:unsafe extern "C" fn(Handle,*const c_char)->Handle,
 pa_context_connect:unsafe extern "C" fn(Handle,*const c_char,c_int,Handle)->c_int,
 pa_context_get_state:unsafe extern "C" fn(Handle)->c_int,
 pa_context_disconnect:unsafe extern "C" fn(Handle),
 pa_context_unref:unsafe extern "C" fn(Handle),
 pa_stream_new:unsafe extern "C" fn(Handle,*const c_char,*const SampleSpec,Handle)->Handle,
 pa_stream_connect_record:unsafe extern "C" fn(Handle,*const c_char,*const BufferAttr,c_int)->c_int,
 pa_stream_get_state:unsafe extern "C" fn(Handle)->c_int,
 pa_stream_peek:unsafe extern "C" fn(Handle,*mut *const c_void,*mut usize)->c_int,
 pa_stream_drop:unsafe extern "C" fn(Handle)->c_int,
 pa_stream_disconnect:unsafe extern "C" fn(Handle)->c_int,
 pa_stream_unref:unsafe extern "C" fn(Handle),
}
struct Capture {
    api: Pulse,
    mainloop: Handle,
    context: Handle,
    stream: Handle,
}
impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            if !self.stream.is_null() {
                (self.api.pa_stream_disconnect)(self.stream);
                (self.api.pa_stream_unref)(self.stream);
            }
            if !self.context.is_null() {
                (self.api.pa_context_disconnect)(self.context);
                (self.api.pa_context_unref)(self.context);
            }
            if !self.mainloop.is_null() {
                (self.api.pa_mainloop_free)(self.mainloop);
            }
        }
    }
}
/// Runs on a dedicated worker. Every read is nonblocking; cancellation is checked
/// at least every 8 ms, even during silence or an unavailable audio server.
pub fn monitor(cancel: &AtomicBool, mut levels: impl FnMut([f32; 3])) -> Result<(), String> {
    unsafe {
        let api = Pulse::load()?;
        let mut capture = Capture {
            api,
            mainloop: ptr::null_mut(),
            context: ptr::null_mut(),
            stream: ptr::null_mut(),
        };
        capture.mainloop = (capture.api.pa_mainloop_new)();
        if capture.mainloop.is_null() {
            return Err("Não foi possível iniciar o monitor de áudio.".into());
        }
        capture.context = (capture.api.pa_context_new)(
            (capture.api.pa_mainloop_get_api)(capture.mainloop),
            c"PeliGames Neon Wave".as_ptr(),
        );
        if capture.context.is_null()
            || (capture.api.pa_context_connect)(capture.context, ptr::null(), 1, ptr::null_mut())
                < 0
        {
            return Err("A saída de áudio PipeWire/PulseAudio não está disponível.".into());
        }
        let started = Instant::now();
        let mut emitted = Instant::now();
        let mut meter = BandMeter::default();
        let mut ready = false;
        while !cancel.load(Ordering::Acquire) {
            if (capture.api.pa_mainloop_iterate)(capture.mainloop, 0, ptr::null_mut()) < 0 {
                return Err("A conexão de áudio foi encerrada.".into());
            }
            match (capture.api.pa_context_get_state)(capture.context) {
                5 | 6 => return Err(
                    "O servidor de áudio desconectou. Selecione o tema novamente para reconectar."
                        .into(),
                ),
                4 => {
                    if capture.stream.is_null() {
                        let spec = SampleSpec {
                            format: 3,
                            rate: 24000,
                            channels: 1,
                        };
                        capture.stream = (capture.api.pa_stream_new)(
                            capture.context,
                            c"Output visualization".as_ptr(),
                            &spec,
                            ptr::null_mut(),
                        );
                        let attr = BufferAttr {
                            maxlength: 48000,
                            tlength: u32::MAX,
                            prebuf: u32::MAX,
                            minreq: u32::MAX,
                            fragsize: 960,
                        };
                        if capture.stream.is_null()
                            || (capture.api.pa_stream_connect_record)(
                                capture.stream,
                                c"@DEFAULT_MONITOR@".as_ptr(),
                                &attr,
                                0,
                            ) < 0
                        {
                            return Err("O monitor da saída de áudio está indisponível.".into());
                        }
                    }
                }
                _ => {}
            }
            if !capture.stream.is_null() {
                match (capture.api.pa_stream_get_state)(capture.stream) {
                    3 | 4 => {
                        return Err("Não foi possível acessar o monitor da saída de áudio.".into())
                    }
                    2 => {
                        ready = true;
                        for _ in 0..8 {
                            let mut bytes: *const c_void = ptr::null();
                            let mut size = 0usize;
                            if (capture.api.pa_stream_peek)(capture.stream, &mut bytes, &mut size)
                                < 0
                            {
                                return Err("Falha ao analisar a saída de áudio.".into());
                            }
                            if size == 0 {
                                break;
                            }
                            if !bytes.is_null() {
                                meter.push(std::slice::from_raw_parts(bytes.cast::<u8>(), size));
                            }
                            (capture.api.pa_stream_drop)(capture.stream);
                        }
                        if emitted.elapsed() >= Duration::from_millis(50) {
                            levels(meter.take());
                            emitted = Instant::now();
                        }
                    }
                    _ => {}
                }
            }
            if !ready && started.elapsed() > Duration::from_secs(8) {
                return Err(
                    "O servidor de áudio não respondeu. O fundo continua no modo ambiente.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        Ok(())
    }
}
