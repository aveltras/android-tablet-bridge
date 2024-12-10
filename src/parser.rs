use core::str;

use nom::{
    bytes::complete::{tag, take, take_until, take_while_m_n},
    character::{
        self,
        complete::{
            alpha1, alphanumeric1, digit1, i32, multispace0, multispace1, newline, not_line_ending,
            space0, space1,
        },
        streaming::char,
    },
    combinator::{map_res, opt},
    multi::many1,
    sequence::separated_pair,
    IResult, Parser,
};

use input_linux::{
    AbsoluteAxis, AbsoluteEvent, AbsoluteInfo, AbsoluteInfoSetup, Event, EventKind, EventTime, Key,
    KeyEvent, KeyState, RelativeAxis, SwitchKind,
};

#[derive(Debug, PartialEq)]
pub struct ADBDevice {
    pub path: String,
    pub name: String,
    pub events: ADBDeviceEvents,
}

#[derive(Debug, PartialEq)]
pub struct ADBDeviceEvents {
    pub keys: Vec<Key>,
    pub relative: Vec<RelativeAxis>,
    pub absolute: Vec<AbsoluteInfoSetup>,
    pub switches: Vec<(SwitchKind, bool)>,
}

enum DeviceEvent {
    Keys(Vec<Key>),
    Relative(Vec<RelativeAxis>),
    Absolute(Vec<AbsoluteInfoSetup>),
    Switch(Vec<(SwitchKind, bool)>),
}

pub fn parse_input_event(input: &str) -> IResult<&str, Option<Event>> {
    let (input, time) = parse_event_time(input)?;
    let (input, _) = char(' ')(input)?;
    let (input, event_kind) = parse_event_kind(input)?;
    let (input, _) = char(' ')(input)?;

    match event_kind {
        EventKind::Key => {
            let (input, key) = parse_device_event_key(input)?;
            let (input, _) = char(' ')(input)?;
            let (input, value) = map_res(take_while_m_n(8, 8, is_hex_digit), |input| {
                i32::from_str_radix(input, 16)
            })
            .parse(input)?;
            Ok((
                input,
                Some(Event::Key(KeyEvent::new(time, key, KeyState::from(value)))),
            ))
        }
        EventKind::Absolute => {
            let (input, axis) = parse_absolute_axis(input)?;

            let (input, _) = char(' ')(input)?;

            let (input, hex_value) = take_while_m_n(8, 8, is_hex_digit)(input)?;
            let value = u32::from_str_radix(hex_value, 16).unwrap();

            Ok((
                input,
                Some(Event::Absolute(AbsoluteEvent::new(
                    time,
                    axis,
                    value as i32,
                ))),
            ))
        }
        _ => Ok((input, None)),
    }
}

pub fn parse_devices(input: &str) -> IResult<&str, Vec<ADBDevice>> {
    many1(parse_device)(input)
}

fn parse_event_kind(input: &str) -> IResult<&str, EventKind> {
    map_res(map_res(take(4usize), from_hex), EventKind::from_type)(input)
}

fn parse_event_time(input: &str) -> IResult<&str, EventTime> {
    let (input, _) = char('[')(input)?;
    let (input, _) = space0(input)?;
    let (input, (secs, usecs)) =
        separated_pair(character::complete::i64, tag("."), character::complete::i64)(input)?;
    let (input, _) = char(']')(input)?;
    Ok((input, EventTime::new(secs, usecs)))
}

fn parse_device(input: &str) -> IResult<&str, ADBDevice> {
    let (input, path) = parse_device_path(input)?;
    let (input, name) = parse_device_name(input)?;
    let (input, events) = parse_device_events(input)?;
    let (input, _) = parse_device_input_properties(input)?;

    Ok((
        input,
        ADBDevice {
            path: path.to_owned(),
            name: name.to_owned(),
            events,
        },
    ))
}

fn parse_device_path(input: &str) -> IResult<&str, &str> {
    let (input, _) = tag("add device ")(input)?;
    let (input, _) = digit1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, _) = space1(input)?;
    let (input, path) = not_line_ending(input)?;
    let (input, _) = newline(input)?;
    Ok((input, path))
}

fn parse_device_name(input: &str) -> IResult<&str, &str> {
    let (input, _) = space1(input)?;
    let (input, _) = tag("name:")(input)?;
    let (input, _) = space1(input)?;
    let (input, _) = char('"')(input)?;
    let (input, name) = take_until("\"")(input)?;
    let (input, _) = char('"')(input)?;
    let (input, _) = newline(input)?;
    Ok((input, name))
}

fn parse_device_events(input: &str) -> IResult<&str, ADBDeviceEvents> {
    let (input, _) = space1(input)?;
    let (input, _) = tag("events:")(input)?;
    let (input, _) = newline(input)?;
    let (input, events) = many1(parse_device_event)(input)?;

    let mut device_events = ADBDeviceEvents {
        keys: vec![],
        relative: vec![],
        absolute: vec![],
        switches: vec![],
    };

    for item in events {
        match item {
            None => {}
            Some(item) => match item {
                DeviceEvent::Keys(keys) => device_events.keys = keys,
                DeviceEvent::Relative(info) => device_events.relative = info,
                DeviceEvent::Absolute(setups) => device_events.absolute = setups,
                DeviceEvent::Switch(switches) => device_events.switches = switches,
            },
        }
    }

    Ok((input, device_events))
}

fn parse_device_input_properties(input: &str) -> IResult<&str, ()> {
    let (input, _) = multispace1(input)?;
    let (input, _) = tag("input props:")(input)?;
    let (input, _) = newline(input)?;
    let (input, _) = multispace1(input)?;
    let (input, res) = opt(tag("<none>"))(input)?;
    match res {
        Some(_) => {
            let (input, _) = newline(input)?;
            Ok((input, ()))
        }
        None => {
            // INPUT_PROP_DIRECT
            let (input, _) = many1(parse_device_input_property)(input)?;
            // let (input, _) = newline(input)?;
            Ok((input, ()))
        }
    }
}

fn parse_device_input_property(input: &str) -> IResult<&str, &str> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("INPUT_PROP_")(input)?;
    let (input, prop) = alpha1(input)?;
    let (input, _) = multispace1(input)?;
    Ok((input, prop))
}

fn parse_device_event(input: &str) -> IResult<&str, Option<DeviceEvent>> {
    let (input, _) = multispace1(input)?;
    let (input, _) = alphanumeric1(input)?;
    let (input, _) = space1(input)?;
    let (input, _) = char('(')(input)?;
    let (input, event_kind) = parse_event_kind(input)?;
    let (input, _) = char(')')(input)?;
    let (input, _) = char(':')(input)?;

    match event_kind {
        EventKind::Key => {
            let (input, keys) = many1(parse_device_event_key)(input)?;
            Ok((input, Some(DeviceEvent::Keys(keys))))
        }
        EventKind::Absolute => {
            let (input, keys) = many1(parse_absolute_info_setup)(input)?;
            Ok((input, Some(DeviceEvent::Absolute(keys))))
        }
        EventKind::Relative => {
            let (input, keys) = many1(parse_device_event_relative)(input)?;
            Ok((input, Some(DeviceEvent::Relative(keys))))
        }
        EventKind::Switch => {
            let (input, keys) = many1(parse_device_event_switch)(input)?;
            Ok((input, Some(DeviceEvent::Switch(keys))))
        }
        _ => Ok((input, None)),
    }
}

fn parse_absolute_axis(input: &str) -> IResult<&str, AbsoluteAxis> {
    map_res(map_res(take(4usize), from_hex), AbsoluteAxis::from_code)(input)
}

fn parse_absolute_info_setup(input: &str) -> IResult<&str, AbsoluteInfoSetup> {
    let (input, _) = multispace0(input)?;
    let (input, absolute_axis) = parse_absolute_axis(input)?;
    let (input, _) = space1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, _) = tag(" value ")(input)?;
    let (input, value) = i32(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = tag(" min ")(input)?;
    let (input, minimum) = i32(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = tag(" max ")(input)?;
    let (input, maximum) = i32(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = tag(" fuzz ")(input)?;
    let (input, fuzz) = i32(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = tag(" flat ")(input)?;
    let (input, flat) = i32(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = tag(" resolution ")(input)?;
    let (input, resolution) = i32(input)?;
    let (input, _) = newline(input)?;

    Ok((
        input,
        AbsoluteInfoSetup {
            axis: absolute_axis,
            info: AbsoluteInfo {
                value,
                minimum,
                maximum,
                fuzz,
                flat,
                resolution,
            },
        },
    ))
}

fn parse_device_event_key(input: &str) -> IResult<&str, Key> {
    let (input, _) = multispace0(input)?;
    map_res(map_res(take(4usize), from_hex), Key::from_code)(input)
}

fn parse_device_event_relative(input: &str) -> IResult<&str, RelativeAxis> {
    let (input, _) = multispace1(input)?;
    map_res(map_res(take(4usize), from_hex), RelativeAxis::from_code)(input)
}

fn parse_device_event_switch(input: &str) -> IResult<&str, (SwitchKind, bool)> {
    let (input, _) = multispace1(input)?;
    let (input, switch) = map_res(map_res(take(4usize), from_hex), SwitchKind::from_code)(input)?;
    let (input, switch_value) = opt(char('*'))(input)?;
    Ok((input, (switch, switch_value.is_some())))
}

fn is_hex_digit(c: char) -> bool {
    c.is_digit(16)
}

fn from_hex(input: &str) -> Result<u16, std::num::ParseIntError> {
    u16::from_str_radix(input, 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_parses_events() {
        let data = include_str!("../events.txt");

        for line in data.lines() {
            if let Err(err) = parse_input_event(line) {
                panic!("Parsing events should succeed, got error: {}", err)
            }
        }
    }

    #[test]
    fn it_parses_devices() {
        let data = include_str!("../devices.txt");

        match parse_devices(data) {
            Err(err) => {
                panic!("Parsing devices should succeed, got error: {}", err)
            }
            Ok((input, devices)) => {
                assert_eq!(
                    input, "",
                    "There should be no input left when parsing devices"
                );
                assert_eq!(
                    devices.len(),
                    13,
                    "There should have been 13 devices parsed"
                );
                let device_opt = devices.iter().find(|x| x.name == "sec_e-pen");

                match device_opt {
                    None => panic!("Device 'sec_e-pen' should have been found in the data"),
                    Some(device) => {
                        assert_eq!(
                            device,
                            &ADBDevice {
                                path: String::from("/dev/input/event9"),
                                name: "sec_e-pen".to_owned(),
                                events: ADBDeviceEvents {
                                    relative: vec![],
                                    keys: vec![
                                        Key::Homepage,
                                        Key::UnknownFD,
                                        Key::ButtonToolPen,
                                        Key::ButtonToolRubber,
                                        Key::ButtonTouch,
                                        Key::ButtonStylus,
                                    ],
                                    absolute: vec![
                                        AbsoluteInfoSetup {
                                            axis: AbsoluteAxis::X,
                                            info: AbsoluteInfo {
                                                value: 0,
                                                minimum: 0,
                                                maximum: 19589,
                                                fuzz: 4,
                                                flat: 0,
                                                resolution: 0
                                            }
                                        },
                                        AbsoluteInfoSetup {
                                            axis: AbsoluteAxis::Y,
                                            info: AbsoluteInfo {
                                                value: 0,
                                                minimum: 0,
                                                maximum: 31376,
                                                fuzz: 4,
                                                flat: 0,
                                                resolution: 0
                                            }
                                        },
                                        AbsoluteInfoSetup {
                                            axis: AbsoluteAxis::Pressure,
                                            info: AbsoluteInfo {
                                                value: 0,
                                                minimum: 0,
                                                maximum: 4095,
                                                fuzz: 0,
                                                flat: 0,
                                                resolution: 0
                                            }
                                        },
                                        AbsoluteInfoSetup {
                                            axis: AbsoluteAxis::Distance,
                                            info: AbsoluteInfo {
                                                value: 0,
                                                minimum: 0,
                                                maximum: 255,
                                                fuzz: 0,
                                                flat: 0,
                                                resolution: 0
                                            }
                                        },
                                        AbsoluteInfoSetup {
                                            axis: AbsoluteAxis::TiltX,
                                            info: AbsoluteInfo {
                                                value: -11,
                                                minimum: -63,
                                                maximum: 63,
                                                fuzz: 0,
                                                flat: 0,
                                                resolution: 0
                                            }
                                        },
                                        AbsoluteInfoSetup {
                                            axis: AbsoluteAxis::TiltY,
                                            info: AbsoluteInfo {
                                                value: 2,
                                                minimum: -63,
                                                maximum: 63,
                                                fuzz: 0,
                                                flat: 0,
                                                resolution: 0
                                            }
                                        },
                                    ],
                                    switches: vec![
                                        (SwitchKind::LineInInsert, true),
                                        (SwitchKind::PenInserted, true),
                                        (SwitchKind::MachineCover, false)
                                    ]
                                }
                            }
                        )
                    }
                }
            }
        }
    }
}
