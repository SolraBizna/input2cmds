This is a simple utility that maps Linux `/dev/input` events to shell commands. Note that this will only work on OSes that support Linux's `/dev/input` interface. **Windows and macOS definitely don't**, though Windows might if you run it through WSL.

Installation
============

Recent Rust is required. If you don't have it, you can quickly install a Rust toolchain with this command:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Download the source code (if you haven't already):

```sh
git clone https://github.com/SolraBizna/input2cmds
```

Install using Cargo:

```sh
cargo install --path input2cmds
```

Quick Start
===========

(The below information is also available by running `input2cmds` with no arguments.)

To get started with `input2cmds`, create a configuration file. The file can be named anything you want. Put one or more "dev" directives inside the file, like so:

```
dev /dev/input/by-id/usb-Gamepad_Name_Goes_Here_USB-event-joystick
```

Make sure you specify an "event-joystick" device and not a "joystick" device here. Also, be aware that input2cmds doesn't distinguish between input devices (so you can't map the same button on different gamepads to different things, for example).

Once that's done, run input2cmds with the -v option and pass it the path to your configuration file. It will produce output like:

```
if type=x code=y value=z then: ...
```

If one of those type/code/value combinations corresponds to a button you want
to map, then you can paste that line into the configuration file, and replace
... with the command you want to execute. input2cmds will wait until the
command has fully executed before executing any further commands (unless you
put a & on the end).

Example Configuration
---------------------

```ini
# Lines beginning with # are comments, which are ignored by input2cmds and
# exist only to inform the person reading the file.

dev /dev/input/by-id/usb-Gravis_GamePad_Pro_USB-event-joystick

# Green → Media Play/Pause
if type=1 code=306 value=1 then: xdotool key XF86AudioPlay

# D-Pad Up → Volume Up
if type=3 code=1 value=0 then: xdotool key XF86AudioRaiseVolume
# D-Pad Down → Volume Down
if type=3 code=1 value=255 then: xdotool key XF86AudioLowerVolume

# D-Pad Left → Keyboard Left
if type=3 code=0 value=0 then: xdotool key Left
# D-Pad Right → Keyboard Right
if type=3 code=0 value=255 then: xdotool key Right

# L1 → Media Prev
if type=1 code=308 value=1 then: xdotool key XF86AudioPrev
# R1 → Media Next
if type=1 code=309 value=1 then: xdotool key XF86AudioNext

# Start → Suspend system
if type=1 code=313 value=1 then: systemctl -i suspend

# R2 → kill Chrome :)
if type=1 code=311 value=1 then: killall chrome
```

License
=======

This program is distributed under the zlib license. This puts very few restrictions on use. See [`LICENSE.md`](LICENSE.md) for the complete, very short text of the copyright notice and license.
