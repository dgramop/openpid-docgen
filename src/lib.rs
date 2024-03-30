//! This documentation generation is intended for to be for implementors of the peripheral, and
//! should document the underlying behavior of a peripheral in a way that a human could write a
//! driver for the peripheral. Documentation for codegen'd output should be part of the given
//! codegen

use std::{error::Error, io::Write, process::{Command, Stdio}};
use indoc::formatdoc;
extern crate mdbook;
use mdbook::MDBook;
use openpid::prelude::*;
use derive_more::Display;

/// Very generic diagram generation utility for packet and packet-like items, including reusable
/// structs, packet formats, and payloads
pub fn generate_packet_diagram(name: &str, contents: Vec<(String, Option<u32>)>) -> String {

    let mut stuffing = String::new();

    let total_bit_width = contents.iter().fold(0, |bits, content| if let Some(content) = content.1 { bits + content } else { 0 });
    if total_bit_width == 0 {
        return "".to_owned();
    }

    let scale = 2000/total_bit_width;

    for (idx, (name, size)) in contents.iter().enumerate() {
        let sizestr = match size {
            Some(size) => format!("{} bits", size),
            None => format!("Variable"),
        };

        stuffing.push_str(&formatdoc!("
        {idx}: {sizestr}
        {idx}: {{
          explanation: |md {name} |
          explanation.style.font-size: 55
          width:{scaledsize}

          style.font-size: 40
        }}
        ", scaledsize = match size { 
            Some(size) => size*scale,
            None => 16*scale
        }))
    }

    formatdoc!("

    vars: {{
      d2-config: {{
        layout-engine: elk
        theme-id: 0
      }}
    }}


    {name} {{
        style.font-size: 50
        grid-rows: 1
        grid-gap: 0
        {stuffing}
    }}
    ")
}

pub fn render_diagram(diagram: String, path: String) -> Result<(), std::io::Error> {
    let mut d2_proc = Command::new("d2")
        .stdin(Stdio::piped())
        .arg("-")
        .arg(path)
        .spawn()?;

    d2_proc.stdin.as_mut().expect("has stdin").write_all(diagram.as_bytes())?;

    println!("finished d2 with {}", d2_proc.wait()?);

    Ok(())
}

// TODO: like action... but subset of variants
#[derive(Debug, Display)]
pub enum Direction {
    Tx,
    Rx
}

struct Book {
    pub src_path: std::path::PathBuf,
}

impl Book {
    /// Generates markdown documentation for the given payload
    pub fn document_payload(&self, payload: &Payload, payload_name: &str, direction: Direction) -> Result<String, std::io::Error> {
        //payload.segments;
        let metadatas = format!("{:?}", payload.metadata);
        let segments = payload.segments.iter().map(|segment| {
            let desc = match segment {
                PacketSegment::Sized { name: _, bits, datatype, description } => {
                    let description = description.as_ref().map_or("", |i| i);
                    formatdoc! ("
                    *{bits}* bit-wide {datatype:?}
                    {description}
                    ")
                },
                PacketSegment::Unsized { name: _, termination, datatype, description } => {
                    let description = description.as_ref().map_or("", |i| i);
                    let termination = termination.as_ref().map_or("no additional termination".to_string(), |i| format!("{:?}",i));
                    formatdoc! ("
                    {datatype:?} with {termination}
                    {description}
                    ")
                },
                PacketSegment::Struct { name: _, struct_name} => {
                    format!("See struct [{struct_name}]({struct_name})")
                },
            };
            format!("### {}\n{desc}",segment.get_name())
        }).collect::<Vec<_>>().join("\n");

        let d2 = generate_packet_diagram(payload_name, payload.segments.iter().map(|segment| {
            match segment {
                PacketSegment::Sized { name, bits, datatype, ..} => {
                    (format!("{name} ({datatype:?})"), Some(*bits))
                },
                PacketSegment::Unsized { name, datatype, ..} => {
                    (format!("{name} ({datatype:?})"), None)
                }
                PacketSegment::Struct { name, struct_name } => {
                    //TODO: deref structs to get their width, if they are sized
                    if name == struct_name {
                        (format!("{name}"), None)
                    } else {
                        (format!("{name} ({struct_name})"), None)
                    }
                }
            }
        }).collect());

        let diagram_path_relative = format!("{payload_direction_path_component}/{payload_name}.png", payload_direction_path_component = match direction {
            Direction::Tx => "tx",
            Direction::Rx => "rx"
        }); 

        println!("source path is {:?}", self.src_path);

        render_diagram(d2, self.src_path.join("payloads").join(std::path::PathBuf::from(diagram_path_relative.clone())).into_os_string().into_string().expect("Path OsString to String"))?;

        // TODO: involve the packet format so it's clear how this goes down the wire
        Ok(formatdoc! ("
        # {payload_name}
        {description}

        ## Payload Segments
        ![Packet Segment Description for {payload_name}]({diagram_path_relative})
        {segments}
        

        ## Hard-coded Values
        {metadatas}


        ", description = payload.description))
    }
}

/// Generates mdbook documentation for an OpenPID config
pub fn document(pid: &OpenPID, path: std::path::PathBuf) -> Result<(), Box<dyn Error>> {

    std::fs::create_dir_all(&path)?;


    //std::fs::write("outputs/book/image.svg", generate_packet_diagram("Packet Format".to_owned(), vec![("Size".to_owned(), Some(8)), ("FrameID".to_owned(), Some(8)),("Payload".to_owned(), None), ("Crc".to_owned(), Some(16))]))?;
    let book  = Book {
        src_path: path.join("src"),
    };

    let _ = std::fs::create_dir_all(book.src_path.join("payloads"));
    let _ = std::fs::create_dir(book.src_path.join("protocol"));
    let _ = std::fs::create_dir(book.src_path.join("structs"));
    let _ = std::fs::create_dir(book.src_path.join("transactions"));

    let mut tx_payloads = String::new();
    let mut tx_payloads_links = String::new();
    for (payload_name, payload) in &pid.payloads.tx {
        tx_payloads_links.push_str(&format!("\t- [{payload_name}](payloads/tx.md#{payload_name})\n"));
        tx_payloads.push_str(&book.document_payload(payload, payload_name, Direction::Tx)?);
    }

    let mut rx_payloads = String::new();
    let mut rx_payloads_links = String::new();
    for (payload_name, payload) in &pid.payloads.rx {
        rx_payloads_links.push_str(&format!("\t- [{payload_name}](payloads/rx.md#{payload_name})\n"));
        rx_payloads.push_str(&book.document_payload(payload, payload_name, Direction::Rx)?);
    }

    // Generate the SUMMARY.md, this has special meaning in mdbook
    let summary = formatdoc!("
    # Contents
    [Sending Packets](index.md)

    # Packet Format
    - [Sent Packets](protocol/tx.md)
    - [Received Packets](protocol/rx.md)

    # Payloads
    - [Common Payload Structs](structs/index.md)
    - [To Device](payloads/tx.md)
    - [From Device](payloads/rx.md)
    
    # Transactions
    [What is a transaction?]()
    - [TODO]()

    ----------
    [About this Document](about.md)
    ");
    std::fs::write(book.src_path.join("SUMMARY.md"), summary)?;


    
    let about = formatdoc!("
    # About this Document

    This document was generated by [OpenPID](TODO) vTODO on TODO.

    The document it was generated from was written in OpenPID {openpid_version:?}

    The document it was generated from's version was {doc_version:?}
    
    ", openpid_version = pid.openpid_version, doc_version = pid.doc_version);
    std::fs::write(book.src_path.join("about.md"), about)?;

    let tx_payloads_index = formatdoc!("
    # Sendable Payloads
    A payload is encapsulated by the [Packet Format](TODO) before it is sent. 

    Sendable payloads are \"sendable\" from your controller to {device_name}.

    {tx_payloads}

    ", device_name = pid.device_info.name);
    std::fs::write(book.src_path.join("payloads").join("tx.md"), tx_payloads_index)?;

    let rx_payloads_index = formatdoc!("
    # Receivable Payloads
    A payload is encapsulated by the [Packet Format](TODO) before it arries at your controller. 

    Recievable payloads are \"recieved\" by your controller from {device_name}.

    {rx_payloads}
    ", device_name = pid.device_info.name);
    std::fs::write(book.src_path.join("payloads").join("rx.md"), rx_payloads_index)?;


    let mut cfg = mdbook::config::Config::default();
    cfg.book.title = Some(format!("{} - Interface Guide",pid.device_info.name));
    cfg.book.authors = vec!["OpenPID DocGen".to_string()];
    cfg.book.description = Some(format!("Communication interface documentation for {}: {}", pid.device_info.name, pid.device_info.description));
    cfg.book.language = Some("English".to_string());

    let mdbook = MDBook::init(&path)
        .with_config(cfg)
        .build()?;
    println!("Rendering book to {:?}", mdbook.build_dir_for("html"));
    mdbook.build()?;
    

    Ok(())
}
