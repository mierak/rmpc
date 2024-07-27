use anyhow::{Context, Result};
use std::{
    io::{Cursor, Write},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    time::Instant,
};

use base64::Engine;
use flate2::Compression;
use ratatui::{
    prelude::{Buffer, Rect},
    widgets::{Block, StatefulWidget, Widget},
};

use crate::{
    utils::{macros::status_error, tmux},
    AppEvent,
};

#[derive(Debug)]
pub struct ImageState {
    idx: u32,
    image: Option<Arc<Vec<u8>>>,
    default_art: Arc<Vec<u8>>,
    needs_transfer: bool,
    transfer_request_channel: Sender<(Arc<Vec<u8>>, usize, usize)>,
    compression_finished_receiver: Receiver<Data>,
}

impl ImageState {
    pub fn new(sender: Sender<AppEvent>, default_art: Vec<u8>) -> Self {
        let compression_request_channel = channel::<(Arc<Vec<_>>, usize, usize)>();
        let rx = compression_request_channel.1;

        let image_data_to_transfer_channel = channel::<Data>();
        let data_sender = image_data_to_transfer_channel.0;

        std::thread::spawn(move || {
            while let Ok((vec, width, height)) = rx.recv() {
                let data = match KittyImage::create_data_to_transfer(&vec, width, height, Compression::new(6)) {
                    Ok(data) => data,
                    Err(err) => {
                        status_error!(err:?; "Failed to compress image data");
                        continue;
                    }
                };

                if let Err(err) = data_sender.send(data) {
                    status_error!(err:?; "Failed to send compressed image data");
                    continue;
                }

                if let Err(err) = sender.send(AppEvent::RequestRender) {
                    status_error!(err:?; "Failed to request rerender after image data compression finished");
                    continue;
                }
            }
        });

        Self {
            idx: 0,
            needs_transfer: true,
            image: None,
            transfer_request_channel: compression_request_channel.0,
            compression_finished_receiver: image_data_to_transfer_channel.1,
            default_art: Arc::new(default_art),
        }
    }

    pub fn force_transfer(&mut self) {
        self.needs_transfer = true;
    }
}

impl ImageState {
    /// Takes image data in buffer
    /// Leaves the provided buffer empty if any data were in there
    /// Has to be called on every image change event, not just when
    /// it is currently displayed
    pub fn image(&mut self, image: &mut Option<Vec<u8>>) -> &Self {
        match (image.as_mut(), &mut self.image) {
            (Some(ref mut new_img), None) => {
                self.image = Some(Arc::new(std::mem::take(new_img)));
                self.needs_transfer = true;
                log::debug!(bytes = self.image.as_ref().map(|v| v.len()); "New image received",);
            }
            (Some(new_img), Some(current_img)) if *new_img != **current_img && !new_img.is_empty() => {
                self.image = Some(Arc::new(std::mem::take(new_img)));
                self.needs_transfer = true;
                log::debug!(bytes = self.image.as_ref().map(|v| v.len()); "New image received");
            }
            (Some(new_img), Some(_)) => {
                log::debug!("New image is identical to the current one, ignoring");
                new_img.clear();
            }
            (None, None) => {} // Default img should be in place already
            (None, Some(_)) => {
                // Show default img
                self.image = None;
                self.needs_transfer = true;
            }
        }

        self
    }
}

#[derive(Debug, Default)]
pub struct KittyImage<'a> {
    block: Option<Block<'a>>,
}

struct Data {
    content: String,
    img_width: u32,
    img_height: u32,
}

impl<'a> KittyImage<'a> {
    fn create_data_to_transfer(
        image_data: &[u8],
        width: usize,
        height: usize,
        compression: Compression,
    ) -> Result<Data> {
        let start_time = Instant::now();
        log::debug!(bytes = image_data.len(); "Compressing image data");
        let (w, h) = KittyImage::get_image_size(width, height)?;
        let image = image::io::Reader::new(Cursor::new(image_data))
            .with_guessed_format()
            .context("Unable to guess image format")?
            .decode()
            .context("Unable to decode image")?
            .resize(w, h, image::imageops::FilterType::Lanczos3);

        let binding = image.to_rgba8();
        let rgba = binding.as_raw();

        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), compression);
        e.write_all(rgba)
            .context("Error occured when writing image bytes to zlib encoder")?;

        let content = base64::engine::general_purpose::STANDARD.encode(
            e.finish()
                .context("Error occured when flushing image bytes to zlib encoder")?,
        );

        log::debug!(input_bytes = image_data.len(), compressed_bytes = content.len(), duration:? = start_time.elapsed(); "Image data compression finished");
        Ok(Data {
            content,
            img_width: image.width(),
            img_height: image.height(),
        })
    }

    fn get_image_size(area_width: usize, area_height: usize) -> Result<(u32, u32)> {
        let size = crossterm::terminal::window_size().context("Unable to query terminal size")?;
        let w = if size.width == 0 {
            800
        } else {
            let cell_width = size.width / size.columns;
            (cell_width as usize * area_width) as u32
        };
        let h = if size.height == 0 {
            600
        } else {
            let cell_height = size.height / size.rows;
            (cell_height as usize * area_height) as u32
        };
        Ok((w, h))
    }

    fn create_unicode_placeholder_grid(state: &ImageState, buf: &mut Buffer, area: Rect) {
        (0..area.height).for_each(|y| {
            let mut res = format!("\x1b[38;5;{}m", state.idx);

            (0..area.width).for_each(|x| {
                res.push_str(&format!(
                    "{DELIM}{row}{col}",
                    row = GRID[y as usize],
                    col = GRID[x as usize],
                ));

                if x > 0 {
                    buf.get_mut(area.left() + x, area.top() + y).set_skip(true);
                }
            });

            res.push_str("\x1b[39m\n");
            buf.get_mut(area.left(), area.top() + y).set_symbol(&res);
        });
    }

    fn transfer_data(content: &str, cols: usize, rows: usize, img_width: u32, img_height: u32, state: &mut ImageState) {
        let start_time = Instant::now();
        log::debug!(bytes = content.len(); "Transferring compressed image data");
        let mut iter = content.chars().peekable();

        let first: String = iter.by_ref().take(4096).collect();
        let delete_all_images = "\x1b_Ga=D\x1b\\";
        let virtual_image_placement = &format!(
            "\x1b_Gi={},f=32,U=1,t=d,a=T,m=1,q=2,o=z,s={},v={},c={},r={};{}\x1b\\",
            state.idx, img_width, img_height, cols, rows, first
        );

        if tmux::is_inside_tmux() {
            log::trace!("Using tmux wrap");
            tmux::wrap_print(delete_all_images);
            tmux::wrap_print(virtual_image_placement);

            while iter.peek().is_some() {
                let chunk: String = iter.by_ref().take(4096).collect();
                let m = i32::from(iter.peek().is_some());
                tmux::wrap_print(&format!("\x1b_Gm={m};{chunk}\x1b\\"));
            }
        } else {
            print!("{delete_all_images}");
            print!("{virtual_image_placement}");

            while iter.peek().is_some() {
                let chunk: String = iter.by_ref().take(4096).collect();
                let m = i32::from(iter.peek().is_some());
                print!("\x1b_Gm={m};{chunk}\x1b\\");
            }
        }
        log::debug!(duration:? = start_time.elapsed(); "Transfer finished");
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> StatefulWidget for KittyImage<'a> {
    type State = ImageState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        let image = match &state.image {
            None => Arc::clone(&state.default_art),
            Some(data) if data.is_empty() => Arc::clone(&state.default_art),
            Some(data) => Arc::clone(data),
        };

        let height = area.height as usize;
        let width = area.width as usize;

        if state.needs_transfer {
            state.needs_transfer = false;

            let delete_all_images = "\x1b_Ga=D\x1b\\";
            if tmux::is_inside_tmux() {
                tmux::wrap_print(delete_all_images);
            } else {
                print!("{delete_all_images}");
            }

            if let Err(err) = state.transfer_request_channel.send((image, width, height)) {
                status_error!(err:?; "Failed to compress image data");
            }
        }

        if let Ok(data) = state.compression_finished_receiver.try_recv() {
            state.idx = state.idx.wrapping_add(1);
            KittyImage::transfer_data(&data.content, width, height, data.img_width, data.img_height, state);
        }

        KittyImage::create_unicode_placeholder_grid(state, buf, area);
    }
}

const DELIM: &str = "\u{10EEEE}";
const GRID: &[&str] = &[
    "\u{0305}",
    "\u{030D}",
    "\u{030E}",
    "\u{0310}",
    "\u{0312}",
    "\u{033D}",
    "\u{033E}",
    "\u{033F}",
    "\u{0346}",
    "\u{034A}",
    "\u{034B}",
    "\u{034C}",
    "\u{0350}",
    "\u{0351}",
    "\u{0352}",
    "\u{0357}",
    "\u{035B}",
    "\u{0363}",
    "\u{0364}",
    "\u{0365}",
    "\u{0366}",
    "\u{0367}",
    "\u{0368}",
    "\u{0369}",
    "\u{036A}",
    "\u{036B}",
    "\u{036C}",
    "\u{036D}",
    "\u{036E}",
    "\u{036F}",
    "\u{0483}",
    "\u{0484}",
    "\u{0485}",
    "\u{0486}",
    "\u{0487}",
    "\u{0592}",
    "\u{0593}",
    "\u{0594}",
    "\u{0595}",
    "\u{0597}",
    "\u{0598}",
    "\u{0599}",
    "\u{059C}",
    "\u{059D}",
    "\u{059E}",
    "\u{059F}",
    "\u{05A0}",
    "\u{05A1}",
    "\u{05A8}",
    "\u{05A9}",
    "\u{05AB}",
    "\u{05AC}",
    "\u{05AF}",
    "\u{05C4}",
    "\u{0610}",
    "\u{0611}",
    "\u{0612}",
    "\u{0613}",
    "\u{0614}",
    "\u{0615}",
    "\u{0616}",
    "\u{0617}",
    "\u{0657}",
    "\u{0658}",
    "\u{0659}",
    "\u{065A}",
    "\u{065B}",
    "\u{065D}",
    "\u{065E}",
    "\u{06D6}",
    "\u{06D7}",
    "\u{06D8}",
    "\u{06D9}",
    "\u{06DA}",
    "\u{06DB}",
    "\u{06DC}",
    "\u{06DF}",
    "\u{06E0}",
    "\u{06E1}",
    "\u{06E2}",
    "\u{06E4}",
    "\u{06E7}",
    "\u{06E8}",
    "\u{06EB}",
    "\u{06EC}",
    "\u{0730}",
    "\u{0732}",
    "\u{0733}",
    "\u{0735}",
    "\u{0736}",
    "\u{073A}",
    "\u{073D}",
    "\u{073F}",
    "\u{0740}",
    "\u{0741}",
    "\u{0743}",
    "\u{0745}",
    "\u{0747}",
    "\u{0749}",
    "\u{074A}",
    "\u{07EB}",
    "\u{07EC}",
    "\u{07ED}",
    "\u{07EE}",
    "\u{07EF}",
    "\u{07F0}",
    "\u{07F1}",
    "\u{07F3}",
    "\u{0816}",
    "\u{0817}",
    "\u{0818}",
    "\u{0819}",
    "\u{081B}",
    "\u{081C}",
    "\u{081D}",
    "\u{081E}",
    "\u{081F}",
    "\u{0820}",
    "\u{0821}",
    "\u{0822}",
    "\u{0823}",
    "\u{0825}",
    "\u{0826}",
    "\u{0827}",
    "\u{0829}",
    "\u{082A}",
    "\u{082B}",
    "\u{082C}",
    "\u{082D}",
    "\u{0951}",
    "\u{0953}",
    "\u{0954}",
    "\u{0F82}",
    "\u{0F83}",
    "\u{0F86}",
    "\u{0F87}",
    "\u{135D}",
    "\u{135E}",
    "\u{135F}",
    "\u{17DD}",
    "\u{193A}",
    "\u{1A17}",
    "\u{1A75}",
    "\u{1A76}",
    "\u{1A77}",
    "\u{1A78}",
    "\u{1A79}",
    "\u{1A7A}",
    "\u{1A7B}",
    "\u{1A7C}",
    "\u{1B6B}",
    "\u{1B6D}",
    "\u{1B6E}",
    "\u{1B6F}",
    "\u{1B70}",
    "\u{1B71}",
    "\u{1B72}",
    "\u{1B73}",
    "\u{1CD0}",
    "\u{1CD1}",
    "\u{1CD2}",
    "\u{1CDA}",
    "\u{1CDB}",
    "\u{1CE0}",
    "\u{1DC0}",
    "\u{1DC1}",
    "\u{1DC3}",
    "\u{1DC4}",
    "\u{1DC5}",
    "\u{1DC6}",
    "\u{1DC7}",
    "\u{1DC8}",
    "\u{1DC9}",
    "\u{1DCB}",
    "\u{1DCC}",
    "\u{1DD1}",
    "\u{1DD2}",
    "\u{1DD3}",
    "\u{1DD4}",
    "\u{1DD5}",
    "\u{1DD6}",
    "\u{1DD7}",
    "\u{1DD8}",
    "\u{1DD9}",
    "\u{1DDA}",
    "\u{1DDB}",
    "\u{1DDC}",
    "\u{1DDD}",
    "\u{1DDE}",
    "\u{1DDF}",
    "\u{1DE0}",
    "\u{1DE1}",
    "\u{1DE2}",
    "\u{1DE3}",
    "\u{1DE4}",
    "\u{1DE5}",
    "\u{1DE6}",
    "\u{1DFE}",
    "\u{20D0}",
    "\u{20D1}",
    "\u{20D4}",
    "\u{20D5}",
    "\u{20D6}",
    "\u{20D7}",
    "\u{20DB}",
    "\u{20DC}",
    "\u{20E1}",
    "\u{20E7}",
    "\u{20E9}",
    "\u{20F0}",
    "\u{2CEF}",
    "\u{2CF0}",
    "\u{2CF1}",
    "\u{2DE0}",
    "\u{2DE1}",
    "\u{2DE2}",
    "\u{2DE3}",
    "\u{2DE4}",
    "\u{2DE5}",
    "\u{2DE6}",
    "\u{2DE7}",
    "\u{2DE8}",
    "\u{2DE9}",
    "\u{2DEA}",
    "\u{2DEB}",
    "\u{2DEC}",
    "\u{2DED}",
    "\u{2DEE}",
    "\u{2DEF}",
    "\u{2DF0}",
    "\u{2DF1}",
    "\u{2DF2}",
    "\u{2DF3}",
    "\u{2DF4}",
    "\u{2DF5}",
    "\u{2DF6}",
    "\u{2DF7}",
    "\u{2DF8}",
    "\u{2DF9}",
    "\u{2DFA}",
    "\u{2DFB}",
    "\u{2DFC}",
    "\u{2DFD}",
    "\u{2DFE}",
    "\u{2DFF}",
    "\u{A66F}",
    "\u{A67C}",
    "\u{A67D}",
    "\u{A6F0}",
    "\u{A6F1}",
    "\u{A8E0}",
    "\u{A8E1}",
    "\u{A8E2}",
    "\u{A8E3}",
    "\u{A8E4}",
    "\u{A8E5}",
    "\u{A8E6}",
    "\u{A8E7}",
    "\u{A8E8}",
    "\u{A8E9}",
    "\u{A8EA}",
    "\u{A8EB}",
    "\u{A8EC}",
    "\u{A8ED}",
    "\u{A8EE}",
    "\u{A8EF}",
    "\u{A8F0}",
    "\u{A8F1}",
    "\u{AAB0}",
    "\u{AAB2}",
    "\u{AAB3}",
    "\u{AAB7}",
    "\u{AAB8}",
    "\u{AABE}",
    "\u{AABF}",
    "\u{AAC1}",
    "\u{FE20}",
    "\u{FE21}",
    "\u{FE22}",
    "\u{FE23}",
    "\u{FE24}",
    "\u{FE25}",
    "\u{FE26}",
    "\u{10A0F}",
    "\u{10A38}",
    "\u{1D185}",
    "\u{1D186}",
    "\u{1D187}",
    "\u{1D188}",
    "\u{1D189}",
    "\u{1D1AA}",
    "\u{1D1AB}",
    "\u{1D1AC}",
    "\u{1D1AD}",
    "\u{1D242}",
    "\u{1D243}",
    "\u{1D244}",
];
