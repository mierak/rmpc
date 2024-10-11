use anyhow::{Context, Result};
use itertools::Itertools;
use std::{
    io::Write,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    time::Instant,
};

use base64::Engine;
use flate2::Compression;
use ratatui::prelude::{Buffer, Rect};

use crate::{
    config::Size,
    utils::{
        image_proto::{get_gif_frames, get_image_area_size_px, resize_image},
        macros::status_error,
        mpsc::RecvLast,
        tmux,
    },
};

use super::ImageProto;

#[derive(Debug)]
pub struct KittyImageState {
    idx: u32,
    image: Arc<Vec<u8>>,
    default_art: Arc<Vec<u8>>,
    needs_transfer: bool,
    transfer_request_channel: Sender<(Arc<Vec<u8>>, u16, u16)>,
    compression_finished_receiver: Receiver<Data>,
}

impl ImageProto for KittyImageState {
    fn render(&mut self, buf: &mut Buffer, rect: Rect) -> Result<()> {
        let state = self;
        let height = rect.height;
        let width = rect.width;

        if state.needs_transfer {
            state.needs_transfer = false;

            let delete_all_images = "\x1b_Ga=D\x1b\\";
            if tmux::is_inside_tmux() {
                tmux::wrap_print(delete_all_images);
            } else {
                print!("{delete_all_images}");
            }

            if let Err(err) = state
                .transfer_request_channel
                .send((Arc::clone(&state.image), width, height))
            {
                status_error!(err:?; "Failed to compress image data");
            }
        }

        if let Ok(data) = state.compression_finished_receiver.try_recv() {
            state.idx = state.idx.wrapping_add(1);
            match data {
                Data::ImageData(data) => {
                    transfer_image_data(&data.content, width, height, data.img_width, data.img_height, state);
                }
                Data::AnimationData(data) => transfer_animation_data(data, width, height, state),
            }
        }

        create_unicode_placeholder_grid(state, buf, rect);
        Ok(())
    }

    fn post_render(&mut self, _: &mut Buffer, _: Option<ratatui::prelude::Color>, _: Rect) -> Result<()> {
        Ok(())
    }

    fn hide(&mut self, _: Option<ratatui::prelude::Color>, _: Rect) -> Result<()> {
        Ok(())
    }

    fn show(&mut self) {
        self.needs_transfer = true;
    }

    fn resize(&mut self) {
        self.needs_transfer = true;
    }

    fn set_data(&mut self, data: Option<Vec<u8>>) -> Result<()> {
        if let Some(data) = data {
            self.image = Arc::new(data);
        } else {
            self.image = Arc::clone(&self.default_art);
        }
        log::debug!(bytes = self.image.len(); "New image received",);
        self.needs_transfer = true;

        Ok(())
    }
}

impl KittyImageState {
    pub fn new(default_art: &'static [u8], max_size: Size, request_render: impl Fn(bool) + Send + 'static) -> Self {
        let compression_request_channel = channel::<(Arc<Vec<_>>, u16, u16)>();
        let rx = compression_request_channel.1;

        let image_data_to_transfer_channel = channel::<Data>();
        let data_sender = image_data_to_transfer_channel.0;

        std::thread::spawn(move || {
            while let Ok((vec, width, height)) = rx.recv_last() {
                let data = match create_data_to_transfer(&vec, width, height, Compression::new(6), max_size) {
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

                request_render(false);
            }
        });

        let default_art = Arc::new(default_art.to_vec());
        Self {
            idx: 0,
            needs_transfer: true,
            image: Arc::clone(&default_art),
            transfer_request_channel: compression_request_channel.0,
            compression_finished_receiver: image_data_to_transfer_channel.1,
            default_art,
        }
    }
}

fn create_data_to_transfer(
    image_data: &[u8],
    width: u16,
    height: u16,
    compression: Compression,
    max_size: Size,
) -> Result<Data> {
    let start_time = Instant::now();
    log::debug!(bytes = image_data.len(); "Compressing image data");
    let (w, h) = get_image_area_size_px(width, height, max_size)?;

    if let Some(data) = get_gif_frames(image_data)? {
        let frames = data.frames;
        let (width, height) = data.dimensions;
        let frames: Vec<AnimationFrame> = frames
            .map_ok(|frame| {
                let delay = frame.delay().numer_denom_ms();

                AnimationFrame {
                    delay: delay.0 / delay.1,
                    content: base64::engine::general_purpose::STANDARD.encode(frame.buffer().as_raw()),
                }
            })
            .try_collect()?;

        Ok(Data::AnimationData(AnimationData {
            frames,
            is_compressed: false,
            img_width: width,
            img_height: height,
        }))
    } else {
        let image = resize_image(image_data, w, h)?;

        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), compression);
        e.write_all(image.to_rgba8().as_raw())
            .context("Error occured when writing image bytes to zlib encoder")?;

        let content = base64::engine::general_purpose::STANDARD.encode(
            e.finish()
                .context("Error occured when flushing image bytes to zlib encoder")?,
        );

        log::debug!(input_bytes = image_data.len(), compressed_bytes = content.len(), duration:? = start_time.elapsed(); "Image data compression finished");
        Ok(Data::ImageData(ImageData {
            content,
            img_width: image.width(),
            img_height: image.height(),
        }))
    }
}

fn create_unicode_placeholder_grid(state: &KittyImageState, buf: &mut Buffer, area: Rect) {
    (0..area.height).for_each(|y| {
        let mut res = format!("\x1b[38;5;{}m", state.idx);

        (0..area.width).for_each(|x| {
            res.push_str(&format!(
                "{DELIM}{row}{col}",
                row = GRID[y as usize],
                col = GRID[x as usize],
            ));

            if x > 0 {
                buf.cell_mut((area.left() + x, area.top() + y))
                    .map(|cell| cell.set_skip(true));
            }
        });

        res.push_str("\x1b[39m\n");
        buf.cell_mut((area.left(), area.top() + y))
            .map(|cell| cell.set_symbol(&res));
    });
}

fn transfer_animation_data(data: AnimationData, cols: u16, rows: u16, state: &mut KittyImageState) {
    let start_time = Instant::now();
    let AnimationData {
        frames,
        is_compressed,
        img_width,
        img_height,
    } = data;

    log::debug!(frames = frames.len(), img_width, img_height, rows, cols; "Transferring animation data");

    if frames.len() < 2 {
        log::warn!("Less than two frames, invalid animation data");
        return;
    }

    let mut first_frame_iter = frames[0].content.chars().peekable();
    let chunk: String = first_frame_iter.by_ref().take(4096).collect();

    let m = i32::from(first_frame_iter.peek().is_some());
    let delay = frames[0].delay;

    // Create image and transfer first frame
    let create_img = &format!(
        "\x1b_Gi={},f=32,U=1,a=T,t=d,m={m},z={delay},q=2,s={img_width},v={img_height},c={cols},r={rows}{compression};{chunk}\x1b\\",
        state.idx, compression = if is_compressed { ",o=z" } else { ""} 
    );
    tmux::wrap_print_if_needed(create_img);

    // Transfer the rest of the first frame if any
    while first_frame_iter.peek().is_some() {
        let chunk: String = first_frame_iter.by_ref().take(4096).collect();
        let m = i32::from(first_frame_iter.peek().is_some());
        tmux::wrap_print_if_needed(&format!("\x1b_Gi={},m={m};{chunk}\x1b\\", state.idx));
    }

    // Transfer rest of the frames, skip first because it was already transferred
    for AnimationFrame { delay, content, .. } in frames.iter().skip(1) {
        let mut frame_iter = content.chars().peekable();
        let chunk: String = frame_iter.by_ref().take(4096).collect();

        let next_frame = &format!(
            "\x1b_Gi={},a=f,t=d,m={m},z={delay},q=2,s={img_width},v={img_height}{compression};{chunk}\x1b\\",
            state.idx,
            compression = if is_compressed { ",o=z" } else { "" }
        );

        tmux::wrap_print_if_needed(next_frame);
        while frame_iter.peek().is_some() {
            let chunk: String = frame_iter.by_ref().take(4096).collect();
            let m = i32::from(frame_iter.peek().is_some());
            tmux::wrap_print_if_needed(&format!("\x1b_Ga=f,i={},m={m};{chunk}\x1b\\", state.idx));
        }
    }

    // Run the animation
    tmux::wrap_print_if_needed(&format!("\x1b_Ga=a,i={},s=3\x1b\\", state.idx));
    log::debug!(duration:? = start_time.elapsed(); "Transfer finished");
}

fn transfer_image_data(
    content: &str,
    cols: u16,
    rows: u16,
    img_width: u32,
    img_height: u32,
    state: &mut KittyImageState,
) {
    let start_time = Instant::now();
    log::debug!(bytes = content.len(), img_width, img_height, rows, cols; "Transferring compressed image data");
    let mut iter = content.chars().peekable();

    let first: String = iter.by_ref().take(4096).collect();
    let delete_all_images = "\x1b_Ga=D\x1b\\";
    let virtual_image_placement = &format!(
        "\x1b_Gi={},f=32,U=1,t=d,a=T,m=1,q=2,o=z,s={},v={},c={},r={};{}\x1b\\",
        state.idx, img_width, img_height, cols, rows, first
    );

    tmux::wrap_print_if_needed(delete_all_images);
    tmux::wrap_print_if_needed(virtual_image_placement);

    while iter.peek().is_some() {
        let chunk: String = iter.by_ref().take(4096).collect();
        let m = i32::from(iter.peek().is_some());
        tmux::wrap_print_if_needed(&format!("\x1b_Gm={m};{chunk}\x1b\\"));
    }
    log::debug!(duration:? = start_time.elapsed(); "Transfer finished");
}

enum Data {
    ImageData(ImageData),
    AnimationData(AnimationData),
}

struct ImageData {
    content: String,
    img_width: u32,
    img_height: u32,
}

struct AnimationFrame {
    content: String,
    delay: u32,
}

struct AnimationData {
    frames: Vec<AnimationFrame>,
    is_compressed: bool,
    img_width: u32,
    img_height: u32,
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
