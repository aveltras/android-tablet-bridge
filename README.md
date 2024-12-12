# android-tablet-bridge

This program allows you to use your android tablet on your Linux host by forwarding its input using ADB.  
Currently only tested on a Samsung Galaxy Tab S10 Ultra but should be device agnostic.

**Linux only.**

## Requirements

[ADB](https://developer.android.com/tools/adb) must be available on your host and you must be authorized to query devices with it.  
This program creates a virtual tablet device using the kernel **uinput** feature.  
[Developer options](https://developer.android.com/studio/debug/dev-options) should be enabled on your device and USB debugging enabled (should also work with wireless debugging).

## Installation

Clone the repository and run in its root directory

```sh
cargo install --path .
```

You can also use it directly without installation if you have the [Nix package manager](https://nixos.org/) installed

```sh
nix --extra-experimental-features "nix-command flakes" run github:aveltras/android-tablet-bridge -- forward
```

## Usage

```
Usage: android-tablet-bridge <COMMAND>

Commands:
  list-device      
  list-sub-device  
  forward          
  help             Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

The main command is `forward` one

```
Usage: android-tablet-bridge forward [OPTIONS]

Options:
      --device <DEVICE>
          
      --subdevice <SUBDEVICE>
          
      --name <NAME>
          [default: "Android Tablet Bridge"]
      --rotation <ROTATION>
          [possible values: rotation90, rotation180, rotation270]
      --fallback-resolution <FALLBACK_RESOLUTION>
          [default: 10]
  -h, --help
          Print help
  -V, --version
          Print version
```

All arguments are optional as the program should automatically detect the right device to use (currently search for a device with a **ButtonToolPen** available).  
The program automatically looks for a suitable device to forward but you can specify which one to use, in order to identify it, you can use `list-device` and `list-sub-device` commands.  
The `rotation` parameter is useful if you want to use your device in a different orientation than the default one.  

## Companion app

You can download a companion android app in the [releases](https://github.com/aveltras/android-tablet-bridge/releases/latest) section.  
This is simply a starter app where I have made the screen black and kept it turn on for easier use.  
Source for this app are located in the `android` directory of this repo.
