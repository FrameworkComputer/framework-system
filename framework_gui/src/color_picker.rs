use framework_lib::chromium_ec::commands::RgbS;
use framework_lib::chromium_ec::CrosEc;
use iced::{
    widget::{button, column, row, text, Button, Container, Row, Text},
    Alignment, Color, Element, Length,
};
use iced_aw::number_input;

use iced_aw::helpers::color_picker;

fn main() -> iced::Result {
    iced::application(
        "Framework RGB GUI",
        FrameworkGui::update,
        FrameworkGui::view,
    )
    .font(iced_fonts::REQUIRED_FONT_BYTES)
    .run()
}

#[derive(Clone, Debug)]
#[allow(clippy::enum_variant_names)]
enum Message {
    ChooseColor,
    SubmitColor(Color),
    CancelColor,
    NumInpChanged(u8),
    NumInpSubmitted,
    LedPressed(u8),
    AllOff,
    SetAll,
}

#[derive(Debug)]
struct FrameworkGui {
    color: Color,
    show_picker: bool,
    leds: u8,
}

impl Default for FrameworkGui {
    fn default() -> Self {
        Self {
            // Default color in color picker
            color: Color::from_rgba8(0xFF, 0xFF, 0xFF, 1.0),
            show_picker: false,
            leds: 8,
        }
    }
}
impl FrameworkGui {
    fn update(&mut self, message: Message) {
        match message {
            Message::NumInpChanged(val) => {
                self.leds = val;
            }
            Message::NumInpSubmitted => {}
            Message::AllOff => {
                let ec = CrosEc::new();
                let start_key = 0;
                let colors = vec![RgbS { r: 0, g: 0, b: 0 }]
                    .iter()
                    .cloned()
                    .cycle()
                    .take(self.leds.into())
                    .collect();
                ec.rgbkbd_set_color(start_key, colors).unwrap();
            }
            Message::SetAll => {
                let ec = CrosEc::new();
                let start_key = 0;
                let colors = vec![RgbS {
                    r: (255f32 * self.color.r) as u8,
                    g: (255f32 * self.color.g) as u8,
                    b: (255f32 * self.color.b) as u8,
                }]
                .iter()
                .cloned()
                .cycle()
                .take(self.leds.into())
                .collect();
                ec.rgbkbd_set_color(start_key, colors).unwrap();
            }
            Message::LedPressed(led) => {
                let ec = CrosEc::new();
                let start_key = led;
                let colors = vec![RgbS {
                    r: (255f32 * self.color.r) as u8,
                    g: (255f32 * self.color.g) as u8,
                    b: (255f32 * self.color.b) as u8,
                }];
                ec.rgbkbd_set_color(start_key, colors).unwrap();
            }
            Message::ChooseColor => {
                self.show_picker = true;
            }
            Message::SubmitColor(color) => {
                self.color = color;
                self.show_picker = false;
            }
            Message::CancelColor => {
                self.show_picker = false;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let but = Button::new(Text::new("Pick Color")).on_press(Message::ChooseColor);

        let color_picker = color_picker(
            self.show_picker,
            self.color,
            but,
            Message::CancelColor,
            Message::SubmitColor,
        );

        let number_picker = number_input(&self.leds, 0..=16, Message::NumInpChanged)
            .style(number_input::number_input::primary)
            .on_submit(Message::NumInpSubmitted)
            .step(1);

        let r = (255f32 * self.color.r) as u8;
        let g = (255f32 * self.color.g) as u8;
        let b = (255f32 * self.color.b) as u8;
        let content = row![column![
            row![text!("Number of LEDs"), number_picker].spacing(10),
            row![
                Text::new(format!("Color: #{:02X}{:02X}{:02X}", r, g, b)),
                color_picker,
                button(text("Red"))
                    .on_press(Message::SubmitColor(Color::from_rgba8(255, 0, 0, 1.0))),
                button(text("Green"))
                    .on_press(Message::SubmitColor(Color::from_rgba8(0, 255, 0, 1.0))),
                button(text("Blue"))
                    .on_press(Message::SubmitColor(Color::from_rgba8(0, 0, 255, 1.0))),
                button(text("White"))
                    .on_press(Message::SubmitColor(Color::from_rgba8(255, 255, 255, 1.0))),
                button(text("Black"))
                    .on_press(Message::SubmitColor(Color::from_rgba8(0, 0, 0, 1.0))),
            ]
            .spacing(10),
            ((0..self.leds).fold(Row::new(), |row, i| {
                if i < 8 {
                    row.push(
                        button(text(format!("LED {:>02}", i + 1))).on_press(Message::LedPressed(i)),
                    )
                } else {
                    row
                }
            }))
            .spacing(10),
            ((0..self.leds).fold(Row::new(), |row, i| {
                if i >= 8 {
                    row.push(
                        button(text(format!("LED {:>02}", i + 1))).on_press(Message::LedPressed(i)),
                    )
                } else {
                    row
                }
            }))
            .spacing(10),
            row![
                button(text("All off")).on_press(Message::AllOff),
                button(text("Set all")).on_press(Message::SetAll),
            ]
            .spacing(10)
        ]
        .spacing(10)]
        .align_y(Alignment::Center);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
