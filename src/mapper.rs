use crate::mapping::Mapping;
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use midly::num::u4;
use midly::{live::LiveEvent, MidiMessage};
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::error::Error;
use std::net::{SocketAddrV4, UdpSocket};
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;

enum Message {
    Midi(MidiMessage),
    Osc(OscMessage),
}

pub struct Mapper {
    mapping: Mapping,
    messages: Receiver<Message>,
    _handle_midi_in: MidiInputConnection<()>,
    _handle_osc_listener: JoinHandle<()>,
    midi_out: MidiOutputConnection,
    osc_out: UdpSocket,
    osc_out_addr: SocketAddrV4,
}

fn handle_osc_packet(packet: OscPacket, msgs: &Sender<Message>) {
    match packet {
        OscPacket::Message(msg) => {
            msgs.send(Message::Osc(msg)).unwrap();
        }
        OscPacket::Bundle(bundle) => {
            for pkg in bundle.content {
                handle_osc_packet(pkg, msgs);
            }
        }
    }
}

impl Mapper {
    pub fn new(
        mapping: Mapping,
        osc_listen_addr: SocketAddrV4,
        osc_out_addr: SocketAddrV4,
        midi_device: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let (messages_tx, messages_rx) = mpsc::channel();
        Ok(Self {
            mapping: mapping,
            messages: messages_rx,
            _handle_midi_in: Self::connect_midi_input(messages_tx.clone(), midi_device)?,
            midi_out: Self::connect_midi_output(midi_device)?,
            _handle_osc_listener: Self::listen_osc(messages_tx.clone(), osc_listen_addr)?,
            osc_out: UdpSocket::bind(SocketAddrV4::from_str("0.0.0.0:0").unwrap())?,
            osc_out_addr: osc_out_addr,
        })
    }

    fn listen_osc(
        msgs: Sender<Message>,
        osc_listen_addr: SocketAddrV4,
    ) -> Result<JoinHandle<()>, Box<dyn Error>> {
        let sock = UdpSocket::bind(osc_listen_addr)?;
        println!("Listening for OSC messages on socket {}", osc_listen_addr);

        let mut buf = [0u8; rosc::decoder::MTU];
        return Ok(thread::spawn(move || loop {
            match sock.recv_from(&mut buf) {
                Ok((size, _addr)) => match rosc::decoder::decode_udp(&buf[..size]) {
                    Ok((_, packet)) => handle_osc_packet(packet, &msgs),
                    Err(e) => println!("Error receiving from socket: {}", e),
                },
                Err(e) => {
                    println!("Error receiving from socket: {}", e);
                    break;
                }
            }
        }));
    }

    pub fn start(&mut self) {
        loop {
            let msg = self.messages.recv().unwrap();
            match msg {
                Message::Midi(m) => self.handle_midi_message(m),
                Message::Osc(m) => self.handle_osc_message(m),
            }
        }
    }

    pub fn all_midi_off(&mut self) {
        for btn in self.mapping.button_mappings().to_owned() {
            self.send_midi_message(MidiMessage::NoteOn {
                key: btn.note.into(),
                vel: btn.vel_off.into(),
            });
        }
    }

    fn handle_osc_message(&mut self, msg: OscMessage) {
        if let Some(suffix) = msg.addr.strip_prefix("/hog/status/led/") {
            let mut name = suffix.to_owned();
            name = name.replace("effects", "effect"); // bug in HOG4
            name = name.strip_suffix("/100").unwrap_or(&name).to_owned(); // maingo, mainhalt etc. are for some reason suffixed with /100 sometimes
            if let Some(btn) = self.mapping.button_from_name(&name) {
                match &msg.args[0] {
                    OscType::Float(val) => match *val as u8 {
                        0u8 => self.send_midi_message(MidiMessage::NoteOn {
                            key: btn.note.into(),
                            vel: btn.vel_off.into(),
                        }),
                        1u8 => self.send_midi_message(MidiMessage::NoteOn {
                            key: btn.note.into(),
                            vel: btn.vel_on.into(),
                        }),
                        value => println!("{}: {:?}", name, value),
                    },
                    value => println!("{}: {:?}", name, value),
                }
            } else {
                if name.starts_with("flash") {
                    return;
                }
                println!("unknown LED key {}", name);
            }
            return;
        }
        if msg.addr.starts_with("/hog/status/time") {
            return;
        }
    }

    fn handle_midi_message(&mut self, message: MidiMessage) {
        match message {
            MidiMessage::NoteOn { key, .. } => {
                if let Some(btn) = self.mapping.button_from_note(key.into()) {
                    self.send_osc_message(OscMessage {
                        addr: format!("/hog/hardware/{}", btn.name),
                        args: vec![OscType::Float(1.0)],
                    })
                }
            }
            MidiMessage::NoteOff { key, .. } => {
                if let Some(btn) = self.mapping.button_from_note(key.into()) {
                    self.send_osc_message(OscMessage {
                        addr: format!("/hog/hardware/{}", btn.name),
                        args: vec![OscType::Float(0.0)],
                    })
                }
            }
            MidiMessage::Controller { controller, value } => {
                if let Some(controller) = self.mapping.controller_from_id(controller.into()) {
                    self.send_osc_message(OscMessage {
                        addr: format!("/hog/hardware/{}", controller.name),
                        args: vec![OscType::Float(
                            (u8::from(value) * 2 + u8::from(value) / 64) as f32,
                        )],
                    })
                }
            }
            _ => {}
        };
    }

    fn send_midi_message(&mut self, msg: MidiMessage) {
        let ev = LiveEvent::Midi {
            channel: u4::default(),
            message: msg,
        };
        let mut buf = Vec::new();
        ev.write(&mut buf).unwrap();
        self.midi_out.send(&buf).unwrap();
    }

    fn send_osc_message(&self, msg: OscMessage) {
        let msg_buf = encoder::encode(&OscPacket::Message(msg)).unwrap();
        self.osc_out.send_to(&msg_buf, self.osc_out_addr).unwrap();
    }

    fn connect_midi_input(
        msgs: Sender<Message>,
        midi_device: &str,
    ) -> Result<MidiInputConnection<()>, Box<dyn Error>> {
        let mut midi_in = MidiInput::new("midir reading input")?;
        midi_in.ignore(Ignore::None);
        let in_port = midi_in
            .ports()
            .into_iter()
            .find(|x| midi_in.port_name(x).unwrap().starts_with(midi_device))
            .ok_or("Could not find midi device with given name")?;
        println!("Connecting to MIDI input {}", midi_in.port_name(&in_port)?);

        return Ok(midi_in.connect(
            &in_port,
            "midir-read-input",
            move |_stamp, msg, _data| {
                let event = LiveEvent::parse(msg).unwrap();
                match event {
                    LiveEvent::Midi { channel, message } if channel == 0 => match message {
                        _ => msgs.send(Message::Midi(message)).unwrap(),
                    },
                    _ => {}
                }
            },
            (),
        )?);
    }

    fn connect_midi_output(midi_device: &str) -> Result<MidiOutputConnection, Box<dyn Error>> {
        let midi_out = MidiOutput::new("midi reading output")?;
        let out_port = midi_out
            .ports()
            .into_iter()
            .find(|x| midi_out.port_name(x).unwrap().starts_with(midi_device))
            .ok_or("Could not find midi device with given name")?;
        println!(
            "Connecting to MIDI output {}",
            midi_out.port_name(&out_port)?
        );
        return Ok(midi_out.connect(&out_port, "midi-out")?);
    }
}
