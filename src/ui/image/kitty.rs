use std::{
    io::Write,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use anyhow::{Context, Result};
use base64::Engine;
use crossbeam::channel::{Sender, unbounded};
use crossterm::{
    execute,
    style::{Colors, SetColors},
};
use flate2::Compression;
use itertools::Itertools;
use ratatui::prelude::Rect;

use super::{
    AlbumArtConfig,
    Backend,
    EncodeRequest,
    ImageBackendRequest,
    csi_move,
    facade::IS_SHOWING,
};
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    shared::{
        image::{create_aligned_area, get_gif_frames, resize_image},
        macros::{status_error, try_cont},
        terminal::TERMINAL,
        tmux::tmux_write,
    },
    try_skip,
    ui::image::recv_data,
};

#[derive(Debug)]
pub struct Kitty {
    sender: Sender<ImageBackendRequest>,
    colors: Colors,
    handle: std::thread::JoinHandle<()>,
}

impl Backend for Kitty {
    fn hide(&mut self, area: Rect) -> Result<()> {
        let writer = TERMINAL.writer();
        let mut writer = writer.lock();
        clear_area(writer.by_ref(), self.colors, area)
    }

    fn show(&mut self, data: Arc<Vec<u8>>, area: Rect) -> Result<()> {
        Ok(self.sender.send(ImageBackendRequest::Encode(EncodeRequest { area, data }))?)
    }

    fn cleanup(self: Box<Self>, area: Rect) -> Result<()> {
        let writer = TERMINAL.writer();
        let mut writer = writer.lock();
        clear_area(writer.by_ref(), self.colors, area)?;
        self.sender.send(ImageBackendRequest::Stop)?;
        self.handle.join().expect("kitty thread to end gracefully");
        Ok(())
    }

    fn set_config(&self, config: AlbumArtConfig) -> Result<()> {
        Ok(self.sender.send(ImageBackendRequest::SetConfig(config))?)
    }
}

impl Kitty {
    pub(super) fn new(config: AlbumArtConfig) -> Self {
        let (sender, receiver) = unbounded::<ImageBackendRequest>();
        let colors = config.colors;

        let handle = std::thread::Builder::new()
            .name("kitty".to_string())
            .spawn(move || {
                let mut config = config;
                let mut pending_req: Option<EncodeRequest> = None;
                'outer: loop {
                    let EncodeRequest { data, area } =
                        match recv_data(&mut pending_req, &mut config, &receiver) {
                            Ok(Some(msg)) => msg,
                            Ok(None) => break,
                            Err(err) => {
                                log::error!("Error receiving ImageBackendRequest message: {err}");
                                break;
                            }
                        };

                    let data = match create_data_to_transfer(
                        &data,
                        area,
                        Compression::new(6),
                        config.max_size,
                        config.halign,
                        config.valign,
                    ) {
                        Ok(data) => data,
                        Err(err) => {
                            status_error!(err:?; "Failed to compress image data");
                            continue;
                        }
                    };

                    // consume all pending messages, skipping older encode requests
                    log::debug!("iterating over pending messages");
                    for msg in receiver.try_iter() {
                        match msg {
                            ImageBackendRequest::Stop => break 'outer,
                            ImageBackendRequest::SetConfig(cfg) => config = cfg,
                            ImageBackendRequest::Encode(req) => {
                                pending_req = Some(req);
                                log::debug!(
                                    "Skipping image because another one is waiting in the queue"
                                );
                                continue 'outer;
                            }
                        }
                    }

                    let w = TERMINAL.writer();
                    let mut w = w.lock();
                    let mut w = w.by_ref();
                    if !IS_SHOWING.load(Ordering::Relaxed) {
                        log::trace!(
                            "Not showing image because its not supposed to be displayed anymore"
                        );
                        continue;
                    }

                    try_cont!(
                        clear_area(&mut w, config.colors, area),
                        "Failed to clear kitty image area"
                    );
                    match data {
                        Data::ImageData(data) => {
                            try_cont!(
                                transfer_image_data(
                                    &mut w,
                                    &data.content,
                                    data.img_width,
                                    data.img_height
                                ),
                                "Failed to transfer image data"
                            );

                            try_skip!(
                                create_unicode_placeholder_grid(
                                    &mut w,
                                    config.colors,
                                    data.aligned_area
                                ),
                                "Failed to create unicode placeholders"
                            );
                        }
                        Data::AnimationData(data) => {
                            let aligned_area = data.aligned_area;
                            try_cont!(
                                transfer_animation_data(&mut w, data),
                                "Failed to transfer animation data"
                            );
                            try_skip!(
                                create_unicode_placeholder_grid(
                                    &mut w,
                                    config.colors,
                                    aligned_area
                                ),
                                "Failed to create unicode placeholders"
                            );
                        }
                    }
                }
            })
            .expect("Kitty thread to be spawned");

        Self { sender, colors, handle }
    }
}

fn create_data_to_transfer(
    image_data: &[u8],
    area: Rect,
    compression: Compression,
    max_size: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> Result<Data> {
    let start_time = Instant::now();
    log::debug!(bytes = image_data.len(); "Compressing image data");

    if let Some(data) = get_gif_frames(image_data)? {
        let frames = data.frames;
        let (width, height) = data.dimensions;

        let frames: Vec<AnimationFrame> = frames
            .map_ok(|frame| {
                let delay = frame.delay().numer_denom_ms();

                AnimationFrame {
                    delay: delay.0 / delay.1,
                    content: base64::engine::general_purpose::STANDARD
                        .encode(frame.buffer().as_raw()),
                }
            })
            .try_collect()?;
        let aligned_area =
            create_aligned_area(area, (width, height), max_size, halign, valign).area;

        Ok(Data::AnimationData(AnimationData {
            frames,
            is_compressed: false,
            img_width: width,
            img_height: height,
            aligned_area,
        }))
    } else {
        let (image, aligned_area) = resize_image(image_data, area, max_size, halign, valign)?;

        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), compression);
        e.write_all(image.to_rgba8().as_raw())
            .context("Error occurred when writing image bytes to zlib encoder")?;

        let content = base64::engine::general_purpose::STANDARD.encode(
            e.finish().context("Error occurred when flushing image bytes to zlib encoder")?,
        );

        log::debug!(input_bytes = image_data.len(), compressed_bytes = content.len(), duration:? = start_time.elapsed(); "Image data compression finished");
        Ok(Data::ImageData(ImageData {
            content,
            aligned_area: aligned_area.area,
            img_width: image.width(),
            img_height: image.height(),
        }))
    }
}

fn clear_area(mut w: impl Write, colors: Colors, area: Rect) -> Result<()> {
    super::clear_area(&mut w, colors, area)?;
    tmux_write!(w, "\x1b_Ga=d,d=A,q=2\x1b\\")?;
    Ok(())
}

fn create_unicode_placeholder_grid(w: &mut impl Write, colors: Colors, area: Rect) -> Result<()> {
    let mut buf = Vec::with_capacity(area.width as usize * area.height as usize * 2);
    execute!(buf, SetColors(colors))?;
    for y in 0..area.height {
        csi_move!(buf, area.left(), area.top() + y)?;
        write!(buf, "\x1b[38;5;1m")?;

        for x in 0..area.width {
            write!(buf, "{DELIM}{row}{col}", row = GRID[y as usize], col = GRID[x as usize])?;
        }

        write!(buf, "\x1b[39m")?;
    }
    w.write_all(&buf)?;
    w.flush()?;

    Ok(())
}

fn transfer_animation_data(w: &mut impl Write, data: AnimationData) -> Result<()> {
    let start_time = Instant::now();
    let AnimationData { frames, is_compressed, img_width, img_height, aligned_area } = data;

    log::debug!(frames = frames.len(), img_width, img_height, aligned_area:?; "Transferring animation data");

    if frames.len() < 2 {
        log::warn!("Less than two frames, invalid animation data");
        return Ok(());
    }

    let mut first_frame_iter = frames[0].content.chars().peekable();
    let chunk: String = first_frame_iter.by_ref().take(4096).collect();

    let m = i32::from(first_frame_iter.peek().is_some());
    let delay = frames[0].delay;

    // Create image and transfer first frame
    tmux_write!(
        w,
        "\x1b_Gi=1,f=32,U=1,a=T,t=d,m={m},z={delay},q=2,s={img_width},v={img_height},c={cols},r={rows}{compression};{chunk}\x1b\\",
        compression = if is_compressed { ",o=z" } else { "" },
        cols = aligned_area.width,
        rows = aligned_area.height
    )?;

    // Transfer the rest of the first frame if any
    while first_frame_iter.peek().is_some() {
        let chunk: String = first_frame_iter.by_ref().take(4096).collect();
        let m = i32::from(first_frame_iter.peek().is_some());
        tmux_write!(w, "\x1b_Gi=1,m={m};{chunk}\x1b\\")?;
    }

    // Transfer rest of the frames, skip first because it was already
    // transferred
    for AnimationFrame { delay, content, .. } in frames.iter().skip(1) {
        let mut frame_iter = content.chars().peekable();
        let chunk: String = frame_iter.by_ref().take(4096).collect();

        tmux_write!(
            w,
            "\x1b_Gi=1,a=f,t=d,m={m},z={delay},q=2,s={img_width},v={img_height}{compression};{chunk}\x1b\\",
            compression = if is_compressed { ",o=z" } else { "" }
        )?;

        while frame_iter.peek().is_some() {
            let chunk: String = frame_iter.by_ref().take(4096).collect();
            let m = i32::from(frame_iter.peek().is_some());
            tmux_write!(w, "\x1b_Ga=f,i=1,m={m};{chunk}\x1b\\")?;
        }
    }

    // Run the animation
    tmux_write!(w, "\x1b_Ga=a,i=1,s=3\x1b\\")?;
    log::debug!(duration:? = start_time.elapsed(); "Transfer finished");

    Ok(())
}

fn transfer_image_data(
    w: &mut impl Write,
    content: &str,
    img_width: u32,
    img_height: u32,
) -> Result<()> {
    let start_time = Instant::now();
    log::debug!(bytes = content.len(), img_width, img_height; "Transferring compressed image data");
    let mut iter = content.chars().peekable();

    let first: String = iter.by_ref().take(4096).collect();
    tmux_write!(
        w,
        "\x1b_Gi=1,f=32,U=1,t=d,a=T,m=1,q=2,o=z,s={img_width},v={img_height};{first}\x1b\\"
    )?;

    while iter.peek().is_some() {
        let chunk: String = iter.by_ref().take(4096).collect();
        let m = i32::from(iter.peek().is_some());
        tmux_write!(w, "\x1b_Gm={m};{chunk}\x1b\\")?;
    }
    log::debug!(duration:? = start_time.elapsed(); "Transfer finished");

    Ok(())
}

enum Data {
    ImageData(ImageData),
    AnimationData(AnimationData),
}

struct ImageData {
    content: String,
    img_width: u32,
    img_height: u32,
    aligned_area: Rect,
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
    aligned_area: Rect,
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
