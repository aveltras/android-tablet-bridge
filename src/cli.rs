use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read},
    os::unix::{fs::OpenOptionsExt, net::UnixStream},
};

use adb_client::{ADBDeviceExt, ADBServer, ADBServerDevice};
use clap::{Parser, ValueEnum};
use cli_table::{print_stdout, Cell, Style, Table};
use input_linux::{
    sys::uinput_abs_setup, AbsoluteAxis, Event, EventKind, InputId, Key, UInputHandle,
};
use nix::libc::O_NONBLOCK;

use crate::parser::{parse_devices, parse_input_event, ADBDevice};

#[derive(Parser, Debug)]
enum AppCli {
    ListDevice,
    ListSubDevice(ListSubDeviceArgs),
    Forward(ForwardArgs),
}

#[derive(clap::Args, Debug)]
#[command(version, about, long_about = None)]
struct ListSubDeviceArgs {
    #[arg(long)]
    device: String,
}

#[derive(clap::Args, Debug)]
#[command(version, about, long_about = None)]
struct ForwardArgs {
    #[arg(long)]
    device: Option<String>,

    #[arg(long)]
    subdevice: Option<String>,

    #[arg(long, default_value_t = String::from("Android Tablet Bridge"))]
    name: String,

    #[arg(long)]
    rotation: Option<Rotation>,

    #[arg(long, default_value_t = 10)]
    fallback_resolution: i32,
}

pub fn run() -> Result<(), io::Error> {
    match AppCli::parse() {
        AppCli::ListDevice => list_device(),
        AppCli::ListSubDevice(command_args) => list_subdevice(command_args),
        AppCli::Forward(command_args) => forward(command_args),
    }
}

fn list_device() -> Result<(), io::Error> {
    let mut server = ADBServer::default();
    let devices = server.devices_long().unwrap();
    let mut lines = vec![];

    for device in devices {
        lines.push(vec![
            device.identifier.cell(),
            device.usb.cell(),
            device.product.cell(),
            device.model.cell(),
            device.device.cell(),
        ])
    }

    let table = lines
        .table()
        .title(vec![
            "Identifier".cell().bold(true),
            "USB".cell().bold(true),
            "Product".cell().bold(true),
            "Model".cell().bold(true),
            "Device".cell().bold(true),
        ])
        .bold(true);

    print_stdout(table)
}

fn list_subdevice(args: ListSubDeviceArgs) -> Result<(), io::Error> {
    let mut server = ADBServer::default();
    let mut server_device = server
        .get_device_by_name(&args.device)
        .expect("Could not get device");

    let (write_end, read_end) = UnixStream::pair().unwrap();

    server_device
        .shell_command(["getevent", "-p"], write_end)
        .unwrap();

    let mut reader = BufReader::new(read_end);
    let mut response = String::new();
    reader.read_to_string(&mut response).expect("toc");

    let (_, devices) = parse_devices(&response).expect("Could not parse device info");

    let mut lines = vec![];

    for device in devices {
        let keys: Vec<String> = device
            .events
            .keys
            .iter()
            .map(|x| format!("{:?}", x))
            .collect();

        lines.push(vec![
            device.name.cell(),
            device.path.cell(),
            keys.join(", ").cell(),
        ])
    }

    let table = lines
        .table()
        .title(vec![
            "Identifier".cell().bold(true),
            "Path".cell().bold(true),
            "Keys".cell().bold(true),
        ])
        .bold(true);

    print_stdout(table)
}

fn forward(args: ForwardArgs) -> Result<(), io::Error> {
    let identify_args = match (args.device, args.subdevice) {
        (Some(device), Some(subdevice)) => {
            IdentityTabletDeviceArgs::DeviceAndSubdevice(device, subdevice)
        }
        (Some(device), None) => IdentityTabletDeviceArgs::Device(device),
        (None, None) => IdentityTabletDeviceArgs::Automatic,
        (None, Some(_)) => panic!("Device identifier must be provided when subdevice is given"),
    };
    let device_opt = identify_tablet_device(identify_args);

    match device_opt {
        None => panic!("Could not identify tablet device with provided arguments"),
        Some((mut server_device, device)) => {
            let device_path = device.path.clone();

            let (uhandle, rotation_data_opt) = setup_virtual_input_device(
                device,
                String::from(args.name),
                args.rotation,
                args.fallback_resolution,
            )?;

            let (event_writer_end, event_reader_end) = UnixStream::pair().unwrap();
            std::thread::spawn(move || {
                server_device
                    .shell_command(["getevent", "-t", &device_path], event_writer_end)
                    .unwrap();
            });

            let event_reader = BufReader::new(event_reader_end);

            for line in event_reader.lines() {
                if let Ok(ref line) = line {
                    if let Some(event) = translate_device_event(&rotation_data_opt, &line) {
                        uhandle.write(&[event.into_event().into_raw()]).unwrap();
                    }
                }
            }

            uhandle.dev_destroy().unwrap();

            Ok(())
        }
    }
}

fn translate_device_event(
    rotation_data_opt: &Option<RotationData>,
    raw_event: &str,
) -> Option<Event> {
    match parse_input_event(&raw_event) {
        Ok((_, event_opt)) => match event_opt {
            None => None,
            Some(event) => match event {
                Event::Absolute(mut absolute_event) => {
                    match rotation_data_opt {
                        None => {}
                        Some(rotation_data) => match rotation_data.rotation {
                            Rotation::Rotation90 => match absolute_event.axis {
                                AbsoluteAxis::X => {
                                    absolute_event.axis = AbsoluteAxis::Y;
                                    absolute_event.value = absolute_event.value;
                                }
                                AbsoluteAxis::Y => {
                                    absolute_event.axis = AbsoluteAxis::X;
                                    absolute_event.value =
                                        rotation_data.maximum_y - absolute_event.value;
                                }
                                AbsoluteAxis::TiltX => {
                                    absolute_event.axis = AbsoluteAxis::TiltY;
                                    absolute_event.value = absolute_event.value;
                                }
                                AbsoluteAxis::TiltY => {
                                    absolute_event.axis = AbsoluteAxis::TiltX;
                                    absolute_event.value = -absolute_event.value;
                                }
                                _ => {}
                            },
                            Rotation::Rotation180 => match absolute_event.axis {
                                AbsoluteAxis::X => {
                                    absolute_event.value =
                                        rotation_data.maximum_x - absolute_event.value;
                                }
                                AbsoluteAxis::Y => {
                                    absolute_event.value =
                                        rotation_data.maximum_y - absolute_event.value;
                                }
                                AbsoluteAxis::TiltX => {
                                    absolute_event.value = -absolute_event.value;
                                }
                                AbsoluteAxis::TiltY => {
                                    absolute_event.value = -absolute_event.value;
                                }
                                _ => {}
                            },
                            Rotation::Rotation270 => match absolute_event.axis {
                                AbsoluteAxis::X => {
                                    absolute_event.axis = AbsoluteAxis::Y;
                                    absolute_event.value =
                                        rotation_data.maximum_x - absolute_event.value;
                                }
                                AbsoluteAxis::Y => {
                                    absolute_event.axis = AbsoluteAxis::X;
                                    absolute_event.value = absolute_event.value;
                                }
                                AbsoluteAxis::TiltX => {
                                    absolute_event.axis = AbsoluteAxis::TiltY;
                                    absolute_event.value = -absolute_event.value;
                                }
                                AbsoluteAxis::TiltY => {
                                    absolute_event.axis = AbsoluteAxis::TiltX;
                                    absolute_event.value = absolute_event.value;
                                }
                                _ => {}
                            },
                        },
                    }

                    Some(Event::Absolute(absolute_event))
                }
                _ => Some(event),
            },
        },
        Err(err) => {
            eprintln!("Got error while parsing input event: {}", err);
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, ValueEnum)]
enum Rotation {
    Rotation90,
    Rotation180,
    Rotation270,
}

struct RotationData {
    rotation: Rotation,
    maximum_x: i32,
    maximum_y: i32,
}

enum IdentityTabletDeviceArgs {
    Automatic,
    Device(String),
    DeviceAndSubdevice(String, String),
}

fn identify_tablet_device(args: IdentityTabletDeviceArgs) -> Option<(ADBServerDevice, ADBDevice)> {
    let mut server = ADBServer::default();

    let mut subdevice_identifier: Option<String> = None;

    let identifiers: Vec<String> = match args {
        IdentityTabletDeviceArgs::Automatic => server
            .devices_long()
            .unwrap()
            .iter()
            .map(|x| x.identifier.clone())
            .collect(),
        IdentityTabletDeviceArgs::Device(identifier) => vec![identifier],
        IdentityTabletDeviceArgs::DeviceAndSubdevice(identifier, subident) => {
            subdevice_identifier = Some(subident);
            vec![identifier]
        }
    };

    let mut found_device = None;

    'device_loop: for identifier in identifiers {
        let mut server_device = server
            .get_device_by_name(&identifier)
            .expect("Could not get device");

        let (write_end, read_end) = UnixStream::pair().unwrap();

        server_device
            .shell_command(["getevent", "-p"], write_end)
            .unwrap();

        let mut reader = BufReader::new(read_end);
        let mut response = String::new();
        reader.read_to_string(&mut response).expect("toc");

        let (_, subdevices) = parse_devices(&response).expect("Could not parse device info");

        for device in subdevices {
            if (Some(device.name.to_owned()) == subdevice_identifier)
                || subdevice_identifier.is_none()
            {
                if device.events.keys.contains(&Key::ButtonToolPen) {
                    found_device = Some((server_device, device));
                    break 'device_loop;
                }
            }
        }
    }

    found_device
}

fn setup_virtual_input_device(
    device: ADBDevice,
    virtual_name: String,
    rotation_opt: Option<Rotation>,
    fallback_resolution: i32,
) -> Result<(UInputHandle<File>, Option<RotationData>), io::Error> {
    let uinput_file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open("/dev/uinput")
        .unwrap();

    let uhandle = UInputHandle::new(uinput_file);

    if !device.events.keys.is_empty() {
        uhandle.set_evbit(EventKind::Key)?;
        for key in device.events.keys {
            uhandle.set_keybit(key)?;
        }
    }

    let mut rotation_data_opt: Option<RotationData> = None;

    if !device.events.absolute.is_empty() {
        uhandle.set_evbit(EventKind::Absolute)?;

        let mut maximum_x_opt: Option<i32> = None;
        let mut maximum_y_opt: Option<i32> = None;

        for mut abs_setup in device.events.absolute {
            if abs_setup.info.resolution == 0 {
                abs_setup.info.resolution = fallback_resolution;
            }

            if let Some(ref rotation) = rotation_opt {
                if abs_setup.axis == AbsoluteAxis::X {
                    maximum_x_opt = Some(abs_setup.info.maximum);
                    if *rotation == Rotation::Rotation90 || *rotation == Rotation::Rotation270 {
                        abs_setup.axis = AbsoluteAxis::Y;
                    }
                } else if abs_setup.axis == AbsoluteAxis::Y {
                    maximum_y_opt = Some(abs_setup.info.maximum);
                    if *rotation == Rotation::Rotation90 || *rotation == Rotation::Rotation270 {
                        abs_setup.axis = AbsoluteAxis::X;
                    }
                }
            }

            uhandle.abs_setup(&uinput_abs_setup::from(abs_setup))?;
            uhandle.set_absbit(abs_setup.axis)?;
        }

        if let (Some(rotation), Some(maximum_x), Some(maximum_y)) =
            (rotation_opt, maximum_x_opt, maximum_y_opt)
        {
            rotation_data_opt = Some(RotationData {
                rotation,
                maximum_x,
                maximum_y,
            });
        }
    }

    let input_id = InputId {
        bustype: input_linux::sys::BUS_USB,
        vendor: 0x1234,
        product: 0x5678,
        version: 0,
    };

    uhandle.create(&input_id, virtual_name.as_bytes(), 0, &[])?;

    Ok((uhandle, rotation_data_opt))
}
