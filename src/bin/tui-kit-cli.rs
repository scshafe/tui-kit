use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{bail, Result};
use tui_kit::events::{AppEvent, TerminalEvent};
use tui_kit::input::InputEvent;
use tui_kit::input_thread;
use tui_kit::remote::{
    parse_run_spec, RemoteInputWriter, RemoteSession, RemoteSessionEvent, RemoteStderrPolicy,
    SshRemoteTransport,
};
use tui_kit::terminal::{Terminal, TerminalConfig};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let spec = parse_run_spec(&args)?;
    let (transport, remote_input) = SshRemoteTransport::spawn(&spec, RemoteStderrPolicy::Inherit)?;
    let mut remote_input = Some(remote_input);
    let mut session = RemoteSession::new(transport);
    let mut terminal = None;
    let mut input_writer = None;
    let mut final_status = None;

    loop {
        match session.next_event()? {
            RemoteSessionEvent::Frame(frame) => {
                if terminal.is_none() {
                    terminal = Some(Terminal::enter_with_config(
                        TerminalConfig::strict_wezterm_kitty(),
                    )?);
                    let writer = remote_input
                        .take()
                        .ok_or_else(|| anyhow::anyhow!("remote input writer already started"))?;
                    input_writer = Some(spawn_input_writer(writer));
                }
                terminal
                    .as_mut()
                    .expect("terminal initialized before frame render")
                    .render_frame(&frame)?;
            }
            RemoteSessionEvent::Passthrough(line) => {
                log::warn!("remote passthrough output before frame: {line}");
            }
            RemoteSessionEvent::Eof => break,
            RemoteSessionEvent::ChildExited(status) => {
                final_status = Some(status);
                break;
            }
        }
    }

    let status = match final_status {
        Some(status) => status,
        None => session.wait_for_exit()?,
    };
    let input_writer_result = input_writer.map(InputWriterHandle::stop_and_join);
    if !status.is_success() {
        bail!("ssh child exited with status {status}");
    }
    if let Some(result) = input_writer_result {
        result?;
    }
    Ok(())
}

struct InputWriterHandle {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Result<()>>,
}

impl InputWriterHandle {
    fn stop_and_join(self) -> Result<()> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle
            .join()
            .map_err(|_| anyhow::anyhow!("remote input writer thread panicked"))?
    }
}

fn spawn_input_writer<W>(mut remote_input: W) -> InputWriterHandle
where
    W: RemoteInputWriter + Send + 'static,
{
    let (input_tx, input_rx) = mpsc::channel();
    let _input_thread = input_thread::spawn(input_tx);
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || -> Result<()> {
        while !thread_stop.load(Ordering::SeqCst) {
            let event = match input_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let input = match event {
                AppEvent::Input(input) => input,
                AppEvent::Terminal(TerminalEvent::Resize { cols, rows }) => {
                    InputEvent::Resize { cols, rows }
                }
                _ => continue,
            };
            remote_input.write_input_event(input)?;
        }
        Ok(())
    });
    InputWriterHandle { stop, handle }
}
