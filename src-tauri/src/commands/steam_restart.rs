//! Bounded restart lifecycle, independently testable without touching Steam.
use std::time::Duration;

pub(super) trait Client {
    fn running(&mut self) -> Result<bool, String>;
    fn games_running(&mut self) -> Result<bool, String>;
    fn request_shutdown(&mut self) -> Result<(), String>;
    /// Only the originally observed, owned Steam client may receive a signal.
    fn terminate(&mut self, force: bool) -> Result<(), String>;
    fn pause(&mut self, duration: Duration);
}

fn wait(client: &mut impl Client, seconds: u32) -> Result<bool, String> {
    for _ in 0..seconds * 4 {
        if !client.running()? { return Ok(true); }
        client.pause(Duration::from_millis(250));
    }
    Ok(!client.running()?)
}

pub(super) fn stop(client: &mut impl Client, progress: &mut impl FnMut(&str)) -> Result<(), String> {
    progress("checking");
    if !client.running()? { return Ok(()); }
    if client.games_running()? {
        return Err("Feche os jogos da Steam antes de aplicar automaticamente. Suas alterações continuam pendentes.".into());
    }
    progress("closing");
    // A failed control command does not justify writing while Steam is alive.
    // The bounded fallback can still close a verified client safely.
    let shutdown_error = client.request_shutdown().err();
    if shutdown_error.is_none() && wait(client, 8)? { return Ok(()); }
    for (force, seconds, phase) in [(false, 4, "terminating"), (true, 3, "finishing")] {
        if !client.running()? { return Ok(()); }
        if client.games_running()? {
            return Err("Um jogo da Steam está em execução. O reinício foi interrompido e as alterações continuam pendentes.".into());
        }
        progress(phase);
        client.terminate(force)?;
        if wait(client, seconds)? { return Ok(()); }
    }
    Err("O cliente da Steam não encerrou. Nenhuma configuração foi aplicada; suas alterações continuam pendentes. Feche a Steam e tente novamente, ou use a configuração manual.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake { alive: bool, game: bool, requests: usize, signals: Vec<bool>, ticks: u32, exit_after: Option<u32>, exit_on: Option<bool>, game_after: Option<u32> }
    impl Default for Fake { fn default() -> Self { Self {alive:true,game:false,requests:0,signals:vec![],ticks:0,exit_after:None,exit_on:None,game_after:None} } }
    impl Client for Fake {
        fn running(&mut self)->Result<bool,String>{Ok(self.alive)}
        fn games_running(&mut self)->Result<bool,String>{Ok(self.game || self.game_after.is_some_and(|t|self.ticks>=t))}
        fn request_shutdown(&mut self)->Result<(),String>{self.requests+=1;Ok(())}
        fn terminate(&mut self,force:bool)->Result<(),String>{self.signals.push(force);if self.exit_on==Some(force){self.alive=false;}Ok(())}
        fn pause(&mut self,_:Duration){self.ticks+=1;if self.exit_after.is_some_and(|t|self.ticks>=t){self.alive=false;}}
    }
    #[test] fn closed_or_cooperative_clients_need_no_signals() {
        let mut closed=Fake{alive:false,..Default::default()};stop(&mut closed,&mut |_|{}).unwrap();assert_eq!(closed.requests,0);
        let mut normal=Fake{exit_after:Some(2),..Default::default()};stop(&mut normal,&mut |_|{}).unwrap();assert_eq!(normal.requests,1);assert!(normal.signals.is_empty());
    }
    #[test] fn stuck_client_gets_bounded_term_then_kill() {
        let mut client=Fake{exit_on:Some(true),..Default::default()};let mut stages=vec![];
        stop(&mut client,&mut |s|stages.push(s.to_owned())).unwrap();
        assert_eq!(client.signals,[false,true]);assert_eq!(client.ticks,48);assert_eq!(stages,["checking","closing","terminating","finishing"]);
    }
    #[test] fn term_is_sufficient_when_client_responds() {
        let mut client=Fake{exit_on:Some(false),..Default::default()};stop(&mut client,&mut |_|{}).unwrap();assert_eq!(client.signals,[false]);
    }
    #[test] fn games_block_shutdown_and_appearing_games_block_escalation() {
        let mut client=Fake{game:true,..Default::default()};assert!(stop(&mut client,&mut |_|{}).is_err());assert_eq!(client.requests,0);assert!(client.signals.is_empty());
        let mut client=Fake{game_after:Some(32),..Default::default()};assert!(stop(&mut client,&mut |_|{}).is_err());assert!(client.signals.is_empty());
    }
    #[test] fn unkillable_client_times_out_without_claiming_success() {
        let mut client=Fake::default();assert!(stop(&mut client,&mut |_|{}).is_err());assert_eq!(client.ticks,60);
    }
}
