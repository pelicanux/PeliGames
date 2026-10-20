//! One overlay host, with independently initialized game tools.
//! Rendering, shortcuts and input capture belong to the host, never to MAKO.
use crate::controls::Controller;

pub(crate) struct Tools {
    mako: Option<Controller>,
    mako_error: String,
}
impl Tools {
    fn load(
        general: bool,
        mako_requested: bool,
        initialize_mako: impl FnOnce() -> Result<Controller, String>,
    ) -> Result<Self, String> {
        let (mako, mako_error) = if mako_requested {
            match initialize_mako() {
                Ok(controller) => (Some(controller), String::new()),
                Err(error) if general => (None, format!("MAKO indisponível: {error}")),
                Err(error) => return Err(error),
            }
        } else if general {
            (None, String::new())
        } else {
            return Err("Overlay PeliGames não habilitado.".into());
        };
        Ok(Self { mako, mako_error })
    }

    pub(crate) fn new() -> Result<Self, String> {
        if std::env::var("PELI_OVERLAY_DISABLED").ok().as_deref() == Some("1") {
            return Err("Overlay PeliGames desativado.".into());
        }
        let tools = Self::load(
            std::env::var("PELI_OVERLAY_GENERAL").ok().as_deref() == Some("1"),
            std::env::var_os("MAKO_CONFIG").is_some(),
            Controller::new,
        )?;
        // Input recovery must remain available with every module combination.
        std::thread::Builder::new()
            .name("peli-overlay-watchdog".into())
            .spawn(|| loop {
                crate::watchdog();
                std::thread::sleep(std::time::Duration::from_millis(10));
            })
            .map_err(|e| e.to_string())?;
        if !tools.mako_error.is_empty() {
            crate::report(&tools.mako_error);
        }
        Ok(tools)
    }

    pub(crate) fn draw(&self, context: &egui::Context, extent: egui::Vec2) {
        let Some(controller) = self.mako.as_ref() else {
            crate::panel::show_tools(context, extent, None, &self.mako_error);
            return;
        };
        let Ok((mut controls, error)) = controller.current.lock().map(|s| s.clone()) else {
            crate::panel::show_tools(context, extent, None, "Controles do MAKO indisponíveis.");
            return;
        };
        if crate::panel::show_tools(context, extent, Some(&mut controls), &error).inner {
            if controller.sender.try_send(controls.clone()).is_ok() {
                if let Ok(mut state) = controller.current.lock() {
                    *state = (controls, String::new());
                }
            } else if let Ok(mut state) = controller.current.lock() {
                state.1 = "Aguarde a aplicação dos controles.".into();
            }
        }
    }
}

/// Common event dispatch; modules do not need their own input hooks.
pub(crate) fn focus(focused: bool) {
    crate::focus_fps::focus(focused);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_without_mako_does_not_initialize_frame_generation() {
        let tools = Tools::load(true, false, || panic!("MAKO is not requested")).unwrap();
        assert!(tools.mako.is_none());
        assert!(tools.mako_error.is_empty());
    }
    #[test]
    fn failed_mako_module_keeps_general_tools_available() {
        let tools = Tools::load(true, true, || Err("Perfil inválido".into())).unwrap();
        assert!(tools.mako.is_none());
        assert!(tools.mako_error.contains("Perfil inválido"));
        assert!(Tools::load(false, true, || Err("Perfil inválido".into())).is_err());
    }
    #[test]
    fn disabled_host_does_not_initialize_modules() {
        assert!(Tools::load(false, false, || panic!("disabled")).is_err());
    }
}
