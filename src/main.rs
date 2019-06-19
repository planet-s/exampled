// This is an implementation of a simple driver for an imaginary device
// It is extremely restricted in capabilities in its main loop
// It also uses no unsafe code

extern crate syscall;

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use syscall::data::{Event, Packet};
use syscall::flag::{EVENT_READ, O_NONBLOCK};
use syscall::scheme::SchemeBlockMut;

mod scheme;

use scheme::ExampleScheme;

// A method to convert from a system call error to a standard Rust I/O error
fn syscall_error(error: syscall::error::Error) -> io::Error {
    io::Error::from_raw_os_error(error.errno)
}

// Entry point in Rust, possibly returning an error
fn main() -> io::Result<()> {
    // The root scheme `:` is used to manage schemes.
    // This will create `example:` by opening `:example` with O_CREAT
    let scheme_name = "example";
    let mut scheme_file = OpenOptions::new()
        .create(true).read(true).write(true)
        .custom_flags(O_NONBLOCK as i32)
        .open(&format!(":{}", scheme_name))?;

    // IRQs can be received using the `irq:` scheme.
    // This will listen for IRQ #1 to trigger
    let irq_number = 1;
    let mut irq_file = OpenOptions::new()
        .read(true).write(true)
        .custom_flags(O_NONBLOCK as i32)
        .open(&format!("irq:{}", irq_number))?;

    // File events are requested and received using the `event:` scheme.
    // This creates a new event queue.
    let mut event_file = OpenOptions::new()
        .read(true).write(true)
        .open("event:")?;

    // Now that all the necessary files are open, the driver can enter namespace `0`.
    // This is also known as the null namespace - no files can be opened from inside it.
    syscall::setrens(0, 0).map_err(syscall_error)?;

    // Add the scheme file to the event queue
    const SCHEME_TOKEN: usize = 1;
    event_file.write(&Event {
        id: scheme_file.as_raw_fd() as usize,
        flags: EVENT_READ,
        data: SCHEME_TOKEN,
    })?;

    // Add the irq file to the event queue
    const IRQ_TOKEN: usize = 2;
    event_file.write(&Event {
        id: irq_file.as_raw_fd() as usize,
        flags: EVENT_READ,
        data: IRQ_TOKEN,
    })?;

    // Create a driver scheme handler
    let mut scheme = ExampleScheme::new();

    // Store blocked packets for later processing
    let mut blocked = VecDeque::new();

    // Process events forever
    loop {
        // Read the next event from the event queue
        let mut event = Event::default();
        event_file.read(&mut event)?;

        match event.data {
            // If the event came from the scheme file
            SCHEME_TOKEN => {
                // Read the next packet
                let mut next_packet = Packet::default();
                scheme_file.read(&mut next_packet)?;

                // Add the packet first so that previously blocked packets are processed after
                blocked.push_front(next_packet);
            },
            // If the event came from the irq file
            IRQ_TOKEN => {
                // Read the IRQ counter
                let mut counter = [0; 8];
                irq_file.read(&mut counter)?;

                // If this IRQ was handled by this driver
                if scheme.irq() {
                    // Acknowledge the IRQ. This allows multiple drivers to service one IRQ
                    irq_file.write(&counter)?;
                }
            },
            _ => (),
        }

        // Iterate on blocked packets to resolve them
        let mut i = 0;
        while i < blocked.len() {
            // If the packet has a response
            if let Some(response) = scheme.handle(&blocked[i]) {
                // Write the response
                let mut packet = blocked.remove(i).unwrap();
                packet.a = response;
                scheme_file.write(&packet)?;
            } else {
                // Continue blocking packet
                i += 1;
            }
        }
    }
}
