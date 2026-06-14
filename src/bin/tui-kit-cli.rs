use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use anyhow::{bail, Context, Result};
use tui_kit::events::{AppEvent, TerminalEvent};
use tui_kit::input::InputEvent;
use tui_kit::input_thread;
use tui_kit::remote::{
    encode_input_event, parse_run_spec, remote_ssh_args, RemoteFrameDecoder, RemoteOutput,
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
    let ssh_args = remote_ssh_args(&spec);

    let mut child = Command::new("ssh")
        .args(&ssh_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn ssh for {}", spec.host))?;

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture ssh stdin"))?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture ssh stdout"))?;

    let mut terminal = Terminal::enter_with_config(TerminalConfig::strict_wezterm_kitty())?;
    let (input_tx, input_rx) = mpsc::channel();
    let _input_thread = input_thread::spawn(input_tx);
    let _writer = thread::spawn(move || -> Result<()> {
        for event in input_rx {
            let input = match event {
                AppEvent::Input(input) => input,
                AppEvent::Terminal(TerminalEvent::Resize { cols, rows }) => {
                    InputEvent::Resize { cols, rows }
                }
                _ => continue,
            };
            writeln!(child_stdin, "{}", encode_input_event(input))?;
            child_stdin.flush()?;
        }
        Ok(())
    });

    let mut decoder = RemoteFrameDecoder::new();
    for line in BufReader::new(child_stdout).lines() {
        let line = line.context("failed to read remote stdout")?;
        match decoder.push_line(line)? {
            Some(RemoteOutput::Frame(frame)) => terminal.render_frame(&frame)?,
            Some(RemoteOutput::Passthrough(line)) => {
                log::warn!("remote passthrough output before frame: {line}");
            }
            None => {}
        }
    }

    if decoder.is_pending() {
        bail!("remote stream ended before completing a frame");
    }

    let status = child.wait().context("failed waiting for ssh child")?;
    if !status.success() {
        bail!("ssh child exited with status {status}");
    }
    Ok(())
}
