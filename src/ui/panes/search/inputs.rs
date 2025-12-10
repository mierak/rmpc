use bon::bon;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Position, Rect},
    style::Style,
    widgets::{Block, Borders, Widget},
};
use strum::{FromRepr, IntoStaticStr, VariantNames};

use crate::{
    config::Search,
    ctx::Ctx,
    mpd::mpd_client::{FilterKind, StickerFilter},
    ui::{
        input::BufferId,
        widgets::{button::Button, input::Input},
    },
};

pub const SEARCH_MODE_KEY: &str = "search_mode";
pub const FOLD_CASE_KEY: &str = "fold_case";
pub const STRIP_DIACRITICS_KEY: &str = "strip_diacritics";
pub const RATING_MODE_KEY: &str = "rating";
pub const RATING_VALUE_KEY: &str = "rating_value";
pub const RESET_BUTTON_KEY: &str = "reset";
pub const SEARCH_BUTTON_KEY: &str = "search_button";
pub const LIKE_KEY: &str = "like";

#[derive(derive_more::Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct InputGroups {
    pub inputs: Vec<InputType>,

    search_button: bool,
    initial_fold_case: bool,
    initial_strip_diacritics: bool,

    focused_idx: usize,
    pub area: Rect,

    text_style: Style,
    separator_style: Style,
    current_item_style: Style,
    highlight_item_style: Style,

    fold_case: bool,
    strip_diacritics: bool,
    search_mode: SearchMode,
    rating_mode: RatingMode,
    liked_mode: LikedMode,
}

#[bon]
impl InputGroups {
    #[builder]
    pub fn new(
        search_config: &Search,
        initial_fold_case: bool,
        initial_strip_diacritics: bool,
        search_button: bool,
        stickers_supported: bool,
        strip_diacritics_supported: bool,
        text_style: Style,
        separator_style: Style,
        current_item_style: Style,
        highlight_item_style: Style,
        ctx: &Ctx,
    ) -> Self {
        let mut inputs = Vec::new();
        for tag in &search_config.tags {
            inputs.push(InputType::Textbox(TextboxInput {
                key: "",
                filter_key: Some(tag.value.clone()),
                label: format!(" {:<18}:", tag.label),
                initial_value: None,
                buffer_id: BufferId::new(),
            }));
        }

        if stickers_supported {
            inputs.push(InputType::Separator);
            inputs.push(InputType::Spinner(SpinnerInput {
                key: RATING_MODE_KEY,
                label: format!(" {:<18}:", "Rating"),
            }));

            let buffer_id = BufferId::new();
            ctx.input.create_buffer(buffer_id, Some("0"));
            inputs.push(InputType::Numberbox(TextboxInput {
                key: RATING_VALUE_KEY,
                filter_key: None,
                label: format!(" {:<18}:", "Value"),
                initial_value: Some("0".to_owned()),
                buffer_id,
            }));

            inputs.push(InputType::Separator);
            inputs.push(InputType::Spinner(SpinnerInput {
                key: LIKE_KEY,
                label: format!(" {:<18}:", "Liked"),
            }));
        }

        inputs.push(InputType::Separator);

        inputs.push(InputType::Spinner(SpinnerInput {
            key: SEARCH_MODE_KEY,
            label: format!(" {:<18}:", "Search mode"),
        }));
        inputs.push(InputType::Spinner(SpinnerInput {
            key: FOLD_CASE_KEY,
            label: format!(" {:<18}:", "Case sensitive"),
        }));
        if strip_diacritics_supported {
            inputs.push(InputType::Spinner(SpinnerInput {
                key: STRIP_DIACRITICS_KEY,
                label: format!(" {:<18}:", "Ignore diacritics"),
            }));
        }

        inputs.push(InputType::Separator);

        inputs.push(InputType::Button(ButtonInput {
            key: RESET_BUTTON_KEY,
            label: " Reset".to_owned(),
        }));

        if search_button {
            inputs.push(InputType::Button(ButtonInput {
                key: SEARCH_BUTTON_KEY,
                label: " Search".to_owned(),
            }));
        }

        Self {
            inputs,

            focused_idx: 0,
            area: Rect::default(),

            search_button,
            initial_fold_case,
            initial_strip_diacritics,

            text_style,
            separator_style,
            current_item_style,
            highlight_item_style,

            fold_case: initial_fold_case,
            strip_diacritics: initial_strip_diacritics,
            search_mode: search_config.mode.into(),
            rating_mode: RatingMode::default(),
            liked_mode: LikedMode::default(),
        }
    }

    pub fn search_mode(&self) -> SearchMode {
        self.search_mode
    }

    pub fn rating_value(&self, ctx: &Ctx) -> String {
        self.textbox_value(RATING_VALUE_KEY, ctx).unwrap_or_default()
    }

    pub fn is_rating_filter_active(&self) -> bool {
        !matches!(self.rating_mode, RatingMode::Any)
    }

    pub fn rating_filter(
        &self,
        ctx: &Ctx,
    ) -> Result<Option<StickerFilter>, std::num::ParseIntError> {
        let value = self.rating_value(ctx);
        let value = if value.is_empty() { 0 } else { value.trim().parse()? };
        Ok(match self.rating_mode {
            RatingMode::Equals => Some(StickerFilter::EqualsInt(value)),
            RatingMode::GreaterThan => Some(StickerFilter::GreaterThanInt(value)),
            RatingMode::LessThan => Some(StickerFilter::LessThanInt(value)),
            RatingMode::Any => None,
        })
    }

    pub fn liked_filter(&self) -> Option<StickerFilter> {
        match self.liked_mode {
            LikedMode::Any => None,
            LikedMode::Liked => Some(StickerFilter::EqualsInt(2)),
            LikedMode::Neutral => Some(StickerFilter::EqualsInt(1)),
            LikedMode::Disliked => Some(StickerFilter::EqualsInt(0)),
        }
    }

    pub fn fold_case(&self) -> bool {
        self.fold_case
    }

    pub fn strip_diacritics(&self) -> bool {
        self.strip_diacritics
    }

    pub fn first(&mut self) {
        self.focused_idx = 0;
    }

    pub fn last(&mut self) {
        self.focused_idx = self.inputs.len() - 1;
    }

    pub fn focused(&self) -> &InputType {
        &self.inputs[self.focused_idx]
    }

    pub fn activate_focused(&mut self, ctx: &Ctx) -> ActionResult {
        match &mut self.inputs[self.focused_idx] {
            InputType::Textbox(input) | InputType::Numberbox(input) => {
                if ctx.input.is_active(input.buffer_id) {
                    ctx.input.normal_mode();
                } else if ctx.input.is_normal_mode() {
                    ctx.input.insert_mode(input.buffer_id);
                }

                if self.search_button { ActionResult::None } else { ActionResult::Search }
            }
            InputType::Spinner(input) => {
                match input.key {
                    FOLD_CASE_KEY => {
                        self.fold_case = !self.fold_case;
                        if !self.fold_case {
                            self.strip_diacritics = false;
                        }
                    }
                    STRIP_DIACRITICS_KEY => {
                        self.strip_diacritics = !self.strip_diacritics;
                        if self.strip_diacritics {
                            self.fold_case = true;
                        }
                    }
                    SEARCH_MODE_KEY => {
                        self.search_mode.cycle();
                    }
                    RATING_MODE_KEY => {
                        self.rating_mode.cycle();
                    }
                    LIKE_KEY => {
                        self.liked_mode.cycle();
                    }
                    _ => {}
                }

                if self.search_button { ActionResult::None } else { ActionResult::Search }
            }
            InputType::Button(ButtonInput { key: RESET_BUTTON_KEY, .. }) => {
                self.reset_all(ctx);

                ActionResult::Reset
            }
            InputType::Button(ButtonInput { key: SEARCH_BUTTON_KEY, .. }) => ActionResult::Search,
            InputType::Button(_) => ActionResult::None,
            InputType::Separator => ActionResult::None,
        }
    }

    fn reset_item(&mut self, idx: usize, ctx: &Ctx) {
        if let Some(input) = self.inputs.get_mut(idx) {
            match input {
                InputType::Textbox(input) | InputType::Numberbox(input) => {
                    if let Some(init) = &input.initial_value {
                        ctx.input.set_buffer(init.clone(), input.buffer_id);
                    } else {
                        ctx.input.clear_buffer(input.buffer_id);
                    }
                }
                InputType::Spinner(spinner) => match spinner.key {
                    FOLD_CASE_KEY => {
                        self.fold_case = self.initial_fold_case;
                    }
                    STRIP_DIACRITICS_KEY => {
                        self.strip_diacritics = self.initial_strip_diacritics;
                    }
                    SEARCH_MODE_KEY => {
                        self.search_mode = SearchMode::default();
                    }
                    RATING_MODE_KEY => {
                        self.rating_mode = RatingMode::default();
                    }
                    LIKE_KEY => {
                        self.liked_mode = LikedMode::default();
                    }
                    _ => {}
                },
                InputType::Button(_) | InputType::Separator => {}
            }
        }
    }

    pub fn reset_all(&mut self, ctx: &Ctx) {
        for idx in 0..self.inputs.len() {
            self.reset_item(idx, ctx);
        }
    }

    pub fn reset_focused(&mut self, ctx: &Ctx) {
        self.reset_item(self.focused_idx, ctx);
    }

    pub fn enter_insert_mode(&mut self, ctx: &Ctx) {
        if let InputType::Textbox(input) | InputType::Numberbox(input) = self.focused() {
            ctx.input.insert_mode(input.buffer_id);
        }
    }

    pub fn next_non_wrapping(&mut self) {
        self.focused_idx = (self.focused_idx + 1).min(self.inputs.len() - 1);

        if matches!(self.focused(), InputType::Separator) {
            self.next_non_wrapping();
        }
    }

    pub fn next(&mut self) {
        self.focused_idx = (self.focused_idx + 1) % self.inputs.len();

        if matches!(self.focused(), InputType::Separator) {
            self.next();
        }
    }

    pub fn prev_non_wrapping(&mut self) {
        if self.focused_idx > 0 {
            self.focused_idx -= 1;
        }

        if matches!(self.focused(), InputType::Separator) {
            self.prev_non_wrapping();
        }
    }

    pub fn prev(&mut self) {
        if self.focused_idx == 0 {
            self.focused_idx = self.inputs.len() - 1;
        } else {
            self.focused_idx -= 1;
        }

        if matches!(self.focused(), InputType::Separator) {
            self.prev();
        }
    }

    pub fn focus_input_at(&mut self, position: Position) {
        if !self.area.contains(position) {
            return;
        }
        let y = (position.y - self.area.y) as usize;

        if let Some(input) = self.inputs.get(y)
            && !matches!(input, InputType::Separator)
        {
            self.focused_idx = y;
        }
    }

    fn textbox_value(&self, key: &str, ctx: &Ctx) -> Option<String> {
        for input in &self.inputs {
            if let InputType::Textbox(input) | InputType::Numberbox(input) = input
                && input.key == key
            {
                return Some(ctx.input.value(input.buffer_id).trim().to_owned());
            }
        }
        None
    }
}

#[derive(Debug)]
pub(super) enum InputType {
    Textbox(TextboxInput),
    Numberbox(TextboxInput),
    Spinner(SpinnerInput),
    Button(ButtonInput),
    Separator,
}

#[derive(Debug)]
pub(super) struct TextboxInput {
    pub label: String,
    pub key: &'static str,
    pub filter_key: Option<String>,
    pub initial_value: Option<String>,
    pub buffer_id: BufferId,
}

#[derive(Debug)]
pub(super) struct SpinnerInput {
    pub key: &'static str,
    pub label: String,
}

#[derive(Debug, Default, PartialEq, VariantNames, Clone, Copy, FromRepr, IntoStaticStr)]
pub(super) enum SearchMode {
    #[strum(serialize = "Contains")]
    #[default]
    Contains,
    #[strum(serialize = "Exact")]
    Exact,
    #[strum(serialize = "Starts with")]
    StartsWith,
    #[strum(serialize = "Regex")]
    Regex,
}

#[derive(Debug, Default, Clone, Copy, IntoStaticStr, VariantNames, FromRepr)]
pub(super) enum LikedMode {
    #[default]
    #[strum(serialize = "Any")]
    Any,
    #[strum(serialize = "Liked")]
    Liked,
    #[strum(serialize = "Neutral")]
    Neutral,
    #[strum(serialize = "Disliked")]
    Disliked,
}

#[derive(Debug, Default, Clone, Copy, IntoStaticStr, VariantNames, FromRepr)]
pub(super) enum RatingMode {
    #[default]
    #[strum(serialize = "Any")]
    Any,
    #[strum(serialize = "Equals")]
    Equals,
    #[strum(serialize = "Greater than")]
    GreaterThan,
    #[strum(serialize = "Less than")]
    LessThan,
}

impl From<SearchMode> for FilterKind {
    fn from(value: SearchMode) -> Self {
        match value {
            SearchMode::Exact => FilterKind::Exact,
            SearchMode::StartsWith => FilterKind::StartsWith,
            SearchMode::Contains => FilterKind::Contains,
            SearchMode::Regex => FilterKind::Regex,
        }
    }
}

impl From<FilterKind> for SearchMode {
    fn from(value: FilterKind) -> Self {
        match value {
            FilterKind::Exact => SearchMode::Exact,
            FilterKind::StartsWith => SearchMode::StartsWith,
            FilterKind::Contains => SearchMode::Contains,
            FilterKind::Regex => SearchMode::Regex,
        }
    }
}

impl SearchMode {
    fn cycle(&mut self) {
        let i = *self as usize;
        if let Some(new) = SearchMode::from_repr((i + 1) % SearchMode::VARIANTS.len()) {
            *self = new;
        }
    }
}

impl RatingMode {
    fn cycle(&mut self) {
        let i = *self as usize;
        if let Some(new) = RatingMode::from_repr((i + 1) % RatingMode::VARIANTS.len()) {
            *self = new;
        }
    }
}

impl LikedMode {
    fn cycle(&mut self) {
        let i = *self as usize;
        if let Some(new) = LikedMode::from_repr((i + 1) % LikedMode::VARIANTS.len()) {
            *self = new;
        }
    }
}

pub(super) enum ActionResult {
    None,
    Search,
    Reset,
}

#[derive(derive_more::Debug)]
pub(super) struct ButtonInput {
    pub key: &'static str,
    pub label: String,
}

impl InputGroups {
    pub fn render(&mut self, mut area: Rect, buf: &mut Buffer, ctx: &Ctx) {
        self.area = area;
        let mut remaining_height = area.height as usize;
        area.height = 1;
        for (idx, input) in self.inputs.iter().enumerate() {
            if remaining_height == 0 {
                break;
            }

            let is_focused = idx == self.focused_idx;

            match input {
                InputType::Textbox(input) => {
                    let mut widget = Input::new(ctx, input.buffer_id)
                        .set_borderless(true)
                        .set_label(&input.label)
                        .set_placeholder("<None>")
                        .set_focused(is_focused && ctx.input.is_active(input.buffer_id))
                        .set_label_style(self.text_style)
                        .set_input_style(self.text_style);

                    widget = if ctx.input.is_active(input.buffer_id) && is_focused {
                        widget.set_label_style(self.highlight_item_style)
                    } else if is_focused {
                        widget
                            .set_label_style(self.current_item_style)
                            .set_input_style(self.current_item_style)
                    } else {
                        widget
                    };

                    widget.render(area, buf);
                }
                InputType::Numberbox(input) => {
                    let mut widget = Input::new(ctx, input.buffer_id)
                        .set_borderless(true)
                        .set_label(&input.label)
                        .set_placeholder("<None>")
                        .set_focused(is_focused && ctx.input.is_active(input.buffer_id))
                        .set_label_style(self.text_style)
                        .set_input_style(self.text_style);

                    widget = if ctx.input.is_active(input.buffer_id) && is_focused {
                        widget.set_label_style(self.highlight_item_style)
                    } else if is_focused {
                        widget
                            .set_label_style(self.current_item_style)
                            .set_input_style(self.current_item_style)
                    } else {
                        widget
                    };

                    widget.render(area, buf);
                }
                InputType::Spinner(input) => {
                    let mut inp = Input::new_static(ctx)
                        .set_borderless(true)
                        .set_label_style(self.text_style)
                        .set_input_style(self.text_style)
                        .set_label(&input.label)
                        .set_text(match input.key {
                            FOLD_CASE_KEY => {
                                if self.fold_case {
                                    "No"
                                } else {
                                    "Yes"
                                }
                            }
                            STRIP_DIACRITICS_KEY => {
                                if self.strip_diacritics {
                                    "Yes"
                                } else {
                                    "No"
                                }
                            }
                            SEARCH_MODE_KEY => self.search_mode.into(),
                            RATING_MODE_KEY => self.rating_mode.into(),
                            LIKE_KEY => self.liked_mode.into(),
                            _ => "",
                        });

                    if is_focused {
                        inp = inp
                            .set_label_style(self.current_item_style)
                            .set_input_style(self.current_item_style);
                    }
                    inp.render(area, buf);
                }
                InputType::Button(input) => {
                    Button::default()
                        .label(&input.label)
                        .label_alignment(Alignment::Left)
                        .style(if is_focused { self.current_item_style } else { self.text_style })
                        .render(area, buf);
                }
                InputType::Separator => {
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(self.separator_style)
                        .render(area, buf);
                }
            }
            area.y += 1;
            remaining_height = remaining_height.saturating_sub(1);
        }
    }
}
