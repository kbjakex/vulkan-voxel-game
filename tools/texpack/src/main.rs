
use std::{fs::File, io::{BufReader, Write}, path::{Path, PathBuf}, collections::HashMap};

use xml::EventReader;

use anyhow::{Result, bail};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("Usage `./texpack <directory containing blocks.xml> or `./texpack <path to .xml>");
        return;
    }

    let path = Path::new(&args[0]);
    if !path.exists() {
        println!("Path \"{}\" does not exist.", &args[0]);
        return;
    }

    let is_xml = path.extension().map_or(false, |f| f == "xml");
    if !is_xml && !path.is_dir() {
        println!(
            "Path \"{}\" is neither a directory nor a .xml file.",
            &args[0]
        );
        return;
    }

    let xml_path = if is_xml {
        args[0].clone()
    } else {
        args[0].clone() + "/blocks.xml"
    };
    let xml_path = Path::new(&xml_path);

    if !xml_path.is_file() || !xml_path.exists() {
        println!(
            ".xml file at path \"{}\" either doesn't exist or is a directory.",
            xml_path.to_str().unwrap()
        );
        return;
    }

    let mut path_buf = PathBuf::from(xml_path);
    path_buf.pop();
    let directory = match path_buf.to_str() {
        Some(str) => str,
        None => {
            println!("Error while extracting the directory? (needs to be valid unicode?)");
            return;
        },
    };

    let out_path = if args.len() >= 2 {
        let path = &args[1];
        if !Path::new(path).is_dir() {
            path.clone()
        } else {
            format!("{}/packed.bin", &path)
        }
    } else {
        format!("{}/packed.bin", directory)
    };
    println!("Output path: \"{}\"", out_path);

    let file = BufReader::new(File::open(xml_path).unwrap());
    let mut parser = EventReader::new(file);

    let mut blocks = None;

    loop {
        let e = match parser.next() {
            Ok(e) => e,
            Err(e) => {
                println!("Error parsing XML: {}. Fix the file and try again.", e);
                return;
            }
        };
        match e {
            xml::reader::XmlEvent::EndDocument => break,

            xml::reader::XmlEvent::StartElement {
                name,
                attributes: _,
                namespace: _,
            } => {
                if name.local_name == "blocks" {
                    if blocks.is_some() {
                        println!("Duplicate <blocks>!");
                        return;
                    }

                    let vec = match parse_blocks(&mut parser) {
                        Some(blocks) => blocks,
                        None => return,
                    };
                    blocks = Some(vec);
                } else {
                    println!("Unexpected block type at root level: \"{}\"", name.local_name);
                    return;
                }
            }
            xml::reader::XmlEvent::EndElement { name } => {
                println!(
                    "(XML: Found END_ELEMENT where one was not expected! Probably invalid XML! Element name: {})", name.local_name
                );
                return;
            }
            xml::reader::XmlEvent::StartDocument { .. } => {}
            xml::reader::XmlEvent::CData(_) => {}
            xml::reader::XmlEvent::Comment(_) => {}
            xml::reader::XmlEvent::Characters(_) => {}
            xml::reader::XmlEvent::Whitespace(_) => {}
            _ => {
                println!("(XML: Ignoring {:?})", e);
                return;
            }
        }
    }

    let mut blocks = match blocks {
        Some(blocks) => blocks,
        None => {
            println!("No <blocks> block found!");
            return;
        },
    };

    blocks.sort_by_key(|def| def.id);

    let num_textures = {
        let mut sum = 0;
        for def in &blocks {
            sum += def.frames;
        }
        sum as usize
    };

    let dir_save = std::env::current_dir().unwrap();
    if let Err(e) = std::env::set_current_dir(directory) {
        println!("Failed to change working directory: {}", e);
        return;
    };

    let mut texture_bytes = Vec::new();
    texture_bytes.resize(num_textures * 16 * 16 * 4, 0u8);

    let mut start_idx = 0;

    for block_def in &blocks {
        if let Err(e) =  read_textures_to_buf(&mut texture_bytes[start_idx..(start_idx + (16*16*4*block_def.frames) as usize)], block_def) {
            println!("Error reading texture: {}", e);
            return;
        }
        println!("Advancing pointer by {} bytes", block_def.frames*16*16*4);
        start_idx += (block_def.frames * 16*16*4) as usize;
    }

    println!("Texture created @ {} bytes, compressing...", texture_bytes.len());

    let compressed = lz4::block::compress(&texture_bytes, Some(lz4::block::CompressionMode::HIGHCOMPRESSION(12)), true).unwrap();
    
    if let Err(e) = std::env::set_current_dir(dir_save) {
        println!("Failed to revert working directory: {}", e);
        return;
    };

    let mut output_file = File::create(&out_path).unwrap();
    println!("Compressed size: {} bytes", compressed.len());
    output_file.write_all(&compressed).unwrap();

    println!("");
    let mut encoder = png::Encoder::new(File::create(out_path.replace(".bin", ".png")).unwrap(), 16, num_textures as u32*16);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&texture_bytes).unwrap();

    println!("Saved to packed.bin");
}

fn read_textures_to_buf(dst: &mut [u8], block: &BlockDef) -> Result<()> {
    let texture_file = match File::open(&block.path) {
        Ok(file) => file,
        Err(e) => {
            bail!("File \"{}\" not found (for block with id={}): {}", block.path, block.id, e);
        },
    };
    let decoder = png::Decoder::new(texture_file);
    let mut reader = match decoder.read_info() {
        Ok(reader) => reader,
        Err(e) => {
            bail!("Something went wrong parsing PNG at path \"{}\": {}", block.path, e);
        },
    };
    let (ctype, cdepth) = reader.output_color_type();
    let mut img_data = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut img_data)?;

    if frame.width != 16 {
        bail!("Image \"{}\" has invalid width, should be 16, was: {}", block.path, frame.width);
    }
    if frame.height != 16 * block.frames {
        bail!("Image \"{}\" has invalid height, should be {} ({} frames * 16), was: {}", block.path, 16*block.frames, block.frames, frame.height);
    }

    println!("Image \"{}\" has format {:?} and bit depth {:?} and takes {} bytes of space", block.path, ctype, cdepth, img_data.len());

    if img_data.len() as u32 != 16*16*4*block.frames {
        bail!("... but conversion from formats with <4 bytes per pixel is not implemented");
    }
    
    dst.copy_from_slice(&img_data[..]);

    Ok(())
}

struct BlockDef {
    path: String,
    id: u32,
    frames: u32,
}

fn parse_blocks(parser: &mut EventReader<BufReader<File>>) -> Option<Vec<BlockDef>> {
    let mut blocks = Vec::new();

    let mut map = HashMap::new();

    println!("Parsing blocks...");
    loop {
        let e = match parser.next() {
            Ok(e) => e,
            Err(e) => {
                println!("Error parsing XML: {}. Fix the file and try again.", e);
                return None;
            }
        };
        match e {
            xml::reader::XmlEvent::StartElement { name, attributes, namespace: _ } => {
                if name.local_name == "block" {
                    map.clear();
                    for attrib in &attributes {
                        map.insert(attrib.name.local_name.clone(), attrib.value.clone());
                    }

                    let block = match parse_block(parser,  &map) {
                        Some(block) => block,
                        None => {
                            return None;
                        },
                    };
                    
                    blocks.push(block);
                } else {
                    println!("Unexpected block type inside <blocks>: \"{}\"", name.local_name);
                }
            },
            xml::reader::XmlEvent::EndElement { name } => {
                if name.local_name == "blocks" {
                    break;
                }
                println!("Error: trailing XML element of type \"{}\"!", name.local_name);
                return None;
            },
            xml::reader::XmlEvent::Comment(_) => {},
            xml::reader::XmlEvent::Whitespace(_) => {},
            e => {
                println!("Unexpected XML element while parsing blocks: {:?}", e);
                return None;
            }
        }
    }

    Some(blocks)
}

fn parse_block(parser: &mut EventReader<BufReader<File>>, attribs: &HashMap<String, String>) -> Option<BlockDef> {
    let id = match attribs.get("id") {
        Some(id) => id,
        None => {
            println!("Block is missing the \"id\" attribute. Double-check the XML.");
            return None;
        },
    };
    let id = match id.parse::<u32>() {
        Ok(id) => id,
        Err(_) => {
            println!("Invalid ID attribute: \"{}\". Either too long (must fit in 32 bits), negative, or not an integer at all.", &id);
            return None;
        },
    };

    let texture_path = match attribs.get("file") {
        Some(path) => path.clone(),
        None => {
            println!("Block is missing the \"file\" attribute. Double-check the XML.");
            return None;
        },
    };

    println!("Parsed block with id {} and file path \"{}\"", id, texture_path);

    loop {
        let e = match parser.next() {
            Ok(e) => e,
            Err(e) => {
                println!("Error parsing XML: {}. Fix the file and try again.", e);
                return None;
            }
        };
        match e {
            xml::reader::XmlEvent::StartElement { name, attributes: _, namespace: _ } => {
                println!("Unexpected element in <block></block>: {}", name.local_name);
                return None;
            },
            xml::reader::XmlEvent::EndElement { name } => {
                if name.local_name == "block" {
                    break;
                }
                println!("Error: trailing XML element of type \"{}\"! (Block ID: {})", name.local_name, id);
                return None;
            },
            xml::reader::XmlEvent::Comment(_) => {},
            xml::reader::XmlEvent::Whitespace(_) => {},
            e => {
                println!("Unexpected XML element while parsing a block: {:?}. Block ID: {}", e, id);
                return None;
            }
        }
    }

    Some(BlockDef {
        path: texture_path,
        id,
        frames: 1
    })
}
