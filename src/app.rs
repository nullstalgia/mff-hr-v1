use std::{
    fmt::Debug,
    sync::mpsc::{Receiver, TryRecvError},
};

use embassy_time::{Duration, Instant};
use embedded_graphics::{
    image::Image,
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoFont, MonoTextStyle,
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{
        Line, Polyline, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable,
    },
    text::{Alignment, Text, TextStyleBuilder},
};
use embedded_iconoir::prelude::IconoirNewIcon;
use esp32_nimble::BLEDevice;
use esp_idf_hal::{prelude::Peripherals, task::block_on};
use log::*;
use xpt2046::{TouchEvent, TouchKind};

use crate::{
    errors::{AppError, Result},
    heart_rate::ble::{BleIdents, BleStuff},
    settings::Settings,
};

pub struct App<'a, DT>
where
    DT: DrawTarget<Color = Rgb565, Error: Debug>,
    AppError: From<<DT as embedded_graphics::draw_target::DrawTarget>::Error>,
{
    touch_rx: Receiver<Option<TouchEvent>>,
    last_touch: Option<TouchEvent>,
    last_doodle_point: Option<Point>,
    view: AppView,
    view_needs_painting: bool,
    display: DT,
    debounce_instant: Instant,
    debounce_duration: Duration,
    doodle_lines: Lines,
    username_scratch: String,
    settings: Settings,
    // ble_handle: BleHrHandle,
    ble: BleStuff<'a>,
}

#[derive(Default)]
struct Lines {
    current: Option<Vec<Point>>,
    total: Vec<Vec<Point>>,
}

pub enum AppView {
    MainMenu,
    BadgeDisplay,
    Doodle,
    HrSelect,
    NameInput,
    // Gif,
    ResetSettings,
}

// pub enum DisplayType {
//     Name,
//     Heartrate,
//     Both,
// }

const INPUT_CHARS: &[char] = &[
    ' ', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
    'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k',
    'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '~', '!', '#', '$',
    '%', '&', '(', ')', '*', '+', ',', '-', '.', '/', ':', ';', '<', '=', '>', '?', '@', '[', ']',
    '^', '_', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

impl<'a, DT> App<'a, DT>
where
    DT: DrawTarget<Color = Rgb565, Error: Debug>,
    AppError: From<<DT as embedded_graphics::draw_target::DrawTarget>::Error>,
{
    pub fn build(touch_rx: Receiver<Option<TouchEvent>>, display: DT) -> Result<Self> {
        Ok(Self {
            display,
            touch_rx,
            view: AppView::MainMenu,
            view_needs_painting: true,
            last_doodle_point: None,
            // last_touch: Touch::Released {
            //     pos: Point::zero(),
            //     at: Instant::now(),
            // },
            last_touch: None,
            debounce_instant: Instant::now(),
            debounce_duration: Duration::from_millis(100),
            doodle_lines: Lines::default(),
            username_scratch: String::new(),
            settings: Settings::littlefs_load()?,
            // ble_handle: BleHrHandle::build()?,
            ble: BleStuff::build(),
        })
    }
    pub fn doodle(&mut self) -> Result<()> {
        const BACK_BUTTON_BOUND: Rectangle =
            Rectangle::new(Point::new(290, 0), Size::new_equal(24));
        if self.paint_check() {
            let title_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
            let text_style = TextStyleBuilder::new().alignment(Alignment::Center).build();
            Text::with_text_style("Doodle", Point::new(160, 10), title_style, text_style)
                .draw(&mut self.display)?;
            let back_icon = embedded_iconoir::icons::size24px::actions::Undo::new(Rgb565::WHITE);
            let image = Image::new(&back_icon, BACK_BUTTON_BOUND.top_left);
            image.draw(&mut self.display)?;

            for set in &self.doodle_lines.total {
                Polyline::new(set).draw_styled(
                    &PrimitiveStyle::with_stroke(Rgb565::BLUE, 2),
                    &mut self.display,
                )?;
            }
        }
        match self.touch() {
            Some(TouchEvent {
                point: new_point,
                kind: TouchKind::Start,
            }) => {
                if BACK_BUTTON_BOUND.contains(*new_point) {
                    self.change_view(AppView::MainMenu)?;
                    return Ok(());
                }
            }
            Some(TouchEvent {
                point: new_point,
                kind: TouchKind::Move,
            }) => {
                let point2 = *new_point;
                let point1: Point = self.last_doodle_point.unwrap_or(point2);
                info!("Drawing!");
                Polyline::new(&[point1, point2]).draw_styled(
                    &PrimitiveStyle::with_stroke(Rgb565::BLUE, 2),
                    &mut self.display,
                )?;
                if self.doodle_lines.current.is_none() {
                    self.doodle_lines.current = Some(Vec::new());
                }
                self.doodle_lines.current.as_mut().unwrap().push(point2);
                self.last_doodle_point = Some(point2);
            }
            Some(TouchEvent {
                kind: TouchKind::End,
                ..
            }) => {
                self.last_doodle_point = None;
                if !self.doodle_lines.current.is_none() {
                    self.doodle_lines
                        .total
                        .push(self.doodle_lines.current.take().unwrap());
                }
                // self.touch_debounce = Instant::now();
            }
            _ => (),
        }

        Ok(())
    }
    fn main_menu(&mut self) -> Result<()> {
        let options_offset = Point::new(20, 50);
        if self.paint_check() {
            info!(
                "My code is running! Core: {:?}, Heap free: {}",
                esp_idf_hal::cpu::core(),
                unsafe { esp_idf_hal::sys::esp_get_free_heap_size() }
            );
            let smol_char_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
            let character_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);
            let text_style = TextStyleBuilder::new().alignment(Alignment::Center).build();
            let line_style = PrimitiveStyleBuilder::new()
                .stroke_width(2)
                .stroke_color(Rgb565::BLUE)
                .build();

            Text::with_text_style(
                "MFF Badge",
                Point::new(160, 15),
                character_style,
                text_style,
            )
            .draw(&mut self.display)?;

            // if let Ok(point) = Text::new(
            //     "Start\n\nName Input\n\nHR Monitor Selection\n\nDoodle",
            //     Point::new(20, 50),
            //     character_style,
            //     // text_style,
            // )
            // .draw(&mut self.display)
            // {
            //     info!("{point}");
            // };
            for (item, point) in MainMenu::vert_regions(
                Some(options_offset),
                // &FONT_10X20
            ) {
                // info!("drawing {item} at {point}");
                if let Ok(point) = Text::new(
                    &item.to_string(),
                    point,
                    character_style,
                    // text_style,
                )
                .draw(&mut self.display)
                {
                    // info!("{point}");
                };
                Line::new(
                    point + Point::new(-5, 0),
                    point + Point::new(-5, -10 as i32),
                )
                .draw_styled(&line_style, &mut self.display)?;
            }

            Text::with_text_style(
                "To reset settings, hold BOOT _after_ tapping RST",
                Point::new(160, 230),
                smol_char_style,
                text_style,
            )
            .draw(&mut self.display)?;

            Text::with_text_style(
                "nullstalgia 2024",
                Point::new(270, 12),
                smol_char_style,
                text_style,
            )
            .draw(&mut self.display)?;

            Text::new(
                &format!(
                    "Heap Free: {}\nMin. Free: {}",
                    unsafe { esp_idf_hal::sys::esp_get_free_heap_size() },
                    unsafe { esp_idf_hal::sys::esp_get_minimum_free_heap_size() }
                ),
                Point::new(5, 10),
                smol_char_style,
            )
            .draw(&mut self.display)?;
        }

        match self.touch() {
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if point.y < 20 && point.x < 100 => {
                self.change_view(AppView::MainMenu)?;
            }
            Some(TouchEvent {
                point,
                // using Move instead of Start since Start's coord isn't always as accurate
                kind: TouchKind::Start,
            }) => {
                // Appeasing borrow checker, probably isn't efficient.
                let point = *point;
                // Setting the pixel that was tapped to help with debugging
                self.display
                    .fill_solid(&Rectangle::new(point, Size::new_equal(1)), Rgb565::BLUE)?;

                if let Some(choice) =
                    MainMenu::from_touch(Some(options_offset), &point, &FONT_10X20)
                {
                    info!("{choice} at {point}");
                    match choice {
                        MainMenu::Start => self.change_view(AppView::BadgeDisplay)?,
                        MainMenu::NameInput => self.change_view(AppView::NameInput)?,
                        MainMenu::HrSelect => self.change_view(AppView::HrSelect)?,
                        // MainMenu::HrSelect => self.ble_stuff.scan_for_select(),
                        // ;
                        MainMenu::Doodle => self.change_view(AppView::Doodle)?,
                        _ => (),
                    }
                } else {
                    info!("Touch item not found at {point}");
                };
                self.debounce_instant = Instant::now();
            }
            _ => (),
        }
        Ok(())
    }
    // awful hardcoded-ness
    // TODO fix at some point
    fn name_input(&mut self) -> Result<()> {
        let offset = Point::new(0, 100);
        let mut width: i32 = self.display.bounding_box().size.width as i32
            / self.username_scratch.len().max(1) as i32;
        if self.paint_check() {
            let title_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);
            let name_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
            let save_style = MonoTextStyle::new(&FONT_10X20, Rgb565::GREEN);
            let text_style = TextStyleBuilder::new().alignment(Alignment::Center).build();
            let line_style = PrimitiveStyleBuilder::new()
                .stroke_width(1)
                .stroke_color(Rgb565::RED)
                .build();

            Text::with_text_style("Name Input", Point::new(160, 15), title_style, text_style)
                .draw(&mut self.display)?;

            if self.username_scratch.is_empty() {
                self.username_scratch = "Bingus".to_string();
            }
            let mut buff: [u8; 4] = [0; 4];

            // Name character "arrows"
            //
            // Top-most line: 50
            // Bottom-most line: 140
            // Whole-screen length

            Line::new(offset + Point::new(0, -25), offset + Point::new(320, -25))
                .draw_styled(&line_style, &mut self.display)?;
            Line::new(offset + Point::new(0, 15), offset + Point::new(320, 15))
                .draw_styled(&line_style, &mut self.display)?;
            Line::new(offset + Point::new(0, -50), offset + Point::new(320, -50))
                .draw_styled(&line_style, &mut self.display)?;
            Line::new(offset + Point::new(0, 40), offset + Point::new(320, 40))
                .draw_styled(&line_style, &mut self.display)?;

            width =
                self.display.bounding_box().size.width as i32 / self.username_scratch.len() as i32;
            for (index, char) in self.username_scratch.chars().enumerate() {
                let cell_x = offset.x + (index as i32 * width);
                let cell_y = offset.y;

                // _ = Line::new(
                //     Point::new(cell_x, cell_y + -45),
                //     Point::new(cell_x, cell_y + 35),
                // )
                // .draw_styled(&line_style, &mut self.display);

                Text::new(
                    char.encode_utf8(&mut buff),
                    Point::new(cell_x + width / 2, cell_y),
                    name_style,
                )
                .draw(&mut self.display)?;
                Text::new(
                    "-\n\n\n+",
                    Point::new(cell_x + width / 2, cell_y - 30),
                    title_style,
                )
                .draw(&mut self.display)?;

                if index != 0 {
                    Line::new(
                        Point::new(cell_x + 5, cell_y + -50),
                        Point::new(cell_x + 5, cell_y + 40),
                    )
                    .draw_styled(&line_style, &mut self.display)?;
                }
            }

            // Name length "arrows"
            //
            // Top-most line: 170-2
            // Bottom-most line: 190-2
            // Left-most: 135
            // Right-most: 185
            // Splits in middle.

            let point = Text::with_text_style(
                "Name Length: ",
                Point::new(320 / 2, 160),
                title_style,
                text_style,
            )
            .draw(&mut self.display)
            .unwrap();
            Text::with_text_style(
                &self.username_scratch.len().to_string(),
                point,
                title_style,
                text_style,
            )
            .draw(&mut self.display)
            .unwrap();

            let box_style = PrimitiveStyleBuilder::new()
                .stroke_color(Rgb565::RED)
                .stroke_width(1)
                .fill_color(Rgb565::BLACK)
                .build();

            Rectangle::with_center(Point::new(320 / 2, 180 - 2), Size::new(50, 20))
                .into_styled(box_style)
                .draw(&mut self.display)?;
            Text::with_text_style("- +", Point::new(320 / 2, 185), title_style, text_style)
                .draw(&mut self.display)?;
            Line::new(
                Point::new(320 / 2, 170 - 2),
                Point::new(320 / 2, 170 + 20 - 2),
            )
            .draw_styled(&line_style, &mut self.display)?;

            // Save/Cancel buttons
            //
            // Top-most line: 200
            // Bottom-most line: 220
            // Left-most line: ~93
            // Right-most line: ~227
            // Splits in middle.

            Rectangle::with_center(Point::new(320 / 2, 210), Size::new(135, 20))
                .into_styled(box_style)
                .draw(&mut self.display)?;
            Text::with_text_style(
                "Cancel",
                Point::new((320 / 2) - 33, 217),
                title_style,
                text_style,
            )
            .draw(&mut self.display)?;
            Text::with_text_style(
                "Save",
                Point::new((320 / 2) + 32, 217),
                save_style,
                text_style,
            )
            .draw(&mut self.display)?;
            // anyhow!();
            Line::new(Point::new(320 / 2, 200), Point::new(320 / 2, 220))
                .draw_styled(&line_style, &mut self.display)?;
        }

        match self.touch() {
            // Some(TouchEvent {
            //     point,
            //     kind: TouchKind::Move,
            // }) if point.y < 20 && point.x < 100 => {
            //     self.change_view(AppView::MainMenu);
            // }

            // Characters
            Some(TouchEvent {
                point,
                // using Move instead of Start since Start's coord isn't always as accurate
                kind: TouchKind::Move,
            }) if point.y >= 50 && point.y <= 140 => {
                // Figure out which char and which dir
                let is_top_half = {
                    let adjusted = point.y - 50;
                    // Max range is now 0-90

                    info!("{point}, {adjusted}");
                    if adjusted >= 90 / 2 {
                        false
                    } else {
                        true
                    }
                };
                let point = *point;
                let mut char_area = Rectangle::default();
                let index_of_chosen =
                    self.username_scratch
                        .chars()
                        .enumerate()
                        .find_map(|(index, _)| {
                            let cell_x = offset.x + (index as i32 * width);
                            char_area =
                                Rectangle::new(Point::new(cell_x, 50), Size::new(width as u32, 90));
                            if char_area.contains(point) {
                                Some(index)
                            } else {
                                None
                            }
                        });

                if let Some(index) = index_of_chosen {
                    info!("{index}");

                    if is_top_half {
                        info!("NameTop!");
                    } else {
                        info!("NameBot!");
                    }
                    string_dingle(&mut self.username_scratch, index, !is_top_half);
                    self.repaint_full()?;
                    // self.change_view(AppView::NameInput);
                }
                self.debounce_instant = Instant::now();
            }
            // Name length
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if (point.y >= (170 - 2) && point.y <= (190 - 2))
                && (point.x >= 135 && point.x <= 185) =>
            {
                let is_add = {
                    if point.x > 320 / 2 {
                        true
                    } else {
                        false
                    }
                };
                if is_add {
                    self.username_scratch.push(' ');
                } else {
                    if self.username_scratch.len() > 1 {
                        self.username_scratch.pop();
                    }
                }
                self.repaint_full()?;
                self.debounce_instant = Instant::now();
                // self.change_view(AppView::NameInput);
                info!("Len! {is_add}");
            }
            // Save/Cancel
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if (point.y >= 200 && point.y <= 220) && (point.x >= 93 && point.x <= 227) => {
                let is_save = {
                    if point.x > 320 / 2 {
                        true
                    } else {
                        false
                    }
                };
                info!("SaveCan! {is_save}");
                if is_save {
                    self.settings.username.clone_from(&self.username_scratch);
                    self.settings.littlefs_save()?;
                }
                self.change_view(AppView::MainMenu)?;
            }
            _ => (),
        }
        Ok(())
    }
    fn hr_select(&mut self) -> Result<()> {
        let has_hr_saved = self.settings.hr.saved.is_some();
        let monitors_discovered = !self.ble.discovered.is_empty();

        const BACK_BUTTON_BOUND: Rectangle =
            Rectangle::new(Point::new(290, 0), Size::new_equal(24));

        const TRASH_BUTTON_BOUND: Rectangle =
            Rectangle::new(Point::new(290, 210), Size::new_equal(24));

        const RESCAN_BUTTON_BOUND: Rectangle =
            Rectangle::new(Point::new(10, 210), Size::new_equal(24));

        const SAVE_BUTTON_BOUND: Rectangle =
            Rectangle::with_center(Point::new(160, 165), Size::new(50, 35));

        const LEFT_BUTTON_BOUND: Rectangle =
            Rectangle::with_center(Point::new(100, 165), Size::new_equal(24));

        const RIGHT_BUTTON_BOUND: Rectangle =
            Rectangle::with_center(Point::new(220, 165), Size::new_equal(24));

        if self.paint_check() {
            let back_icon = embedded_iconoir::icons::size24px::actions::Undo::new(Rgb565::WHITE);
            let image = Image::new(&back_icon, BACK_BUTTON_BOUND.top_left);
            image.draw(&mut self.display)?;

            let rescan_icon =
                embedded_iconoir::icons::size24px::actions::Refresh::new(Rgb565::WHITE);
            let image = Image::new(&rescan_icon, RESCAN_BUTTON_BOUND.top_left);
            image.draw(&mut self.display)?;

            let title_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);
            let name_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
            let small_name_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
            let save_style = MonoTextStyle::new(&FONT_10X20, Rgb565::GREEN);
            let text_style = TextStyleBuilder::new().alignment(Alignment::Center).build();
            let line_style = PrimitiveStyleBuilder::new()
                .stroke_width(1)
                .stroke_color(Rgb565::RED)
                .build();
            let green_line_style = PrimitiveStyleBuilder::new()
                .stroke_width(1)
                .stroke_color(Rgb565::RED)
                .build();

            if monitors_discovered {
                let left_icon =
                    embedded_iconoir::icons::size24px::navigation::ArrowLeft::new(Rgb565::WHITE);
                let image = Image::new(&left_icon, LEFT_BUTTON_BOUND.top_left);
                image.draw(&mut self.display)?;

                let right_icon =
                    embedded_iconoir::icons::size24px::navigation::ArrowRight::new(Rgb565::WHITE);
                let image = Image::new(&right_icon, RIGHT_BUTTON_BOUND.top_left);
                image.draw(&mut self.display)?;

                let text = format!(
                    "Select HR Monitor\n{index}/{total}",
                    index = self.ble.chosen_discovered + 1,
                    total = self.ble.discovered.len()
                );

                Text::with_text_style(&text, Point::new(160, 15), title_style, text_style)
                    .draw(&mut self.display)?;

                let device = {
                    let (mac, name) = self
                        .ble
                        .discovered
                        .iter()
                        .nth(self.ble.chosen_discovered)
                        .unwrap();
                    BleIdents {
                        mac: *mac,
                        name: name.clone(),
                    }
                };

                Text::with_text_style(
                    &device.to_string(),
                    Point::new(160, 65),
                    name_style,
                    text_style,
                )
                .draw(&mut self.display)?;

                SAVE_BUTTON_BOUND.draw_styled(&line_style, &mut self.display)?;
                Text::with_text_style("Save", Point::new(160, 170), save_style, text_style)
                    .draw(&mut self.display)?;
            } else {
                Text::with_text_style(
                    "No monitors found!\n\nRescan with bottom-left button?",
                    Point::new(160, 40),
                    title_style,
                    text_style,
                )
                .draw(&mut self.display)?;
            }

            if let Some(saved) = &self.settings.hr.saved {
                let trash_icon =
                    embedded_iconoir::icons::size24px::actions::Trash::new(Rgb565::WHITE);
                let image = Image::new(&trash_icon, TRASH_BUTTON_BOUND.top_left);
                image.draw(&mut self.display)?;

                Text::with_text_style(
                    &format!("Saved:\n{saved}"),
                    Point::new(160, 210),
                    small_name_style,
                    text_style,
                )
                .draw(&mut self.display)?;
            } else {
                Text::with_text_style(
                    "No monitor saved",
                    Point::new(160, 210),
                    small_name_style,
                    text_style,
                )
                .draw(&mut self.display)?;
            }
        }

        match self.touch() {
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if BACK_BUTTON_BOUND.contains(*point) => {
                self.change_view(AppView::MainMenu)?;
                return Ok(());
            }
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if TRASH_BUTTON_BOUND.contains(*point) && has_hr_saved => {
                info!("Trashing saved device!");
                self.settings.hr.saved = None;
                self.settings.littlefs_save()?;
                self.repaint_full()?;
                return Ok(());
            }
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if SAVE_BUTTON_BOUND.contains(*point) && monitors_discovered => {
                let device = {
                    let (mac, name) = self
                        .ble
                        .discovered
                        .iter()
                        .nth(self.ble.chosen_discovered)
                        .unwrap();
                    BleIdents {
                        mac: *mac,
                        name: name.clone(),
                    }
                };
                info!("Saving {device}!");
                self.settings.hr.saved = Some(device);
                self.settings.littlefs_save()?;
                self.change_view(AppView::MainMenu)?;
                return Ok(());
            }
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if RESCAN_BUTTON_BOUND.contains(*point) => {
                self.change_view(AppView::HrSelect)?;
                return Ok(());
            }
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if LEFT_BUTTON_BOUND.contains(*point) && monitors_discovered => {
                self.display.clear(Rgb565::BLACK)?;

                if let None = self.ble.chosen_discovered.checked_sub(1) {
                    self.ble.chosen_discovered = self.ble.discovered.len() - 1;
                }

                self.repaint();
                return Ok(());
            }
            Some(TouchEvent {
                point,
                kind: TouchKind::Start,
            }) if RIGHT_BUTTON_BOUND.contains(*point) && monitors_discovered => {
                self.display.clear(Rgb565::BLACK)?;

                self.ble.chosen_discovered += 1;
                if self.ble.chosen_discovered >= self.ble.discovered.len() {
                    self.ble.chosen_discovered = 0;
                }

                self.repaint();
                return Ok(());
            }
            _ => (),
        }

        Ok(())
    }
    pub fn main_loop(&mut self) -> Result<()> {
        match self.view {
            AppView::Doodle => {
                self.doodle()?;
            }
            AppView::MainMenu => {
                self.main_menu()?;
            }
            AppView::NameInput => {
                self.name_input()?;
            }
            AppView::HrSelect => {
                self.hr_select()?;
            }
            _ => (),
        }
        Ok(())
    }
    fn change_view(&mut self, new_view: AppView) -> Result<()> {
        self.repaint_full()?;
        self.view = new_view;
        self.debounce_instant = Instant::now();

        // Extra actions based on new view
        match self.view {
            AppView::NameInput => {
                self.debounce_duration = Duration::from_millis(100);
                self.username_scratch.clone_from(&self.settings.username);
            }
            AppView::MainMenu => {
                self.ble.discovered.clear();
                self.debounce_duration = Duration::from_millis(500);
            }
            AppView::HrSelect => {
                let character_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);
                let text_style = TextStyleBuilder::new().alignment(Alignment::Center).build();

                Text::with_text_style(
                    "Scanning for BLE HR Monitors...\nPlease wait 10s...",
                    Point::new(320 / 2, 240 / 2),
                    character_style,
                    text_style,
                )
                .draw(&mut self.display)?;

                self.ble.discovered = block_on(async { self.ble.scan_for_select().await })?;

                info!("{:?}", self.ble.discovered);

                // Filtering out all the nameless monitors
                // (easy enough to just have the user rescan)
                self.ble.discovered.retain(|_, name| !name.is_empty());

                self.ble.chosen_discovered = 0;

                // Repaint again since we drew here
                self.repaint_full()?;
                // self.change_view(AppView::MainMenu)?;
            }
            AppView::BadgeDisplay => {
                if let Some(addr) = self.settings.hr.saved.as_ref() {
                    block_on(async { self.ble.connect_to_monitor(addr.mac).await })?;
                }
                info!("Done.");
            }
            _ => (),
        }

        Ok(())
    }
    fn touch(&mut self) -> &Option<TouchEvent> {
        match self.touch_rx.recv() {
            Ok(event) => self.last_touch = event,
            // Ok(Some(event)) => {
            //     self.last_touch = Touch::Pressed(event.point);
            // }
            // Ok(None) => match &self.last_touch {
            //     Touch::Pressed(point) => {
            //         self.last_touch = Touch::Released {
            //             pos: *point,
            //             at: Instant::now(),
            //         };
            //     }
            //     Touch::Released { .. } => (),
            // },
            Err(_) => (),
            // Err(TryRecvError::Empty) => (),
            // Err(TryRecvError::Disconnected) => panic!("Touch DCd!"),
        }

        if self.debounce_instant.elapsed() < self.debounce_duration {
            return &None;
        }

        &self.last_touch
    }
    /// Only sets the `view_needs_painting` flag to `true`.
    fn repaint(&mut self) {
        self.view_needs_painting = true;
        // Ok(())
    }
    /// Full clears the display and sets the `view_needs_painting` flag to `true`.
    fn repaint_full(&mut self) -> Result<()> {
        self.repaint();
        self.display.clear(Rgb565::BLACK)?;
        Ok(())
    }
    /// Just to make sure I don't forget any part of the logic.
    ///
    /// Returns `true` if a repaint is requested, and sets flag to `false`.
    fn paint_check(&mut self) -> bool {
        let repaint = self.view_needs_painting;
        self.view_needs_painting = false;
        repaint
    }
}

const SPACING: usize = 30;

trait MenuTest: strum::VariantArray + Clone + Copy {
    fn from_touch(offset: Option<Point>, touch: &Point, font: &MonoFont) -> Option<Self> {
        // let adjusted_point = Point::new((touch.x - offset.x).max(0), (touch.y - offset.y).max(0));
        // Simple bound check
        let font_correction = -(font.character_size.height as i32);
        let mut top_bound = font_correction;
        if let Some(Point { x: _, y }) = offset.as_ref() {
            top_bound = *y + font_correction;
            if touch.y < top_bound {
                return None;
            }
        }
        for (
            variant,
            Point {
                x: _,
                y: bottom_bound,
            },
        ) in Self::vert_regions(offset)
        {
            if touch.y >= top_bound && touch.y <= bottom_bound {
                return Some(variant);
            }
        }

        None
    }
    // fn print_points(offset: Option<Point>, font: &MonoFont) -> impl Iterator<Item = (Self, Point)> {
    //     let mut offset = offset.unwrap_or_default();
    //     offset.y += font.character_size.height as i32;
    //     Self::vert_regions(Some(offset))
    // }
    fn vert_regions(offset: Option<Point>) -> impl Iterator<Item = (Self, Point)> {
        Self::VARIANTS
            .iter()
            .enumerate()
            .map(move |(index, &variant)| {
                if let Some(Point { x, y }) = offset {
                    (variant, Point::new(x, (index * SPACING) as i32 + y))
                } else {
                    (variant, Point::new(0, (index * SPACING) as i32))
                }
            })
    }
}

impl MenuTest for MainMenu {}

#[derive(strum_macros::Display, strum_macros::VariantArray, Clone, Copy)]
enum MainMenu {
    #[strum(to_string = "Start Badge")]
    Start,
    #[strum(to_string = "Name Input")]
    NameInput,
    #[strum(to_string = "BLE HR Monitor Selection")]
    HrSelect,
    Doodle,
}

// #[derive(Debug, Clone, Copy)]
// enum Touch {
//     Pressed(Point),
//     Released { pos: Point, at: Instant },
// }
// fn touch(&mut self) -> Touch {
//     match self.touch_rx.try_recv() {
//         Ok(Some(event)) => {
//             self.last_touch = Touch::Pressed(event.point);
//         }
//         Ok(None) => match &self.last_touch {
//             Touch::Pressed(point) => {
//                 self.last_touch = Touch::Released {
//                     pos: *point,
//                     at: Instant::now(),
//                 };
//             }
//             Touch::Released { .. } => (),
//         },
//         Err(TryRecvError::Empty) => (),
//         Err(TryRecvError::Disconnected) => panic!("Touch DCd!"),
//     }

//     if self.touch_debounce.elapsed() < Duration::from_millis(100) {
//         return Touch::Released {
//             pos: Point::zero(),
//             at: Instant::from_millis(0),
//         };
//     }

//     self.last_touch
// }

fn string_dingle(input: &mut String, index: usize, up: bool) {
    let Some(char) = input.get(index..index + 1) else {
        return;
    };

    let mut buff: [u8; 4] = [0; 4];
    let old_index = INPUT_CHARS
        .iter()
        .position(|c| c.encode_utf8(&mut buff) == char)
        .unwrap_or(0);

    let new_index = if up {
        (old_index + 1) % INPUT_CHARS.len()
    } else {
        (old_index + INPUT_CHARS.len() - 1) % INPUT_CHARS.len()
    };
    let new_char = INPUT_CHARS[new_index];
    input.replace_range(index..index + 1, new_char.encode_utf8(&mut buff));
    // *char = new_char.encode_utf8(&mut buff);
}
