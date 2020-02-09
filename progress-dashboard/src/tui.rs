use crate::tree::TreeRoot;
use futures_timer::Delay;

use futures::channel::mpsc;
use futures::{channel::oneshot, future::select, future::Either, SinkExt, StreamExt};
use std::io::Write;
use std::{io, time::Duration};
use termion::event::Key;
use termion::{input::TermRead, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    widgets::{Block, Borders, Widget},
    Terminal,
};

#[derive(Clone)]
pub struct Config {
    pub frames_per_second: u8,
}

pub fn render(
    _progress: TreeRoot,
    Config { frames_per_second }: Config,
    mut should_stop: oneshot::Receiver<()>,
) -> Result<(impl std::future::Future<Output = ()>, oneshot::Receiver<()>), std::io::Error> {
    let mut terminal = {
        let stdout = io::stdout().into_raw_mode()?;
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        Terminal::new(backend)?
    };

    let duration_per_frame = Duration::from_secs(1) / frames_per_second as u32;
    let (send_gui_aborted, receive_gui_aborted) = oneshot::channel::<()>();
    let (mut key_send, mut key_receive) = mpsc::channel::<Key>(1);

    // This brings blocking key-handling into the async world
    std::thread::spawn(move || -> Result<(), io::Error> {
        for key in io::stdin().keys() {
            let key = key?;
            futures::executor::block_on(key_send.send(key)).ok();
        }
        Ok(())
    });

    let render_fut = async move {
        loop {
            terminal
                .draw(|mut f| {
                    let size = f.size();
                    Block::default()
                        .title("Block")
                        .borders(Borders::ALL)
                        .render(&mut f, size);
                })
                .ok();
            let delay = Delay::new(duration_per_frame);
            match select(delay, select(key_receive.next(), &mut should_stop)).await {
                Either::Left(_delay_timed_out) => continue,
                Either::Right((Either::Left((Some(key), _should_stop)), _delay)) => match key {
                    Key::Esc | Key::Ctrl('c') | Key::Ctrl('[') => {
                        send_gui_aborted.send(()).ok();
                        drop(terminal);
                        io::stdout().flush().ok();
                        return ();
                    }
                    _ => continue,
                },
                Either::Right((Either::Right((Ok(()), _key)), _delay)) => {
                    drop(terminal);
                    io::stdout().flush().ok();
                    return ();
                }
                _ => continue,
            };
        }
    };
    Ok((render_fut, receive_gui_aborted))
}
