use std::{
    io::Write,
    ops::AddAssign,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    time::Instant,
};

use anyhow::{bail, Result};
use color_quant::NeuQuant;
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Colors, SetColors},
};
use image::{GenericImageView, Rgba};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

use crate::{
    config::Size,
    utils::{
        image_proto::{get_image_area_size_px, resize_image},
        macros::{try_cont, try_skip},
        tmux,
    },
    AppEvent,
};

use super::ImageProto;

#[derive(Debug)]
enum State {
    Initial,
    Resize,
    Rerender,
    Encoding,
    Showing,
    Encoded,
}

#[derive(Debug)]
pub struct Sixel {
    default_art: Arc<Vec<u8>>,
    image_data_to_encode: Arc<Vec<u8>>,
    encoded_data: Option<Vec<u8>>,
    sender: Sender<(u16, u16, bool, Arc<Vec<u8>>)>,
    encoded_data_receiver: Receiver<Vec<u8>>,
    state: State,
}

impl Sixel {
    pub fn new(app_event_sender: Sender<AppEvent>, default_art: &[u8], max_size: Size) -> Self {
        let (sender, receiver) = channel::<(u16, u16, bool, Arc<Vec<u8>>)>();
        let (encoded_tx, encoded_rx) = channel::<Vec<u8>>();

        std::thread::spawn(move || loop {
            if let Ok((width, height, full_render, data)) = receiver.recv() {
                let buf = try_cont!(encode(width, height, &data, max_size), "Failed to encode");

                try_skip!(encoded_tx.send(buf), "Failed to send encoded data");
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

    fn skip_area(&self, buf: &mut Buffer, x: u16, y: u16, width: u16, height: u16) {
        for y in y..y + height {
            for x in x..x + width {
                buf.get_mut(x, y).set_skip(true).reset();
            }
        }
    }
}

fn encode(width: u16, height: u16, data: &[u8], max_size: Size) -> Result<Vec<u8>> {
    let start = Instant::now();

    let (iwidth, iheight) = match get_image_area_size_px(width, height, max_size) {
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

    let tmux = tmux::is_inside_tmux();
    let mut buf = Vec::new();

    if tmux {
        write!(buf, "\x1bPtmux;\x1b\x1bP0;7q\"1;1;{};{}", image.width(), image.height())?;
    } else {
        write!(buf, "\x1bP0;7q\"1;1;{};{}", image.width(), image.height())?;
    }

    let quantized = NeuQuant::new(10, 256, &image.to_rgba8());
    for (i, [r, g, b]) in quantized.color_map_rgb().u16_triples(|v| v * 100 / 255).enumerate() {
        write!(buf, "#{i};2;{r};{g};{b}")?;
    }

    for y in 0..image.height() {
        let character: u8 = 63 + 2u8.pow(y % 6);
        let mut repeat = 0;
        let mut last_color = None;

        for x in 0..image.width() {
            let Rgba(current_pixel) = image.get_pixel(x, y);
            let color = quantized.index_of(&current_pixel);

            if last_color.is_some_and(|c| c == color) || last_color.is_none() {
                repeat.add_assign(1);
                last_color = Some(color);
                continue;
            }

            if repeat == 0 {
                write!(buf, "#{color}{}", character as char)?;
            } else {
                write!(buf, "#{color}!{repeat}{}", character as char)?;
            }

            last_color = Some(color);
            repeat = 1;
        }

        if repeat == 0 {
            write!(buf, "#{}{}", last_color.unwrap_or(0), character as char)?;
        } else {
            write!(buf, "#{}!{repeat}{}", last_color.unwrap_or(0), character as char)?;
        }

        buf.push(if y % 6 == 5 { b'-' } else { b'$' });
    }

    if tmux {
        write!(buf, "\x1b\\\x1b\\")?;
    } else {
        write!(buf, "\x1b\\")?;
    }

    log::debug!(bytes = buf.len(), elapsed:? = start.elapsed(); "encoded data");
    Ok(buf)
}

struct U16Triples {
    data: Vec<u8>,
    current: usize,
    mapfn: fn(u16) -> u16,
}

trait IntoU16Triples {
    fn u16_triples(self, mapfn: fn(u16) -> u16) -> U16Triples;
}

impl IntoU16Triples for Vec<u8> {
    fn u16_triples(self, mapfn: fn(u16) -> u16) -> U16Triples {
        U16Triples {
            data: self,
            current: 0,
            mapfn,
        }
    }
}

impl Iterator for U16Triples {
    type Item = [u16; 3];

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.len() - self.current < 3 {
            return None;
        }
        let a = u16::from(self.data[self.current]);
        self.current += 1;
        let b = u16::from(self.data[self.current]);
        self.current += 1;
        let c = u16::from(self.data[self.current]);
        self.current += 1;

        Some([(self.mapfn)(a), (self.mapfn)(b), (self.mapfn)(c)])
    }
}

impl ImageProto for Sixel {
    fn render(&mut self, _buf: &mut Buffer, Rect { width, height, .. }: Rect) -> anyhow::Result<()> {
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
        buf: &mut ratatui::prelude::Buffer,
        bg_color: Option<ratatui::prelude::Color>,
        rect @ Rect { x, y, width, height }: Rect,
    ) -> anyhow::Result<()> {
        if !matches!(self.state, State::Encoded | State::Rerender) {
            return Ok(());
        }

        if let Some(data) = &self.encoded_data {
            self.clear_area(bg_color, rect)?;
            self.skip_area(buf, x, y, width, height);
            let mut stdout = std::io::stdout();
            queue!(stdout, SavePosition)?;
            queue!(stdout, MoveTo(x, y))?;
            stdout.write_all(data)?;
            stdout.flush()?;
            queue!(stdout, RestorePosition)?;
            self.state = State::Showing;
        }

        Ok(())
    }

    fn hide(&mut self, bg_color: Option<Color>, size: Rect) -> anyhow::Result<()> {
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

    fn set_data(&mut self, data: Option<Vec<u8>>) -> anyhow::Result<()> {
        if let Some(data) = data {
            self.image_data_to_encode = Arc::new(data);
        } else {
            self.image_data_to_encode = Arc::clone(&self.default_art);
        }

        self.state = State::Initial;
        self.encoded_data = None;
        Ok(())
    }
}

// for y in (0..image.height() as usize).step_by(6) {
//     if y + 6 >= image.height() as usize {
//         break;
//     }
//     let mut visited_colors = [false; 256];
//     let mut colors_to_visit: Vec<u8> = vec![
//         color_data[y][0],
//         color_data[y + 1][0],
//         color_data[y + 2][0],
//         color_data[y + 3][0],
//         color_data[y + 4][0],
//         color_data[y + 5][0],
//     ]
//     .into_iter()
//     .sorted()
//     .dedup()
//     .collect();
//
//     while let Some(color_idx) = colors_to_visit.pop() {
//         if visited_colors[color_idx as usize] {
//             continue;
//         }
//         let mut prevchar = b'?' as char;
//         let mut prevcolor = 0;
//         let mut repeat = 0;
//         for x in 0..image.width() as usize {
//             let mut character: u8 = 63;
//
//             let a = color_data[y][x];
//             let b = color_data[y + 1][x];
//             let c = color_data[y + 2][x];
//             let d = color_data[y + 3][x];
//             let e = color_data[y + 4][x];
//             let f = color_data[y + 5][x];
//
//             if a == color_idx {
//                 character += 1;
//             } else if !visited_colors[a as usize] {
//                 colors_to_visit.push(a);
//             }
//             if b == color_idx {
//                 character += 2;
//             } else if !visited_colors[b as usize] {
//                 colors_to_visit.push(b);
//             }
//             if c == color_idx {
//                 character += 4;
//             } else if !visited_colors[c as usize] {
//                 colors_to_visit.push(c);
//             }
//             if d == color_idx {
//                 character += 8;
//             } else if !visited_colors[d as usize] {
//                 colors_to_visit.push(d);
//             }
//             if e == color_idx {
//                 character += 16;
//             } else if !visited_colors[e as usize] {
//                 colors_to_visit.push(e);
//             }
//             if f == color_idx {
//                 character += 32;
//             } else if !visited_colors[f as usize] {
//                 colors_to_visit.push(f);
//             }
//
//             let character = character as char;
//             if (color_idx == prevcolor && prevchar == character) || repeat == 0 {
//                 repeat += 1;
//                 prevchar = character as char;
//                 prevcolor = color_idx;
//                 continue;
//             }
//             visited_colors[color_idx as usize] = true;
//
//             if repeat > 1 {
//                 write!(buf, "#{color_idx}!{repeat}{character}")?;
//             } else {
//                 write!(buf, "#{color_idx}{character}")?;
//             }
//
//             prevchar = character as char;
//             prevcolor = color_idx;
//             repeat = 1;
//         }
//         if repeat > 1 {
//             write!(buf, "#{color_idx}!{repeat}{prevchar}")?;
//         } else {
//             write!(buf, "#{color_idx}{prevchar}")?;
//         }
//         buf.push(b'$');
//     }
//     buf.push(b'-');
// }
