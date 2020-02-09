use crate::tree::TreeRoot;
use futures_timer::Delay;

use futures::channel::mpsc;
use futures::{future::select, future::Either, SinkExt, StreamExt};
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
) -> Result<impl std::future::Future<Output = ()>, std::io::Error> {
    let mut terminal = {
        let stdout = io::stdout().into_raw_mode()?;
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        Terminal::new(backend)?
    };

    let duration_per_frame = Duration::from_secs(1) / frames_per_second as u32;
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
                    let mut progress_pane = Block::default()
                        .title("Progress Tree")
                        .borders(Borders::ALL);
                    progress_pane.render(&mut f, size);
                    let _entries_rect = progress_pane.inner(size);
                    //                    for (tree_id, progress) in progress.sorted_snapshot().into_iter() {}
                })
                .ok();
            let delay = Delay::new(duration_per_frame);
            match select(delay, key_receive.next()).await {
                Either::Left(_delay_timed_out) => continue,
                Either::Right((Some(key), _delay)) => match key {
                    Key::Esc | Key::Ctrl('c') | Key::Ctrl('[') => {
                        return ();
                    }
                    _ => continue,
                },
                _ => continue,
            };
        }
    };
    Ok(render_fut)
}
