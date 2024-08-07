use anyhow::{bail, Result};
use base64::Engine;
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Colors, SetColors},
};
use std::{io::Write, sync::Arc};

use ratatui::{buffer::Buffer, layout::Rect, style::Color};

use crate::{
    utils::{
        image_proto::{get_image_size, jpg_encode, resize_image},
        macros::try_cont,
        tmux,
    },
    AppEvent,
};

use super::ImageProto;

#[derive(Debug)]
struct EncodedData {
    content: String,
    size: usize,
    width: u32,
    height: u32,
}

#[derive(Debug)]
pub struct Iterm2 {
    image_data_to_encode: Arc<Vec<u8>>,
    encoded_data: Option<EncodedData>,
    default_art: Arc<Vec<u8>>,
    sender: std::sync::mpsc::Sender<(u16, u16, bool, Arc<Vec<u8>>)>,
    encoded_data_receiver: std::sync::mpsc::Receiver<EncodedData>,
    state: State,
}

#[derive(Debug)]
enum State {
    Initial,
    Resize,
    Rerender,
    Encoding,
    Showing,
    Encoded,
}

impl ImageProto for Iterm2 {
    fn render(
        &mut self,
        Rect {
            x: _,
            y: _,
            width,
            height,
        }: Rect,
    ) -> Result<()> {
        match self.state {
            State::Initial => {
                self.sender
                    .send((width, height, false, Arc::clone(&self.image_data_to_encode)))?;
                self.state = State::Encoding;
            }
            State::Resize => {
                self.sender
                    .send((width, height, true, Arc::clone(&self.image_data_to_encode)))?;
                self.state = State::Encoding;
            }
            _ => {
                if let Ok(data) = self.encoded_data_receiver.try_recv() {
                    self.encoded_data = Some(data);
                    self.state = State::Encoded;
                }
            }
        }
        Ok(())
    }

    fn post_render(
        &mut self,
        buf: &mut Buffer,
        bg_color: Option<Color>,
        Rect { x, y, width, height }: Rect,
    ) -> Result<()> {
        if !matches!(self.state, State::Encoded | State::Rerender) {
            return Ok(());
        }

        if let Some(data) = &self.encoded_data {
            self.clear_area(bg_color, Rect { x, y, width, height })?;
            self.skip_area(buf, x, y, width, height);

            let mut stdout = std::io::stdout();
            queue!(stdout, SavePosition)?;
            queue!(stdout, MoveTo(x, y))?;
            let EncodedData {
                content,
                size,
                width,
                height,
            } = data;
            if tmux::is_inside_tmux() {
                write!(stdout, "{}", &format!("\x1bPtmux;\x1b\x1b]1337;File=inline=1;size={size};width={width}px;height={height}px;preserveAspectRatio=1;doNotMoveCursor=1:{content}\x07\x1b\\"))?;
            } else {
                write!(stdout, "{}", &format!("\x1b]1337;File=inline=1;size={size};width={width}px;height={height}px;preserveAspectRatio=1;doNotMoveCursor=1:{content}\x07"))?;
            }
            queue!(stdout, RestorePosition)?;

            self.state = State::Showing;
        };
        Ok(())
    }

    fn hide(&mut self, bg_color: Option<Color>, size: Rect) -> Result<()> {
        self.clear_area(bg_color, size)?;
        Ok(())
    }

    fn show(&mut self) {
        if self.encoded_data.is_some() {
            self.state = State::Rerender;
        } else {
            self.state = State::Initial;
        }
    }

    fn resize(&mut self) {
        self.state = State::Resize;
    }

    fn set_data(&mut self, data: Option<Vec<u8>>) {
        if let Some(data) = data {
            self.image_data_to_encode = Arc::new(data);
        } else {
            self.image_data_to_encode = Arc::clone(&self.default_art);
        }

        self.state = State::Initial;
        self.encoded_data = None;
    }
}

impl Iterm2 {
    pub fn new(app_event_sender: std::sync::mpsc::Sender<AppEvent>, default_art: &[u8]) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<(u16, u16, bool, Arc<Vec<u8>>)>();
        let (encoded_tx, encoded_rx) = std::sync::mpsc::channel::<EncodedData>();

        std::thread::spawn(move || loop {
            if let Ok((w, h, full_render, data)) = receiver.recv() {
                let encoded = try_cont!(Iterm2::encode(w, h, &data), "Failed to encode data");
                try_cont!(encoded_tx.send(encoded), "Failed to send encoded data");
                try_cont!(
                    app_event_sender.send(AppEvent::RequestRender(full_render)),
                    "Failed to request render"
                );
            }
        });
        let default_art = Arc::new(default_art.to_vec());

        Self {
            image_data_to_encode: Arc::clone(&default_art),
            default_art,
            encoded_data: None,
            sender,
            encoded_data_receiver: encoded_rx,
            state: State::Initial,
        }
    }

    fn encode(width: u16, height: u16, data: &[u8]) -> Result<EncodedData> {
        let (iwidth, iheight) = match get_image_size(width.into(), height.into()) {
            Ok(v) => v,
            Err(err) => {
                bail!("Failed to get image size, err: {}", err);
            }
        };
        let image = match resize_image(data, iwidth, iheight) {
            Ok(v) => v,
            Err(err) => {
                bail!("Failed to resize image, err: {}", err);
            }
        };
        let Ok(jpg) = jpg_encode(&image) else {
            bail!("Failed to encode image as jpg")
        };

        let content = base64::engine::general_purpose::STANDARD.encode(&jpg);

        Ok(EncodedData {
            content,
            size: jpg.len(),
            width: image.width(),
            height: image.height(),
        })
    }

    fn skip_area(&self, buf: &mut Buffer, x: u16, y: u16, width: u16, height: u16) {
        for y in y..y + height {
            for x in x..x + width {
                buf.get_mut(x, y).set_skip(true).reset();
            }
        }
    }

    fn clear_area(&self, bg_color: Option<Color>, Rect { x, y, width, height }: Rect) -> Result<()> {
        let mut stdout = std::io::stdout();
        queue!(stdout, SavePosition)?;

        let set_color = SetColors(Colors {
            background: bg_color.map(|c| c.into()),
            foreground: None,
        });
        for y in y..y + height {
            for x in x..x + width {
                queue!(stdout, MoveTo(x, y))?;
                queue!(stdout, set_color)?;
                write!(stdout, " ")?;
            }
        }
        queue!(stdout, RestorePosition)?;
        Ok(())
    }
}
