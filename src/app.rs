use std::{
    fmt::Debug,
    fs,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Instant,
};

// use display_interface::WriteOnlyDataCommand;
use eg_seven_segment::SevenSegmentStyleBuilder;
use embedded_canvas::{CCanvasAt, Canvas, CanvasAt};
use embedded_graphics::{
    geometry::Point,
    image::Image,
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoFont, MonoTextStyle,
    },
    pixelcolor::{BinaryColor, Rgb565},
    prelude::*,
    primitives::{
        Line, Polyline, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable,
    },
    text::{Alignment, Text, TextStyleBuilder},
};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};
use embedded_iconoir::prelude::IconoirNewIcon;

use crate::{
    errors::{AppError, Result},
    heart_rate::ble::{MonitorHandle, MonitorReply, MonitorStatus},
    settings::Settings,
};
use embedded_plots::curve::{Curve, PlotPoint};
use log::*;
use serde::{Deserialize, Serialize};
use strum::VariantArray;
use u8g2_fonts::{
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

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
    // ResetSettings,
}

// pub enum DisplayType {
//     Name,
//     Heartrate,
//     Both,
// }

const HR_HISTORY_AMOUNT: usize = 100;

const INPUT_CHARS: &[char] = &[
    ' ', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
    'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k',
    'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '~', '!', '#', '$',
    '%', '&', '(', ')', '*', '+', ',', '-', '.', '/', ':', ';', '<', '=', '>', '?', '@', '[', ']',
    '^', '_', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

pub struct App
// where
// DT: DrawTarget<Color = Rgb565, Error: Debug>,
// AppError: From<<DT as embedded_graphics::draw_target::DrawTarget>::Error>,
// DI: WriteOnlyDataCommand,
// M: Model,
// RST: OutputPin,
// MODEL: Model<ColorFormat = Rgb565>,
// RST: OutputPin,

// Bounds from impl:
// DI: PixelInterface,
// MODEL: Model,
// MODEL::ColorFormat: PixelFormat<DI::PixelWord>,
// RST: OutputPin,
// AppError: From<<DI as CommandInterface>::Error>,
{
    pub display: SimulatorDisplay<Rgb565>,
    view: AppView,
    view_needs_painting: bool,

    monitor: Option<MonitorHandle>,

    // ble: BleStuff<'a>,

    // hr_rx: Option<Receiver<MonitorStatus>>,
    // current_hr: Option<MonitorStatus>,
    // hr_bound: Rectangle,
    hr_history: Vec<PlotPoint>,
    plot_bpm_high: u8,
    plot_bpm_low: u8,

    username_scratch: String,
    settings: Settings,

    hr_canvas: Canvas<BinaryColor>,
    name_canvas: Canvas<BinaryColor>,

    image_index: Option<usize>,
    image_count: usize,

    init_time: Instant,
}

impl App
// where
// DT: DrawTarget<Color = Rgb565, Error: Debug>,
// AppError: From<<DT as embedded_graphics::draw_target::DrawTarget>::Error>,
// DI: WriteOnlyDataCommand,
// // M: Model,
// MODEL: Model<ColorFormat = Rgb565>,
// RST: OutputPin,

// Bounds from impl:
// DI: PixelInterface,
// MODEL: Model,
// MODEL::ColorFormat: PixelFormat<DI::PixelWord>,
// RST: OutputPin,
// AppError: From<<DI as CommandInterface>::Error>,
{
    pub fn build(display: SimulatorDisplay<Rgb565>) -> Result<Self> {
        Ok(Self {
            display,
            view: AppView::BadgeDisplay,
            view_needs_painting: true,
            // last_touch: Touch::Released {
            //     pos: Point::zero(),
            //     at: Instant::now(),
            // },
            username_scratch: String::new(),
            settings: Settings::default(),
            // ble_handle: BleHrHandle::build()?,
            // ble: BleStuff::build(),
            monitor: Some(MonitorHandle::build()?),
            hr_canvas: Canvas::new(Size::new(240, 60)),
            name_canvas: Canvas::new(Size::new(240, 40)),
            hr_history: Vec::with_capacity(HR_HISTORY_AMOUNT),
            plot_bpm_high: 0,
            plot_bpm_low: 0,
            image_index: None,
            image_count: 0,
            init_time: Instant::now(),
        })
    }
    // fn custom_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<()>
    // where
    //     I: IntoIterator<Item = Rgb565>,
    // {
    //     self.display.set_pixels(
    //         area.top_left.x as u16,
    //         area.top_left.y as u16,
    //         area.bottom_right().ok_or(AppError::BoundlessRectangle)?.x as u16,
    //         area.bottom_right().ok_or(AppError::BoundlessRectangle)?.y as u16,
    //         self.hr_canvas.pixels.iter().map(|p| match p {
    //             Some(BinaryColor::On) => Rgb565::RED,
    //             Some(BinaryColor::Off) => Rgb565::BLACK,
    //             None => Rgb565::BLACK,
    //         }),
    //     )?;
    //     Ok(())
    // }
    fn badge_view(&mut self) -> Result<()> {
        let time = self.init_time.elapsed().as_millis() as f64;
        let oscillator_value = (time / 650.0).sin().abs();
        // info!("{oscillator_value}");
        // let font_style =
        // MonoTextStyle::new(&u8g2_fonts::fonts::u8g2_font_fub30_tf, BinaryColor::On);
        let font = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_fub25_tf>();
        // const HEART_ICON_BOUND: Rectangle = Rectangle::new(Point::new(), Size::new_equal(24));
        // let mut numeric_canvas: CanvasAt<BinaryColor> =
        //     CanvasAt::new(Point::new(71, 91), Size::new(100, 60));

        // let font = FontRenderer::new::<fonts::u8g2_font_haxrcorp4089_t_cyrillic>();

        const NAME_BOUND: Rectangle = Rectangle::new(Point::new(0, 0), Size::new(240, 40));
        const NUMERIC_BOUND: Rectangle = Rectangle::new(Point::new(0, 260), Size::new(240, 60));

        let bpm_style = SevenSegmentStyleBuilder::new()
            .digit_size(Size::new(10 * 3, 20 * 3)) // digits are 10x20 pixels
            .digit_spacing(5) // 5px spacing between digits
            .segment_width(5) // 5px wide segments
            // .segment_color(Rgb565::RED)
            .segment_color(BinaryColor::On)
            .build();
        let left_style = TextStyleBuilder::new().alignment(Alignment::Left).build();
        let center_style = TextStyleBuilder::new().alignment(Alignment::Center).build();

        self.name_canvas
            .pixels
            .iter_mut()
            .for_each(|pixel| *pixel = None);

        // let text = Text::with_text_style(
        //     &self.settings.username,
        //     // Point::new(240 / 2, 150),
        //     // Point::new(0, 60),
        //     Point::new(240 / 2, 15 + (20.0 * oscillator_value) as i32),
        //     font_style,
        //     center_style,
        // );

        // _ = text.draw(&mut self.name_canvas);

        font.render_aligned(
            self.settings.username.as_str(),
            Point::new(240 / 2, 25 + (10.0 * oscillator_value) as i32),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut self.name_canvas,
        )
        .unwrap();

        self.display.fill_contiguous(
            &NAME_BOUND,
            self.name_canvas.pixels.iter().map(|p| match p {
                Some(BinaryColor::On) => Rgb565::WHITE,
                Some(BinaryColor::Off) => Rgb565::BLACK,
                None => Rgb565::BLACK,
            }),
        )?;

        if self.paint_check() {
            // let title_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);
            // I don't like this positioning of this var but it works for now

            if self.monitor.is_some() {
                let heart_icon =
                    embedded_iconoir::icons::size48px::health::Heart::new(BinaryColor::On);
                // Text::with_text_style("Badge!", Point::new(240 / 2, 20), title_style, center_style)
                // .draw(&mut self.display)?;
                // _ = heart_icon.draw(&mut self.display.color_converted());
                let image = Image::new(&heart_icon, Point::new(6, 6));
                _ = image.draw(&mut self.hr_canvas);

                self.display.fill_contiguous(
                    &NUMERIC_BOUND,
                    self.hr_canvas.pixels.iter().map(|p| match p {
                        Some(BinaryColor::On) => Rgb565::RED,
                        Some(BinaryColor::Off) => Rgb565::BLACK,
                        None => Rgb565::BLACK,
                    }),
                )?;
            }

            let mut index = {
                match &self.image_index {
                    Some(index) => index + 1,
                    None => 0,
                }
            };

            let bmp_data = include_bytes!("../a.qoi");
            let bmp = tinyqoi::Qoi::new(bmp_data)?;

            Image::with_center(&bmp, Point::new(240 / 2, (320 / 2) - 10))
                .draw(&mut self.display.color_converted())?;

            // self.display
            //     .set_pixels(0, 260, 240, 320, std::iter::repeat(Rgb565::RED))?;
            // Rectangle::new(Point::new(0, 200), Size::new(240, 120))
            //     .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
            //     .draw(&mut self.display)?;
        }
        let mut rebuild_monitor = false;
        if let Some(monitor) = &self.monitor {
            let msg = monitor.reply_rx.try_recv();
            // let text = format!("{msg:#?}");
            match msg {
                Ok(MonitorReply::MonitorStatus(status)) if status.heart_rate_bpm > 0 => {
                    if self.plot_bpm_low == 0 && self.plot_bpm_high == 0 {
                        self.plot_bpm_low = (status.heart_rate_bpm as u8).saturating_sub(5);
                    } else {
                        self.plot_bpm_low = self
                            .plot_bpm_low
                            .min((status.heart_rate_bpm as u8).saturating_sub(5));
                    }

                    self.plot_bpm_high = self
                        .plot_bpm_high
                        .max((status.heart_rate_bpm as u8).saturating_add(5));

                    if self.hr_history.len() == self.hr_history.capacity() {
                        self.hr_history.pop();
                    }
                    self.hr_history.insert(
                        0,
                        PlotPoint {
                            x: 0,
                            y: status.heart_rate_bpm as i32,
                        },
                    );

                    self.hr_history
                        .iter_mut()
                        .enumerate()
                        .for_each(|(index, point)| point.x = index as i32);
                    // self.display.fill_solid(&self.hr_bound, Rgb565::BLACK)?;
                    let bpm_string = format!(
                        // "{}",
                        "{:3}",
                        status.heart_rate_bpm
                    );
                    let text = Text::with_text_style(
                        &bpm_string,
                        // Point::new(240 / 2, 150),
                        // Point::new(0, 60),
                        Point::new(60, 60),
                        bpm_style,
                        left_style,
                    );

                    // self.hr_bound = text.bounding_box();

                    // info!("{:?}", self.hr_bound);

                    let width = self.hr_canvas.size().width;
                    self.hr_canvas
                        .pixels
                        .iter_mut()
                        .enumerate()
                        .for_each(|(index, color)| {
                            let coords = (index % width as usize, index / width as usize);
                            if color.is_some() && coords.0 > 50 {
                                *color = Some(BinaryColor::Off);
                            }
                        });

                    _ = text.draw(&mut self.hr_canvas);

                    let mut curve = Curve::from_data(self.hr_history.as_slice());
                    // let curve_list = [(curve, BinaryColor::On)];
                    curve.x_range = 0..self.hr_history.capacity() as i32;
                    curve.y_range = self.plot_bpm_low as i32..self.plot_bpm_high as i32;
                    _ = curve
                        .into_drawable_curve(&Point { x: 165, y: 0 }, &Point { x: 240, y: 60 })
                        .set_color(BinaryColor::On)
                        .set_thickness(3)
                        .draw(&mut self.hr_canvas);

                    self.display.fill_contiguous(
                        &NUMERIC_BOUND,
                        self.hr_canvas.pixels.iter().map(|p| {
                            match p {
                                Some(BinaryColor::On) => Rgb565::RED,
                                Some(BinaryColor::Off) => Rgb565::BLACK,
                                None => Rgb565::BLACK,
                                // Some(BinaryColor::Off) => Rgb565::new(50, 0, 0),
                            }
                            // if let Some(BinaryColor::On) = p {
                            //     Rgb565::RED
                            // } else {
                            //     Rgb565::BLACK
                            // }
                        }),
                    )?;

                    // let plot = SinglePlot::new(
                    // &curve_list,
                    // Scale::RangeFraction(3),
                    // Scale::RangeFraction(2),
                    // )
                    // .into_drawable(Point { x: 18, y: 2 }, Point { x: 120, y: 30 })
                    // .set_color(BinaryColor::On);

                    // plot.draw(&mut self.display.color_converted())?;
                }
                Ok(msg) => (),
                Err(TryRecvError::Empty) => (),
                Err(TryRecvError::Disconnected) => {
                    // Monitor disconnected or errored!
                    error!("Monitor disconnected or errored! Rebuilding...");
                    rebuild_monitor = true;
                }
            }
        }

        let slideshow_enabled = self.settings.slideshow_length_sec != SlideshowLength::Off;
        let image_count = self.image_count;

        Ok(())
    }

    fn main_menu(&mut self) -> Result<()> {
        let options_offset = Point::new(20, 50);
        if self.paint_check() {
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
                let mut button_text = item.to_string();
                if matches!(item, MainMenu::Slideshow) {
                    button_text.push_str(&format!(" ({})", self.settings.slideshow_length_sec));
                }
                if let Ok(point) = Text::new(
                    &button_text,
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
                "To clear settings,\nhold inner button _after_ powering on.",
                Point::new(160, 220),
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
        }

        Ok(())
    }

    pub fn main_loop(&mut self) -> Result<()> {
        match self.view {
            AppView::Doodle => {
                // self.doodle()?;
            }
            AppView::MainMenu => {
                self.main_menu()?;
            }
            AppView::NameInput => {
                // self.name_input()?;
            }
            AppView::HrSelect => {
                // self.hr_select()?;
            }
            AppView::BadgeDisplay => {
                self.badge_view()?;
            }
        }

        Ok(())
    }
    pub fn change_view(&mut self, new_view: AppView) -> Result<()> {
        self.repaint_full()?;
        self.view = new_view;
        // self.debounce_instant = Instant::now();

        let character_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);
        let text_style = TextStyleBuilder::new().alignment(Alignment::Center).build();
        // Extra actions based on new view
        match self.view {
            // AppView::BadgeDisplay => {
            //     self.set_display_to_vertical()?;
            //     if let Some(addr) = self.settings.hr.saved.as_ref() {
            //         Text::with_text_style(
            //             "Trying to find\nsaved HR monitor!\nGiving up in 30s...\n\n\nTrash saved device\nto skip this.",
            //             Point::new(240 / 2, 100),
            //             character_style,
            //             text_style,
            //         )
            //         .draw(&mut self.display)?;
            //         // let (hr_tx, hr_rx) = mpsc::sync_channel(5);
            //         // self.hr_rx = Some(hr_rx);
            //         // let res =
            //         //     block_on(async { self.ble.connect_to_monitor(addr.mac, hr_tx).await });

            //         let addr = block_on(async { self.ble.scan_for_connect(addr).await })?;

            //         if let Some(addr) = addr {
            //             let monitor = MonitorHandle::build(addr, self.delay)?;
            //             if let Ok(MonitorReply::Error(err)) = monitor
            //                 .reply_rx
            //                 .recv_timeout(std::time::Duration::from_secs(30))
            //             {
            //                 // self.clear_vertical()?;
            //                 Text::with_text_style(
            //                     &format!("{err}"),
            //                     Point::new(240 / 2, 120),
            //                     character_style,
            //                     text_style,
            //                 )
            //                 .draw(&mut self.display);
            //                 self.delay.delay_ms(10000);
            //                 panic!();
            //             }

            //             self.monitor = Some(monitor);
            //             // info!("{:?}", self.monitor)
            //         } else {
            //             // self.clear_vertical()?;
            //             Text::with_text_style(
            //                 &format!("HR Monitor not found!"),
            //                 Point::new(240 / 2, 120),
            //                 character_style,
            //                 text_style,
            //             )
            //             .draw(&mut self.display);
            //             self.delay.delay_ms(5000);
            //         }
            //     }
            //     info!("Done.");
            //     // self.clear_vertical()?;
            //     self.debounce_duration = Duration::from_millis(1000);
            // }
            AppView::MainMenu => {
                // self.ble.discovered.clear();
                // self.debounce_duration = Duration::from_millis(500);
            }
            AppView::NameInput => {
                // self.debounce_duration = Duration::from_millis(100);
                self.username_scratch.clone_from(&self.settings.username);
            }
            AppView::HrSelect => {
                // Text::with_text_style(
                //     "Scanning for BLE HR Monitors...\nPlease wait 10s...",
                //     Point::new(320 / 2, 240 / 2),
                //     character_style,
                //     text_style,
                // )
                // .draw(&mut self.display)?;

                // self.ble.discovered = block_on(async { self.ble.scan_for_select().await })?;

                // info!("{:?}", self.ble.discovered);

                // // Filtering out all the nameless monitors
                // // (easy enough to just have the user rescan)
                // self.ble.discovered.retain(|_, name| !name.is_empty());

                // self.ble.chosen_discovered = 0;

                // // Repaint again since we drew here
                // self.repaint_full()?;
                // // self.change_view(AppView::MainMenu)?;
            }
            _ => (),
        }
        // while let Ok(_) = self.touch_rx.try_recv() {}
        Ok(())
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
    // fn set_display_to_vertical(&mut self) -> Result<()> {
    //     let new = Rotation::Deg0;
    //     self.display
    //         .set_orientation(Orientation::new().rotate(new))?;
    //     Ok(())
    // }
    // fn set_display_to_horizontal(&mut self) -> Result<()> {
    //     let new = Rotation::Deg90;
    //     self.display
    //         .set_orientation(Orientation::new().rotate(new))?;
    //     Ok(())
    // }
    // /// Since `mipidsi::Display::set_orientation` is borked.
    // fn clear_vertical(&mut self) -> Result<()> {
    //     self.set_display_to_horizontal()?;
    //     self.display.clear(Rgb565::BLACK);
    //     self.set_display_to_vertical()?;
    //     Ok(())
    // }
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
    #[strum(to_string = "Slideshow")]
    Slideshow,
    #[strum(to_string = "BLE HR Monitor Selection")]
    HrSelect,
    Doodle,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    strum_macros::VariantArray,
    strum_macros::Display,
    Serialize,
    Deserialize,
)]
pub enum SlideshowLength {
    #[default]
    Off,
    #[strum(to_string = "5s")]
    FiveSec,
    #[strum(to_string = "10s")]
    TenSec,
    #[strum(to_string = "30s")]
    ThirtySec,
    #[strum(to_string = "1m")]
    OneMin,
    #[strum(to_string = "3m")]
    ThreeMin,
}

// impl From<SlideshowLength> for Duration {
//     fn from(value: SlideshowLength) -> Self {
//         match value {
//             SlideshowLength::Off => Duration::from_secs(0),
//             SlideshowLength::FiveSec => Duration::from_secs(5),
//             SlideshowLength::TenSec => Duration::from_secs(10),
//             SlideshowLength::ThirtySec => Duration::from_secs(30),
//             SlideshowLength::OneMin => Duration::from_secs(60),
//             SlideshowLength::ThreeMin => Duration::from_secs(60 * 3),
//         }
//     }
// }

// impl From<&SlideshowLength> for Duration {
//     fn from(value: &SlideshowLength) -> Self {
//         Self::from(*value)
//     }
// }

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
