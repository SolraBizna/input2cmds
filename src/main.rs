//! This is a simple utility that maps Linux `/dev/input` events to shell
//! commands. See [the README][1] for more information.
//!
//! [1]: https://github.com/SolraBizna/input2cmds/blob/master/README.md

use std::{
    io::{Read, BufRead, BufReader},
    process::{exit, Command},
    sync::mpsc::{channel, Sender},
    thread::spawn,
};
use libc::input_event as InputEvent;

use anyhow::{anyhow, Context};

/// Contains a parsed "if ... then ..." line, describing a command to execute
/// if a certain event is seen.
#[derive(Clone,Debug,PartialEq,Eq,PartialOrd,Ord)]
struct InputMatch {
    /// If not `None`, run this command only if the event type matches this
    /// value.
    wants_type: Option<u16>,
    /// If not `None`, run this command only if the event code matches this
    /// value.
    wants_code: Option<u16>,
    /// If not `None`, run this command only if the event value matches this
    /// value.
    wants_value: Option<i32>,
    /// If all of the above fields matched (or were `None`), run this command
    /// via `/bin/sh -c command_to_run`.
    command_to_run: String,
}

/// Reads a configuration file. For every "dev" directive, opens the given
/// device and spawns a reader thread that sends events via `event_sender`. For
/// every "if" directive, adds a match to the `matches` vector.
fn load_config(path: &str, event_sender: &Sender<InputEvent>,
               matches: &mut Vec<InputMatch>) -> anyhow::Result<()> {
    let f = std::fs::File::open(path).context("opening the file")?;
    let reader = BufReader::new(f);
    let mut line_number: usize = 0;
    for line in reader.lines() {
        line_number = line_number + 1;
        let line = line.context("reading from the file")?;
        let line = line.split('#').next().unwrap_or("");
        let (line, colon) = if let Some(colon_pos) = line.find(':') {
            let mut colon = &line[colon_pos+1..];
            while !colon.is_empty() && colon.chars().next().unwrap()
                .is_whitespace() {
                colon = &colon[1..];
            }
            (&line[..colon_pos], Some(colon))
        }
        else {
            (line, None)
        };
        let mut splat: Vec<&str> = line.split(char::is_whitespace).collect();
        if let Some(colon) = colon { splat.push(colon) }
        if splat.is_empty() || splat[0].is_empty() { continue }
        match splat[0] {
            "dev" => {
                if splat.len() != 2 {
                    return Err(anyhow!("{}:{}: dev wants only one parameter",
                                       path, line_number));
                }
                let event_sender = event_sender.clone();
                let dev_path = splat[1].to_owned();
                let dev_file = std::fs::File::open(&dev_path)
                    .with_context(|| format!("opening device {:?}",dev_path))?;
                spawn(move || {
                    let error = format!("Error reading from {:?}", dev_path);
                    let mut dev_file = BufReader::new(dev_file);
                    const EVENT_SIZE: usize
                        = std::mem::size_of::<InputEvent>();
                    let mut buf = [0u8; EVENT_SIZE];
                    loop {
                        dev_file.read_exact(&mut buf[..]).expect(&error);
                        let event: &InputEvent = unsafe {
                            std::mem::transmute(&buf)
                        };
                        match event.type_ {
                            0 /* EV_SYN */ | 4 /* EV_MSC */ => continue,
                            _ => (),
                        }
                        if !event_sender.send(*event).is_ok() {
                            // quietly end the loop, our parent thread is no
                            // longer listening :(
                            break
                        }
                    }
                });
            },
            "if" => {
                let mut rest = &splat[1..];
                let mut wants_type = None;
                let mut wants_code = None;
                let mut wants_value = None;
                while !rest.is_empty() && rest[0] != "then" {
                    let el = rest[0];
                    rest = &rest[1..];
                    if el.starts_with("type=") {
                        if wants_type.is_some() {
                            return Err(anyhow!("{}:{}: multiple \"type=\"s",
                                               path, line_number));
                        }
                        let parsed = &el[5..].parse();
                        match parsed {
                            Err(_) => {
                                return Err(anyhow!("{}:{}: invalid \"type=\"",
                                                   path, line_number));
                            },
                            Ok(x) => {
                                wants_type = Some(*x);
                            }
                        }
                    }
                    else if el.starts_with("code=") {
                        if wants_code.is_some() {
                            return Err(anyhow!("{}:{}: multiple \"code=\"",
                                               path, line_number));
                        }
                        let parsed = &el[5..].parse();
                        match parsed {
                            Err(_) => {
                                return Err(anyhow!("{}:{}: invalid \"code=\"s",
                                                   path, line_number));
                            },
                            Ok(x) => {
                                wants_code = Some(*x);
                            }
                        }
                    }
                    else if el.starts_with("value=") {
                        if wants_value.is_some() {
                            return Err(anyhow!("{}:{}: multiple \"value=\"s",
                                               path, line_number));
                        }
                        let parsed = &el[6..].parse();
                        match parsed {
                            Err(_) => {
                                return Err(anyhow!("{}:{}: invalid \"value=\"",
                                                   path, line_number));
                            },
                            Ok(x) => {
                                wants_value = Some(*x);
                            }
                        }
                    }
                    else {
                        return Err(anyhow!("{}:{}: wanted \"type=\", \
                                            \"code=\", \"value\"=, or \
                                            \"then\" after \"if\", saw {:?}",
                                           path, line_number, el));
                    }
                }
                rest = &rest[1..]; // skip "then"
                if rest.is_empty() {
                    return Err(anyhow!("{}:{}: \"if\" needs a \"then\"",
                                       path, line_number));
                }
                else if rest.len() >= 2 {
                    return Err(anyhow!("{}:{}: put a colon after \"then\"",
                                       path, line_number));
                }
                matches.push(InputMatch {
                    wants_type, wants_code, wants_value,
                    command_to_run: rest[0].to_owned()
                })
            },
            x => {
                return Err(anyhow!("{}:{}: Unknown config directive {:?}",
                                   path, line_number, x));
            },
        }
    }
    // All done!
    Ok(())
}

/// Prints a usage string.
fn print_usage(program_name: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [OPTIONS] path/to/config_file.conf \
                         [ ... more configs ... ]", program_name);
    print!("{}", opts.usage(&brief));
}

/// The main function of the program. Parses the command line, calls
/// [`load_config`](fn.load_config.html) as needed, and then loops reading
/// events and attempting to match them.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program_name = args[0].clone();
    let mut opts = getopts::Options::new();
    opts.optflag("h", "help", "");
    opts.optflag("?", "usage", "Print this help.");
    opts.optflag("v", "verbose", "Print out all received events, and the \
                                  commands that they execute (great for if \
                                  you're still editing your configuration)");
    let matches = match opts.parse(&args[1..]) {
        Ok(x) => x,
        Err(x) => {
            eprintln!("Error parsing command line: {}", x);
            print_usage(&program_name, opts);
            exit(1)
        },
    };
    if matches.opt_present("?") || matches.opt_present("h") {
        print_usage(&program_name, opts);
        exit(0);
    }
    let verbose = matches.opt_present("v");
    let free = matches.free;
    if free.is_empty() {
        print!(r#"
To get started with input2cmds, create a configuration file. The file can be
named anything you want. Put one or more "dev" directives inside the file,
like so:

dev /dev/input/by-id/usb-Gamepad_Name_Goes_Here_USB-event-joystick

Make sure you specify an "event-joystick" device and not a "joystick" device
here. Also, be aware that input2cmds doesn't distinguish between input devices
(so you can't map the same button on different gamepads to different things,
for example).

Once that's done, run input2cmds with the -v option and pass it the path to
your configuration file. It will produce output like:

if type=x code=y value=z then: ...

If one of those type/code/value combinations corresponds to a button you want
to map, then you can paste that line into the configuration file, and replace
... with the command you want to execute. input2cmds will wait until the
command has fully executed before executing any further commands (unless you
put a & on the end).
"#);
        exit(0)
    }
    let (event_tx, event_rx) = channel();
    let mut matches = Vec::new();
    for conf in free.into_iter() {
        if let Err(x) = load_config(&conf, &event_tx, &mut matches) {
            eprintln!("{}", x);
            exit(1);
        }
    }
    std::mem::drop(event_tx); // we've cloned this poor thing enough
    while let Ok(event) = event_rx.recv() {
        let mut command = None;
        for possibility in matches.iter() {
            match possibility.wants_type {
                Some(x) if event.type_ != x => continue,
                _ => (),
            }
            match possibility.wants_code {
                Some(x) if event.code != x => continue,
                _ => (),
            }
            match possibility.wants_value {
                Some(x) if event.value != x => continue,
                _ => (),
            }
            command = Some(possibility.command_to_run.as_str());
            break
        }
        match command {
            Some(command) => {
                if verbose {
                    print!("if type={} code={} value={} then: {}",
                           event.type_, event.code, event.value, command);
                }
                let mut child = Command::new("/bin/sh").arg("-c").arg(command)
                    .spawn().expect("Couldn't execute /bin/sh");
                let exit_status = child.wait()
                    .expect("Couldn't wait on child process (?!!)");
                if exit_status.success() {
                    println!(" # OK");
                }
                else {
                    println!(" # {}", exit_status);
                }
            },
            None => {
                if verbose {
                    println!("if type={} code={} value={} then: ...",
                             event.type_, event.code, event.value);
                }
            }
        }
    }
    std::process::exit(1)
}
